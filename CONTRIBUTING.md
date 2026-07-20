# Contributing to SysForge

Thanks for your interest in SysForge. This guide covers the development
workflow and — more importantly — how to extend the project without eroding
its architecture. SysForge is built around a few deliberate invariants; a
contribution that preserves them is far more valuable than one that adds a
feature by working around them.

If you haven't yet, read the [architecture section of the
README](README.md#architecture) first. This guide assumes it.

## Development workflow

A stable Rust toolchain (edition 2024, Rust 1.85+) is required. Every change
must pass the same checks CI runs, in this order:

```bash
cargo fmt --all                                         # format
cargo clippy --workspace --all-targets -- -D warnings   # lint (warnings are errors)
cargo test --workspace                                  # test
cargo doc --workspace --no-deps                         # docs must build clean
```

Running these before every commit is the fastest way to keep the pipeline
green. A local pre-commit hook that runs the first two is a good idea:

```bash
cat > .git/hooks/pre-commit <<'EOF'
#!/bin/sh
set -e
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
EOF
chmod +x .git/hooks/pre-commit
```

### Why warnings are errors

CI denies all warnings, including two that are easy to miss locally:

- **`missing_docs`** — every public item needs a doc comment. This is what
  makes the published API documentation complete. If clippy complains about a
  missing doc, add one; don't `#[allow]` it.
- **`unwrap_used`** — prefer `?` or explicit error handling. `.expect("...")`
  with a message that states the invariant is acceptable where a panic truly
  cannot happen.

Project-wide lint configuration lives in `clippy.toml` (for example, valid
documentation identifiers) and in `[workspace.lints]` in the root
`Cargo.toml`. Prefer fixing those over sprinkling local `#[allow]`s.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org):

```
feat: add network domain with per-interface throughput
fix: skip processes that exit mid-scan
refactor: generalize collector runner to Fn
docs: restore missing doc comments
ci: publish cargo doc to Pages
```

Keep each commit focused; a commit should leave the tree green. Pull requests
should describe *what* changed and *why*, and note any architectural decision
made along the way. If a change touches an invariant below, call that out
explicitly so it can be reviewed with care.

## Workspace layout

```
crates/
├── common/     # Collector trait and shared error type — depended on by all
├── system/     # CPU, memory, process collectors (/proc)
├── docker/     # container listing, stats, logs
├── git/        # branch, working-tree status, commits
├── network/    # per-interface throughput
└── app/        # orchestration, state, config, event loop, rendering
```

The dependency rule is strict: **domain crates depend only on `common`.**
They never depend on each other or on `app`. If two domains need to share
something, it belongs in `common`. Composition — wiring domains into the
running application — happens only in `app`.

## Adding a new domain

Every domain in SysForge (system, docker, git, network) was built the same
way. Follow these steps and the new domain will fit the architecture by
construction. Use an existing domain of similar shape as a template:

- reading a file → `network` (parses `/proc/net/dev`)
- talking to an external service → `docker` (Docker socket)
- invoking a binary → `git` (`git` subprocess)

### 1. Create the crate

Add `crates/<domain>/` with a `Cargo.toml` that depends on
`sysforge-common`, and register it in the root `[workspace.dependencies]`.
Depend on `serde` if it has configuration, and on whatever it needs to read
its data — nothing else.

### 2. Implement the `Collector`

The collector reads the underlying source and returns a **snapshot**: plain,
UI-ready data. Keep parsing in a separate pure function so it can be
unit-tested without the real source. The collector implements
`sysforge_common::collector::Collector`, which asks for a name, an interval,
and an async `collect` that produces one sample.

If the domain derives rates or percentages from change over time (like CPU or
network), the collector may keep the *previous sample* internally to compute
deltas — but never UI history and never presentation state.

### 3. Define the snapshot

Snapshots are plain data: `#[derive(Debug, Clone, ...)]` structs of owned,
already-interpreted fields (bytes, not raw counters; percentages, not
jiffies). Normalize messy source data (optional API fields, raw strings) at
the boundary so the UI never deals with it.

### 4. Model unavailability as data

If the domain can be legitimately absent — a stopped daemon, a directory
that isn't a repository, a missing binary — model that as an enum variant of
the snapshot, not as an error. Errors are for the genuinely unexpected. Log
only state *transitions*, never every failed sample.

### 5. Own the configuration

If the domain is configurable, define its config struct **in the domain
crate** (`config.rs`), deriving `Deserialize` with `#[serde(default,
deny_unknown_fields)]` and a `Default` impl. The `app` crate composes it into
the top-level `Config`; the domain owns its own contract.

### 6. Integrate with `AppState`

Add a field to `AppState` holding the latest snapshot (or a UI-state enum
wrapping it, if the domain has a disabled/pending/observed lifecycle). In
`app`, spawn the collector with `spawn_collector`, whose closure applies each
sample to the state. UI-side history (for sparklines) lives in `AppState`,
keyed if the domain has multiple entities — never in the collector.

### 7. Add the panel and view

Create `render/<domain>.rs` exposing a `render` function that takes the
snapshot data it needs plus a `RenderCtx`. Never name colors directly — use
theme roles (`ctx.theme.accent`, `.success`, `.muted`). Add a `PanelId` and,
for a full-screen view, a `ViewId` variant and its entry in `ViewId::panels`.

### 8. Add the key binding and documentation

Add the view's binding to the `BINDINGS` table in `input.rs` — the help
overlay updates itself from that table. Document every public item (CI
enforces this). Add tests for the parser and any derivation logic.

## Architectural invariants

These are the properties that keep SysForge maintainable. A change that
breaks one needs a very good reason and explicit discussion:

- **Domains are independent.** A domain crate depends only on `common`, knows
  nothing about the terminal or the UI, and never references another domain.
- **Collectors are stateless producers.** They emit snapshots. They never
  touch the UI, never accumulate presentation history, and hold at most the
  previous sample for delta computation.
- **Snapshots are plain, interpreted data.** No raw counters, no optional
  soup, no references to the source mechanism. The UI consumes clean types.
- **Rendering is a pure function of state.** `render` reads `AppState` and
  `UiState` and draws; it holds no hidden state and performs no I/O.
- **Data flows one way.** Source → collector → snapshot → `AppState` →
  render → terminal. Input becomes an `Action`; asynchronous work is a
  `Command` whose result returns as a `UiEvent`. The render loop never awaits
  I/O directly.
- **Offline is an observation, not an error.** Absence is modeled as data the
  UI can render.
- **Configuration flows through constructors.** No global state, no
  environment reads deep in the call tree.

## Questions

Open an issue for anything unclear, or to discuss a change before building
it — especially one that touches an invariant above.
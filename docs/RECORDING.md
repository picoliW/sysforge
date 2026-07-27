# Recording the demo GIF

The README GIF is generated from [`demo.tape`](demo.tape) with
[vhs](https://github.com/charmbracelet/vhs), so it can be regenerated
deterministically whenever the UI changes — no manual re-recording.

## One-time setup

Install vhs (it also needs `ttyd` and `ffmpeg`):

```bash
# vhs — via Go, or grab a release binary from the repo
go install github.com/charmbracelet/vhs@latest
# dependencies
sudo apt install ffmpeg
go install github.com/tsl0922/ttyd@latest   # or a release binary
```

(If you prefer not to install Go: both vhs and ttyd publish prebuilt
binaries on their GitHub releases pages.)

## Generate the GIF

From the repository root:

```bash
cargo build --release      # so the demo launches instantly
vhs docs/demo.tape         # writes docs/media/demo.gif
```

Then commit the regenerated `docs/media/demo.gif`.

## Tuning the demo

Edit [`demo.tape`](demo.tape) — it's a plain script. Common tweaks:

- `Set Theme` — any of the vhs themes (`vhs themes` lists them).
- `Sleep` durations — how long each view lingers.
- `Set PlaybackSpeed` — global speed multiplier.
- The `Type "N"` lines choose which views the tour visits.

For the richest demo, record on a machine where the interesting domains
are live: Docker running with a few containers, a Git repo, and — for the
Kubernetes view — a local cluster with some pods.

### A quick Kubernetes cluster for the demo

`kind` runs a whole cluster inside Docker (which you already have):

```bash
# install kind (single binary)
curl -Lo ./kind https://kind.sigs.k8s.io/dl/v0.24.0/kind-linux-amd64
chmod +x ./kind && sudo mv ./kind /usr/local/bin/kind

# create the cluster (writes ~/.kube/config automatically)
kind create cluster --name sysforge-demo
```

Then enable Kubernetes in `~/.config/sysforge/config.toml`:

```toml
[k8s]
enabled = true
```

For a demo that shows the status colors off, add a healthy deployment and a
deliberately-broken pod:

```bash
kubectl create deployment web --image=nginx --replicas=3   # green, Running
kubectl run crasher --image=busybox --restart=Always -- /bin/sh -c "exit 1"  # red, CrashLoopBackOff
kubectl run badimage --image=does-not-exist:v9             # red, ImagePullBackOff
```

Wait ~30s, then the Kubernetes view shows healthy pods in green and the
broken ones in red at the top (not-ready first). Clean up afterwards with
`kind delete cluster --name sysforge-demo`.

## Why not asciinema?

asciinema 2.x (the apt version) does not capture the terminal's alternate
screen, which is exactly where a full-screen TUI like SysForge draws — so
the recording comes out blank. vhs captures it correctly and, being
script-driven, keeps the GIF reproducible. (asciinema 3.x also fixes the
alternate-screen issue if you prefer a `.cast` you can embed as a web
player, but GitHub renders GIFs inline and not the player, so the GIF wins
for the README header.)

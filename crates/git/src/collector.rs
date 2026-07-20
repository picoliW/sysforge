//! Repository state via the `git` binary.

use std::time::Duration;

use sysforge_common::collector::{Collector, CollectorError};
use tokio::process::Command;

use crate::config::GitConfig;

/// A single commit summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// Abbreviated hash.
    pub short_hash: String,
    /// First line of the commit message.
    pub summary: String,
    /// Author name.
    pub author: String,
    /// Relative date ("2 hours ago").
    pub when: String,
}

/// Working-tree file counts, from `git status --porcelain`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkingTree {
    /// Files with staged changes.
    pub staged: usize,
    /// Tracked files with unstaged changes.
    pub modified: usize,
    /// Untracked files.
    pub untracked: usize,
}

impl WorkingTree {
    /// Whether the working tree has no changes at all.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.staged == 0 && self.modified == 0 && self.untracked == 0
    }
}

/// One reading of the repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitSnapshot {
    /// Current branch, or a short hash when detached.
    pub branch: String,
    /// Working-tree summary.
    pub working_tree: WorkingTree,
    /// Most recent commits, newest first.
    pub commits: Vec<Commit>,
}

/// What the collector observed about the Git domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitStatus {
    /// The directory is a repository.
    Repository(GitSnapshot),
    /// A valid directory that is not a Git repository.
    NotARepository,
    /// Git itself could not be used.
    Unavailable {
        /// Short human-readable cause.
        reason: String,
    },
}

/// How many commits to show.
const LOG_LIMIT: usize = 10;
/// Field separator unlikely to appear in commit metadata.
const SEP: &str = "\x1f";

/// Samples the working-directory repository at a configurable interval.
pub struct GitCollector {
    config: GitConfig,
    was_repo: Option<bool>,
}

impl GitCollector {
    /// Creates a collector from its configuration.
    #[must_use]
    pub fn new(config: GitConfig) -> Self {
        Self {
            config,
            was_repo: None,
        }
    }

    async fn git(&self, args: &[&str]) -> Result<std::process::Output, String> {
        let mut command = Command::new("git");
        if !self.config.repo_path.is_empty() {
            command.args(["-C", &self.config.repo_path]);
        }
        command
            .args(args)
            .output()
            .await
            .map_err(|e| format!("running git: {e}"))
    }

    async fn try_collect(&self) -> Result<GitStatus, String> {
        // `rev-parse` is the cheapest "is this a repo?" probe.
        let probe = self.git(&["rev-parse", "--is-inside-work-tree"]).await?;
        if !probe.status.success() {
            return Ok(GitStatus::NotARepository);
        }

        let branch = self.current_branch().await?;
        let working_tree = self.working_tree().await?;
        let commits = self.recent_commits().await?;
        Ok(GitStatus::Repository(GitSnapshot {
            branch,
            working_tree,
            commits,
        }))
    }

    async fn current_branch(&self) -> Result<String, String> {
        let out = self.git(&["branch", "--show-current"]).await?;
        let branch = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if branch.is_empty() {
            // Detached HEAD: fall back to the short hash.
            let head = self.git(&["rev-parse", "--short", "HEAD"]).await?;
            return Ok(format!(
                "({})",
                String::from_utf8_lossy(&head.stdout).trim()
            ));
        }
        Ok(branch)
    }

    async fn working_tree(&self) -> Result<WorkingTree, String> {
        let out = self.git(&["status", "--porcelain"]).await?;
        Ok(parse_porcelain(&String::from_utf8_lossy(&out.stdout)))
    }

    async fn recent_commits(&self) -> Result<Vec<Commit>, String> {
        let format = format!("%h{SEP}%s{SEP}%an{SEP}%cr");
        let out = self
            .git(&[
                "log",
                &format!("-{LOG_LIMIT}"),
                &format!("--pretty=format:{format}"),
            ])
            .await?;
        Ok(parse_log(&String::from_utf8_lossy(&out.stdout)))
    }

    fn note_repo(&mut self, is_repo: bool) {
        if self.was_repo == Some(is_repo) {
            return;
        }
        tracing::info!(is_repo, "git repository state changed");
        self.was_repo = Some(is_repo);
    }
}

impl Collector for GitCollector {
    type Output = GitStatus;

    fn name(&self) -> &'static str {
        "git"
    }

    fn interval(&self) -> Duration {
        Duration::from_millis(self.config.interval_ms)
    }

    async fn collect(&mut self) -> Result<GitStatus, CollectorError> {
        let status = match self.try_collect().await {
            Ok(status) => status,
            Err(reason) => GitStatus::Unavailable { reason },
        };
        self.note_repo(matches!(status, GitStatus::Repository(_)));
        Ok(status)
    }
}

/// Counts staged / modified / untracked files from porcelain v1 output.
///
/// Each line is `XY path`, where column X is the staged status and Y
/// the unstaged one; `??` marks an untracked file.
fn parse_porcelain(raw: &str) -> WorkingTree {
    let mut tree = WorkingTree::default();
    for line in raw.lines() {
        let bytes = line.as_bytes();
        if bytes.len() < 2 {
            continue;
        }
        let (x, y) = (bytes[0] as char, bytes[1] as char);
        if x == '?' && y == '?' {
            tree.untracked += 1;
            continue;
        }
        if x != ' ' {
            tree.staged += 1;
        }
        if y != ' ' {
            tree.modified += 1;
        }
    }
    tree
}

/// Parses the separator-delimited `git log` output.
fn parse_log(raw: &str) -> Vec<Commit> {
    raw.lines()
        .filter_map(|line| {
            let mut fields = line.split(SEP);
            Some(Commit {
                short_hash: fields.next()?.to_owned(),
                summary: fields.next()?.to_owned(),
                author: fields.next()?.to_owned(),
                when: fields.next()?.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_counts_each_category() {
        let raw = "\
M  staged.rs
 M modified.rs
MM both.rs
?? new.rs
";
        let tree = parse_porcelain(raw);
        assert_eq!(tree.staged, 2); // "M " and "MM"
        assert_eq!(tree.modified, 2); // " M" and "MM"
        assert_eq!(tree.untracked, 1);
        assert!(!tree.is_clean());
    }

    #[test]
    fn clean_tree_is_clean() {
        assert!(parse_porcelain("").is_clean());
    }

    #[test]
    fn log_parses_all_fields() {
        let raw = format!("abc123{SEP}fix the bug{SEP}Luiz{SEP}2 hours ago");
        let commits = parse_log(&raw);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].short_hash, "abc123");
        assert_eq!(commits[0].summary, "fix the bug");
        assert_eq!(commits[0].author, "Luiz");
    }

    #[test]
    fn malformed_log_lines_are_skipped() {
        let commits = parse_log("incomplete line without separators");
        assert!(commits.is_empty());
    }
}

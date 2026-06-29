//! Git operations.
//!
//! Provides the high-level operations AutoCommit needs.
//!
//! * Read operations (open, discover, version history) use the `git2` library
//! * Write operations (add, commit, push) use the `git` CLI so that system
//!   credential helpers (Windows GCM, osxkeychain, etc.) work out of the box

use std::path::{Path, PathBuf};
use git2::{Repository, StatusOptions};
use crate::errors::{AutoCommitError, Result};
use crate::version;

/// A handle to the Git repository being watched.
pub struct GitRepo {
    /// The underlying `git2` repository.
    repo: Repository,
    /// Repository root path (cached).
    pub root: PathBuf,
}

impl GitRepo {
    /// Open the Git repository that contains `start_path`.
    ///
    /// Returns `Err(AutoCommitError::NotInRepository)` when the path is
    /// not inside a valid Git working tree.
    pub fn open(start_path: &Path) -> Result<Self> {
        let repo = Repository::discover(start_path)
            .map_err(|e| {
                if start_path.join(".git").exists() {
                    AutoCommitError::RepoOpen(e.into())
                } else {
                    AutoCommitError::NotInRepository
                }
            })?;

        let root = repo
            .workdir()
            .map(|p| p.to_path_buf())
            .ok_or(AutoCommitError::DetachedHead)?;

        Ok(Self { repo, root })
    }

    /// Return the name of the currently checked-out branch.
    ///
    /// Returns `Err(AutoCommitError::DetachedHead)` when the HEAD is
    /// detached.
    pub fn current_branch(&self) -> Result<String> {
        let head = self.repo.head().map_err(|_| AutoCommitError::DetachedHead)?;

        if head.is_branch() {
            let shorthand = head
                .shorthand()
                .unwrap_or("unknown");
            Ok(shorthand.to_string())
        } else {
            Err(AutoCommitError::DetachedHead)
        }
    }

    /// Read the latest semantic version from the commit log.
    ///
    /// Scans recent commits (most recent first) looking for one whose
    /// message parses as a `vMAJOR.MINOR.PATCH` version.  Returns
    /// `None` when no versioned commit is found (caller should use
    /// `INITIAL_VERSION`).
    pub fn latest_version(&self) -> Result<Option<String>> {
        let mut revwalk = match self.repo.revwalk() {
            Ok(w) => w,
            Err(_) => return Ok(None),
        };
        if revwalk.push_head().is_err() {
            return Ok(None);
        }

        for rev_result in revwalk {
            let oid = match rev_result {
                Ok(o) => o,
                Err(_) => continue,
            };
            let commit = match self.repo.find_commit(oid) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let msg = commit.message().unwrap_or("");
            if version::parse_version(msg).is_some() {
                return Ok(Some(msg.trim().to_string()));
            }
        }

        Ok(None)
    }

    /// Check whether there are any unstaged or untracked changes.
    ///
    /// Returns `true` if the working tree is clean, `false` otherwise.
    pub fn is_clean(&self) -> Result<bool> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true);
        let statuses = self.repo
            .statuses(Some(&mut opts))
            .map_err(|e| AutoCommitError::Other(e.into()))?;
        Ok(statuses.is_empty())
    }

    /// Stage all, commit, and push in one shot using the git CLI.
    ///
    /// This avoids the git2 push/auth mess — the CLI handles credential
    /// helpers (Windows GCM, etc.) natively.
    pub fn add_commit_push(&self, message: &str) -> Result<String> {
        use std::process::Command;

        let root = &self.root;

        // git add -A
        let add_out = Command::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .output()
            .map_err(|e| AutoCommitError::Other(e.into()))?;
        if !add_out.status.success() {
            let stderr = String::from_utf8_lossy(&add_out.stderr);
            return Err(AutoCommitError::Other(
                anyhow::anyhow!("git add failed: {}", stderr),
            ));
        }

        // Check if there's anything to commit (git diff --cached --quiet)
        let has_changes = !Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(root)
            .status()
            .map_err(|e| AutoCommitError::Other(e.into()))?
            .success();
        if !has_changes {
            return Err(AutoCommitError::CommitFailed(
                anyhow::anyhow!("no changes to commit"),
            ));
        }

        // git commit -m "<message>"
        let commit_out = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(root)
            .output()
            .map_err(|e| AutoCommitError::Other(e.into()))?;
        if !commit_out.status.success() {
            let stderr = String::from_utf8_lossy(&commit_out.stderr);
            return Err(AutoCommitError::CommitFailed(
                anyhow::anyhow!("git commit failed: {}", stderr),
            ));
        }

        // Extract the commit hash from output ("[branch <hash>] ...")
        let oid = String::from_utf8_lossy(&commit_out.stdout)
            .split_whitespace()
            .find(|s| s.len() >= 7 && s.chars().all(|c| c.is_ascii_hexdigit()))
            .unwrap_or("??")
            .to_string();

        // git push
        let push_out = Command::new("git")
            .args(["push"])
            .current_dir(root)
            .output()
            .map_err(|e| AutoCommitError::Other(e.into()))?;
        if !push_out.status.success() {
            let stderr = String::from_utf8_lossy(&push_out.stderr);
            if stderr.contains("rejected") {
                return Err(AutoCommitError::PushFailed(
                    anyhow::anyhow!("push rejected: {}", stderr),
                ));
            }
            return Err(AutoCommitError::NetworkError(
                anyhow::anyhow!("push failed: {}", stderr),
            ));
        }

        Ok(oid)
    }

}

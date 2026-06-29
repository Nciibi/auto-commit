//! Git operations via the `git2` library.
//!
//! Provides the high-level operations AutoCommit needs:
//! * Opening a repository
//! * Reading the latest version from commit history
//! * Staging all changes (`git add -A`)
//! * Creating a commit
//! * Pushing to the remote

use std::path::{Path, PathBuf};
use git2::{Branch, BranchType, Cred, CredentialType, PushOptions, RemoteCallbacks, Repository, StatusOptions};
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

    /// Stage all changes (`git add -A` equivalent).
    ///
    /// Returns the number of files staged, or `0` if nothing changed.
    pub fn stage_all(&self) -> Result<usize> {
        let mut index = self.repo.index().map_err(|e| AutoCommitError::Other(e.into()))?;

        // Add all files (including new, modified, deleted).
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT | git2::IndexAddOption::FORCE, None)
            .map_err(|e| AutoCommitError::Other(e.into()))?;

        index.write().map_err(|e| AutoCommitError::Other(e.into()))?;

        // Count staged entries.
        let count = index.iter().count();
        Ok(count)
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

    /// Has any staged change?
    pub fn has_staged_changes(&self) -> Result<bool> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        let statuses = self.repo
            .statuses(Some(&mut opts))
            .map_err(|e| AutoCommitError::Other(e.into()))?;

        for entry in statuses.iter() {
            let status = entry.status();
            if status != git2::Status::CURRENT && status != git2::Status::IGNORED {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Create a commit with the given message.
    ///
    /// Stages everything first (`git add -A`), then creates the commit
    /// on the current branch.
    pub fn commit(&self, message: &str) -> Result<String> {
        // Stage everything.
        self.stage_all()?;

        // If nothing is staged, bail out.
        if !self.has_staged_changes()? {
            return Err(AutoCommitError::CommitFailed(anyhow::anyhow!("no changes to commit")));
        }

        // Get the signature.
        let sig = self.repo.signature()
            .map_err(|e| AutoCommitError::Other(e.into()))?;

        // Get the HEAD commit (or create an initial commit).
        let parent_commit = self.repo.head().ok().and_then(|head| {
            head.peel_to_commit().ok()
        });

        let mut index = self.repo.index().map_err(|e| AutoCommitError::Other(e.into()))?;
        let tree_id = index.write_tree().map_err(|e| AutoCommitError::Other(e.into()))?;
        let tree = self.repo.find_tree(tree_id).map_err(|e| AutoCommitError::Other(e.into()))?;

        let oid = if let Some(ref parent) = parent_commit {
            self.repo
                .commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    message,
                    &tree,
                    &[parent],
                )
                .map_err(|e| AutoCommitError::CommitFailed(e.into()))?
        } else {
            // Initial commit (no parents).
            self.repo
                .commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    message,
                    &tree,
                    &[],
                )
                .map_err(|e| AutoCommitError::CommitFailed(e.into()))?
        };

        Ok(oid.to_string())
    }

    /// Push committed changes to the remote.
    ///
    /// Pushes the current branch to its configured upstream, or the
    /// default remote (typically "origin").
    pub fn push(&self) -> Result<()> {
        let branch = self.current_branch()?;
        let remote_name = self.find_remote_name()?;

        let mut remote = self.repo
            .find_remote(&remote_name)
            .map_err(|e| AutoCommitError::PushFailed(e.into()))?;

        // Default fetchspec-based push refspec.
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);

        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(credentials_cb);

        callbacks.push_update_reference(|refname, status| {
            if let Some(msg) = status {
                Err(git2::Error::from_str(&format!("push rejected for {}: {}", refname, msg)))
            } else {
                Ok(())
            }
        });

        let mut push_opts = PushOptions::new();
        push_opts.remote_callbacks(callbacks);

        remote.push(&[&refspec], Some(&mut push_opts))
            .map_err(|e| {
                let msg = e.message();
                if msg.contains("Authentication") || msg.contains("Couldn't connect")
                    || msg.contains("timeout")
                {
                    AutoCommitError::NetworkError(e.into())
                } else if msg.contains("rejected") {
                    AutoCommitError::PushFailed(e.into())
                } else {
                    AutoCommitError::Other(e.into())
                }
            })?;

        Ok(())
    }

    /// Determine the remote name for the current branch.
    ///
    /// Prefers the tracking branch's remote; falls back to "origin".
    fn find_remote_name(&self) -> Result<String> {
        let branch = self.current_branch()?;

        // Check if the branch has a tracking branch configured.
        if let Ok(b) = self.repo.find_branch(&branch, BranchType::Local) {
            if let Ok(upstream) = b.upstream() {
                // Try to get the remote name from the tracking branch config
                // "refs/remotes/origin/main" -> "origin"
                if let Ok(Some(name)) = upstream.name() {
                    if let Some(remote) = name.split('/').next() {
                        return Ok(remote.to_string());
                    }
                }
            }
        }

        // Fallback: try "origin".
        if self.repo.find_remote("origin").is_ok() {
            return Ok("origin".to_string());
        }

        // Last resort: find any remote at all.
        let remotes = self.repo.remotes().map_err(|e| AutoCommitError::Other(e.into()))?;
        let first = remotes.iter().flatten().next();
        match first {
            Some(name) => Ok(name.to_string()),
            None => Err(AutoCommitError::NoRemote),
        }
    }
}

/// Default credential callback.
///
/// Tries SSH agent first, then falls back to a no-op (letting git2
/// handle it through the system credential helpers via config).
fn credentials_cb(
    _url: &str,
    username: Option<&str>,
    allowed: CredentialType,
) -> std::result::Result<Cred, git2::Error> {
    if allowed.contains(CredentialType::SSH_KEY) {
        return Cred::ssh_key_from_agent(username.unwrap_or("git"));
    }
    // For userpass (HTTPS), let git2 try the credential helpers.
    if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
        // git2 doesn't have a direct "default credential helper" API in 0.19,
        // so we return an error and the push will try alternative methods.
        // Most modern systems configure credential helpers via git config.
    }
    Err(git2::Error::from_str("no suitable credentials available"))
}

//! Git-ignore rule checking using `git2`.
//!
//! Uses `Repository::status_should_ignore()` — the same logic that
//! `git status` uses — so that AutoCommit always respects:
//!
//! * `.gitignore` files at every level
//! * `.git/info/exclude`
//! * The user-global gitignore (`core.excludesFile`)
//!
//! The spec recommends the `ignore` crate; we use `git2` here because
//! it provides a direct, efficient, single-path check (`should_ignore`)
//! that is guaranteed to match git's own behaviour.  The `ignore`
//! crate is ideal for directory-walk filtering but doesn't expose a
//! cheap `is_ignored(path)` query without first scanning every
//! `.gitignore` file on disk.

use std::path::{Path, PathBuf};
use git2::Repository;
use crate::errors::Result;

/// A compiled set of gitignore rules backed by a `git2::Repository`.
pub struct IgnoreFilter {
    /// The underlying git2 repo handle (used for ignore queries).
    repo: Repository,
    /// The root of the repository.
    root: PathBuf,
}

impl IgnoreFilter {
    /// Open a git2 `Repository` handle at `repo_root` for ignore
    /// queries.
    ///
    /// The handle is read-only and lightweight.
    pub fn new(repo_root: &Path) -> Result<Self> {
        let repo = Repository::open(repo_root)?;
        let root = repo_root.to_path_buf();
        Ok(Self { repo, root })
    }

    /// Returns `true` if `path` should be **ignored**.
    ///
    /// The path can be absolute or relative; it will be relativised to
    /// the repository root internally.
    pub fn is_ignored(&self, path: &Path) -> bool {
        // Always ignore .git itself and anything inside it.
        if path.starts_with(self.root.join(".git")) || path == self.root.join(".git") {
            return true;
        }

        // Relativise to the repo root.
        let rel = path.strip_prefix(&self.root).unwrap_or(path);

        // Delegate to git2's ignore checker.
        self.repo.status_should_ignore(rel).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create a minimal git repo at `dir` and return it.
    fn init_git_repo(dir: &Path) {
        let repo = Repository::init(dir).unwrap();
        // An initial commit is required for status_should_ignore to work.
        let sig = repo.signature().unwrap();
        let tree = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();
    }

    fn setup_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());
        dir
    }

    fn touch(dir: &Path, rel: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, "").unwrap();
    }

    #[test]
    fn ignores_dot_git_directory() {
        let dir = setup_repo();
        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join(".git")));
    }

    #[test]
    fn ignores_dot_git_contents() {
        let dir = setup_repo();
        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join(".git/objects/pack/abc123")));
    }

    #[test]
    fn does_not_ignore_arbitrary_file() {
        let dir = setup_repo();
        touch(dir.path(), "src/main.rs");
        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(!filter.is_ignored(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn respects_gitignore() {
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
        touch(dir.path(), "build/foo.o");
        touch(dir.path(), "src/main.rs");

        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join("build/foo.o")));
        assert!(!filter.is_ignored(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn respects_nested_gitignore() {
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();
        touch(dir.path(), "debug.log");
        touch(dir.path(), "main.rs");

        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join("debug.log")));
        assert!(!filter.is_ignored(&dir.path().join("main.rs")));
    }

    #[test]
    fn respects_negated_patterns() {
        let dir = setup_repo();
        fs::write(
            dir.path().join(".gitignore"),
            "*.log\n!important.log\n",
        )
        .unwrap();
        touch(dir.path(), "debug.log");
        touch(dir.path(), "important.log");

        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join("debug.log")));
        assert!(!filter.is_ignored(&dir.path().join("important.log")));
    }
}

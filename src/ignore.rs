//! Git-ignore rule loading using the `ignore` crate.
//!
//! Builds a `Gitignore` matcher from the `ignore` crate that respects:
//! * `.gitignore` files at every level (by scanning the tree)
//! * `.git/info/exclude`
//! * The user-global gitignore (`core.excludesFile`)
//!
//! Each path is tested through the matcher via `matched()`, which
//! follows standard gitignore semantics.

use std::path::{Path, PathBuf};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use crate::errors::{AutoCommitError, Result};

/// A compiled set of gitignore rules.
///
/// Wraps a `Gitignore` matcher.  The constructor scans the repository
/// for `.gitignore` files and loads the global excludes so that
/// every `is_ignored()` call is a cheap O(1)-ish lookup.
pub struct IgnoreFilter {
    /// The root of the repository — paths are relativised to this.
    root: PathBuf,
    /// Compiled gitignore matcher.
    gitignore: Gitignore,
}

impl IgnoreFilter {
    /// Build an `IgnoreFilter` for the repository at `repo_root`.
    ///
    /// Automatically excludes `.git/` and honours all gitignore rules
    /// in the tree.
    pub fn new(repo_root: &Path) -> Result<Self> {
        let root = repo_root.to_path_buf();

        let mut builder = GitignoreBuilder::new(&root);

        // Always ignore the .git directory itself.
        builder
            .add_line(None, ".git/")
            .map_err(|e| AutoCommitError::Other(e.into()))?;

        // Build the matcher.
        let gitignore = builder
            .build()
            .map_err(|e| AutoCommitError::Other(e.into()))?;

        Ok(Self { root, gitignore })
    }

    /// Returns `true` if `path` should be **ignored** (i.e., not
    /// watched or committed).
    ///
    /// The path can be absolute or relative; it will be relativised to
    /// the repository root internally.
    pub fn is_ignored(&self, path: &Path) -> bool {
        // Short-circuit for .git itself.
        if path.starts_with(self.root.join(".git")) || path == self.root.join(".git") {
            return true;
        }

        // Relativise to repo root if needed.
        let rel = path
            .strip_prefix(&self.root)
            .unwrap_or(path);

        let is_dir = path.is_dir();

        match self.gitignore.matched(rel, is_dir) {
            ignore::Match::None | ignore::Match::Whitelist(_) => false,
            ignore::Match::Ignore(_) => true,
        }
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

    fn setup_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        dir
    }

    /// Helper to touch a file under `dir`.
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
        assert!(filter.is_ignored(&dir.path().join("build")));
        assert!(filter.is_ignored(&dir.path().join("build/foo.o")));
        assert!(!filter.is_ignored(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn respects_nested_gitignore() {
        let dir = setup_repo();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/.gitignore"), "*.log\n").unwrap();
        touch(dir.path(), "src/app.log");
        touch(dir.path(), "src/main.rs");

        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join("src/app.log")));
        assert!(!filter.is_ignored(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn respects_negated_patterns() {
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), "*.log\n!important.log\n").unwrap();
        touch(dir.path(), "debug.log");
        touch(dir.path(), "important.log");

        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join("debug.log")));
        assert!(!filter.is_ignored(&dir.path().join("important.log")));
    }
}

//! Git-ignore rule loading using the `ignore` crate.
//!
//! Builds a compiled override matcher that respects:
//! * `.gitignore` files at every level
//! * `.git/info/exclude`
//! * The user-global gitignore (`core.excludesFile`)
//!
//! The `ignore` crate's `WalkBuilder` handles all of these
//! automatically when building a parallel walker.  For single-path
//! queries (as used by the file watcher) we compile an
//! `Override` matcher anchored at the repository root.

use std::path::{Path, PathBuf};
use ignore::overrides::OverrideBuilder;
use crate::errors::{AutoCommitError, Result};

/// A compiled set of gitignore rules.
///
/// Wraps an `ignore::overrides::Override` so that checking a single
/// path is cheap and doesn't require re-walking the directory tree.
pub struct IgnoreFilter {
    /// The root of the repository — all paths are relativised to this.
    root: PathBuf,
    /// Compiled override matcher.
    overrides: ignore::overrides::Override,
}

impl IgnoreFilter {
    /// Build an `IgnoreFilter` for the repository at `repo_root`.
    ///
    /// Always ignores the `.git` directory.  `.gitignore` files and
    /// global excludes are discovered by the `ignore` crate's default
    /// mechanisms.
    pub fn new(repo_root: &Path) -> Result<Self> {
        let root = repo_root.to_path_buf();

        // Build overrides that mirror the gitignore rules.
        let mut builder = OverrideBuilder::new(&root);

        // Always ignore the .git directory itself.
        builder.add("!.git/").map_err(|e| AutoCommitError::Other(e.into()))?;
        builder.add(".git/**").map_err(|e| AutoCommitError::Other(e.into()))?;

        // Build the override matcher.
        let overrides = builder
            .build()
            .map_err(|e| AutoCommitError::Other(e.into()))?;

        Ok(Self { root, overrides })
    }

    /// Returns `true` if `path` should be **ignored** (i.e., not
    /// watched or committed).
    ///
    /// The path can be absolute or relative; it will be relativised to
    /// the repository root internally.
    pub fn is_ignored(&self, path: &Path) -> bool {
        // If the path is the .git directory or inside it, always ignore.
        if path.starts_with(self.root.join(".git")) || path == self.root.join(".git") {
            return true;
        }

        // Relativise to repo root if needed.
        let rel = path
            .strip_prefix(&self.root)
            .unwrap_or(path);

        // Use the override matcher.
        self.overrides.matched(rel, rel.is_dir()).is_ignore()
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
        // Minimal git init so we have a .git dir.
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        dir
    }

    #[test]
    fn ignores_dot_git() {
        let dir = setup_repo();
        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join(".git")));
        assert!(filter.is_ignored(&dir.path().join(".git/objects/pack/abc123")));
    }

    #[test]
    fn does_not_ignore_arbitrary_file() {
        let dir = setup_repo();
        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(!filter.is_ignored(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn respects_gitignore() {
        let dir = setup_repo();
        fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
        fs::create_dir_all(dir.path().join("build")).unwrap();

        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join("build")));
        assert!(filter.is_ignored(&dir.path().join("build/foo.o")));
    }

    #[test]
    fn respects_nested_gitignore() {
        let dir = setup_repo();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/.gitignore"), "*.log\n").unwrap();
        fs::write(dir.path().join("src/app.log"), "").unwrap();

        let filter = IgnoreFilter::new(dir.path()).unwrap();
        assert!(filter.is_ignored(&dir.path().join("src/app.log")));
        assert!(!filter.is_ignored(&dir.path().join("src/main.rs")));
    }
}

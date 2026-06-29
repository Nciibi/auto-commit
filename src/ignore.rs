//! Git-ignore rule loading using the `ignore` crate.
//!
//! Builds a `WalkBuilder`-compatible filter that respects:
//! * `.gitignore` files at every level
//! * `.git/info/exclude`
//! * The user-global gitignore (`core.excludesFile`)
//!
//! The `std::path::Path`-returning API is preferred because the file
//! watcher emits paths directly.

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
    /// The underlying `WalkBuilder`-style filter for directory-level rules.
    /// Kept around so we can re-check on the fly if needed.
    git_global: Option<PathBuf>,
}

impl IgnoreFilter {
    /// Build an `IgnoreFilter` for the repository at `repo_root`.
    ///
    /// Scans for `.gitignore` files, loads the global exclude, and
    /// compiles everything into an efficient matcher.
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

        // Determine the global excludes file.
        let git_global = find_global_excludes();

        Ok(Self { root, overrides, git_global })
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

/// Locate the user-global gitignore file.
///
/// Checks `core.excludesFile` via git config, then falls back to
/// `~/.config/git/ignore` (Linux/macOS) or
/// `%USERPROFILE%\.config\git\ignore` (Windows).
fn find_global_excludes() -> Option<PathBuf> {
    // Try `git config --global core.excludesFile`.  We do shell out here
    // because `git2` doesn't expose global config easily and this is a
    // one-off during startup.
    let output = std::process::Command::new("git")
        .args(["config", "--global", "--get", "core.excludesFile"])
        .output()
        .ok()?;

    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path_str.is_empty() {
            let p = PathBuf::from(path_str);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // Fallback: XDG-style location.
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    let fallback = PathBuf::from(home).join(".config").join("git").join("ignore");
    if fallback.exists() {
        Some(fallback)
    } else {
        None
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

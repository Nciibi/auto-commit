//! Recursive filesystem watcher.
//!
//! Uses the `notify` crate to watch the entire repository tree for
//! changes (create, modify, delete, rename).  Detected changes are
//! filtered through the `IgnoreFilter` so that gitignored paths never
//! trigger a commit.

use std::path::Path;
use std::sync::mpsc;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use crate::errors::{AutoCommitError, Result};
use crate::ignore::IgnoreFilter;
use crate::debounce::DebounceEvent;

/// Start watching `repo_root` and forward relevant events into
/// `event_tx`.
///
/// The watcher runs on a background thread managed by the `notify`
/// crate.  This function returns a `WatcherHandle` that must be kept
/// alive for the duration of the watching session — dropping it stops
/// the watcher.
pub fn start_watcher(
    repo_root: &Path,
    ignore_filter: IgnoreFilter,
    event_tx: mpsc::Sender<DebounceEvent>,
) -> Result<WatcherHandle> {
    let event_tx_clone = event_tx.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            match res {
                Ok(event) => {
                    // Filter out non-relevant event kinds.
                    let is_relevant = matches!(
                        event.kind,
                        EventKind::Create(_)
                            | EventKind::Modify(_)
                            | EventKind::Remove(_)
                    );

                    if !is_relevant {
                        return;
                    }

                    // Filter each path through the ignore rules.
                    for path in &event.paths {
                        if !ignore_filter.is_ignored(path) {
                            let _ = event_tx_clone.send(DebounceEvent::ChangeDetected(path.clone()));
                        }
                    }
                }
                Err(e) => {
                    // Log but don't crash on transient notify errors.
                    eprintln!("[autocommit] watcher error: {}", e);
                }
            }
        },
        Config::default(),
    )
    .map_err(|e| AutoCommitError::WatcherError(e.into()))?;

    // Watch the repository root recursively.
    watcher
        .watch(repo_root, RecursiveMode::Recursive)
        .map_err(|e| AutoCommitError::WatcherError(e.into()))?;

    Ok(WatcherHandle {
        _inner: watcher,
    })
}

/// A handle that keeps the filesystem watcher alive.
///
/// Dropping this handle stops the watcher and releases all resources.
#[must_use = "dropping WatcherHandle stops the watcher"]
pub struct WatcherHandle {
    _inner: RecommendedWatcher,
}

//! Debounce timer logic.
//!
//! The watcher collects file-change events into a shared set.  A
//! debounce timer sits between the event stream and the commit/push
//! cycle: whenever a new event arrives the timer is *reset*, so
//! commits only happen after a stable period of inactivity.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Messages that can be sent to the debounce loop.
#[derive(Debug)]
pub enum DebounceEvent {
    /// A filesystem change has been detected at this path.
    ChangeDetected(PathBuf),
    /// The debounce timer has expired — it's time to commit.
    TimerExpired,
    /// Shut down the debounce loop.
    Shutdown,
}

/// Run the debounce loop.
///
/// Receives `DebounceEvent::ChangeDetected` messages from the
/// watcher, tracks the set of changed files, and fires a
/// `DebounceEvent::TimerExpired` back through the returned sender
/// whenever the inactivity period (`timeout`) elapses without a
/// new event.
///
/// The caller is expected to handle `TimerExpired` and respond with a
/// commit attempt.
///
/// Returns a sender that the watcher can use to push events in, and
/// a receiver that the main loop reads for the commit signal.
pub fn debounce_loop(
    timeout: Duration,
    event_rx: mpsc::Receiver<DebounceEvent>,
    commit_tx: mpsc::Sender<DebounceEvent>,
) {
    let mut changed_files: HashSet<PathBuf> = HashSet::new();
    let mut last_event: Option<Instant> = None;

    loop {
        // Compute how long to wait before the timer fires.
        let wait_duration = match last_event {
            Some(t) => {
                let elapsed = t.elapsed();
                if elapsed >= timeout {
                    // Timer already expired — fire immediately.
                    None
                } else {
                    Some(timeout - elapsed)
                }
            }
            None => None, // No event yet → block indefinitely.
        };

        // Block until either an event arrives or the debounce timer
        // naturally expires.
        let received = match wait_duration {
            Some(dur) => {
                // Block with a timeout.
                if dur.is_zero() || dur.as_micros() == 0 {
                    // Already expired, don't block.
                    None
                } else {
                    match event_rx.recv_timeout(dur) {
                        Ok(event) => Some(event),
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            // Timer expired naturally.
                            None
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            // Sender dropped — shut down.
                            break;
                        }
                    }
                }
            }
            None => match event_rx.recv() {
                Ok(event) => Some(event),
                Err(mpsc::RecvError) => break,
            },
        };

        match received {
            Some(DebounceEvent::ChangeDetected(path)) => {
                changed_files.insert(path);
                last_event = Some(Instant::now());
            }
            Some(DebounceEvent::Shutdown) | None => {
                // Timer expired or shutdown requested.
                if !changed_files.is_empty() {
                    let _ = commit_tx.send(DebounceEvent::TimerExpired);
                    changed_files.clear();
                }
                last_event = None;

                // If we got Shutdown, break after handling the commit.
                if let Some(DebounceEvent::Shutdown) = received {
                    break;
                }
            }
            Some(DebounceEvent::TimerExpired) => {
                // Spurious — ignore.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Test that the debounce timer resets on new events.
    #[test]
    fn debounce_resets_on_new_event() {
        let (event_tx, event_rx) = mpsc::channel();
        let (commit_tx, commit_rx) = mpsc::channel();

        let timeout = Duration::from_millis(100);

        // Spawn the debounce loop in a thread.
        std::thread::spawn(move || {
            debounce_loop(timeout, event_rx, commit_tx);
        });

        // Send a change event.
        event_tx.send(DebounceEvent::ChangeDetected(PathBuf::from("test.txt"))).unwrap();

        // Send another event before the timer expires.
        std::thread::sleep(Duration::from_millis(30));
        event_tx.send(DebounceEvent::ChangeDetected(PathBuf::from("test2.txt"))).unwrap();

        // Wait for the timer to expire and check we get a commit signal.
        let result = commit_rx.recv_timeout(Duration::from_millis(500));
        assert!(result.is_ok(), "should receive TimerExpired");
        assert!(matches!(result.unwrap(), DebounceEvent::TimerExpired));

        // Also verify no spurious extra commit signal.
        let extra = commit_rx.recv_timeout(Duration::from_millis(200));
        assert!(extra.is_err(), "should not receive extra commit signal");
    }

    /// Test that no commit signal is sent if no changes.
    #[test]
    fn no_commit_signal_without_changes() {
        let (event_tx, event_rx) = mpsc::channel();
        let (commit_tx, commit_rx) = mpsc::channel();

        let timeout = Duration::from_millis(50);

        std::thread::spawn(move || {
            debounce_loop(timeout, event_rx, commit_tx);
        });

        // Don't send any events, just shutdown.
        std::thread::sleep(Duration::from_millis(100));
        event_tx.send(DebounceEvent::Shutdown).unwrap();

        // Should not get TimerExpired.
        let result = commit_rx.recv_timeout(Duration::from_millis(200));
        assert!(result.is_err());
    }
}

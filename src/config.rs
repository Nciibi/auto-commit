//! Configuration defaults for AutoCommit.
//!
//! Currently hard-coded; designed so a config-file or CLI-override layer
//! can be added later without changing call sites (see "Future
//! Extensibility" in the spec).

use std::time::Duration;

/// Default inactivity (debounce) duration before a commit is created.
pub const DEBOUNCE_TIMEOUT: Duration = Duration::from_secs(5);

/// Returns the debounce timeout, respecting an optional CLI override.
///
/// When no override is provided, the default (5 s) is returned.
pub fn debounce_timeout(cli_override_secs: Option<u64>) -> Duration {
    cli_override_secs
        .map(Duration::from_secs)
        .unwrap_or(DEBOUNCE_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_debounce_is_five_seconds() {
        assert_eq!(DEBOUNCE_TIMEOUT, Duration::from_secs(5));
    }

    #[test]
    fn cli_override_takes_precedence() {
        assert_eq!(debounce_timeout(Some(3)), Duration::from_secs(3));
        assert_eq!(debounce_timeout(None), DEBOUNCE_TIMEOUT);
    }
}

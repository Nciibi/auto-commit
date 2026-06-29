//! Semantic-version parsing and PATCH-number incrementing.
//!
//! The latest commit message is expected to contain a version like
//! `v0.2.1`.  We parse it, increment only the PATCH component, and
//! produce the next version string.

use semver::Version;

/// Attempt to extract a `semver::Version` from a commit-message string.
///
/// The message is expected to be a bare version prefixed with `v`,
/// e.g. `"v0.2.1"`.  Everything after the `v` is parsed with the
/// standard semver crate.
///
/// Returns `None` when the message doesn't look like a version at all
/// (which is *not* an error — the first commit simply gets
/// `v0.0.1`).
pub fn parse_version(message: &str) -> Option<Version> {
    let trimmed = message.trim();
    if let Some(rest) = trimmed.strip_prefix('v') {
        Version::parse(rest).ok()
    } else {
        None
    }
}

/// Increment the PATCH component of a `Version`, returning a new
/// `Version`.
///
/// `v0.2.1` → `v0.2.2`,  `v1.8.99` → `v1.8.100`.
pub fn increment_patch(version: &Version) -> Version {
    Version::new(version.major, version.minor, version.patch + 1)
}

/// Format a `Version` back into the `vMAJOR.MINOR.PATCH` string used
/// as a commit message.
pub fn format_version(version: &Version) -> String {
    format!("v{}", version)
}

/// The very first version when no prior versioned commit exists.
pub const INITIAL_VERSION: &str = "v0.0.1";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_semver_with_v_prefix() {
        let v = parse_version("v0.2.1").expect("should parse");
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 1);
    }

    #[test]
    fn parses_version_without_leading_v() {
        assert!(parse_version("0.2.1").is_none());
    }

    #[test]
    fn parses_version_with_trailing_text() {
        assert!(parse_version("v0.2.1\n\nsome commit body").is_none());
    }

    #[test]
    fn parses_trimmed_message() {
        let v = parse_version("  v1.8.99  ").expect("should parse after trim");
        assert_eq!(v.patch, 99);
    }

    #[test]
    fn non_version_message_returns_none() {
        assert!(parse_version("fix: typo").is_none());
        assert!(parse_version("").is_none());
        assert!(parse_version("v").is_none());
        assert!(parse_version("vabc").is_none());
    }

    #[test]
    fn increments_patch_only() {
        let v = Version::new(0, 2, 1);
        let next = increment_patch(&v);
        assert_eq!(next.major, 0);
        assert_eq!(next.minor, 2);
        assert_eq!(next.patch, 2);
    }

    #[test]
    fn increments_patch_wrapping_within_patch() {
        let v = Version::new(1, 8, 99);
        let next = increment_patch(&v);
        assert_eq!(next.major, 1);
        assert_eq!(next.minor, 8);
        assert_eq!(next.patch, 100);
    }

    #[test]
    fn increments_initial_version() {
        let v = Version::new(0, 0, 0);
        let next = increment_patch(&v);
        assert_eq!(format_version(&next), "v0.0.1");
    }

    #[test]
    fn formats_correctly() {
        let v = Version::new(3, 0, 0);
        assert_eq!(format_version(&v), "v3.0.0");
    }

    #[test]
    fn large_patch_increment() {
        let v = Version::new(5, 10, 3);
        let next = increment_patch(&v);
        assert_eq!(format_version(&next), "v5.10.4");
    }

    #[test]
    fn initial_version_constant_is_correct() {
        assert_eq!(INITIAL_VERSION, "v0.0.1");
    }
}

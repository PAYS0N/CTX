//! The deny-by-default access gate (pure; the security boundary).
//!
//! A path is accessible iff it is not gitignored AND not a secret AND
//! not binary. Gitignore status is supplied by the caller (each crate's
//! own `is_ignored`); the secret/binary predicates here are pure name/
//! extension rules. Callers enforce this even when a path is explicitly
//! requested, so a careless `read .env` is impossible, not merely
//! discouraged. This module is the ONLY copy of these rules.

use std::collections::BTreeSet;
use std::path::Path;

// Secret/binary are absolute hard denials. A new untracked-but-not-
// ignored source file is legitimately readable/writable, so the gate
// keys on *gitignored*, not on "untracked".

/// Lowercase final extension of `path`, or empty.
fn ext(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Final path component, or the whole string.
fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Secret material that must never reach a model or a summary.
#[must_use]
pub fn is_secret(path: &str) -> bool {
    let name = base(path);
    name == ".env"
        || name.starts_with(".env.")
        || matches!(
            name,
            "id_rsa" | "id_ed25519" | "id_dsa" | ".netrc" | ".npmrc"
        )
        || matches!(
            ext(path).as_str(),
            "pem" | "key" | "p12" | "pfx" | "keystore" | "jks" | "asc"
        )
}

/// Extensions treated as non-text (a code reader/summarizer has no use
/// for them).
const BINARY_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "pdf", "zip", "gz", "tgz", "bz2", "xz",
    "tar", "7z", "rar", "exe", "dll", "so", "dylib", "a", "o", "rlib", "bin", "wasm", "class",
    "jar", "woff", "woff2", "ttf", "otf", "mp3", "mp4", "mov",
];

/// Non-text content a code summarizer/reader has no use for.
#[must_use]
pub fn is_binary(path: &str) -> bool {
    BINARY_EXTS.contains(&ext(path).as_str())
}

/// Why `path` is refused, or `None` if accessible. Neutral string so
/// each crate can wrap it in its own typed error. `ignored` is the
/// caller's gitignore check for `path`.
#[must_use]
pub fn deny_reason(path: &str, ignored: bool) -> Option<&'static str> {
    if is_secret(path) {
        Some("secret")
    } else if is_binary(path) {
        Some("binary")
    } else if ignored {
        Some("gitignored")
    } else {
        None
    }
}

/// The accessible subset of `tracked`, sorted — the manifest's body.
/// (Tracked paths are not gitignored, so only secret/binary filter.)
#[must_use]
pub fn accessible_set(tracked: &BTreeSet<String>) -> Vec<String> {
    tracked
        .iter()
        .filter(|p| !is_secret(p) && !is_binary(p))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{accessible_set, deny_reason, BTreeSet};

    #[test]
    fn secret_binary_and_gitignored_are_denied() {
        assert_eq!(deny_reason(".env", false), Some("secret"));
        assert_eq!(deny_reason("a/b/.env.local", false), Some("secret"));
        assert_eq!(deny_reason("deploy/server.pem", false), Some("secret"));
        assert_eq!(deny_reason("assets/logo.png", false), Some("binary"));
        assert_eq!(deny_reason("src/lib.rs", true), Some("gitignored"));
        assert_eq!(deny_reason("src/lib.rs", false), None);
    }

    #[test]
    fn accessible_set_filters_secret_and_binary() {
        let tracked: BTreeSet<String> = ["src/a.rs", ".env", "logo.png", "Cargo.toml"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let acc = accessible_set(&tracked);
        assert!(acc.contains(&"src/a.rs".to_owned()));
        assert!(acc.contains(&"Cargo.toml".to_owned()));
        assert!(!acc.iter().any(|p| p == ".env" || p == "logo.png"));
    }
}

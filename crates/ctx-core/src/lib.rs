//! Shared, dependency-free primitives for the CTX tooling.
//!
//! Currently the single source of truth for the access gate. It lives
//! here, not duplicated per crate, because a divergent secret denylist
//! between `ctx-access` and `ctx-summarize` would be a silent
//! secret-exfiltration bug (see DECISIONS ADR-023). Each consumer maps
//! the neutral deny reason into its own typed error.

pub mod access;

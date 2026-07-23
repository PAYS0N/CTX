//! Shared foundational primitives for the CTX tooling.
//!
//! Home for definitions that more than one tool must agree on exactly,
//! so a divergent copy can't drift into a silent bug. The access gate
//! ([`access`]) is the single source of truth for the secret/binary
//! denylist (a divergent copy is a secret-exfiltration bug — DECISIONS
//! ADR-023). The content-hash sidecar schema ([`hashtree`]) is shared by
//! the generator (`ctx-scan`) and the chain server (`ctx-context`) so a
//! hash recorded by one is comparable by the other. The `docs/STATUS.md`
//! table model ([`status_table`]) is shared by `ctx-brief` (matching a
//! request against the table) and `ctx-status` (migrating and rendering
//! it), so the one parser/escaper is never copied. Each consumer maps
//! neutral results into its own typed error.

pub mod access;
pub mod hashtree;
pub mod status_table;

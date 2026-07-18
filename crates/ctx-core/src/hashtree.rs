//! The content-hash sidecar schema shared by the tree generator
//! (`ctx-scan`) and the chain server (`ctx-context`).
//!
//! `DirNode` is the on-disk shape of a `.context/<dir>/hashes.json`
//! sidecar; `hex_hash` is the one hash function both sides must agree on.
//! The definitions live here, not duplicated per crate, because a
//! divergent hash or schema would make every freshness comparison
//! silently wrong — the generator would record one value and the server
//! would compute another. `ctx-scan` aggregates these into a whole-tree
//! diff; `ctx-context` reads a single sidecar to tag a served node.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One directory's hash node as stored in its `hashes.json` sidecar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirNode {
    /// Aggregate hash over the sorted `children` entries.
    pub hash: String,
    /// Child name -> `f:<hex>` (file) or `d:<hex>` (subdirectory).
    pub children: BTreeMap<String, String>,
}

/// Hex SHA-256 of `bytes`. The single hash both the generator and the
/// server compute, so a leaf entry written by one is comparable by the
/// other.
#[must_use]
pub fn hex_hash(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        // Writing a byte as hex into a String is infallible; the Result
        // is discarded rather than unwrapped (unwrap is banned).
        let r = write!(s, "{b:02x}");
        if r.is_err() {}
    }
    s
}

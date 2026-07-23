//! Typed error taxonomy for ctx-status.

use thiserror::Error;

/// Every failure surfaced by the runner and its boundaries.
#[derive(Debug, Error)]
pub enum StatusError {
    /// `impact` was not one of the fixed known values.
    #[error("impact must be one of high|medium|low, got {0:?}")]
    BadImpact(String),
    /// `difficulty` was not one of the fixed known values.
    #[error("difficulty must be one of easy|medium|hard, got {0:?}")]
    BadDifficulty(String),
    /// The JSON store could not be decoded.
    #[error("store at {path:?} is not valid JSON: {detail}")]
    StoreCorrupt {
        /// Path of the unreadable store.
        path: String,
        /// Human-readable cause.
        detail: String,
    },
    /// A one-time migration was attempted against a store that already
    /// has content (migration is additive-bootstrap, not a merge).
    #[error("store at {0:?} already has entries; migration only seeds an empty store")]
    StoreNotEmpty(String),
    /// An underlying filesystem operation failed.
    #[error("io error on {path:?}: {detail}")]
    Io {
        /// Path the failed operation targeted.
        path: String,
        /// Human-readable cause.
        detail: String,
    },
}

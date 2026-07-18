//! The agent-facing tool contract for `ctx-scan`.

/// One-paragraph, agent-facing contract for this binary.
///
/// Single source of truth: the generated tool-contract block in
/// `CLAUDE.md`/`README.md` is assembled from `--contract` output, and the
/// `contracts` battery check fails if that block drifts from this string.
pub const CONTRACT: &str = "ctx-scan <dir> maintains the `.context/` \
summary tree beside the source, using a content-hash tree (not git) to \
decide staleness. `--check` reports stale directories and leaves, plus \
expected summaries that are missing (never generated or hand-deleted), \
without calling the model; `--update` regenerates only the \
stale leaves and rollups, then rewrites the hash sidecars; `--dry-run` \
lists the files in scope; `--stop-hook` reports staleness as a Claude \
Code Stop `systemMessage` and always exits 0 (fail-open). Regeneration \
is a post-session concern — the hook never bills the model.";

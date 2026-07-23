//! The agent-facing tool contract for `ctx-status`.

/// One-paragraph, agent-facing contract for this binary.
///
/// Single source of truth: the generated tool-contract block in
/// `CLAUDE.md`/`README.md` is assembled from `--contract` output, and the
/// `contracts` battery check fails if that block drifts from this string.
pub const CONTRACT: &str = "ctx-status list prints the current backlog \
from the JSON store at `docs/status.json` (source of truth), sorted by \
impact (high → low) then difficulty (easy → hard) within each band — the \
on-demand way an agent surfaces priorities, no hook required. `ctx-status \
add-task <description> --task <title> --impact <high|medium|low> \
--difficulty <easy|medium|hard>` appends one row — never reordering, \
editing, or deleting existing ones, preserving operator curation \
authority — and regenerates `docs/STATUS.md` from the store in the same \
step, so the human-readable view can never drift from what the store \
holds. `--task` is required: omitting it would otherwise duplicate the \
full description into the task column too.";

//! The agent-facing tool contract for `ctx-brief`.

/// One-paragraph, agent-facing contract for this binary.
///
/// Single source of truth: the generated tool-contract block in
/// `CLAUDE.md`/`README.md` is assembled from `--contract` output, and the
/// `contracts` battery check fails if that block drifts from this string.
pub const CONTRACT: &str = "ctx-brief [--headless] (<request> | --id <id>) \
turns a backlog item into a self-contained task brief for `ctx-cage \
--task-file`. `<request>` is matched as a case-insensitive substring of \
`docs/STATUS.md`'s task column, or used as the raw text when nothing \
matches; `--id <id>` instead looks the row up directly by its stable id \
in `docs/status.json` (the only place that id lives), erroring if none \
matches. Exactly one of `<request>`/`--id` is required. It runs two \
subscription-billed `claude` stages inside the target repo so that repo's \
own context hooks ground every read: a cheap read-only gather pass \
(`--gather-model`, default haiku) produces a verified dossier (state, \
constraints, waypoints, unknowns), then a plan pass composes the brief — \
interviewing the human on open decisions by default, or (`--headless`) \
adjudicating tactical decisions itself and escalating doctrinal ones. The \
brief is written to `.context/.reports/briefs/<slug>.md` (`item-<id>.md` \
when resolved by `--id`; never pruned by ctx-scan) unless `--out` \
overrides it, and its path is printed for the `ctx-cage` hand-off.";

Doctrine: `.context/intent.md`. Rationale:
`docs/DECISIONS.md`. Current focus: `docs/STATUS.md`.

- Source: native Read/Edit/Grep — never `cat` or `sed` for file reads. The context
  chain is hook-injected on read; request it on demand with
  `target/debug/ctx-context <path>` (dir or `.`), and read a directory's
  chain before changing its contents. Go only as deep as needed. Never
  hand-edit `.context/`.
- Do not run cargo yourself; use `target/debug/ctx-verify [crate]`. If
  cargo is unavoidable: `-q --message-format=short`, never paste build dumps.

<!-- BEGIN GENERATED tool-contracts (scripts/gen_tool_contracts.sh --write) -->
- **ctx-context** — ctx-context <path> prints the context chain an agent must read before touching <path>: the ancestor rollup.ctx + intent.md at each directory level, plus the file's own leaf .ctx for a file target (`.` targets the repo root). Read-only and fail-open — a missing node renders as an explicit `(absent: …)` marker, never an error; a served summary whose source changed since the last regen is prefixed `[STALE …]`, and one whose source exists but was never summarized `[NEVER GENERATED …]`. `--hook` reads a Claude Code PostToolUse event from stdin and emits deduplicated additional-context for the session.
- **ctx-verify** — ctx-verify [crate] is the agent checkpoint: it applies `cargo fmt`, then builds, lints (clippy + rustdoc, warnings denied), tests, and runs the repo's script battery in one call; an optional crate name scopes the cargo-based checks. The default terse render prints the single word `pass` when every check passed, otherwise one FAIL:/ERROR:/SKIP: block per failing, errored, or skipped (missing-tool) check — a skipped check is never a silent pass — and the `{"status":"pass"}` JSON envelope is emitted only under `--json` and only when every check passed. Serving fails open; this gate fails closed.
- **ctx-scan** — ctx-scan <dir> maintains the `.context/` summary tree beside the source, using a content-hash tree (not git) to decide staleness. `--check` reports stale directories and leaves, expected summaries that are missing (never generated or hand-deleted), and orphaned artifacts (summaries/sidecars whose source was deleted or scoped out), without calling the model; `--prune` deletes the orphaned artifacts and sweeps emptied mirror directories, also model-free; `--update` prunes, regenerates only the stale leaves and rollups, then rewrites the hash sidecars (`intent.md` and `.context/.cache|.reports` are never pruned); `--dry-run` lists the files in scope; `--stop-hook` reports staleness as a Claude Code Stop `systemMessage` and always exits 0 (fail-open). Regeneration is a post-session concern — the hook never bills the model.
- **ctx-cage** — ctx-cage <target> runs an agent subprocess in an offline sandbox over the target project — bwrap with a masked filesystem, fresh namespaces, and no egress except a proxied API relay — and guarantees teardown. Billed modes (`--task`/`--task-file`, or the interactive default) require `--allow-spend` or `CTX_CAGE_ALLOW_SPEND=1`; `--self-test stub` is the always-available no-spend, no-network containment probe.
- **ctx-brief** — ctx-brief [--headless] (<request> | --id <id>) turns a backlog item into a self-contained task brief for `ctx-cage --task-file`. `<request>` is matched as a case-insensitive substring of `docs/STATUS.md`'s task column, or used as the raw text when nothing matches; `--id <id>` instead looks the row up directly by its stable id in `docs/status.json` (the only place that id lives), erroring if none matches. Exactly one of `<request>`/`--id` is required. It runs two subscription-billed `claude` stages inside the target repo so that repo's own context hooks ground every read: a cheap read-only gather pass (`--gather-model`, default haiku) produces a verified dossier (state, constraints, waypoints, unknowns), then a plan pass composes the brief — interviewing the human on open decisions by default, or (`--headless`) adjudicating tactical decisions itself and escalating doctrinal ones. The brief is written to `.context/.reports/briefs/<slug>.md` (`item-<id>.md` when resolved by `--id`; never pruned by ctx-scan) unless `--out` overrides it, and its path is printed for the `ctx-cage` hand-off.
- **ctx-status** — ctx-status list prints the current backlog from the JSON store at `docs/status.json` (source of truth), each row prefixed with its id, sorted by impact (high → low) then difficulty (easy → hard) within each band — the on-demand way an agent surfaces priorities, no hook required. `ctx-status add-task <description> --task <title> --impact <high|medium|low> --difficulty <easy|medium|hard>` appends one row under a freshly assigned id — never reordering or editing existing ones — and regenerates `docs/STATUS.md` from the store in the same step, so the human-readable view can never drift from what the store holds. `--task` is required: omitting it would otherwise duplicate the full description into the task column too. `ctx-status delete-task <id>` removes the row with that id — run `list` first to find it — refusing if no row has it, and regenerates `docs/STATUS.md` in the same step. Ids are internal to `ctx-status`: they never appear in `docs/STATUS.md` itself, whose 4-column shape stays shared with `ctx-brief`.
<!-- END GENERATED tool-contracts -->

- Lints: `#[allow]` is banned. unwrap/expect compile only inside
  `#[test]`/`#[cfg(test)]` bodies — test helpers outside them must
  return `Result`. A 30-line fn / 250-line file: refactor first; a
  single-line `// rationale:` directly above (fn) or after `//!`
  (file) is the last resort, and multi-line is not recognized.
- `.env` holds the summarizer key: never feed it to a model, never
  commit it. `.cagevars` (also gitignored) is separate: non-secret
  `ctx-cage` sandbox-inclusion config, loaded automatically from CWD at
  cage startup — never put secrets in it, and never move `.env`'s
  contents into it. `CTX_CAGE_EXTRA_PATH` binds extra host dirs; every
  other `KEY=VALUE` line is exported into the caged agent's own
  environment (ADR-058), except `CTX_CAGE_ALLOW_SPEND` (never read
  from here) and any key the cage already manages, which hard-errors
  the run instead of silently overriding it.
- `template/` and root workspace lint configs mirror each other;
  change both.
- Retiring a tool/identifier: add it to `scripts/retired_terms_check.sh`'s
  `BANNED` array and record the ADR in `docs/DECISIONS.md`.
- when debugging, always ensure that the thing you are changing is correct. 
  Do not reach a logical conclusion and change code; verify the issue.
- the task is not done until ctx-verify passes.

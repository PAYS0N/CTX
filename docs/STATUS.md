# Status

Architecture is derived: `ctx-context .`.
Rationale: `docs/DECISIONS.md`.

Sorted by impact (high → low), then difficulty (easy → hard) within each
impact band.

| task | description | impact | difficulty |
|---|---|---|---|
| mechanical validation of generated summaries | nothing checks generator output before it is written and served: line budgets, banned/hedging phrases, truncation, or cheap factual sanity (root rollup says "five Rust crates"; six exist — persisted into README's generated block). Add a write-time lint in ctx-scan with retry-once | high | medium |
| force template to be kept in sync | nothing enforces that template is kept up to date; copying it should get you the full system. | high | medium |
| wire the Layer 3 auditor | `prompts/auditor.md` and the `intent_divergence:` label exist but nothing invokes the audit against regenerated rollups; the root rollup itself flags this divergence. Wire it into the regeneration flow | high | medium |
| work on summarization prompts | do research research into how to modify the prompts to convey more important info | high | hard |
| phase 5 e2e smoke fixture | throwaway workspace, buggy file + failing test, generated tree, `--stub` and billed modes; asserts hook injection, native Edit, `ctx-verify` pass, post-session rollup regen, no egress beyond proxy, no writes outside workspace; include degraded-tree cases (stale, absent, truncated) | high | hard |
| clean rollup prompts | prevent text like 'No intent.md exists for this directory.', just omit it. | medium | easy |
| remove ctx-brief from claude.md | only tools relevant to an agent should be included | medium | easy |
| move model choice into rust | model choice should be passed to the tool, not an env var. `ctx-brief` (ADR-054) already takes `--gather-model`/`--plan-model` flags; the summarizer path (`ctx-scan`/`ctx-summarize`) still reads the model from the environment and remains open | medium | easy |
| summarizer adapter: handle truncation | `agents/summarizer-claude.py` never checks `stop_reason`, so a `max_tokens` cut is written to disk as truth; `CTX_AGENT_TEMPERATURE` is documented in the docstring but never read (dead option) | medium | easy |
| hook matcher: add Edit\|Write | `.claude/settings.json` matches `Read\|Grep\|Glob` only, though `hook.rs` already parses `file_path` from Edit events; an agent that Writes a new file gets no chain. Also document post-compaction recovery ("if you've lost the chain, run `ctx-context <path>`") | medium | easy |
| ctx-verify: scoped runs aren't scoped for doc/test | `effective_args` inserts `-p <pkg>` but leaves `--workspace` in the doc and test specs (`checks.rs:103,113`), so `ctx-verify <crate>` still docs/tests the whole workspace; strip `--workspace` when a package is given, and pin the real cargo flag semantics with one boundary-contract test | medium | easy |
| template/root sync check in the battery | doctrine says the lint configs mirror, but no script compares them (comments in the Cargo.toml lint tables already differ); every "must stay in sync" claim should name its checking script | medium | easy |
| fix .ctxignore scope inversion | `Cargo.lock` (584 lines) is summarized — re-billed on every dep bump — while `*.toml` excludes `Cargo.toml`/`clippy.toml`, the load-bearing lint regime; swap them | medium | easy |
| ctx-core test gaps | `hashtree.rs` (shared writer/reader schema; "silently breaks" on mismatch) has zero in-crate tests; `access.rs` has two. Concrete case: `is_secret` compares the basename case-sensitively, so `.ENV` evades the secret gate | medium | easy |
| add visual feedback during context generation and while waiting on ctx-brief| medium | easy |
| fix ctx-scan flags | --dry-run and --check return different vals. Should --dry-run be retired? What is the difference? | medium | medium |
| inject verify and context output to start of run | print `ctx-verify` and `ctx-context` results at session start to ground the agent in actual tree state and recent check results | medium | medium |
| make sure interrupting context regen doesn't break anything | verify that stopping mid-scan (SIGINT/timeout) leaves `.context/` in a consistent state; no partial hashes, no orphaned sidecars | medium | medium |
| stop consuming tools from target/debug | the hook, CLAUDE.md, and permissions all point at debug binaries with `2>/dev/null \|\| true`; a fresh clone serves no context and a stale binary serves old behavior (the ADR-035 incident class). Install to a versioned path or add a binary-staleness check | medium | medium |
| prompt version in freshness | content hashes don't cover the summarizer prompts, so a prompt rewrite never invalidates existing summaries — `.context/crates/ctx-core/src/access.rs.ctx` still carries the pre-ADR-039 YAML format. Fold a prompt hash into the freshness input | medium | medium |
| resolve cpath/RepoPath duplication | the `.context` mirror-path logic is still duplicated between `ctx-summarize/src/cpath.rs` and `ctx-context/src/repo_path.rs`+`chain.rs` (ADR-020's deferred debt, survived the rename); generated rollups cite the retired `docs/UNIMPLEMENTED.md` as its tracker | medium | medium |
| integrate status more closely into the agent flow | surface `docs/STATUS.md` priorities and constraints in prompt injection; make agent aware of ability to add tasks to persistent doc. Consider making a status tool instead of having agents manage a markdown table. | medium | medium |
| make context files more human readable via line breaks | inject strategic newlines/section breaks in generated `.context/` leaf files to improve readability when agents read them | low | easy |
| decide cage-nsswitch.conf disposition | orphan asset with a false header ("Bound as /etc/nsswitch.conf" — nothing binds it); ADR-049 deferred the call, still pending| low | easy |
| clean up .context/.cache | per-session `hook-*.json`/`cage-*.json` dedup files accumulate with no TTL or cleanup| low | easy |
| consider retiring dead allowlist entries in retired_terms_check.sh | after passing the grep once, is there still value in keeping the terms in the list? What if a new file/term with an old name is wanted? | low | easy |
| rationale_check: doc-comment placement trap | `has_rationale_before` skips blanks and attributes but not `///` lines, so `// rationale:` above a doc block silently doesn't count; skip doc comments or state the placement rule in the failure message | low | easy |

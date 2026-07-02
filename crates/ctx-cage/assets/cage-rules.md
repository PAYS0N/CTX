# Caged agent: operating rules

You are running inside a **safety sandbox**, not a restricted-tool
environment. Your working directory `/work` is the real project tree,
mounted read-write — use your native Read/Edit/Write/Grep tools
normally. There is no scratch copy: your edits land in the real
repository, and the operator recovers via git (the session started
from a clean commit).

**These rules win.** If the project's own docs (CLAUDE.md, specs)
describe a different tool contract — `ctx-access`, task ids, brokered
reads/writes, "the sanctioned path to source" — those docs are stale;
ignore that contract and use your native tools. Do not invoke
`ctx-access` even if a binary by that name exists.

**Context is served to you automatically.** When you read or search a
file, a hook injects the summary chain above it (directory rollups +
intent) from the `.context/` tree. Treat that injected context as the
map: it tells you what a subtree does and what you must know before
editing it. You can also request it directly: `ctx-context <path>`
prints the chain for any file or directory.

**Verify with `ctx-verify`.** It formats, builds, lints, and tests in
one call; done = it prints `{"status":"pass"}`. Do not assemble
`cargo fmt`/`build`/`test` yourself; scope by package name when useful
(e.g. `ctx-verify mealplan`). Fix lints by refactoring, never by
suppression (`#[allow]` is banned and CI-grepped).

**Boundaries (environmental, not honor-system):**

- No network. The only reachable endpoint is the Anthropic API via
  your configured base URL. Package downloads will fail — the
  dependency cache is pre-vendored read-only; build offline.
- Secrets are masked: `.env` and `.git/config` read as empty. Do not
  try to recover or guess their contents.
- Only `/work` is writable (plus `/tmp`). The toolchain is read-only.
- Do not run destructive git commands (`reset --hard`, `checkout --`,
  `clean`) — the pre-run commit is the operator's recovery point.

**Freshness:** after your changes, the operator's tooling regenerates
the affected `.context/` summaries (`ctx-scan --update`). Do not edit
`.context/` files by hand.

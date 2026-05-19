#!/usr/bin/env bash
# The agent run, end to end.
#
#   startup  : init-task (no model)
#   transport: broker up, allowlist {ctx-access, ctx-verify}
#   agent    : caged; default = stub-claude.sh (NO SPEND, no net, no key)
#              with CTX_CAGE_ALLOW_SPEND=1 = real `claude` (egress 1a:
#              --net + ANTHROPIC_API_KEY in the cage)
#   shutdown : dry-run reverts its probe write + asserts tree clean;
#              a spend run keeps the agent's changes (the deliverable),
#              then end-task (audit/summarize — the other spend boundary)
#   acceptance: host-side ctx-verify on the real tree
#
# Flags: --interactive  (relay a dedicated cage pty to this terminal)
set -euo pipefail

INTERACTIVE=()
[[ "${1:-}" == "--interactive" ]] && { INTERACTIVE=(--interactive); shift; }

HERE="$(cd "$(dirname "$0")" && pwd)"
MEALPLAN_DIR="$(cd "$HERE/../../meal-planning" && pwd)"
CTX_BIN="$(cd "$HERE/.." && pwd)/target/debug"
TASK="${CTX_CAGE_TASK:-agent-dryrun}"
TARGET="crates/mealplan/src/profile.rs"
SPEND="${CTX_CAGE_ALLOW_SPEND:-0}"

for b in ctx-access ctx-verify; do
    [[ -x "$CTX_BIN/$b" ]] || { echo "demo: $CTX_BIN/$b not built" >&2; exit 1; }
done

# Host-prepared seed for the no-spend write probe: HEAD content + a
# visible marker. Bound RO into the cage via $HERE -> /opt/cage.
mkdir -p "$HERE/.seed"
{ git -C "$MEALPLAN_DIR" show "HEAD:$TARGET"
  printf '\n// ctx agent-dryrun: write path exercised; reverted by harness\n'
} > "$HERE/.seed/_seed_profile.rs"

SOCKDIR="$(mktemp -d "${TMPDIR:-/tmp}/ctxagent.XXXXXX")"
SOCK="$SOCKDIR/ctx.sock"
BROKER_PID=""
cleanup() { [[ -n "$BROKER_PID" ]] && kill "$BROKER_PID" 2>/dev/null || true; rm -rf "$SOCKDIR"; }
trap cleanup EXIT

echo "== startup: init-task (no model) =="
( cd "$MEALPLAN_DIR" && "$CTX_BIN/ctx-access" init-task --task-id "$TASK" --force )

echo "== transport: broker up =="
"$HERE/broker.sh" "$SOCK" & BROKER_PID=$!
for _ in $(seq 1 50); do [[ -S "$SOCK" ]] && break; sleep 0.1; done
[[ -S "$SOCK" ]] || { echo "demo: broker socket never appeared" >&2; exit 1; }

# Choose the agent + cage options.
if [[ "$SPEND" == "1" ]]; then
    : "${ANTHROPIC_API_KEY:?spend run needs ANTHROPIC_API_KEY}"
    read -r -d '' BRIEF <<'EOF' || true
You are extending the `mealplan` crate. Source is reachable ONLY via the
`ctx-access` tool (read <path> --task-id; write <path> <content>
--task-id, which requires a prior read of that path). Verify ONLY via
`ctx-verify`; the task is done when it prints {"status":"pass"}. Do not
attempt to cat/find/edit files directly — there is no source on disk.
Task: add a `profile edit` subcommand to the CLI that updates the saved
profile, mirroring existing command structure. Keep ctx-verify green.
EOF
    OPTS=(--net --pass-key)
    if [[ -n "${INTERACTIVE[*]:-}" ]]; then
        AGENT=(claude "$BRIEF"); echo "== agent: REAL claude (SPEND), interactive =="
    else
        AGENT=(claude -p "$BRIEF"); echo "== agent: REAL claude (SPEND), headless =="
    fi
else
    OPTS=(); AGENT=(/opt/cage/stub-claude.sh)
    echo "== agent: stub-claude (NO SPEND) ${INTERACTIVE[*]:+[interactive]} =="
fi

set +e
CTX_SOCKDIR="$SOCKDIR" CTX_SOCKNAME="ctx.sock" \
    "$HERE/cage-run.sh" "${INTERACTIVE[@]}" "${OPTS[@]}" "$TASK" "${AGENT[@]}"
RC=$?
set -e

echo "== transport: broker down =="
kill "$BROKER_PID" 2>/dev/null || true; wait "$BROKER_PID" 2>/dev/null || true; BROKER_PID=""

RC_INTEGRITY=0
if [[ "$SPEND" == "1" ]]; then
    echo "== shutdown: keep deliverable; end-task (SPEND) =="
    ( cd "$MEALPLAN_DIR" && "$CTX_BIN/ctx-access" end-task --task-id "$TASK" )
else
    echo "== shutdown: revert probe write, assert tree clean =="
    ( cd "$MEALPLAN_DIR" && git checkout -- crates Cargo.toml Cargo.lock 2>/dev/null || true )
    DIRTY="$(cd "$MEALPLAN_DIR" && git status --porcelain -- crates Cargo.toml Cargo.lock)"
    if [[ -n "$DIRTY" ]]; then
        echo "INTEGRITY BREACH after revert:" >&2; echo "$DIRTY" >&2; RC_INTEGRITY=1
    else
        echo "integrity: reference tree clean after probe revert — ok"
    fi
    echo "end-task SKIPPED (spend boundary); reclaiming cache"
    rm -f "$MEALPLAN_DIR/.context/.cache/$TASK.json"
fi

echo "== acceptance: host-side ctx-verify on the real tree =="
( cd "$MEALPLAN_DIR" && "$CTX_BIN/ctx-verify" >/tmp/accept 2>&1 ); RC_ACCEPT=$?
head -c 200 /tmp/accept; echo

RC_ALL=$(( RC != 0 || RC_INTEGRITY != 0 || RC_ACCEPT != 0 ? 1 : 0 ))
echo "=="
if [[ $RC_ALL -eq 0 ]]; then
    echo "AGENT RUN PASS — loop wired (init→read/write/verify→shutdown), tree consistent${INTERACTIVE[*]:+, interactive}, spend=$SPEND"
else
    echo "AGENT RUN FAIL — agent rc=$RC, integrity rc=$RC_INTEGRITY, acceptance rc=$RC_ACCEPT"
fi
exit $RC_ALL

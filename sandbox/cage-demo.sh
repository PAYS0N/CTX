#!/usr/bin/env bash
# Cage D: the full lifecycle, no spend.
#
#   init-task (startup, no model)
#     → broker up
#       → cage runs the stub agent (proves the cage holds)
#     → broker down
#   shutdown: end-task is the audit/summarize SPEND boundary — run only
#   with CTX_CAGE_ALLOW_SPEND=1 (the dry-run never sets it); otherwise
#   the per-task cache is just reclaimed.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
MEALPLAN_DIR="$(cd "$HERE/../../meal-planning" && pwd)"
CTX_ACCESS="$(cd "$HERE/.." && pwd)/target/debug/ctx-access"
TASK="${CTX_CAGE_TASK:-cage-dryrun}"

[[ -x "$CTX_ACCESS" ]] || { echo "demo: build ctx-access first (cargo/ctx-verify)" >&2; exit 1; }

SOCKDIR="$(mktemp -d "${TMPDIR:-/tmp}/ctxcage.XXXXXX")"
SOCK="$SOCKDIR/ctx.sock"
BROKER_PID=""

cleanup() {
    [[ -n "$BROKER_PID" ]] && kill "$BROKER_PID" 2>/dev/null || true
    rm -rf "$SOCKDIR"
}
trap cleanup EXIT

echo "== startup: init-task (no model) =="
( cd "$MEALPLAN_DIR" && "$CTX_ACCESS" init-task --task-id "$TASK" --force )

echo "== transport: broker up =="
"$HERE/broker.sh" "$SOCK" & BROKER_PID=$!
for _ in $(seq 1 50); do [[ -S "$SOCK" ]] && break; sleep 0.1; done
[[ -S "$SOCK" ]] || { echo "demo: broker socket never appeared" >&2; exit 1; }

# The adversary runs under its OWN fresh task with zero served reads, so
# write-requires-prior-read is tested as the invariant (not satisfied by
# the reachability phase's legitimate read of the same path).
ADV_TASK="${TASK}-adv"
( cd "$MEALPLAN_DIR" && "$CTX_ACCESS" init-task --task-id "$ADV_TASK" --force )

echo "== cage: reachability (stub agent) =="
set +e
CTX_SOCKDIR="$SOCKDIR" CTX_SOCKNAME="ctx.sock" \
    "$HERE/cage-run.sh" "$TASK" /opt/cage/stub-agent.sh
RC_STUB=$?

echo "== cage: enforcement preserved (adversary, fresh task) =="
CTX_SOCKDIR="$SOCKDIR" CTX_SOCKNAME="ctx.sock" \
    "$HERE/cage-run.sh" "$ADV_TASK" /opt/cage/cage-adversary.sh
RC_ADV=$?
set -e

# Safety net: the dry-run must not have mutated the reference tree. If
# anything tracked changed (or a scratch probe leaked), restore and fail
# loudly — a destructive test is itself a defect.
DIRTY="$(cd "$MEALPLAN_DIR" && git status --porcelain -- crates Cargo.toml Cargo.lock 2>/dev/null)"
STRAY="$(ls "$MEALPLAN_DIR"/crates/mealplan/src/__adv_probe__.rs 2>/dev/null || true)"
RC_INTEGRITY=0
if [[ -n "$DIRTY" || -n "$STRAY" ]]; then
    echo "INTEGRITY BREACH — dry-run mutated the reference tree:" >&2
    [[ -n "$DIRTY" ]] && echo "$DIRTY" >&2
    [[ -n "$STRAY" ]] && echo "stray: $STRAY" >&2
    ( cd "$MEALPLAN_DIR" && git checkout -- crates Cargo.toml Cargo.lock 2>/dev/null || true )
    rm -f "$MEALPLAN_DIR"/crates/mealplan/src/__adv_probe__.rs
    RC_INTEGRITY=1
else
    echo "integrity: reference tree unmutated by the dry-run — ok"
fi
RC=$(( RC_STUB != 0 || RC_ADV != 0 || RC_INTEGRITY != 0 ? 1 : 0 ))

echo "== transport: broker down =="
kill "$BROKER_PID" 2>/dev/null || true; wait "$BROKER_PID" 2>/dev/null || true; BROKER_PID=""

echo "== shutdown =="
if [[ "${CTX_CAGE_ALLOW_SPEND:-0}" == "1" ]]; then
    echo "end-task (audit/summarize — SPENDING, explicit go given)"
    ( cd "$MEALPLAN_DIR" && "$CTX_ACCESS" end-task --task-id "$TASK" )
else
    echo "end-task SKIPPED (spend boundary); reclaiming caches only"
    rm -f "$MEALPLAN_DIR/.context/.cache/$TASK.json" \
          "$MEALPLAN_DIR/.context/.cache/$ADV_TASK.json"
fi

echo "=="
if [[ $RC -eq 0 ]]; then
    echo "CAGE D PASS — source unreachable except via ctx-access; enforcement intact through the transport; reference tree unmutated; lifecycle clean"
else
    echo "CAGE D FAIL — reachability rc=$RC_STUB, enforcement rc=$RC_ADV, integrity rc=$RC_INTEGRITY"
fi
exit $RC

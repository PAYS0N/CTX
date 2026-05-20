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

INTERACTIVE=(); PREFLIGHT=0; CHECK_ONB=0
while [[ $# -gt 0 ]]; do
    case "${1:-}" in
        --interactive)      INTERACTIVE=(--interactive); shift ;;
        --preflight)        PREFLIGHT=1; shift ;;
        --check-onboarding) CHECK_ONB=1; shift ;;
        *)                  break ;;
    esac
done

HERE="$(cd "$(dirname "$0")" && pwd)"
MEALPLAN_DIR="$(cd "$HERE/../../meal-planning" && pwd)"
CTX_BIN="$(cd "$HERE/.." && pwd)/target/debug"
TASK="${CTX_CAGE_TASK:-agent-dryrun}"
TARGET="crates/mealplan/src/profile.rs"
SPEND="${CTX_CAGE_ALLOW_SPEND:-0}"

# Only the no-spend STUB run probe-writes (and then reverts) profile.rs.
# preflight/check-onboarding never touch crates; a spend run keeps its
# deliverable. So "is this the stub?" decides what may be reverted.
IS_STUB=0
[[ "$SPEND" != "1" && $PREFLIGHT -ne 1 && $CHECK_ONB -ne 1 ]] && IS_STUB=1

for b in ctx-access ctx-verify; do
    [[ -x "$CTX_BIN/$b" ]] || { echo "demo: $CTX_BIN/$b not built" >&2; exit 1; }
done

# ADR-027: a harness must NEVER destroy work it did not create. The stub
# overwrites and reverts profile.rs; if crates already has uncommitted
# changes (e.g. an unsaved billed deliverable), refuse outright rather
# than clobber them.
if [[ $IS_STUB -eq 1 ]]; then
    PRE_DIRTY="$(cd "$MEALPLAN_DIR" && git status --porcelain -- crates Cargo.toml Cargo.lock)"
    if [[ -n "$PRE_DIRTY" ]]; then
        echo "REFUSING: a no-spend stub run reverts crates, but meal-planning has" >&2
        echo "uncommitted changes it did not create:" >&2
        echo "$PRE_DIRTY" >&2
        echo "Commit or stash them first (or this is a deliverable to preserve)." >&2
        exit 3
    fi
    # Seed for the probe write: HEAD content + a visible marker.
    mkdir -p "$HERE/.seed"
    { git -C "$MEALPLAN_DIR" show "HEAD:$TARGET"
      printf '\n// ctx agent-dryrun: write path exercised; reverted by harness\n'
    } > "$HERE/.seed/_seed_profile.rs"
fi

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

# --check-onboarding: NO SPEND. Launch interactive claude in the real
# --claude cage, immediately send `/exit` (a client command, never a
# model message → no token spend), and assert the first-run wizard and
# the API-key prompt do NOT appear. A short timeout bounds a hang.
if [[ $CHECK_ONB -eq 1 ]]; then
    echo "== check-onboarding: interactive claude, immediate /exit (no spend) =="
    CAP="$SOCKDIR/onboard.out"
    set +e
    printf '/exit\n' | CTX_SOCKDIR="$SOCKDIR" CTX_SOCKNAME="ctx.sock" \
        timeout 60 "$HERE/cage-run.sh" --interactive --claude "$TASK" claude \
        >"$CAP" 2>&1
    set -e
    kill "$BROKER_PID" 2>/dev/null || true; wait "$BROKER_PID" 2>/dev/null || true; BROKER_PID=""
    rm -f "$MEALPLAN_DIR/.context/.cache/$TASK.json"

    # Claude's TUI separates words with ANSI cursor-forward (ESC[<n>C)
    # rather than spaces; turn those back into spaces, drop the rest of
    # the escapes, so phrase matching is meaningful. (Reaching the TUI
    # without submitting a message = no model call = no spend; the probe
    # then times out and is killed — expected, not a failure.)
    clean=$(sed -e 's/\x1b\[[0-9]*C/ /g' -e 's/\x1b\][^\x07]*\x07//g' \
                -e 's/\x1b\[[0-9;?]*[A-Za-z]//g' -e 's/\x1b[()][AB0]//g' \
                -e 's/[\x00-\x08\x0b\x0c\x0e-\x1f]//g' "$CAP")

    WIZ=0 SESS=0
    grep -qiE 'choose .*(theme|text style)|select (a )?login|log ?in with|sign in to|how would you like to (log ?in|authenticate)|detected .*api key|api key .*environment|use the .*api key|anthropic_api_key' <<<"$clean" && WIZ=1
    grep -qiE 'welcome back|payson|what.?s new|for shortcuts|bypass permissions|claude code v' <<<"$clean" && SESS=1

    echo "---- normalized capture (first 400B) ----"
    printf '%s' "$clean" | tr -s ' ' | head -c 400; echo; echo "----"
    if [[ $WIZ -eq 0 && $SESS -eq 1 ]]; then
        echo "ONBOARDING CHECK PASS — no wizard, no API-key prompt; authenticated returning-user UI reached (subscription auto-detected); no spend"
        exit 0
    fi
    [[ $WIZ -eq 1 ]] && echo "ONBOARDING CHECK FAIL — wizard / API-key prompt appeared" >&2
    [[ $SESS -eq 0 ]] && echo "ONBOARDING CHECK FAIL — could not confirm the authenticated session UI" >&2
    exit 1
fi

# Choose the agent + cage options.
if [[ $PREFLIGHT -eq 1 ]]; then
    OPTS=(--claude); AGENT=(/opt/cage/claude-preflight.sh)
    echo "== agent: claude-preflight (NO SPEND, real --net --claude env) =="
elif [[ "$SPEND" == "1" ]]; then
    # House rules live in /work/CLAUDE.md (auto-discovered by Claude
    # Code — we do not use --bare). The brief is just the task.
    read -r -d '' BRIEF <<'EOF' || true
Follow CLAUDE.md in the working directory. Task: add a `profile edit`
subcommand to the mealplan CLI that updates fields of the saved profile,
mirroring the existing command structure. Done when `ctx-verify mealplan`
prints {"status":"pass"}.
EOF
    OPTS=(--claude)
    # --dangerously-skip-permissions: no one-time "accept bypass mode"
    # prompt (which blocked the autonomous run); the cage is the
    # sandbox the flag asks for (its "no internet" guidance is the
    # knowingly-accepted 1a residual, ADR-028/029).
    if [[ -n "${INTERACTIVE[*]:-}" ]]; then
        AGENT=(claude --dangerously-skip-permissions "$BRIEF")
        echo "== agent: REAL claude (SPEND), interactive =="
    else
        AGENT=(claude -p --dangerously-skip-permissions "$BRIEF")
        echo "== agent: REAL claude (SPEND), headless =="
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
elif [[ $IS_STUB -eq 1 ]]; then
    echo "== shutdown: revert ONLY the stub probe (profile.rs), assert clean =="
    # Scoped to the exact probe target — never a blanket `checkout
    # crates` (that is what destroyed a billed deliverable; ADR-027).
    ( cd "$MEALPLAN_DIR" && git checkout -- "$TARGET" 2>/dev/null || true )
    DIRTY="$(cd "$MEALPLAN_DIR" && git status --porcelain -- crates Cargo.toml Cargo.lock)"
    if [[ -n "$DIRTY" ]]; then
        echo "INTEGRITY BREACH after revert:" >&2; echo "$DIRTY" >&2; RC_INTEGRITY=1
    else
        echo "integrity: profile.rs probe reverted; crates clean — ok"
    fi
    echo "end-task SKIPPED (spend boundary); reclaiming cache"
    rm -f "$MEALPLAN_DIR/.context/.cache/$TASK.json"
else
    echo "== shutdown: preflight — no crates mutation, nothing to revert =="
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

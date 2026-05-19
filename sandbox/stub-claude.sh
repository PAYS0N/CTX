#!/bin/sh
# Runs INSIDE the cage as the no-spend stand-in for `claude`. It walks
# the same path a real builder agent would — brokered verify, chain
# read, gated write, re-verify — proving the whole loop wires end to
# end. NO model is called. The write uses a host-prepared seed (HEAD
# content + a marker) and is reverted by the harness afterwards.
set -u
FAIL=0
say()  { printf '%s\n' "$*"; }
bad()  { printf 'WIRING FAIL: %s\n' "$*"; FAIL=1; }
TARGET=crates/mealplan/src/profile.rs
SEED=/opt/cage/.seed/_seed_profile.rs

say "== stub-claude: agent loop over the brokered tools =="

# 0. The lazy shortcut must still be dead.
if cat "$TARGET" 2>/dev/null | grep -q '[^[:space:]]'; then
    bad "direct cat of $TARGET leaked source"
else
    say "  cat $TARGET: blocked (source absent) — ok"
fi

# 1. Brokered ctx-verify baseline (no toolchain/source in the cage).
if ctx-verify >/tmp/v0 2>&1 && grep -q '"status": "pass"' /tmp/v0; then
    say "  ctx-verify (baseline): pass via broker"
else
    bad "baseline ctx-verify not pass: $(head -c 200 /tmp/v0)"
fi

# 2. Read the chain for the target (serves ancestors + summary + source).
if ctx-access read "$TARGET" --task-id "$TASK" >/tmp/served 2>&1 && [ -s /tmp/served ]; then
    say "  ctx-access read $TARGET: $(wc -c </tmp/served | tr -d ' ') bytes served"
else
    bad "ctx-access read failed: $(head -c 200 /tmp/served)"
fi

# 3. Gated write (prior read of this path satisfied in step 2).
if [ -r "$SEED" ]; then
    if ctx-access write "$TARGET" "$(cat "$SEED")" --task-id "$TASK" >/tmp/w 2>&1; then
        say "  ctx-access write $TARGET: accepted (write-requires-prior-read satisfied)"
    else
        bad "ctx-access write failed: $(head -c 200 /tmp/w)"
    fi
else
    bad "seed $SEED not readable in cage"
fi

# 4. Re-verify: the loop must close green on the just-written tree.
if ctx-verify >/tmp/v1 2>&1 && grep -q '"status": "pass"' /tmp/v1; then
    say "  ctx-verify (after write): pass via broker"
else
    bad "post-write ctx-verify not pass: $(head -c 200 /tmp/v1)"
fi

say "=="
if [ "$FAIL" = 0 ]; then
    say "RESULT: agent loop wired end-to-end (verify+read+write+verify), no spend"
    exit 0
fi
say "RESULT: wiring incomplete"
exit 1

#!/bin/sh
# Runs INSIDE the cage. The no-spend proof: a capable, lazy process
# cannot reach source directly and must go through ctx-access. No model
# is ever called (that is the explicit-go boundary, not this).
set -u
FAIL=0
say() { printf '%s\n' "$*"; }
bad() { printf 'LEAK: %s\n' "$*"; FAIL=1; }

say "== environment =="
# /proc is mounted (fresh, reflects the new net ns); /sys is not. The
# interface list in /proc/net/dev must be loopback-only.
ifaces=$(sed -n 's/^[[:space:]]*\([A-Za-z0-9@._-]*\):.*/\1/p' /proc/net/dev 2>/dev/null | tr '\n' ' ')
say "uid=$(id -u) cwd=$(pwd) net_ifaces=[$ifaces]"
nonlo=$(printf '%s\n' $ifaces | grep -v '^lo$' || true)
if [ -z "$nonlo" ]; then
    say "net: loopback only (no network) — ok"
else
    bad "unexpected network interface(s): $nonlo"
fi

say "== direct source reads (must all fail) =="
for f in crates/mealplan/src/profile.rs crates/mealplan/src/cli/mod.rs \
         crates/mealplan/src/lib.rs crates/mealplan/tests/planning.rs; do
    if cat "$f" 2>/dev/null | grep -q '[^[:space:]]'; then
        bad "readable: $f"
    else
        say "  blocked: $f"
    fi
done

n=$(find /work -name '*.rs' 2>/dev/null | wc -l | tr -d ' ')
say "  *.rs files anywhere under /work: $n (expect 0)"
[ "$n" = 0 ] || bad "source .rs present in namespace ($n files)"

if grep -rqs 'fn main' /work/crates 2>/dev/null; then
    bad "grep -r found source under /work/crates"
else
    say "  grep -r over /work/crates: nothing (source absent)"
fi

say "== via the tool (must succeed) =="
if ctx-access manifest --task-id "$TASK" >/tmp/man 2>&1 && grep -qs 'profile.rs' /tmp/man; then
    say "  ctx-access manifest: lists source paths ($(wc -l </tmp/man | tr -d ' ') entries)"
else
    bad "ctx-access manifest failed: $(head -c 200 /tmp/man)"
fi

if ctx-access read crates/mealplan/src/profile.rs --task-id "$TASK" >/tmp/served 2>&1 \
   && [ -s /tmp/served ]; then
    say "  ctx-access read profile.rs: served $(wc -c </tmp/served | tr -d ' ') bytes via the tool"
else
    bad "ctx-access read failed: $(head -c 200 /tmp/served)"
fi

say "=="
if [ "$FAIL" = 0 ]; then
    say "RESULT: source unreachable except via ctx-access — cage holds"
    exit 0
fi
say "RESULT: cage breached"
exit 1

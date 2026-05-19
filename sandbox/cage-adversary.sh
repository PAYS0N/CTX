#!/bin/sh
# Runs INSIDE the cage under a FRESH task with zero prior reads (the
# harness guarantees this — distinct from the reachability task). Proves
# the transport does not weaken enforcement: every request still executes
# in the real host-side ctx-access, so the deny gate, repo-boundary, and
# write-requires-prior-read all still bite even though the caller is
# caged. Every attempt here MUST be denied, so nothing is ever written;
# the blind write targets a scratch path as belt-and-suspenders. No model
# is called.
set -u
FAIL=0
say() { printf '%s\n' "$*"; }
# A defense that must hold: the command must FAIL (non-zero).
must_deny() {
    desc=$1; shift
    if out=$("$@" 2>&1); then
        printf 'BREACH: %s SUCCEEDED (should be denied): %s\n' "$desc" "$(printf %s "$out" | head -c 160)"
        FAIL=1
    else
        printf '  denied (ok): %s\n' "$desc"
    fi
}

say "== enforcement preserved through the forwarder =="

# 1. Secret deny-by-default (ctx-core gate), even named explicitly.
must_deny "read .env (secret)"            ctx-access read .env --task-id "$TASK"
must_deny "read id_rsa (secret)"          ctx-access read id_rsa --task-id "$TASK"

# 2. Repo boundary: no escaping the project root via the tool.
must_deny "read ../../../etc/passwd"      ctx-access read ../../../etc/passwd --task-id "$TASK"
must_deny "read /etc/passwd (absolute)"   ctx-access read /etc/passwd --task-id "$TASK"

# 3. Write requires a prior non-shallow read of that path. This task has
#    served NO reads, and the target is a scratch path that does not
#    exist — denial here is the invariant, not incidental.
must_deny "blind write (no prior read)"   ctx-access write crates/mealplan/src/__adv_probe__.rs "pwned" --task-id "$TASK"

# 4. Unknown task id must not be honored.
must_deny "read with bogus task id"       ctx-access read crates/mealplan/src/lib.rs --task-id no-such-task

say "=="
if [ "$FAIL" = 0 ]; then
    say "RESULT: every bypass attempt denied host-side — enforcement intact"
    exit 0
fi
say "RESULT: enforcement weakened by the transport"
exit 1

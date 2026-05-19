#!/usr/bin/env bash
# Host side. Launch a command inside the cage: meal-planning mounted
# READ-ONLY with its source + build tree replaced by empty tmpfs, the
# generalized forwarder installed as BOTH `ctx-access` and `ctx-verify`,
# and (by default) no network. Requires the broker socket to already
# exist (the *-demo.sh harnesses order this).
#
# Usage:
#   CTX_SOCKDIR=<dir> cage-run.sh [opts] <task-id> <cmd> [args...]
# Options:
#   --interactive  give the cage its own pty, relayed to this terminal
#   --net          do NOT unshare the network (egress 1a: real claude)
#   --pass-key     pass ANTHROPIC_API_KEY into the cage env (with --net)
set -euo pipefail

INTERACTIVE=0; NET=0; PASS_KEY=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --interactive) INTERACTIVE=1; shift ;;
        --net)         NET=1; shift ;;
        --pass-key)    PASS_KEY=1; shift ;;
        --)            shift; break ;;
        --*)           echo "cage-run: unknown option $1" >&2; exit 2 ;;
        *)             break ;;
    esac
done

TASK="${1:?usage: cage-run.sh [opts] <task-id> <cmd...>}"; shift
[[ $# -ge 1 ]] || { echo "cage-run: no command" >&2; exit 2; }

HERE="$(cd "$(dirname "$0")" && pwd)"
MEALPLAN_DIR="$(cd "$HERE/../../meal-planning" && pwd)"
: "${CTX_SOCKDIR:?cage-run: CTX_SOCKDIR unset (the dir holding the broker socket)}"
SOCKNAME="${CTX_SOCKNAME:-ctx.sock}"
[[ -S "$CTX_SOCKDIR/$SOCKNAME" ]] || { echo "cage-run: socket $CTX_SOCKDIR/$SOCKNAME not up" >&2; exit 1; }

bw=(bwrap
    --unshare-user --unshare-pid --unshare-ipc --unshare-uts
    --die-with-parent --new-session
    --ro-bind /usr /usr
    --ro-bind /bin /bin
    --ro-bind /lib /lib
    --ro-bind /lib64 /lib64
    --ro-bind /etc/alternatives /etc/alternatives
    --proc /proc
    --dev /dev
    --tmpfs /tmp
    --ro-bind "$MEALPLAN_DIR" /work
    --tmpfs /work/crates/mealplan/src
    --tmpfs /work/crates/mealplan/tests
    --tmpfs /work/target
    --ro-bind "$HERE" /opt/cage
    --tmpfs /cage/bin
    --ro-bind "$HERE/tool-client.sh" /cage/bin/ctx-access
    --ro-bind "$HERE/tool-client.sh" /cage/bin/ctx-verify
    --ro-bind "$CTX_SOCKDIR" /run/ctx
    --chdir /work
    --setenv PATH /cage/bin:/usr/bin:/bin
    --setenv HOME /tmp
    --setenv CTX_SOCK "/run/ctx/$SOCKNAME"
    --setenv TASK "$TASK")

# Default: no network (the proof / stub path). --net = egress 1a.
if [[ $NET -eq 0 ]]; then
    bw+=(--unshare-net)
fi
if [[ $PASS_KEY -eq 1 ]]; then
    [[ $NET -eq 1 ]] || { echo "cage-run: --pass-key needs --net" >&2; exit 2; }
    : "${ANTHROPIC_API_KEY:?cage-run: --pass-key set but ANTHROPIC_API_KEY unset}"
    bw+=(--setenv ANTHROPIC_API_KEY "$ANTHROPIC_API_KEY")
fi

bw+=(-- "$@")

if [[ $INTERACTIVE -eq 1 ]]; then
    exec python3 "$HERE/pty-relay.py" "${bw[@]}"
fi
exec "${bw[@]}"

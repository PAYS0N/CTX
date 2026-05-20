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
#   --net          do NOT unshare the network (egress 1a)
#   --claude       provision a real claude runtime: bind the claude
#                  binary, DNS/TLS plumbing, and ONLY
#                  ~/.claude/.credentials.json (subscription auth).
#                  Implies --net.
set -euo pipefail

INTERACTIVE=0; NET=0; CLAUDE=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --interactive) INTERACTIVE=1; shift ;;
        --net)         NET=1; shift ;;
        --claude)      CLAUDE=1; NET=1; shift ;;
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
    --clearenv
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
    --setenv USER cage
    --setenv LANG C.UTF-8
    --setenv TERM "${TERM:-xterm-256color}"
    --setenv CTX_SOCK "/run/ctx/$SOCKNAME"
    --setenv TASK "$TASK")
# --clearenv above: nothing from the host environment leaks in (no
# ANTHROPIC_API_KEY → no "use detected key?" prompt; also a blinding
# fix). Exactly the vars set here, plus the --claude block's, exist.

# Default: no network (the proof / stub path). --net = egress 1a.
if [[ $NET -eq 0 ]]; then
    bw+=(--unshare-net)
fi

if [[ $CLAUDE -eq 1 ]]; then
    CLAUDE_BIN="$(readlink -f "$(command -v claude)")"
    CREDS="$HOME/.claude/.credentials.json"
    [[ -x "$CLAUDE_BIN" ]] || { echo "cage-run: claude binary not found" >&2; exit 2; }
    [[ -r "$CREDS" ]] || { echo "cage-run: $CREDS not readable (subscription auth)" >&2; exit 2; }
    RESOLV="$(readlink -f /etc/resolv.conf)"
    HOST_CFG="$HOME/.claude.json"
    [[ -r "$HOST_CFG" ]] || { echo "cage-run: $HOST_CFG not readable (need oauthAccount)" >&2; exit 2; }
    command -v jq >/dev/null || { echo "cage-run: jq required to synthesize the cage config" >&2; exit 2; }

    # Synthesize a MINIMAL ~/.claude.json so the cage's fresh HOME does
    # not trigger first-run onboarding (theme/login/trust). Marks
    # onboarding complete, pre-trusts /work, and carries ONLY the
    # account object (so the bound OAuth credential is auto-detected —
    # no login prompt). Nothing else from the host config (no projects/
    # history). rw + ephemeral (under CTX_SOCKDIR, cleaned by the
    # harness) so claude may update its own counters without error.
    CAGE_CFG="$CTX_SOCKDIR/claude.json"
    VER="$("$CLAUDE_BIN" --version 2>/dev/null | awk '{print $1}')"
    AID="$(head -c16 /dev/urandom | od -An -tx1 | tr -d ' \n')"
    jq -n \
        --argjson oauth "$(jq -c '.oauthAccount // {}' "$HOST_CFG")" \
        --arg ver "${VER:-2.1.123}" --arg aid "$AID" '{
            hasCompletedOnboarding: true,
            lastOnboardingVersion: $ver,
            firstStartTime: "2026-02-24T20:59:45.765Z",
            numStartups: 22,
            autoUpdates: false,
            theme: "dark",
            anonymousId: $aid,
            oauthAccount: $oauth,
            projects: { "/work": {
                hasTrustDialogAccepted: true,
                projectOnboardingSeenCount: 9,
                hasClaudeMdExternalIncludesWarningShown: true,
                allowedTools: []
            } }
        }' > "$CAGE_CFG"

    bw+=(
        --ro-bind "$CLAUDE_BIN" /cage/bin/claude
        # DNS/TLS plumbing for the shared (1a) network.
        --ro-bind "$RESOLV" /etc/resolv.conf
        --ro-bind /etc/hosts /etc/hosts
        --ro-bind /etc/ssl /etc/ssl
        --ro-bind "$HERE/cage-nsswitch.conf" /etc/nsswitch.conf
        # ONLY the credential (RO) + the synthesized config (rw,
        # ephemeral) — nothing else from ~/.claude (blinding).
        --ro-bind "$CREDS" /tmp/.claude/.credentials.json
        --bind "$CAGE_CFG" /tmp/.claude.json
    )
fi

bw+=(-- "$@")

if [[ $INTERACTIVE -eq 1 ]]; then
    exec python3 "$HERE/pty-relay.py" "${bw[@]}"
fi
exec "${bw[@]}"

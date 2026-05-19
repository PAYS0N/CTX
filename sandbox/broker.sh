#!/usr/bin/env bash
# Host side. The UNIX-socket transport: one allowlisted tool per
# connection ({ctx-access, ctx-verify}), in the real meal-planning tree.
# Foreground; `agent-demo.sh`/`cage-demo.sh` run it in the background.
#
# Usage: broker.sh <socket-path>
set -euo pipefail

SOCK="${1:?usage: broker.sh <socket-path>}"
HERE="$(cd "$(dirname "$0")" && pwd)"

MEALPLAN_DIR="$(cd "$HERE/../../meal-planning" && pwd)"
CTX_BIN="$(cd "$HERE/.." && pwd)/target/debug"
CTX_ACCESS="$CTX_BIN/ctx-access"
CTX_VERIFY="$CTX_BIN/ctx-verify"
export MEALPLAN_DIR CTX_ACCESS CTX_VERIFY

for b in "$CTX_ACCESS" "$CTX_VERIFY"; do
    [[ -x "$b" ]] || { echo "broker: $b not built" >&2; exit 1; }
done
[[ -d "$MEALPLAN_DIR/.context" ]] || { echo "broker: $MEALPLAN_DIR/.context missing" >&2; exit 1; }

rm -f "$SOCK"
echo "broker: listening on $SOCK -> {ctx-access, ctx-verify} in $MEALPLAN_DIR" >&2
# -t 86400: match the client; a slow handler (ctx-verify) must not be
# torn down mid-compile after the client half-closes its request.
exec socat -t 86400 "UNIX-LISTEN:$SOCK,fork,mode=600" "EXEC:$HERE/broker-handler.sh"

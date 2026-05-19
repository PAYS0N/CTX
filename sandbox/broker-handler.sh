#!/usr/bin/env bash
# Host side, one invocation per cage connection (socat ...,fork EXEC:).
# stdio IS the socket. Protocol:
#   in : line 1 = base64(NUL-joined [tool, arg1, arg2, ...])
#   out: the tool's stdout+stderr, then a final line `__CTXRC__<exit>`
#
# This is the trust boundary. It runs ONLY an allowlisted tool, in the
# real tree. Enforcement (deny gate, chain, lifecycle, verification)
# lives in those binaries here — never in the cage.
set -uo pipefail

: "${MEALPLAN_DIR:?broker-handler: MEALPLAN_DIR unset}"
: "${CTX_ACCESS:?broker-handler: CTX_ACCESS unset}"
: "${CTX_VERIFY:?broker-handler: CTX_VERIFY unset}"

emit_rc() { printf '__CTXRC__%s\n' "$1"; }

IFS= read -r b64 || { echo "broker: empty request" >&2; emit_rc 91; exit 0; }

mapfile -d '' -t parts < <(printf '%s' "$b64" | base64 -d 2>/dev/null)
if [[ ${#parts[@]} -lt 1 ]]; then
    echo "broker: undecodable request" >&2
    emit_rc 92
    exit 0
fi

tool="${parts[0]}"
argv=("${parts[@]:1}")

case "$tool" in
    ctx-access) bin="$CTX_ACCESS" ;;
    ctx-verify) bin="$CTX_VERIFY" ;;
    *)
        echo "broker: tool '$tool' not in allowlist {ctx-access, ctx-verify}" >&2
        emit_rc 94
        exit 0
        ;;
esac

cd "$MEALPLAN_DIR" 2>/dev/null || { echo "broker: cd $MEALPLAN_DIR failed" >&2; emit_rc 93; exit 0; }

"$bin" ${argv[@]+"${argv[@]}"} 2>&1
emit_rc "$?"

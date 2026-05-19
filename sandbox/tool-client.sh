#!/bin/sh
# Inside the cage this IS both `ctx-access` and `ctx-verify` (the only
# tools of those names on PATH; same file bound twice). It holds no
# source, no toolchain, no logic: pick the tool from argv0, NUL-join
# [tool, args...], base64 it, ship one line over the bound UNIX socket,
# replay the response, propagate the real exit code. The host broker
# runs the real binary in the real tree. No stdin is forwarded (neither
# tool consumes any).
set -eu

: "${CTX_SOCK:?tool-client: CTX_SOCK unset}"

tool=$(basename "$0")

enc=$( { printf '%s\0' "$tool"; for a in "$@"; do printf '%s\0' "$a"; done; } \
       | base64 | tr -d '\n' )

# -t 86400: a slow tool (ctx-verify recompiles for seconds, with silent
# gaps) must not be reaped by socat's default 0.5s half-close timeout
# after our one-line request EOFs.
resp=$(printf '%s\n' "$enc" | socat -t 86400 - "UNIX-CONNECT:$CTX_SOCK")

rc=$(printf '%s\n' "$resp" | sed -n 's/^__CTXRC__\([0-9][0-9]*\)$/\1/p' | tail -n1)
printf '%s\n' "$resp" | sed '/^__CTXRC__[0-9][0-9]*$/d'

exit "${rc:-1}"

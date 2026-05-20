#!/bin/sh
# Runs INSIDE the cage with --net --claude. Proves the REAL-RUN
# environment is sound WITHOUT spending: claude executes, the
# subscription credential is visible, DNS+TLS to the API works, the
# source jail still holds, and the broker is still reachable. It makes
# NO model call (a TLS handshake to api.anthropic.com is not a billed
# API request; no HTTP is sent).
set -u
FAIL=0
say() { printf '%s\n' "$*"; }
bad() { printf 'PREFLIGHT FAIL: %s\n' "$*"; FAIL=1; }

say "== claude runtime =="
if v=$(timeout 30 claude --version 2>&1); then
    say "  claude --version: $v"
else
    bad "claude --version failed: $(printf %s "$v" | head -c 160)"
fi

if [ -s "$HOME/.claude/.credentials.json" ]; then
    say "  credential present: \$HOME/.claude/.credentials.json ($(wc -c <"$HOME/.claude/.credentials.json" | tr -d ' ') bytes, not shown)"
else
    bad "subscription credential missing/empty in cage"
fi

say "== network to the API (no model call) =="
if getent hosts api.anthropic.com >/dev/null 2>&1; then
    say "  DNS: api.anthropic.com resolves"
else
    bad "DNS resolution of api.anthropic.com failed"
fi
if printf '' | timeout 30 openssl s_client -connect api.anthropic.com:443 \
        -servername api.anthropic.com -brief >/tmp/tls 2>&1 \
   && grep -qiE 'protocol|cipher|handshake|verification' /tmp/tls; then
    say "  TLS: handshake to api.anthropic.com:443 OK ($(grep -i protocol /tmp/tls | head -1 | tr -d '\r'))"
else
    bad "TLS handshake failed: $(tr -d '\r' </tmp/tls | head -1)"
fi

say "== house rules provisioned (auto-discovered by Claude Code) =="
if [ -s /work/CLAUDE.md ] && grep -q ctx-access /work/CLAUDE.md; then
    say "  /work/CLAUDE.md present ($(wc -l </work/CLAUDE.md | tr -d ' ') lines)"
else
    bad "/work/CLAUDE.md missing — agent would reverse-engineer the rules"
fi

say "== jail still holds under --net --claude =="
if cat crates/mealplan/src/profile.rs 2>/dev/null | grep -q '[^[:space:]]'; then
    bad "source leaked (cat profile.rs succeeded)"
else
    say "  cat profile.rs: blocked — ok"
fi
if ctx-access manifest --task-id "$TASK" >/tmp/man 2>&1 && grep -qs profile.rs /tmp/man; then
    say "  broker reachable: ctx-access manifest ok"
else
    bad "broker unreachable under --net: $(head -c 160 /tmp/man)"
fi

say "=="
if [ "$FAIL" = 0 ]; then
    say "PREFLIGHT PASS — real-run environment sound; only the spend switch remains"
    exit 0
fi
say "PREFLIGHT FAIL"
exit 1

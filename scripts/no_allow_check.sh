#!/usr/bin/env bash
# Reject any `#[allow(...)]` attribute outside of permitted locations.
# Permitted: nowhere in MVP. Tests get unwrap/expect via clippy config, not
# via #[allow].
#
# This is intentionally strict. The appeal mechanism for legitimate
# suppressions is deferred past MVP.

set -euo pipefail

ROOT="${1:-.}"
FAIL=0

# Search .rs files only. Allow #![...] inner attributes at crate roots only
# if they appear in the lints we explicitly set; otherwise even those are
# rejected here. The grep covers both #[allow(...)] and #![allow(...)].
while IFS= read -r line; do
    echo "FAIL: $line" >&2
    FAIL=1
done < <(grep -rnE '^[[:space:]]*#!?\[allow\(' "$ROOT" \
    --include='*.rs' \
    --exclude-dir=target \
    --exclude-dir=.git \
    || true)

exit $FAIL

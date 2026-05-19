#!/usr/bin/env python3
"""Enforces soft-tier rationale comments.

MVP placeholder for dylint rule 1. See docs/DYLINT_RULES.md.

Rules (refactor first; rationale is the backup, not the fix)
-----
- Functions of 30+ lines (`SOFT_FN_LINES`) should be reduced under the
  tier by extraction/splitting. Only if genuinely irreducible, a
  `// rationale: <text>` on the line immediately preceding the `fn`
  keyword clears it (attributes/blank lines between are allowed).
- Functions of 80+ lines (`HARD_FN_LINES`) fail unconditionally.
- Files of 250+ lines (`SOFT_FILE_LINES`) should be split into modules.
  Only if genuinely cohesive and irreducible, a leading `// rationale:
  <text>` (after any `//!` block) clears it.
- Files of 400+ lines (`HARD_FILE_LINES`) fail unconditionally.

This is intentionally conservative. The eventual dylint rule will use the
real AST and will be more precise. The function-detection heuristic here
uses brace balance starting from the `fn` keyword's first `{`, which is
robust enough for normal Rust but may miss exotic macros.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

SOFT_FN_LINES = 30
HARD_FN_LINES = 80
SOFT_FILE_LINES = 250
HARD_FILE_LINES = 400

FN_RE = re.compile(
    r"""^[ \t]*
        (pub(\([^)]*\))?[ \t]+)?
        ((async|const|unsafe|extern[ \t]+"[^"]*")[ \t]+)*
        fn[ \t]+[A-Za-z_]
    """,
    re.VERBOSE,
)
RATIONALE_RE = re.compile(r"^\s*//\s*rationale:", re.IGNORECASE)
ATTR_RE = re.compile(r"^\s*#\[")
BLANK_RE = re.compile(r"^\s*$")
DOC_INNER_RE = re.compile(r"^\s*//!")


def strip_strings_and_comments(line: str) -> str:
    """Strip string literals and `//` comments so brace counting is accurate.

    Imperfect for multi-line strings and `/* */` block comments; acceptable
    for an MVP placeholder.
    """
    out = []
    i = 0
    in_str = False
    while i < len(line):
        ch = line[i]
        if in_str:
            if ch == "\\" and i + 1 < len(line):
                i += 2
                continue
            if ch == '"':
                in_str = False
            i += 1
            continue
        if ch == '"':
            in_str = True
            i += 1
            continue
        if ch == "/" and i + 1 < len(line) and line[i + 1] == "/":
            break
        out.append(ch)
        i += 1
    return "".join(out)


def find_fn_end(lines: list[str], start_idx: int) -> int | None:
    """Return the 0-indexed line of the closing brace for the fn at start_idx.

    `start_idx` is the line containing the `fn` keyword. Returns None if no
    matching brace is found (malformed source).
    """
    depth = 0
    started = False
    for idx in range(start_idx, len(lines)):
        stripped = strip_strings_and_comments(lines[idx])
        for ch in stripped:
            if ch == "{":
                depth += 1
                started = True
            elif ch == "}":
                depth -= 1
        if started and depth == 0:
            return idx
    return None


def is_bodyless_fn_decl(lines: list[str], start_idx: int) -> bool:
    """True if the fn at start_idx is a declaration with no body.

    Trait method signatures and `extern` fn declarations terminate with
    `;` before any `{`. They have no body to measure; the brace-balance
    heuristic would otherwise run past them into unrelated code and report
    a wildly inflated span. Scanning is bounded to a few lines because a
    signature never spans many.
    """
    for idx in range(start_idx, min(start_idx + 12, len(lines))):
        stripped = strip_strings_and_comments(lines[idx])
        for ch in stripped:
            if ch == "{":
                return False
            if ch == ";":
                return True
    return False


def has_rationale_before(lines: list[str], fn_idx: int) -> bool:
    """Check that a `// rationale:` comment precedes the fn at fn_idx.

    Allows attributes and blank lines between the comment and `fn`.
    """
    scan = fn_idx - 1
    while scan >= 0:
        line = lines[scan]
        if BLANK_RE.match(line) or ATTR_RE.match(line):
            scan -= 1
            continue
        return bool(RATIONALE_RE.match(line))
    return False


def file_has_rationale_at_top(lines: list[str]) -> bool:
    """Check for `// rationale:` at the top of the file, after any `//!` block."""
    for line in lines:
        if DOC_INNER_RE.match(line) or BLANK_RE.match(line):
            continue
        return bool(RATIONALE_RE.match(line))
    return False


def check_file(path: Path) -> list[str]:
    """Return a list of failure messages for `path`. Empty list = pass."""
    failures: list[str] = []
    text = path.read_text(encoding="utf-8", errors="replace")
    lines = text.splitlines()
    total = len(lines)

    if total >= HARD_FILE_LINES:
        failures.append(
            f"FAIL: {path} has {total} lines (>= {HARD_FILE_LINES} hard limit)"
        )
    elif total >= SOFT_FILE_LINES and not file_has_rationale_at_top(lines):
        failures.append(
            f"FAIL: {path} has {total} lines (>= {SOFT_FILE_LINES}); "
            f"split into modules to get under {SOFT_FILE_LINES}. Only if "
            f"genuinely cohesive and irreducible, add '// rationale:' at top"
        )

    for idx, line in enumerate(lines):
        if not FN_RE.match(line):
            continue
        if is_bodyless_fn_decl(lines, idx):
            continue
        end = find_fn_end(lines, idx)
        if end is None:
            continue
        fn_lines = end - idx + 1
        if fn_lines >= HARD_FN_LINES:
            failures.append(
                f"FAIL: {path}:{idx + 1} function spans {fn_lines} lines "
                f"(>= {HARD_FN_LINES} hard limit)"
            )
        elif fn_lines >= SOFT_FN_LINES and not has_rationale_before(lines, idx):
            failures.append(
                f"FAIL: {path}:{idx + 1} function spans {fn_lines} lines "
                f"(>= {SOFT_FN_LINES}); extract a helper or split to get "
                f"under {SOFT_FN_LINES}. Only if genuinely irreducible, add "
                f"a single-line '// rationale:' directly above it"
            )
    return failures


def tracked_rs_files(root: Path) -> list[Path]:
    """`.rs` files tracked or untracked-but-not-ignored, via `git ls-files`.

    NOT a filesystem walk: `rglob` descends into `target/` before the
    skip and can race a concurrent writer (cargo, rust-analyzer),
    aborting mid-walk — a phantom, non-deterministic failure. git prunes
    ignored paths itself, so the inspected set is a pure function of repo
    state, never the live build tree.
    """
    try:
        out = subprocess.run(
            [
                "git", "ls-files", "-z", "--cached", "--others",
                "--exclude-standard", "--", "*.rs",
            ],
            cwd=root,
            check=True,
            capture_output=True,
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        print(
            f"FAIL: rationale_check.py: cannot enumerate source via git "
            f"in {root} ({exc})",
            file=sys.stderr,
        )
        sys.exit(1)
    rel = (n for n in out.stdout.decode("utf-8", "replace").split("\0") if n)
    return [root / n for n in rel]


def main() -> int:
    """Check every tracked .rs file under the given root (default cwd)."""
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.cwd()
    targets = [root] if root.is_file() else tracked_rs_files(root)

    all_failures: list[str] = []
    for p in targets:
        all_failures.extend(check_file(p))

    for msg in all_failures:
        print(msg, file=sys.stderr)
    return 1 if all_failures else 0


if __name__ == "__main__":
    sys.exit(main())

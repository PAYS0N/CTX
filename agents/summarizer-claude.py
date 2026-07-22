#!/usr/bin/env python3
"""Reference summarization-agent adapter (Anthropic API).

This is the pluggable, model-specific edge of the system — deliberately
NOT Rust and NOT under the lint regime, exactly like `prompts/`. It is the
default target for `CTX_AGENT_CMD`:

    export CTX_AGENT_CMD='python3 agents/summarizer-claude.py'

Contract (identical for any provider's adapter; see agents/README.md):

  stdin  : one JSON object {"system": <str>, "user": <str>, "model": <str>}
  stdout : the model's completion text, verbatim, nothing else
  exit   : 0 on success; non-zero with a message on stderr otherwise

`system` is the verbatim prompt file; `user` is the runner-assembled
dynamic data; `model` is the model to call, chosen by the Rust CLI's
required `--model` flag. This adapter adds NO instructions of its own (it
honors the prompts/README.md decoupling rule): it is a thin transport only.

Environment:
  ANTHROPIC_API_KEY     required
  CTX_AGENT_MAX_TOKENS  default: 2048 (summaries are small)
  CTX_AGENT_TEMPERATURE default: 0 (stable, strict-format output)
  ANTHROPIC_BASE_URL    default: https://api.anthropic.com

The verbatim prompt is sent as a cached system block: the summarizer
reuses one identical system prompt across every leaf/rollup call in a
task, so prompt caching cuts cost/latency materially. Reasoning is
fixed at its minimum (thinking disabled, output_config effort "low")
since summarization is a short, strict-format task — except on haiku
models, which reject the `effort` param outright (HTTP 400) and so get
neither.
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request

DEFAULT_MAX_TOKENS = 2048
API_VERSION = "2023-06-01"


def die(message: str) -> "None":
    """Write a message to stderr and exit non-zero."""
    print(f"summarizer-claude: {message}", file=sys.stderr)
    raise SystemExit(1)


def read_request() -> "tuple[str, str, str]":
    """Parse the {"system","user","model"} JSON object from stdin."""
    raw = sys.stdin.read()
    try:
        obj = json.loads(raw)
    except json.JSONDecodeError as exc:
        die(f"stdin is not valid JSON: {exc}")
    if not isinstance(obj, dict):
        die("stdin JSON must be an object")
    system = obj.get("system")
    user = obj.get("user")
    model = obj.get("model")
    if not isinstance(system, str) or not isinstance(user, str):
        die("JSON must have string 'system' and 'user' fields")
    if not isinstance(model, str):
        die("JSON must have a string 'model' field")
    return system, user, model


def build_body(system: str, user: str, model: str) -> "dict":
    """Assemble the Messages API request body (verbatim; no extra text)."""
    max_tokens = int(os.environ.get("CTX_AGENT_MAX_TOKENS", DEFAULT_MAX_TOKENS))
    body = {
        "model": model,
        "max_tokens": max_tokens,
        # Summaries are short, strict-format, low-intelligence tasks: keep
        # reasoning at its minimum (thinking off, lowest effort).
        "thinking": {"type": "disabled"},
        # List form with cache_control so the identical per-task system
        # prompt is cached across many leaf/rollup calls.
        "system": [
            {
                "type": "text",
                "text": system,
                "cache_control": {"type": "ephemeral"},
            }
        ],
        "messages": [{"role": "user", "content": user}],
    }
    # Haiku models reject the effort param outright (HTTP 400); only
    # non-haiku models get the lowest-effort hint.
    if "haiku" not in model:
        body["output_config"] = {"effort": "low"}
    return body


def call_api(body: "dict") -> "dict":
    """POST to the Messages API and return the parsed response."""
    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        die("ANTHROPIC_API_KEY is not set")
    base = os.environ.get("ANTHROPIC_BASE_URL", "https://api.anthropic.com")
    request = urllib.request.Request(
        f"{base.rstrip('/')}/v1/messages",
        data=json.dumps(body).encode("utf-8"),
        method="POST",
        headers={
            "x-api-key": api_key,
            "anthropic-version": API_VERSION,
            "content-type": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(request) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", "replace")
        die(f"HTTP {exc.code}: {detail}")
    except urllib.error.URLError as exc:
        die(f"network error: {exc.reason}")


def extract_text(response: "dict") -> str:
    """Concatenate the text content blocks of a Messages response."""
    blocks = response.get("content")
    if not isinstance(blocks, list):
        die(f"unexpected response shape: {json.dumps(response)[:300]}")
    text = "".join(
        b.get("text", "") for b in blocks if isinstance(b, dict) and b.get("type") == "text"
    )
    if not text.strip():
        die("model returned empty text")
    return text


def main() -> "None":
    """Stdin JSON -> one Messages call -> completion text on stdout."""
    system, user, model = read_request()
    response = call_api(build_body(system, user, model))
    sys.stdout.write(extract_text(response))


if __name__ == "__main__":
    main()

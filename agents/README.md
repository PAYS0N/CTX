# Agents

Model-specific adapters that bridge the system's model-agnostic agent
contract to a concrete LLM. **Decoupled, non-Rust, not under the lint
regime** — the same posture as `prompts/`. The Rust core
(`ctx-summarize`'s `Agent` seam) never names a provider; these scripts do.

## The contract (every adapter must implement exactly this)

```
stdin  : one JSON object {"system": <string>, "user": <string>, "model": <string>}
stdout : the model's completion text, verbatim — nothing else
exit   : 0 on success; non-zero with a message on stderr otherwise
```

- `system` is the verbatim prompt file (`prompts/summarizer-leaf.md` or
  `prompts/summarizer-rollup.md`). `user` is the runner-assembled dynamic
  data (source path + contents, or directory children + intent). `model`
  is the model to call, set by the Rust CLI's required `--model` flag —
  there is no adapter-side default or fallback.
- An adapter is a **thin transport**: it MUST NOT add instructions,
  reformat, or wrap the prompt/data (honors `prompts/README.md`). The
  prompt files alone govern behavior so they stay portable across models.
- Empty output or any failure must be a non-zero exit so the runner
  surfaces it as an agent error rather than writing a bad summary.

## Wiring

```
export CTX_AGENT_CMD='python3 agents/summarizer-claude.py'
export ANTHROPIC_API_KEY=sk-...
ctx-summarize from-cache --task-id <uuid>
```

`ctx-summarize` shells `$CTX_AGENT_CMD` via `sh -c`, so the value may
include arguments or be any executable implementing the contract.

### `.env` convenience

Rather than exporting the key, put it in a gitignored `.env` (see
`.env.example`) and have the command source it. This makes the key
available regardless of which shell launches `ctx-summarize`:

```
export CTX_AGENT_CMD='sh -c "set -a; . /ABS/PATH/.env; set +a; exec python3 /ABS/PATH/agents/summarizer-claude.py"'
```

The adapter itself stays dependency-free and only reads the environment;
the `.env` sourcing is purely in the wrapper.

## Reference adapter

`summarizer-claude.py` — Anthropic Messages API, python3 stdlib only (no
pip/curl/jq). Env: `ANTHROPIC_API_KEY` (required), `CTX_AGENT_MAX_TOKENS`
(2048), `CTX_AGENT_TEMPERATURE` (0), `ANTHROPIC_BASE_URL`. The model
called is the stdin JSON's `model` field, not an env var. Sends the per-task
system prompt as a cached block (it is identical across every leaf/rollup
call in a task), which materially cuts summarizer cost and latency.
Reasoning is fixed at its minimum (`thinking: disabled`, `output_config.
effort: low`) since leaf/rollup summarization is a short, strict-format
task that doesn't benefit from extended thinking.

## Adding another provider

Drop a sibling script implementing the contract (e.g.
`summarizer-openai.py`, or one that shells a local CLI) and point
`CTX_AGENT_CMD` at it. No Rust changes are needed or wanted — that
decoupling is the point.

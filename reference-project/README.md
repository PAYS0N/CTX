# Reference Project

This is a placeholder. Build it once the `ctx-access` CLI and summarization
runner are working.

## Proposed scope

A CLI tool that parses, validates, and pretty-prints a non-trivial config
format. Suggested format: a TOML-superset that adds schema validation, or
a JSON-Schema-validated YAML.

The goal is to exercise:

- Multiple modules with non-trivial public surface.
- Error types other than `Box<dyn Error>`.
- File IO without async.
- A real public API surface that other crates could consume.
- Enough size (target ~2000-4000 LoC) that the context tree has multiple
  levels and the lint thresholds bite.

## Procedure

1. Initialize the project from `template/`.
2. Write an `intent.md` describing the config validator's purpose.
3. Run an agent task under `ctx-access` to add the first module.
4. Run the summarization agent.
5. Inspect the produced `.ctx` and `rollup.ctx` files. Revise the prompts.
6. Repeat. Note what is intolerable.

## Output

When complete, document:

- Which lints triggered most often, and which felt counterproductive.
- Which `// rationale:` comments would be better expressed as smaller
  functions, vs. which are genuinely irreducible.
- Whether the chain-read access pattern slowed agents intolerably.
- Whether prompts produced summaries that the next agent could actually
  use, vs. summaries that were technically correct but useless.

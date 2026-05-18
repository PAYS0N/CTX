# Reference Project — CLI Meal Planner

Placeholder until `ctx-access` (done) and `ctx-check` are working and the
summarization runner exists. This is the system's end-to-end smoke test:
it is built entirely through `ctx-access`, verified through `ctx-check`,
and summarized by the summarizer agent. See `docs/SPEC.md` § Reference
project for the sealed definition (rev 3).

## Scope

A CLI meal planner that:

- Captures a user nutritional profile (weight, age, gender, activity,
  allergies, dietary restrictions) and persists it permanently.
- Generates a one-week meal plan close to the WHO/FAO dietary guidelines.
- Saves favorite meals and incorporates them into future plans.
- Revises an existing plan from free-text user feedback.
- Emits a full shopping list aggregated from a plan.

The core ideation step (proposing meals for a profile + constraints) is an
LLM call.

## Hard design constraints

These are not negotiable; they are what make this a valid test of the
rule set rather than just an app.

1. **Integer / fixed-point numerics only.** kcal as integers, nutrients
   as integer milligrams, energy ratios as basis points (1% = 100 bp),
   explicit documented rounding at every boundary. `float_arithmetic` is
   denied and stays denied. Capture where this is clean and where it is
   genuinely painful — that judgement is a primary output.
2. **LLM behind a `MealIdeator` trait**, with a deterministic in-memory
   fake for all tests. Same seam pattern as `Env`/`Summarizer` in
   `ctx-access`. The non-deterministic boundary never enters the testable
   core.
3. **Network/client quarantined** behind that trait. Expect to widen
   `deny.toml` (license allowlist, `multiple-versions`/`skip-tree`)
   deliberately; that exercise is wanted.
4. Persistence is local file IO (profile + favorites + saved plans),
   typed errors (no `Box<dyn Error>` in library modules).

Target ~2000-4000 LoC across multiple modules so the context tree has
several levels and the lint thresholds bite.

## Procedure

1. Initialize the project from `template/`.
2. Write `intent.md` describing the planner's purpose and the constraints
   above.
3. Run agent tasks under `ctx-access` to add modules; verify each via
   `ctx-check`.
4. Run the summarization agent; inspect `.ctx`/`rollup.ctx`; revise
   prompts. Repeat. Note what is intolerable.

## Output (document when complete)

- Which lints triggered most often; which felt counterproductive.
- How tolerable integer/fixed-point modeling was for guideline math —
  the central question for the `float_arithmetic` rule.
- Which `// rationale:` comments would be better as smaller functions vs.
  genuinely irreducible.
- Dependency-policy friction introduced by the LLM/HTTP client.
- Whether the chain-read access pattern slowed agents intolerably.
- Whether summaries were usable by the next agent vs. correct-but-useless.

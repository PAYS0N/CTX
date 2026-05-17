# Deferred Dylint Rules

Each rule is specified for future implementation in a custom dylint crate.
None are built at MVP. The pre-commit scripts in `scripts/` cover the
mechanical subset (rule 1, line-count portion) for now.

## 1. Rationale-comment enforcement

Replaces the pre-commit `rationale_check.sh` script. Lint flags functions
30-80 lines without a `// rationale:` comment immediately preceding the
`fn` keyword, and files 250+ lines without `// rationale:` at the top.

Also covers cognitive complexity 15-25 (the soft tier deferred at MVP)
once a clippy-compatible complexity metric is exposed to dylint.

## 2. Deep-modules rule (Ousterhout)

Flags modules whose public-interface complexity is large relative to
implementation size. Heuristic: count of `pub fn`, `pub struct`, `pub enum`,
`pub trait` items weighted by parameter count and generic-parameter count,
divided by total non-blank non-comment lines in the module. Threshold
tunable per project in `clippy.toml`.

Intent: penalize "shallow" modules that expose nearly everything they do,
encourage modules that hide significant work behind small interfaces.

## 3. Shallow-function rule

Flags functions whose body is a single call passing arguments through
unchanged or with only trivial transformations (renames, struct wrapping
that adds no validation). These functions add interface surface without
adding behavior.

Exception: explicit `#[inline]` trampolines, trait impls, builder-pattern
methods.

## 4. No-circular-module-deps as first-class lint

Currently enforced by `scripts/cycle_check.sh` invoking `cargo-modules`.
Move detection inside dylint so it runs as part of the normal lint pipeline
with clippy-style error reporting.

## 5. Public-surface stability

Flags changes to `pub` signatures in non-bin crates without a corresponding
entry in `CHANGES.md` or a `#[stable_since = "..."]` attribute on the
changed item. Requires baseline storage between lint runs (probably a
`pub_surface.lock` file checked into the repo).

This rule bridges into the deferred architecture-audit layer.

## 6. Float-arithmetic wrapper enforcement

The `float_arithmetic` clippy restriction denies raw float math. This rule
provides the positive side: a sanctioned wrapper pattern (`Real`, `Money`,
`Probability`, etc.) and a lint that flags any `+`, `-`, `*`, `/`, `%`
operator applied to a value whose type ultimately wraps an `f32` or `f64`
unless the type implements a sealed `SafeFloat` marker trait.

## 7. Error-type ergonomics

Flags `Result<T, Box<dyn Error>>` and `Result<T, anyhow::Error>` in
non-binary crates (library crates and shared modules). Requires concrete
error enums (typically derived via `thiserror`) so callers can match on
specific failure modes.

Binary crates may use boxed/`anyhow` errors freely.

## 8. No-string-typing

Flags `String` and `&str` parameters and struct fields whose names suggest
they carry semantic identity (`user_id`, `email`, `password`, `path`,
`url`, `token`, suffixed with `_id`, `_name`, `_email`, etc.). Suggests
newtype wrapping.

Heuristic; needs tuning and a project-level allowlist.

## 9. Match-exhaustiveness over closed enums

Flags `match` arms that use `_` on a user-defined enum (an enum defined in
the current workspace, not from an external crate or `std`). Closed enums
should be exhaustively listed so that adding a variant causes compile
errors at every match site, not silent fall-through.

Exception: `match` arms with `_` that immediately call `unreachable!()`
are allowed when the enum has a documented "impossible at this point"
variant. (Note: `unreachable` is itself a restriction lint denied at MVP;
this exception applies only if/when that lint is relaxed.)

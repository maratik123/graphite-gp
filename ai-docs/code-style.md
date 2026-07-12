# Code style — graphite-gp

The summary in AGENTS.md § Code Style is the quick reference; this is the canonical detail, and it **grows through the learning loop** (`/improve` escalates recurring style corrections here). Start:

## Source files
Rust-only (`.rs`) under `crates/*/src/`. Format with `cargo fmt` (workspace), never `rustfmt <file>`. rustfmt defaults (100 cols).

## Linter posture
Strict clippy: `cargo clippy --workspace --all-targets -- -D warnings`. No blanket `#[allow]` without a justifying comment.

## Rust idioms
Prefer idiomatic Rust over literal ports. Comparison/combinator helpers (`.min`/`.max`/`.clamp`/`Option::or`/`Option::filter`) over explicit `if`/`match`. **`gp-core` is integer-only and deterministic** — no floating point in `geom`/`sim` (design doc §3a); floats are confined to `gp-render` and `gp-ai` feature/curve code.

## Magic numbers
Semantic numeric literals → module-level `const SCREAMING_SNAKE_CASE`. Self-evident constants (`0`, `1`, `-1`, `2`) and test fixtures exempt.

## Error types
`thiserror` for new error enum/struct; hand-rolled `Display`/`Error` only where the derive cannot express it.

## File size
Target 200–400 lines per `.rs` file (excluding `#[cfg(test)]`); refactor larger files before merge unless exempt (single `match`/state machine, `macro_rules!`). Counter-rule: one-struct-per-file is not Rust idiom — don't over-split.

# Code style — graphite-gp

The summary in AGENTS.md § Code Style is the quick reference; this is the canonical detail, and it **grows through the learning loop** (`/improve` escalates recurring style corrections here). Start:

## Source files
Rust-only (`.rs`) under `crates/*/src/`. Format with `cargo fmt` (workspace), never `rustfmt <file>`. rustfmt defaults (100 cols).

## Linter posture
Strict clippy: `cargo clippy --workspace --all-targets -- -D warnings`. No blanket `#[allow]` without a justifying comment.

**Where the policy lives.** Workspace-wide lint policy is declared once in the root `Cargo.toml` — `[workspace.lints.rust]` + `[workspace.lints.rustdoc]` + `[workspace.lints.clippy]` — plus a root `clippy.toml` carrying the size-aware thresholds (`stack-size-threshold` / `array-size-threshold`), which clippy auto-discovers from the workspace root. The root is a **virtual** workspace (no root package), so there is no package to carry the lints implicitly: every member crate opts in explicitly with `[lints]` / `workspace = true` in its own `Cargo.toml`.

**The enforced set.** `missing_docs = "deny"` (rust), `rustdoc::broken_intra_doc_links = "deny"` (rustdoc), and for clippy: `pedantic` and `nursery` at `{ level = "deny", priority = -1 }` — the `priority = -1` keeps the group denies *below* the specific `clippy::* = "allow"` entries so the allows win — plus `large_stack_frames`, `large_stack_arrays`, and `undocumented_unsafe_blocks` listed separately as `deny` (each written out so it survives a future per-group rollback of pedantic/nursery).

**Allow-list discipline.** Every `clippy::* = "allow"` entry in `[workspace.lints.clippy]` MUST carry a one-line `#` justification comment. The current allows are `must_use_candidate`, `redundant_pub_crate`, and `return_self_not_must_use` (rationale in the `Cargo.toml` comments). In-source `#[allow(clippy::…)]` is reserved for the **unavoidable** case and still needs a justifying comment (AGENTS.md § Code Style / § Rust Test Conventions — the rule lives there, not duplicated here). Where a clean fix isn't possible, a **justified carve-out** is preferred over a behaviour-changing fix. Shipped example: the two `#[allow(clippy::cast_sign_loss)]` carve-outs in `crates/core/src/geom.rs` (`Corridor::new` and `Corridor::index`), each justified by the preceding non-negativity guarantee, leaving the integer arithmetic unchanged. The attribute sits on the enclosing `let` statement because expression-level attributes are unstable on stable Rust.

**CI enforcement.** The clippy job runs `cargo clippy --workspace --all-targets -- -D warnings` and the workflow sets `CARGO_BUILD_WARNINGS: deny`; together they mean *anything not carved out is denied* — the lint-table allows are the carve-outs. Denial is CI-side by design: there is no manifest-level `deny(warnings)` (toolchain-brittle).

## Rust idioms
Prefer idiomatic Rust over literal ports. Comparison/combinator helpers (`.min`/`.max`/`.clamp`/`Option::or`/`Option::filter`) over explicit `if`/`match`. **`gp-core` is integer-only and deterministic** — no floating point in `geom`/`sim` (design doc §3a); floats are confined to `gp-render` and `gp-ai` feature/curve code.

## Magic numbers
Semantic numeric literals → module-level `const SCREAMING_SNAKE_CASE`. Self-evident constants (`0`, `1`, `-1`, `2`) and test fixtures exempt.

## Error types
`thiserror` for new error enum/struct; hand-rolled `Display`/`Error` only where the derive cannot express it.

## File size
Target 200–400 lines per `.rs` file (excluding `#[cfg(test)]`); refactor larger files before merge unless exempt (single `match`/state machine, `macro_rules!`). Counter-rule: one-struct-per-file is not Rust idiom — don't over-split.

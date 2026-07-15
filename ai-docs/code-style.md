# Code style — graphite-gp

The summary in AGENTS.md § Code Style is the quick reference; this is the canonical detail, and it **grows through the learning loop** (`/improve` escalates recurring style corrections here). Start:

## Source files
Rust-only (`.rs`) under `crates/*/src/`. Format with `cargo fmt` (workspace), never `rustfmt <file>`. rustfmt defaults (100 cols).

## Linter posture
Strict clippy: `cargo clippy --workspace --all-targets -- -D warnings`. No blanket `#[allow]` without a justifying comment.

**Where the policy lives.** Workspace-wide lint policy is declared once in the root `Cargo.toml` — `[workspace.lints.rust]` + `[workspace.lints.rustdoc]` + `[workspace.lints.clippy]` — plus a root `clippy.toml` carrying the size-aware thresholds (`stack-size-threshold` / `array-size-threshold`), which clippy auto-discovers from the workspace root. The root is a **virtual** workspace (no root package), so there is no package to carry the lints implicitly: every member crate opts in explicitly with `[lints]` / `workspace = true` in its own `Cargo.toml`.

**The enforced set.** `missing_docs = "deny"` (rust), `rustdoc::broken_intra_doc_links = "deny"` (rustdoc), and for clippy: `pedantic` and `nursery` at `{ level = "deny", priority = -1 }` — the `priority = -1` keeps the group denies *below* the specific `clippy::* = "allow"` entries so the allows win — plus `large_stack_frames`, `large_stack_arrays`, and `undocumented_unsafe_blocks` listed separately as `deny` (each written out so it survives a future per-group rollback of pedantic/nursery).

**Allow-list discipline.** Every `clippy::* = "allow"` entry in `[workspace.lints.clippy]` MUST carry a one-line `#` justification comment. The current allows are `must_use_candidate`, `redundant_pub_crate`, and `return_self_not_must_use` (rationale in the `Cargo.toml` comments). In-source `#[allow(clippy::…)]` is reserved for the **unavoidable** case and still needs a justifying comment (AGENTS.md § Code Style / § Rust Test Conventions — the rule lives there, not duplicated here). Where a clean fix isn't possible, a **justified carve-out** is preferred over a behaviour-changing fix.

**CI enforcement.** The clippy job runs `cargo clippy --workspace --all-targets -- -D warnings` and the workflow sets `CARGO_BUILD_WARNINGS: deny`; together they mean *anything not carved out is denied* — the lint-table allows are the carve-outs. Denial is CI-side by design: there is no manifest-level `deny(warnings)` (toolchain-brittle).

## Rust idioms
Prefer idiomatic Rust over literal ports. Comparison/combinator helpers (`.min`/`.max`/`.clamp`/`Option::or`/`Option::filter`) over explicit `if`/`match`. **`gp-core` is integer-only and deterministic** — no floating point in `geom`/`sim` (design doc §3a); floats are confined to `gp-render` and `gp-ai` feature/curve code.

## Magic numbers
Semantic numeric literals → module-level `const SCREAMING_SNAKE_CASE`. Self-evident constants (`0`, `1`, `-1`, `2`) and test fixtures exempt.

## Error types
`thiserror` for new error enum/struct; hand-rolled `Display`/`Error` only where the derive cannot express it.

## Deterministic collections
`gp-core` physics is deterministic (integer-only; `docs/design.md §3a`), and track generation / replay / AI training must reproduce bit-for-bit. **Production code MUST NOT use `std::collections::HashMap` / `HashSet`** — their iteration order is randomised (per-process `RandomState` seed), which silently breaks reproducibility. Prefer:

- `indexmap::IndexMap` / `IndexSet` when insertion-order iteration + hashing is wanted;
- `std::collections::BTreeMap` / `BTreeSet` when sorted-key iteration is wanted.

Test-only `HashSet` / `HashMap` under `#[cfg(test)]` (order-independent membership asserts) are **unaffected** — the ban is on production iteration determinism, not on test scratch collections.

## `#[inline]` on concrete cross-crate functions
Mark simple **concrete** functions that a caller in another crate invokes with `#[inline]`, so they inline across the crate boundary without relying on LTO. A concrete (non-generic) fn's MIR is **not** exported downstream, so without the attribute a cross-crate caller gets a real function call.

- **Typical targets:** field getters (`self.x`), trivial wrappers (`.as_deref()`, a single delegation call), `const fn` struct-literal constructors, single-delegation wrappers. The `gp-core` → `gp-game` integer kernel is the primary surface — `Point` / `Size` / `Rect` accessors and ops, `supercover`, corridor math.
- **Concrete only.** This rule covers the concrete case exclusively. A generic fn is already monomorphized per concrete type into the downstream crate, so `#[inline]` is redundant on it — do not add it there. (graphite-gp does **not** adopt any `_Simple._` doc-tag / recursive-cascade marker machinery — concrete half only.)
- **Maintenance.** When an edit makes a previously-simple concrete fn non-simple (gains branches/loops or > 1 non-trivial call), strip the now-misleading `#[inline]` in the same edit.

## Enum repr
`#[repr(...)]` on an enum is justified in exactly two cases:

1. **`enumflags2::bitflags` contract** — the macro requires `#[repr(uN)]` on its target enum to keep the bitfield arithmetic sound (the anticipated graphite-gp case: `enumflags2` for `legal_mask`, a near-term adoption — so this rule is preventive for now).
2. **External numeric spec carried in discriminants** — when an enum's discriminants are fixed by an external standard and the raw integer type matters.

In all other cases `#[repr]` MUST NOT be added. Decorative annotations (e.g. `#[repr(i64)]` to "match" a wire format the runtime already handles) add noise without correctness value and are forbidden.

## File size
Ladder (lines per `.rs` file):

- **Soft limit:** 500 excl. `#[cfg(test)]` / 800 incl. tests. On crossing, trigger a split-by-responsibility check (e.g. `sim.rs` / `geom.rs` / `graph.rs`) — do **not** split mechanically by line count.
- **Hard limit:** 1000 excl. tests / 1500 incl. tests. Refactor before merge unless an exemption applies.
- **Exemptions:** auto-generated / codegen output; a single state machine or `match` where splitting would obscure the control flow; `macro_rules!` definitions.
- **Counter-rule — do not over-split.** One-struct-per-file (Java / C# habit) is not Rust idiom and bloats the `mod` tree; prefer one cohesive ~300-line file over three ~100-line fragments.
- **Per-function:** Clippy's `too_many_lines` (> 100) is the canonical fn-level signal — keep functions under it; small functions naturally yield small files.

## Lints that mechanically enforce parts of this convention
CI runs `cargo clippy --workspace --all-targets -- -D warnings` (with `CARGO_BUILD_WARNINGS: deny`), so every lint below is a hard error in practice. The workspace declares them once in the root `Cargo.toml` `[workspace.lints.*]` (+ root `clippy.toml` for the size-aware thresholds); each member crate opts in via `[lints] workspace = true`.

- `missing_docs = "deny"` (rust) — every public item has at least a one-line doc. Owned by [`doc-convention.md`](doc-convention.md).
- `rustdoc::broken_intra_doc_links = "deny"` (rustdoc) — every intra-doc link resolves. Owned by [`doc-convention.md`](doc-convention.md).
- `clippy::undocumented_unsafe_blocks = "deny"` — every `unsafe` block carries a `// SAFETY:` comment. Owned by [`doc-convention.md`](doc-convention.md).
- `clippy::pedantic` / `clippy::nursery` (`deny`, `priority = -1`) — the group denies sit *below* the specific `clippy::* = "allow"` carve-outs (`must_use_candidate`, `redundant_pub_crate`, `return_self_not_must_use`) so the allows win. Owned by [Linter posture](#linter-posture).
- `clippy::missing_errors_doc` (via pedantic) — `# Errors` section on every `Result`-returning public fn. Owned by [`doc-convention.md`](doc-convention.md).
- `clippy::missing_panics_doc` (via pedantic) — `# Panics` section on every fn that can panic. Owned by [`doc-convention.md`](doc-convention.md).
- `clippy::doc_markdown` (via pedantic) — flags un-backticked `CamelCase` identifiers in prose. Owned by [`doc-convention.md`](doc-convention.md).
- `clippy::too_many_lines` (via pedantic, > 100) — canonical fn-level size signal. Owned by [File size](#file-size).
- `large_stack_frames` / `large_stack_arrays` (`deny`) — size-aware thresholds from the root `clippy.toml` (`stack-size-threshold` / `array-size-threshold`). Owned by [Linter posture](#linter-posture).
- `-D warnings` posture — every clippy warning is an error. Owned by [Linter posture](#linter-posture).
- `cargo fmt -- --check` (rustfmt enforcement) — line length, whitespace, layout. Owned by [Source files](#source-files).

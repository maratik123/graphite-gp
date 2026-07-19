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

## Integer safety
`gp-core` targets zero production panics; integer arithmetic and conversions MUST be overflow- and signedness-safe by construction.

- Reach for the explicit-semantics method that matches intent: `checked_*` → `Option`/`None` on out-of-range; `saturating_*` → clamp to bound; `wrapping_*`/`overflowing_*` → explicit modular; `strict_*` → always-panic (even in release) for a true invariant; `abs_diff` → unsigned magnitude of a difference; `carrying_*`/`borrowing_*` → multi-word chains.
- Prefer `usize::try_from(i32)?` / `u32::try_from(...)` over `as` casts wherever an out-of-domain input could overflow or lose sign — `try_from` folds the negative-value guard into the conversion (no explicit `< 0` check, no `#[allow(clippy::cast_sign_loss)]`).
- Raw `+`/`-`/`*`/`/` are allowed only where operands are knowingly bounded so no overflow, signedness issue, or division-by-zero can occur — and that safety rests on an assumption, so it MUST be covered by a test that exercises the bound.
- Do NOT defer overflow/signedness safety as "out of scope" when the surrounding change is already hardening the same code path.

Mechanically enforced by `clippy::arithmetic_side_effects = "deny"` (workspace-wide) plus pedantic's cast lints — every raw op / unsafe cast is flagged at commit/CI time; bounded-counter exceptions are allow-listed with a justifying comment. See [Lints that mechanically enforce…](#lints-that-mechanically-enforce-parts-of-this-convention).

## Magic numbers
Semantic numeric literals → module-level `const SCREAMING_SNAKE_CASE`. Self-evident constants (`0`, `1`, `-1`, `2`) and test fixtures exempt.

## Golden-image thresholds
Golden snapshot tests (`egui_kittest`) pick their compare tolerance by **content class**, not by copy-paste from a sibling golden:

- **Text-bearing goldens** — anything that renders glyphs, labels, numerals, or icons (arrows, sublabels) — use the crate's established **measured** text threshold `.threshold(1.0).failed_pixel_count_threshold(0)`. Cross-renderer AA / font rounding gives 1-level channel deltas on text pixels that exact compare (`threshold(0.0)`) fails in CI while passing local mint. In-tree precedent (all four text goldens): `widget_gallery` (`gallery.rs`), `forms_gallery`, `game_gallery`, `movepad_gallery`.
- **Flat / byte-stable goldens** — solid fills, no text (e.g. `placeholder`) — stay exact `.threshold(0.0).failed_pixel_count_threshold(0)`. In-tree precedent: `placeholder` (`placeholder.rs`).

`failed_pixel_count_threshold(0)` stays exact in both classes — the colour `threshold` is the sole absorbing lever. Do **not** adopt a "mint at 0.0, bump to 1.0 only if CI reds" strategy for a text golden: it knowingly schedules a wasted red-CI round on a question the precedent has already answered. `image-check` cannot catch a wrong threshold — it owns presence / shape / colour / position and explicitly disclaims AA / rounding, which the golden's exact compare owns.

## Error types
`thiserror` for new error enum/struct; hand-rolled `Display`/`Error` only where the derive cannot express it.

## Deterministic collections
`gp-core` physics is deterministic (integer-only; `docs/design.md §3a`), and track generation / replay / AI training must reproduce bit-for-bit — including **across platforms and toolchain versions**: a replay stores only the seed and *regenerates* its track (`docs/design.md §2 [N4]`, `§5 [M3]` — the seeded integer path is bit-deterministic; only `f32` bot features may diverge, and those are mitigated separately). **Production code MUST NOT rely on `std::collections::HashMap` / `HashSet` iteration order.** That order is the `hashbrown` table (slot) layout — std documents every `HashMap`/`HashSet` iterator as *"arbitrary order"* and guarantees nothing about it across releases, so relying on it silently breaks cross-toolchain reproducibility.

For order that reaches output, pick by the order semantics you need (both keep membership O(1)):

- `std::collections::BTreeMap` / `BTreeSet` — sorted-key iteration, zero-dependency, stable across toolchains. Needs `Ord` on the key; `gp-core`'s integer `Point` derives `Eq + Hash` but **not** `Ord` yet, so this costs a small, deliberate derive addition.
- `indexmap::IndexMap` / `IndexSet` — insertion-order iteration, independent of any hasher. Needs only `Eq + Hash` (`Point` already has them) but adds the `indexmap` dependency.

**When the key or element is itself an enum** (a closed variant set), reach past the arbitrary-key options above for an enum-specialized container: `enum_map::EnumMap<K, V>` for a map, `enumflags2::BitFlags<K>` for a set. Both are array-/bit-backed, **total** (every variant is present / representable), allocation- and hasher-free, and deterministic *by construction* — iteration follows the enum's **declaration order**, with no `Ord`/`Hash` obligation and no extra footgun to remember. Prefer them over `HashMap<Enum, _>` / `HashSet<Enum>` and even over `IndexMap` / `BTreeMap` / `BTreeSet` whenever the domain is a closed enum; `EnumMap` is also the idiomatic replacement for a hand-written `[V; N]` indexed by an enum's discriminant surrogate (`variant as usize`), and its `enum_map! { Variant => … }` constructor makes an omitted variant a **compile error** (exhaustiveness), which a positional array cannot. `EnumMap` needs `K: enum_map::Enum` (a derive); `BitFlags` needs `#[bitflags] #[repr(uN)]` on the enum (see [Enum repr](#enum-repr)). In-tree precedents: `gp-render`'s `IconSet` (`EnumMap<Icon, TextureHandle>`) and `gp-core`'s `legal_mask` (`BitFlags<Action>`).

**A fixed / seeded `BuildHasher` (`HashMap::with_hasher`) is NOT an escape hatch.** It pins the hash *values*, not the *slot layout* that determines iteration order, so a `HashMap`/`HashSet` iterated into output still diverges after a `hashbrown` / toolchain bump. Seeding removes only the per-process `RandomState`; same-binary determinism ≠ cross-build reproducibility. A fixed-hasher `HashMap`/`HashSet` used purely for **membership** (`contains` / `insert`, never iterated into output) is the sole production-safe use — but `IndexSet` / `BTreeSet` already give O(1) membership without the footgun, so prefer them.

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
- `clippy::arithmetic_side_effects = "deny"` — raw `+`/`-`/`*`/`/` on integers flagged unless allow-listed with a justifying comment. Owned by [Integer safety](#integer-safety).
- `-D warnings` posture — every clippy warning is an error. Owned by [Linter posture](#linter-posture).
- `cargo fmt -- --check` (rustfmt enforcement) — line length, whitespace, layout. Owned by [Source files](#source-files).

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

## Construct from fixed inputs directly — don't route through iterator plumbing
When the input is a **fixed, known set** — an array literal, a known-length tuple, or a closed enum's variants — build the target with a direct `From` / constructor, not an iterator chain that immediately re-collects. Same result, clearer intent, no throwaway adaptor.

| Instead of | Write |
|---|---|
| `HashSet::from_iter([a, b, c])` | `HashSet::from([a, b, c])` |
| `[a, b, c].into_iter().collect()` | `HashSet::from([a, b, c])` |
| `std::iter::once(x).collect()` | `HashSet::from([x])` |
| `[a, b, c].to_vec()` | `vec![a, b, c]` |
| `arr.into_iter().map(f).collect()` | `HashSet::from(arr.map(f))` — mind the `[T; N]::map` caveat below |
| `once(a).chain(once(b))` | `<[T; 2]>::from(pair).into_iter()` — a 2-tuple `From`-converts to `[T; 2]` |
| hand-listing every variant `[V::A, V::B, V::C].into_iter()` | derive `strum::VariantArray`, iterate `V::VARIANTS` |

**When NOT to.** The target must be a fixed input. Keep `.collect()` / `from_iter` when the source is a genuine iterator of unknown length, a filtered/lazy stream, or a generic context where the concrete array type isn't known. The `vec![…]` rule targets only an **array literal** `.to_vec()` (`[a, b, c].to_vec()`) — calling `.to_vec()` to clone an existing slice or `Vec` into an owned one (`slice.to_vec()`, `layer.to_vec()`) is the correct, idiomatic use and is unaffected.

**`[T; N]::map` caveat.** In `HashSet::from(arr.map(f))` the `arr.map(f)` is the *sanctioned* use — `From` genuinely wants an array, and std says "if you're doing a one-step `map` and really want an array as the result, then absolutely use this method." But `array::map` is **eager** (it evaluates `f` all `N` times up front) and can carry **high stack usage** for long arrays, complex mapping closures, or debug builds — see std `[T; N]::map` → *"Note on performance and stack usage."* Short, simple arrays (a handful of tuples/`Point`s, `[u8; 3]`, `[f32; 4]`) are fine; when the array is long or the closure heavy, keep `arr.into_iter().map(f).collect()` — a lazy iterator that never materializes the intermediate array.

**`strum::VariantArray` for closed variant sets.** Deriving `VariantArray` and iterating `V::VARIANTS` is the idiom for walking every variant of a closed enum — it also removes a maintenance hazard: a newly-added variant is picked up automatically instead of being silently dropped from a hand-maintained list. `strum` (0.28, `derive` feature) is already a workspace dependency.

In-tree precedent: PR [#148](https://github.com/maratik123/graphite-gp/pull/148) adopted `EnumIter` (`geom/graph.rs`, `gen/phase5b.rs`, `render/icons.rs`, `render/track/{regions,walls}.rs`, `render/widgets/{card,gallery}.rs`); PR [#168](https://github.com/maratik123/graphite-gp/pull/168) then migrated every one of those sites — plus `core/sim`, `core/track`, `gen/phase{1,2,3,5,5b,5_runout,6_arms}` — from `EnumIter`/`V::iter()` to `VariantArray`/`V::VARIANTS`. **`VARIANTS` is a `&'static [V]` slice, so it composes with slice APIs an iterator cannot**: `RaceDir::VARIANTS.choose(rng)` (`gen/phase1.rs:255`, `rand::seq::IndexedRandom`) replaced an `IteratorRandom::choose` over `RaceDir::iter()` that depended on strum's generated `size_hint`/`nth` being correct. Do not reintroduce `EnumIter` — there is no `EnumIter` derive left in-tree.

## Generic over the RNG, not over one engine
A function that draws randomness takes **`rng: &mut impl Rng`** (or a `R: Rng` bound), never a concrete engine type. The per-domain engine choice (`Xoshiro256PlusPlus` for generation / collision / AI-inference, `ChaCha8Rng` for AI-learning — issue #139) belongs to `gp_core::rng::Seeds` at the call site; a callee that names the engine hard-codes that choice into its signature and forces a mechanical retype of every consumer when a domain's engine changes.

- **Applies to test helpers too** — `fn draws(mut rng: impl Rng)` (`gen/src/lib.rs`) over `fn draws(mut rng: Xoshiro256PlusPlus)`, so a helper is reusable across engines.
- **The concrete type stays where the stream is *created*** — `Seeds::generation_rng() -> Xoshiro256PlusPlus`, `#[cfg(test)]` fixtures that `seed_from_u64`. Those are engine decisions, not engine plumbing.
- **Import the concrete engine under `#[cfg(test)]` only** when production code no longer names it, or the unused-import lint fires.

In-tree precedent: PR [#167](https://github.com/maratik123/graphite-gp/pull/167) — `resolve_collisions`, `phase1_coarse_ring`(`_attempts`), `widen`, `choose_dir`, `grow_blocks`, `build_p` all moved from `&mut Xoshiro256PlusPlus` to `&mut impl Rng`.

## Re-export a generic instantiation as a type alias
When a public API returns a *specific instantiation* of a third-party generic (Rust API guideline C-REEXPORT — the consumer must not need a direct dependency on that crate), publish a **`pub type` alias**, not a `pub use` of the bare generic.

| Instead of | Write |
|---|---|
| `pub use enumflags2::BitFlags;` + `-> BitFlags<Action>` at every site | `pub type Actions = BitFlags<Action>;` + `-> Actions` |

The alias names the domain concept once, shortens every signature and every `Actions::all()` / `Actions::empty()` / `Actions::from(..)` construction site, and keeps the third-party generic itself out of the crate's public surface. Declare it **next to the enum it wraps**, not beside the re-export it replaces.

In-tree precedent: PR [#169](https://github.com/maratik123/graphite-gp/pull/169) introduced `gp_core::sim::Actions`; PR [#170](https://github.com/maratik123/graphite-gp/pull/170) finished the sweep across `gp-core`/`gp-ai`/`gp-game`/`gp-render`, including prose and doc comments. `BitFlags` still has no `Sub` — `.remove(Action::North)` on a `mut` binding is the single-flag removal.

## Integer safety
`gp-core` targets zero production panics, and currently holds it — [`panic-index.md`](panic-index.md) has no `crates/core/` row. (PR #171's `supercover` interval solver briefly added two `i32::try_from(..).expect(..)` bounds; the revert removed them.) Integer arithmetic and conversions MUST be overflow- and signedness-safe by construction.

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

## Shared-boundary fill/stroke consistency
When a render layers a **fill** and a **stroke** for the *same* boundary, both MUST be built from the **same boundary geometry**. A smoothed outline (e.g. Chaikin) stroked over square per-cell fills — or the reverse — disagrees at every corner *by construction*, producing staircase notches / colour bleed that every automated gate misses: it passes `image-check`, exact compare, `self-review`, and CI; the product owner caught it twice (`track.png` per-cell `rect_filled` under a Chaikin-smoothed `closed_line` wall stroke; the speed-heatmap staircase past the same smoothed boundary).

- **Rule:** fill the region by recolouring the **shared** smoothed mesh — triangulate the boundary once, then tint / clip per-cell against that mesh — never draw independent unit-square `rect_filled` cells beneath a smoothed stroke of the same boundary.
- A design's § Risks flagging "convex-corner bleed" is **not** a substitute: the second occurrence shipped *with* that note. The geometry must be shared in the code, not merely risk-annotated.

## Golden setup fidelity — fixture & harness must match the real thing

A golden can PASS `image-check`, exact/measured compare, `self-review`, AND CI while the *real* output is broken — whenever the golden's **setup** (its fixture, or the harness conditions it forces) diverges from what an owner already approved or from what the binary actually does. Two occurrences, both 2026-07-22:

- **Reuse an established, owner-approved fixture — do not hand-roll a fresh one.** When a new golden/gallery test needs a domain fixture (a track, a scene) that an earlier PR already built *and had owner-approved by eye*, reuse it — share it via a `pub(crate)` `#[cfg(test)]` move, keeping the prior golden byte-identical — rather than deriving a new fixture from the nearest unit-test fixture. A hand-rolled fixture can silently reproduce a defect a prior review already fixed; `image-check`/`self-review` pass it because the golden correctly matches the drawing code — they never flag that the *fixture* is the pre-fix shape. Before hand-building a fixture for a visual golden, grep for an existing `scene_*` / gallery fixture of the same kind and prefer it. Corollary: when a reviewer cites a specific earlier commit/PR as "already fixed", read that commit first and apply *its* approach — do not re-derive a fix from scratch.
- **The golden harness must render under the binary's real conditions.** A golden that forces runtime conditions the binary does not set — a theme via `.with_theme(...)`, a window size, a visuals palette — can pass while the binary is visibly broken, because it tests the draw code under harness-chosen conditions, not the binary's real ones. Prefer making the draw code **self-sufficient**: a composition-root / full-screen widget paints its OWN background and uses its OWN palette tokens rather than the host's ambient `Visuals`, so golden == binary regardless of theme. Pin the binary's palette explicitly (e.g. `set_visuals(...)`) rather than inheriting the framework's system-following default. When a reviewer says "we need goldens here," first check whether a golden already exists but is passing under harness conditions that hide the real-runtime bug.

See `ai-docs/learnings.md` 2026-07-22 (fixture-reuse and forced-theme entries).

## Error types
`thiserror` for new error enum/struct; hand-rolled `Display`/`Error` only where the derive cannot express it.

## Deterministic collections
`gp-core` physics is deterministic (integer-only; `docs/design.md §3a`), and track generation / replay / AI training must reproduce bit-for-bit — including **across platforms and toolchain versions**: a replay stores only the seed and *regenerates* its track (`docs/design.md §2 [N4]`, `§5 [M3]` — the seeded integer path is bit-deterministic; only `f32` bot features may diverge, and those are mitigated separately). **Production code MUST NOT rely on `std::collections::HashMap` / `HashSet` iteration order.** That order is the `hashbrown` table (slot) layout — std documents every `HashMap`/`HashSet` iterator as *"arbitrary order"* and guarantees nothing about it across releases, so relying on it silently breaks cross-toolchain reproducibility.

For order that reaches output, pick by the order semantics you need (both keep membership O(1)):

- `std::collections::BTreeMap` / `BTreeSet` — sorted-key iteration, zero-dependency, stable across toolchains. Needs `Ord` on the key; `gp-core`'s integer `Point` derives `Eq + Hash` but **not** `Ord` yet, so this costs a small, deliberate derive addition.
- `indexmap::IndexMap` / `IndexSet` — insertion-order iteration, independent of any hasher. Needs only `Eq + Hash` (`Point` already has them) but adds the `indexmap` dependency.

**When the key or element is itself an enum** (a closed variant set), reach past the arbitrary-key options above for an enum-specialized container: `enum_map::EnumMap<K, V>` for a map, `enumflags2::BitFlags<K>` for a set. Both are array-/bit-backed, **total** (every variant is present / representable), allocation- and hasher-free, and deterministic *by construction* — iteration follows the enum's **declaration order**, with no `Ord`/`Hash` obligation and no extra footgun to remember. Prefer them over `HashMap<Enum, _>` / `HashSet<Enum>` and even over `IndexMap` / `BTreeMap` / `BTreeSet` whenever the domain is a closed enum; `EnumMap` is also the idiomatic replacement for a hand-written `[V; N]` indexed by an enum's discriminant surrogate (`variant as usize`), and its `enum_map! { Variant => … }` constructor makes an omitted variant a **compile error** (exhaustiveness), which a positional array cannot. `EnumMap` needs `K: enum_map::Enum` (a derive); `BitFlags` needs `#[bitflags] #[repr(uN)]` on the enum (see [Enum repr](#enum-repr)). In-tree precedents: `gp-render`'s `IconSet` (`EnumMap<Icon, TextureHandle>`) and `gp-core`'s `legal_mask` (`gp_core::sim::Actions`, the alias for `BitFlags<Action>` — see [Re-export a generic instantiation as a type alias](#re-export-a-generic-instantiation-as-a-type-alias)).

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

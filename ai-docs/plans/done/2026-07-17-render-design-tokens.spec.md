# gp-render: design tokens → Rust consts (colors, spacing, type, effects)

**Source:** issue #12
**Date:** 2026-07-17
**Tracked in:** #12

Block 2 — `gp-render`, build-order 9/40. Downstream of #11
(`2026-07-16-render-backend-decision`, ✅ implemented), which picked **eframe/egui
0.35** and made `gp-render` a **draw-only** library. Upstream of the component
units #13–#16, which style egui widgets from these consts.

> **Size.** The issue is labelled **S**. The product owner has accepted that #12
> grows past S by vendoring the font faces (round-1 answer, *Names + faces*).
> **Do not re-litigate the size.**

## Scope

Port the design-system CSS tokens in [`docs/design-system/tokens/`](../../docs/design-system/tokens/)
to module-level `SCREAMING_SNAKE_CASE` Rust consts in `gp-render`, and vendor the
two font faces the design system names.

### A. The token module

1. **A new token module in `gp-render`** holding every portable token from
   `colors.css`, `spacing.css`, `typography.css`, `effects.css`. Module path and
   internal grouping are the design phase's call.
2. **Colors → `egui::Color32`.** `colors.css` is the palette source of truth
   (56 tokens: 38 base values + 18 `var()` aliases).
3. **Spacing / sizing → `f32` logical points** (30 tokens): the 4px `--space-*`
   scale, the `--cell*` graph-paper pitch, `--radius-*`, `--bw-*` border widths,
   `--control-h-*` / `--tap-min`, `--content-max` / `--panel-max`.
4. **Typography** (26 tokens): the `--fs-*` px scale, `--fw-*` weights, `--lh-*`
   line heights, `--ls-*` letter spacing, and the three `--font-*` family names.
5. **Effects** (15 tokens): the `--shadow-*` ramp, `--ease-*` curves, `--dur-*`
   durations, the `--bg-grid` / `--bg-dots` graph-paper background parameters,
   and `--focus-shadow`.
6. **Ramps as ordered arrays**: the 6 car chalk hues (index → hue, car 1 = accent
   vermilion) and the 4-stop speed heatmap (slow → fast).
7. **Semantic aliases** (surface / text / border / focus) defined as references to
   their base consts, so the alias relationship is compile-checked rather than
   re-typed.

**Token inventory (verified 2026-07-17** by `grep -cE '^\s*--[a-z0-9-]+:'` over the
four files**):** 56 + 30 + 26 + 15 = **127 tokens**. This count is the AC1 denominator.

### B. Fonts — vendored faces + a `FontDefinitions` builder

Round-1 answer: *Names + faces*. Every fact below is verified live (see
*Technical constraints → Fonts*); none is carried over from the issue text.

8. **Vendor two variable font files** plus their licence texts into `gp-render`:
   `SpaceGrotesk[wght].ttf` and `JetBrainsMono[wght].ttf`, each beside its `OFL.txt`.
   `gp-render`'s `Cargo.toml` declares `license = "(MIT OR Apache-2.0) AND OFL-1.1"`
   to match (AC15) — see *Key decisions → Font licence declaration*. No top-level
   licence aggregation file.
9. **Expose a `FontDefinitions` builder from `gp-render`.** It registers seven
   named weight instances from the two files and returns an `egui::FontDefinitions`.
10. **`gp-game` calls `Context::set_fonts`** with that value, in the existing
    `eframe::run_native` creation closure in `crates/game/src/main.rs`.
    **`gp-render` never holds an `egui::Context`** — the draw-only constraint from
    #11 is preserved; `gp-render` only *produces* the value, `gp-game` *applies* it.

### C. `placeholder.rs` migration

Round-1 answer: *Colors + geometry*.

11. Repoint **all six** colour consts in `crates/render/src/placeholder.rs`
    (`PAPER`, `CARD_FILL`, `CARD_STROKE`, `HAIRLINE`, `GRID_LINE`, `GRID_DOT`) at
    the token module.
12. Repoint the **two** geometry consts that match tokens exactly:
    `GRID_SPACING` (16.0 = `--cell-sm`) and `HAIRLINE_STROKE_WIDTH` (1.0 = `--bw-hair`).
13. **`CARD_CORNER_RADIUS = 4` stays scaffold-local** — it matches no token (the
    radius ramp is 0/3/6/10).
14. The golden PNG stays **byte-identical** (AC12) — the migration is pixel-neutral.

### D. Tests

15. Unit tests per AC6–AC10 and AC12 below.

## Out of scope

- **`fonts.css`'s `@import` line.** It is a Google-Fonts URL — a web loading
  mechanism with no Rust equivalent. The *faces it names* are vendored (Scope B);
  the `@import` statement itself ports to nothing.
- **Italic faces.** `typography.css` requests no italic; `JetBrainsMono-Italic[wght].ttf`
  exists upstream and is deliberately **not** vendored.
- **A dark palette.** `colors.css` defines exactly one `:root` block. See
  *Key decisions → Theming*.
- **`egui::Style` / `Visuals` wiring.** #12 emits values; #13–#16 consume them.
- **Applying fonts to text.** #12 registers the faces and hands `gp-game` the
  install call. No widget in this repo renders text yet — the first *use* of a
  weight instance lands with #13–#16.
- **Component widgets** (#13–#16), **screens** (#19–#22), and **`render_frame`**,
  which stays `todo!()` until `gp-gen` can produce a `TrackArtifact`.
- **`docs/design-system/**` edits.** The import is a read-only visual reference
  ([`IMPORT.md`](../../docs/design-system/IMPORT.md)).

## Deferred

| What | Why | Separate issue needed? |
|---|---|---|
| `--text-eyebrow-transform: uppercase` | A CSS text-transform (a rendering behaviour), not a value token. Belongs to whichever component draws an eyebrow label. | No — folded into #13–#16 |
| Tolerance/AA revisit for the golden | Inherited from #11's deferral; triggers on a dx12/metal CI lane. | No — already tracked by #11 |

## Key decisions

| Question | Decision |
|---|---|
| Color type | **`egui::Color32`**. `Color32::from_rgb` is `const` (`ecolor-0.35.0/src/color32.rs:108`) and `gp-render` already depends on `egui`. Matches the existing scaffold consts. |
| Alpha'd colors (shadows, `--focus-shadow`) | **`Color32::from_rgba_unmultiplied_const`** (`color32.rs:164`) — the plain `from_rgba_unmultiplied` (`:133`) is **not** `const` (it builds a `OnceLock` LUT). |
| Spacing / typography unit | **`f32` logical points**, 1 CSS px = 1 point. `--bw-1: 1.5px` rules out an integer type. |
| Theming | **Single palette, no light/dark switching.** The design system ships one `:root` palette and no dark variant; `gp-render` draws with explicit token colors and holds no `Context`/`Visuals` to theme. Inventing a dark ramp is design work the source does not authorise. |
| Non-portable tokens | **Documented exclusion table over silent omission.** **Ten** tokens have no faithful Rust/egui counterpart (table below, itemised there). Each is ported with a documented deviation or listed with a rationale — AC1 counts both dispositions. |
| `--radius-pill` | **Ports exactly as `f32` 999.0 — not a deviation, not an exclusion.** `epaint::CornerRadius`'s fields are `u8`, but `impl From<f32>` (`corner_radius.rs:41`) is `Self::same(radius.round() as u8)` and Rust's `f32`→`u8` `as` cast saturates, so 999.0 → 255 happens automatically at conversion time — a fully-rounded corner, which is what "pill" means. AC1 disposition (a). See *Technical constraints* for the probe. |
| Ramp indexing | CSS is 1-based (`--car-1`..`--car-6`); Rust arrays are 0-based. **Expose 0-based arrays and document the offset** (`CAR_COLORS[0]` = car 1 = accent). A total, non-panicking accessor is the AGENTS.md § *API Naming* default; the design phase picks its exact shape. |
| Crate placement | **`gp-render` owns the module and the faces.** Consumers are `gp-render` (draw) and `gp-game` (window/`Context`/fonts) — **2 crates**, below the ≥3 threshold that would make shared-crate placement a design question. |
| **Font delivery: variable, not static** | **Vendor the two `[wght]` variable files and register seven weight instances from them** — *not* seven static face files. `FontTweak::coords` (`epaint-0.35.0/src/text/fonts.rs:256`) overrides variation axes per registration, and `FontData::from_static` borrows (`Cow::Borrowed`, `:131`), so all instances of a family **share one byte array**. Two files (≈316 KiB) replace seven. |
| Which weights | **Exactly what `fonts.css` requests**: Space Grotesk 400/500/600/700; JetBrains Mono 400/500/**700** (no 600). The mono axis would happily produce 600, but `fonts.css` is the source of truth for what the design system asks for; symmetry is not a reason to invent a weight. |
| Vendoring source | **The `google/fonts` `ofl/` copies** — they are what `fonts.css`'s `@import` actually serves, so they are the faithful match to the design system's intent. Pin by upstream commit + record a SHA-256 per file. |
| **Font licence declaration** | **Follow the `epaint_default_fonts` precedent fully** (product owner's call, round 3): (1) a per-face `OFL.txt` beside each vendored face, **and** (2) `gp-render`'s `Cargo.toml` declares `license = "(MIT OR Apache-2.0) AND OFL-1.1"`, **and** (3) **no** top-level `NOTICE` / `THIRD-PARTY-LICENSES` aggregation. The precedent does both (1) and (2) — verified at `epaint_default_fonts-0.35.0/Cargo.toml:46`, which declares `license = "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0"` (the `Ubuntu-font-1.0` term covers its Ubuntu-Light face; `gp-render` vendors only OFL faces, so its string omits that term). The field is **metadata only** — per AGENTS.md § *API Stability* this workspace is a game app, never published to crates.io. |
| Font family/face mapping | `--font-display` and `--font-ui` are the **same** stack (`'Space Grotesk', …`), so three CSS family tokens map to **two** faces. |
| Builder base | **Start from `FontDefinitions::default()`, not `::empty()`** — forced by `set_fonts` overwrite semantics (see *Technical constraints*). |
| `placeholder.rs` | **Colors + the two matching geometry consts** migrate; `CARD_CORNER_RADIUS` stays local; golden byte-identical (AC12). |

## Technical constraints

Every egui/epaint fact below was verified against the vendored 0.35.0 sources on
2026-07-17, and every font fact against its live upstream — none is recalled.

### The ten tokens that cannot round-trip faithfully

**The count, itemised** (keep this breakdown in sync with the table — an unitemised
count carried the wrong number through two rounds of review before anyone recomputed
it): `--shadow-inset` (1) + the three `--font-*` stacks (3)
+ the three `--ease-*` curves (3) + `--bg-grid`/`--bg-dots` (2) +
`--text-eyebrow-transform` (1) = **10**. `--shadow-0` appears in the table for
completeness but ports cleanly, so it is **not** one of the ten.

| Token | Constraint | Disposition |
|---|---|---|
| `--shadow-inset: inset 0 1px 2px rgba(…)` | `epaint::Shadow` (`shadow.rs:10–27`) has `offset: [i8;2]`, `blur: u8`, `spread: u8`, `color` — and **no inset flag**. egui has no inner-shadow primitive. | Design phase's call: port the numeric parameters under a distinct type/name, or exclude with rationale. |
| `--font-display`, `--font-ui`, `--font-mono` | CSS font *stacks* whose fallbacks (`ui-sans-serif`, `system-ui`, `ui-monospace`, `SFMono-Regular`, `Menlo`) are browser concepts. | Port the **primary family name only**. egui supplies the fallback role structurally, via `FontDefinitions`' per-family fallback list. |
| `--ease-standard`, `--ease-out`, `--ease-in` | `cubic-bezier(a,b,c,d)` — no egui type. | Port as the four control points (e.g. `[f32; 4]`). |
| `--bg-grid`, `--bg-dots` | CSS `linear-gradient` / `radial-gradient` recipes, not values. | Decompose into the numeric parameters they encode (1px ruling width; 1.2px dot radius with a 1.4px transparent stop) and pair with `--cell` + `--grid-line` / `--grid-dot`. |
| `--text-eyebrow-transform: uppercase` | A CSS text-*transform* — a rendering behaviour applied to a string, not a value a const can hold. | **Excluded**, with this rationale as its AC1 disposition (b). Belongs to whichever component draws an eyebrow label — see *Deferred*, folded into #13–#16. Listed here so AC1's "no token is silently absent" is literally true. |
| `--shadow-0: none` | — *(listed for completeness; not one of the ten)* | Ports cleanly to `Shadow::NONE` (`shadow.rs:40`) — AC1 disposition (a). |

**`--radius-pill: 999px` is NOT in this table — it ports exactly.** This was
mis-filed as unportable on the premise that `epaint::CornerRadius`'s four `u8`
fields (`corner_radius.rs:13–25`) cannot hold 999. The fields are indeed `u8`, but
that does not make the *token* unportable: `RADIUS_PILL: f32 = 999.0` matches the
CSS exactly, and `impl From<f32> for CornerRadius` (`corner_radius.rs:41`) is
`Self::same(radius.round() as u8)` — Rust's float→int `as` cast **saturates**
(guaranteed since Rust 1.45), so the clamp to 255 happens automatically at
conversion time. Verified by compiled probe on 2026-07-17:
`999.0f32.round() as u8 == 255`. A fully-rounded corner is precisely what "pill"
denotes, so nothing is lost. **Do not re-derive the `u8` objection and revert this
to a hardcoded 255** — the saturation is what makes the exact port safe.

**Alpha round-trip is inexact.** `from_rgba_unmultiplied_const` premultiplies; its
own rustdoc warns that `to_srgba_unmultiplied` "might be slightly different
(rounding errors)" for transparent colors. AC6's exact-round-trip assertion
therefore covers **opaque** color tokens and numeric tokens only; alpha'd shadow
and focus tokens are asserted on their stored (premultiplied) representation. The
issue's own exemplars (`accent = #E24A2B`, `cell = 24`, `bw-heavy = 3px`) are all
opaque or numeric, so this narrows nothing the issue asked for.

### Fonts

**epaint 0.35 is on the `fontations` stack, not `ab_glyph`.** Verified via
`cargo tree -p gp-render --edges no-dev`: `epaint` pulls **`skrifa` 0.42.1**,
**`harfrust` 0.7.0**, `read-fonts` 0.39.2, `font-types` 0.11.3, and rasterizes
through **`vello_cpu` 0.0.9**. This is what makes the variable-font route viable,
and it is exactly the "backend font surprise" #11 predicted would surface at #12 —
it landed in our favour.

**Verified font facts:**

| | Space Grotesk | JetBrains Mono |
|---|---|---|
| Role | `--font-display`, `--font-ui` | `--font-mono` |
| Upstream | `floriankarsten/space-grotesk` | `JetBrains/JetBrainsMono` |
| Licence | **OFL-1.1**, `OFL.txt` 4,495 B | **OFL-1.1**, `OFL.txt` 4,399 B |
| Copyright | "Copyright 2020 The Space Grotesk Project Authors" | "Copyright 2020 The JetBrains Mono Project Authors" |
| Reserved Font Name | **None declared** | **None declared** |
| Variable file | `SpaceGrotesk[wght].ttf`, **136,676 B** | `JetBrainsMono[wght].ttf`, **187,208 B** |
| `wght` axis range | **300–700** | **100–800** |
| Weights needed | 400/500/600/700 — all in range | 400/500/700 — all in range |

Licence and axis data read from `google/fonts` `ofl/spacegrotesk/METADATA.pb` and
`ofl/jetbrainsmono/METADATA.pb`; RFN status from each project's own `OFL.txt`.
**No Reserved Font Name is declared by either**, so the faces may be vendored
unrenamed. OFL-1.1 requires the copyright notice and licence to travel with the
font — satisfied by shipping `OFL.txt` beside each face.

Total vendored payload: **≈316 KiB** of font + **≈8.7 KiB** of licence text. These
are the repo's **second and third binary assets** (#11's golden PNG was the first);
plain git, no LFS, per #11's precedent.

**In-tree precedent — `epaint_default_fonts` 0.35.0** vendors exactly this way:
`fonts/Hack-Regular.ttf` + `fonts/Hack-Regular.txt`, `fonts/Ubuntu-Light.ttf` +
`fonts/UFL.txt`, `NotoEmoji-Regular.ttf` + `OFL.txt`, each exposed as
`pub const NAME: &[u8] = include_bytes!("../fonts/…")`. Its bundled faces are
**Hack, Ubuntu-Light, NotoEmoji, emoji-icon-font** — confirming that neither
Space Grotesk nor JetBrains Mono ships with egui.

**The verified egui font API** (a design phase guessing these will not compile):

| Item | Path / signature | Source |
|---|---|---|
| `FontData` | `{ font: Cow<'static,[u8]>, index: u32, tweak: FontTweak }` | `fonts.rs:118` |
| Construct from `include_bytes!` | `FontData::from_static(&'static [u8])` → `Cow::Borrowed` | `fonts.rs:131` |
| Attach a tweak | `FontData::tweak(self, FontTweak) -> Self` | `fonts.rs:147` |
| Variation override | `FontTweak::coords: VariationCoords` | `fonts.rs:256` |
| Build coords | `VariationCoords::new([(b"wght", 500.0)])` | `text_layout_types.rs:425` |
| `FontDefinitions` | `{ font_data: BTreeMap<String, Arc<FontData>>, families: BTreeMap<FontFamily, Vec<String>> }` | `fonts.rs:437` |
| `FontFamily` | `Proportional \| Monospace \| Name(Arc<str>)` | `fonts.rs:80` |
| Axis discovery | `FontData::variation_axes() -> Vec<FontVariationAxis>` | `fonts.rs:159` |
| Install (gp-game) | `Context::set_fonts(&self, FontDefinitions)` | `egui/src/context.rs:2038` |
| Install site | `eframe::CreationContext::egui_ctx: egui::Context` | `eframe/src/epi.rs:53–58` |

**Import paths differ — this is a real trap.** `egui` re-exports
`{FontData, FontDefinitions, FontFamily, FontId, FontTweak}` at its top level
(`egui/src/lib.rs:450`), but **`VariationCoords` is not among them**. It reaches
`gp-render` as **`egui::epaint::text::VariationCoords`** (`egui/src/lib.rs:436`
`pub use epaint;` → `epaint/src/lib.rs:42` `pub mod text;` →
`epaint/src/text/mod.rs:18` `pub use text_layout_types::*;` →
`text_layout_types.rs:411` `pub struct VariationCoords`). No new crate dependency
is involved — `egui` already carries all of it.

**`set_fonts` overwrites.** Its rustdoc states: *"This will overwrite the existing
fonts."* `FontDefinitions::default()` populates egui's bundled faces when the
`default_fonts` feature is on — and it **is** on (`cargo tree -p gp-render
--edges no-dev -f '{p} {f}'` shows `egui v0.35.0 default,default_fonts`), whereas
`::empty()` yields empty families (`fonts.rs:567`). The builder must therefore
**start from `default()` and add to it**, or the emoji/fallback coverage is
silently dropped.

**`VariationCoords::new` is not `const`** (it collects into a `SmallVec`). The
builder is therefore a **function**, not a const. This does not weaken AC2: the
weights it feeds (`--fw-*` = 400/500/600/700) and the family names are the consts;
the builder consumes them.

**`gp-render` must stay draw-only.** #11's AC7 is enforceable and load-bearing:
`cargo tree -p gp-render --edges no-dev` must show no `eframe`/`winit`/`wgpu`
normal edge. A token module and an `include_bytes!`-backed `FontDefinitions`
builder are pure data and cannot breach it — AC13 re-asserts this.

**Lint posture.** `missing_docs = "deny"` and `clippy::pedantic`/`nursery = deny`
are workspace-wide — every `pub const` needs a `///`.

### `placeholder.rs` colour equality

Verified by reading both files (2026-07-17). All six private scaffold colour
consts already equal their tokens exactly:

| `placeholder.rs` | Value | Token |
|---|---|---|
| `PAPER` | `#F5F1E6` | `--paper-1` |
| `CARD_FILL` | `#ECE6D6` | `--paper-2` |
| `CARD_STROKE` | `#201E1A` | `--graphite-900` |
| `HAIRLINE` | `#C4BBAA` | `--graphite-300` |
| `GRID_LINE` | `#C3CEDD` | `--grid-line` |
| `GRID_DOT` | `#93A2B8` | `--grid-dot` |

Plus `GRID_SPACING = 16.0` = `--cell-sm` and `HAIRLINE_STROKE_WIDTH = 1.0` =
`--bw-hair`. `CARD_CORNER_RADIUS = 4` matches no token (ramp is 0/3/6/10).

Because every migrated value is an exact match, the migration is **pixel-neutral**:
the golden at `crates/render/tests/snapshots/placeholder.png` is unaffected, so no
regen and therefore **no `image-check` spawn** is triggered. Vendoring fonts does
not disturb it either — the placeholder draws **no text**, and the golden harness
never calls the builder. That file's own module doc says: *"Colors below are
scaffold-local `Color32` consts, not the design-token module — #12 owns 'design
tokens → Rust consts' and supersedes these."*

> **Amendment 1 (2026-07-17, #73 `render-onest-font-swap`) — AC9 and AC10 only.**
> #73 swapped the vendored display/UI face **Space Grotesk → Onest** and turned
> egui's `default_fonts` feature **off**, which falsified two clauses as merged
> here. AC9 named `SpaceGrotesk[wght].ttf` and pinned its axis cover as *"SG
> 400–700 within 300–700"*; the vendored face is now `Onest[wght].ttf`, whose
> `fvar` `wght` axis is min=100 / default=400 / max=900. AC10 required the builder
> be *"built on `FontDefinitions::default()`, so egui's bundled fallback faces
> survive"*, and its test to assert *"that egui's default fallback entries are
> still present"*; with `default_fonts` off, those faces are no longer in the
> dependency graph, so there are none to preserve. That clause does not merely
> become wrong — it becomes **unfalsifiable**: `builtin_font_names()` returns `&[]`
> under `#[cfg(not(feature = "default_fonts"))]`
> (`epaint-0.35.0/src/text/fonts.rs:590-593`), so the old assertion's loop
> **iterates zero times and still passes, asserting nothing**. A vacuous-but-green
> test is precisely why AC10 is **replaced** rather than deleted — the
> exact-family-list equality it now mandates cannot go vacuous. See #73's spec
> § *Technical constraints* 1–3
> (`ai-docs/plans/2026-07-17-render-onest-font-swap.spec.md`). **Every other
> criterion stands as merged** — only AC9 and AC10 are amended.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Each of the **127** tokens across `colors.css` (56) / `spacing.css` (30) / `typography.css` (26) / `effects.css` (15) is either (a) a module-level const whose value matches the CSS, or (b) an entry in the module's documented exclusion table with a rationale. No token is silently absent. |
| AC2 | Every token const is **module-level** and `SCREAMING_SNAKE_CASE`; none is an inline literal. |
| AC3 | The car ramp is an ordered 6-element `Color32` array with `[0]` = car 1 = the accent (`#E24A2B`); the 0-based/1-based offset is documented. Its accessor is total (no panic on out-of-range index). |
| AC4 | The heatmap is an ordered 4-element `Color32` ramp, slow → fast: `#2E6FB5`, `#17999B`, `#E8B23A`, `#E24A2B`. |
| AC5 | All 18 `colors.css` semantic aliases (`--surface-*`, `--text-*`, `--border-*`, `--focus-ring`) exist and are **defined as references to their base consts**, not re-typed literals. |
| AC6 | Tests assert exact round-trip for the issue's exemplars — accent = `#E24A2B`, `--cell` = 24, `--bw-heavy` = 3 — plus at least one alias identity (e.g. `SURFACE_PAGE == PAPER_1`) and one cross-file identity (`CAR_COLORS[0] == ACCENT == HEAT_RAMP[3]`). |
| AC7 | Tests assert both ramps' **length and ordering** (car = 6, heatmap = 4, heatmap ordered slow → fast). |
| AC8 | A test asserts the AC1 count, so a token added to the CSS later cannot silently go unported. |
| AC9 | `Onest[wght].ttf` and `JetBrainsMono[wght].ttf` are vendored in `gp-render`, each beside its `OFL.txt`. A test asserts each face's bytes parse and that `FontData::variation_axes()` reports a `wght` axis whose range covers every weight the builder registers (Onest 400–700 within 100–900; JBM 400–700 within 100–800). |
| AC10 | The `FontDefinitions` builder returns a value registering **7** weight instances (Onest 400/500/600/700; JBM 400/500/700) built **explicitly** on `FontDefinitions::empty()` — egui's bundled faces are no longer in the dependency graph (`default_fonts` off), so there are none to preserve. A test asserts the instance count and the **exact** family lists by **full-vector equality**: `FontFamily::Proportional == ["Onest-Regular"]` and `FontFamily::Monospace == ["JetBrainsMono-Regular", "Onest-Regular"]` — not `first()` / non-empty / `contains` checks. `Monospace`'s second entry is a deliberate proportional fallback for glyphs JetBrains Mono lacks (`✓` U+2713); JetBrains Mono stays **first** and the ordering is load-bearing. |
| AC11 | `gp-game` calls `Context::set_fonts` with the builder's value in its `eframe::run_native` creation closure. `gp-render` still constructs no `Context`. |
| AC12 | The golden `crates/render/tests/snapshots/placeholder.png` is **byte-unchanged** — no regen, no `image-check` spawn — with `placeholder.rs`'s six colour consts and two matching geometry consts now sourced from the token module. |
| AC13 | `cargo tree -p gp-render --edges no-dev` still shows no `eframe` / `winit` / `wgpu` normal edge (#11 AC7 holds). |
| AC14 | `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` are all green. |
| AC15 | `crates/render/Cargo.toml` declares `license = "(MIT OR Apache-2.0) AND OFL-1.1"`, and no top-level `NOTICE` / `THIRD-PARTY-LICENSES` file is added. (Metadata only — the workspace is never published; see *Key decisions → Font licence declaration*.) |

## Open questions

**None.** Both questions raised at round 1 are now closed:

1. ~~Repo-level licence notice for the bundled OFL faces.~~ **Resolved** (round 3,
   product owner): *follow the `epaint_default_fonts` precedent fully* — per-face
   `OFL.txt`, a `license` field on `gp-render`, and no top-level aggregation. See
   *Key decisions → Font licence declaration* and AC15. The supporting licence
   facts are unchanged and still hold: both faces are OFL-1.1 with **no Reserved
   Font Name declared**, so they may be vendored unrenamed, and OFL-1.1's bundling
   requirement is satisfied by shipping `OFL.txt` with each face.
2. ~~Exact upstream pin.~~ **Discharged by the design**, which pinned `google/fonts`
   at commit `389b770410cc0b7c21c85673bfa2077420fe7f65` with a SHA-256 **and** a git
   blob hash per vendored file, and re-confirmed all four byte sizes exactly
   (136,676 / 187,208 / 4,495 / 4,399). The design also found that EOL
   normalisation corrupts `OFL.txt` on `git add`, and orders the byte-exactness
   check to run **after** staging; that mitigation lives in the design, not here.

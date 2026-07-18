# Design: gp-render core widgets — Button, IconButton, Badge, Tag, Card

**Issue:** #13
**Date:** 2026-07-18
**Spec:** `ai-docs/plans/2026-07-18-render-core-widgets.spec.md`
**Prerequisite:** #88 (SVG icon pipeline) — **landed** via merged PR #89; API live at
`crates/render/src/icons.rs` `[measured: git log → 6fc43fb "Merge pull request #89 …render-svg-icon-pipeline"; gh issue view 88 → {"state":"CLOSED"}; git ls-files → icons.rs tracked]`.

## Approach

Port the five design-system core components to native `gp-render` widgets under a new
`crates/render/src/widgets/` submodule. Each widget is split into three layers so the
interaction shell and the pure style mapping are independently testable:

1. **Pure style-resolution layer** (AC7) — a `const fn resolve(...)` per widget that maps
   `(variant | tone, size, interaction-state)` → a plain `…Style` struct of `Color32` +
   `f32` metrics + `Option<InsetShadow>`. **No `egui::Ui`, no pointer input, no
   allocation.** This is the Miri-clean, unit-tested core.
2. **Paint layer** — a private `paint(painter, rect, &style, …)` that draws the resolved
   style with `egui::Painter` primitives (the same surface `placeholder.rs` uses) plus
   icon draws via `icons::draw_icon`. Takes the visual state *already resolved*, so the
   gallery can force any state.
3. **Interaction shell** — a public `show(self, ui) -> Response` (egui builder idiom) that
   allocates a rect + `Response`, reads live egui input (`hovered()`,
   `is_pointer_button_down_on()`), computes the effective state, calls `resolve` then
   `paint`, and returns the `Response` (so `onClick` → `Response::clicked()`).

### Key decisions (resolving the spec's flagged questions)

**1. Interaction model — CONFIRM the spec default.** Interactive `egui::Response`-returning
widgets driven by a borrowed `&mut egui::Ui`, not pure `Painter` draws. Grounded: egui 0.35
supplies exactly the needed surface — `Ui::allocate_at_least/allocate_exact_size(desired,
Sense::click())` → `(Rect, Response)`, `Response::{hovered, is_pointer_button_down_on,
clicked, on_hover_text}`
`[measured: grep egui-0.35.0/src/{ui,response,sense}.rs → allocate_exact_size:1150, allocate_at_least:1161, Response::clicked:183, hovered:313, is_pointer_button_down_on:575, on_hover_text:707, Sense::{hover:45,click:60}]`.
Compatible with `gp-game` window ownership: `gp-game` passes the `Ui` down
`[measured: crates/game/src/main.rs:21 App::ui(&mut self, ui: &mut egui::Ui, …)]`. Toggle/
selected/active state is **caller-owned** (`bool` passed in), the widget renders it and
returns a `Response` for click detection (egui `selectable_label` pattern) — applies to
IconButton `active`, Tag `selected`, Card `selected`.

**2. Module split (soft 500/800 limit + no one-struct-per-file over-split).** A
`widgets/` directory, **one file per widget** (each holding its props builder + `…Style` +
`resolve` + `paint` + `show` + `#[cfg(test)] mod tests`), plus one shared `widgets/common.rs`
and a `widgets/mod.rs`. This mirrors egui's own `widgets/{button,label,…}.rs` layout
`[measured: egui-0.35.0/src/widgets/mod.rs defines the module tree]`. Each widget file is a
cohesive ~180–300-line unit (props + 3 layers + tests), well under the soft 500/800 limit —
**not** one-struct-per-file (each file carries a props struct, a style struct, three fns, and
a test module). `[derived → cargo clippy + file-size review]`

**3. Pure style-resolution layer (AC7) is the testability lever.** `resolve` takes plain
enums/bools and returns a plain `Copy` struct — zero `egui::Ui` — so its tests need **no**
context, no window, no pointer, and are Miri-clean. It performs **no arithmetic** (pure
selection via `match`/`if`) and references only existing token consts, so it is
**const-eligible**; workspace `clippy::missing_const_for_fn` (nursery = deny) therefore
**FORCES `const fn`** on every `resolve` (live precedent: `Size::area` `geom/mod.rs:117`).
Radii are returned as raw `f32` (the token value) and saturated via `CornerRadius::from(f32)`
at the **paint** use site — never inside `resolve` — because `From::from` is not const-stable
on stable rustc (same class as `car_color`'s documented non-const status,
`color.rs:165`). `[measured: Cargo.toml [workspace.lints.clippy] nursery=deny priority -1; epaint-0.35.0/src/corner_radius.rs:41 impl From<f32> for CornerRadius; crates/render/src/tokens/color.rs:161-165 car_color non-const rationale (E0658, <[T]>::get not const-stable)]` `[derived → cargo clippy --workspace --all-targets -- -D warnings]`

**4. Icon slots — resolved concretely against #88's landed API.** A slot
(Button `icon_left`/`icon_right`, IconButton glyph) types as **`Option<&egui::TextureHandle>`**,
i.e. a **pre-baked handle** the caller obtains from an `IconSet` it owns
(`icon_set.get(icons::Icon::Play)`), **not** an `icons::Icon` the widget resolves internally.
Rationale: `gp-render`'s own draw helper already takes a `&TextureHandle`
(`icons::draw_icon(painter, handle, rect, tint)` `[measured: icons.rs:224]`), so a
`&TextureHandle` slot keeps the widget icon-source-agnostic and avoids threading a
`&IconSet` through every builder. It satisfies AC6 ("icon-slot props type against #88's icon
handle") since `&TextureHandle` **is** #88's baked handle. **Icon size ↔ button size:** the
curated set bakes at **one** logical size, `icons::ICON_LOGICAL_SIZE_PX = 18.0`
`[measured: icons.rs:23]`, matching the design system's own fixed
`[data-lucide]{width:18px;height:18px}` `[measured: core.card.html:10]`. The widget reserves
an `ICON_LOGICAL_SIZE_PX`-square rect (vertically centered, `gap` before/after per size) and
draws the texture 1:1 via `icons::draw_icon` tinted with the resolved `fg` — no scaling, and
the future `(Icon, size)` cache flagged additive in `icons.rs:167` stays **out of scope**.

**5. Non-token source colors — placement (flagging the 2-site count).**
- **Ghost hover/press overlays** `rgba(32,30,26, 0.06/0.12)` appear at **2 sites in 2 files**
  (Button ghost + IconButton ghost) `[measured: Button.jsx:45-48, IconButton.jsx:27,36]`.
  → Lift to **two module consts in `widgets/common.rs`** shared by both files:
  `GHOST_HOVER_OVERLAY` / `GHOST_PRESS_OVERLAY` = `Color32::from_rgba_unmultiplied_const(0x20,
  0x1E, 0x1A, a)` with `a = round(0.06·255)=15` and `round(0.12·255)=31` (the RGB equals
  `GRAPHITE_900`'s channels). Intra-crate 2-site duplication → a shared *module const* is the
  right home (the ≥3-**crate** shared-crate rule does not apply — this is one crate, two
  modules). `[measured: ecolor color32.rs:164 from_rgba_unmultiplied_const is const]`
  `[derived → common.rs unit test pins a=15,31]`
- **Badge `ok`/`warn` tinted foregrounds** `#1E6B3C` / `#8A6410` `[measured: Badge.jsx:11-12]`
  are used at **1 site each**, only in Badge → **two local module consts in `badge.rs`**
  (`BADGE_OK_FG = Color32::from_rgb(0x1E,0x6B,0x3C)`, `BADGE_WARN_FG =
  Color32::from_rgb(0x8A,0x64,0x10)`). Named consts regardless of site count per the
  magic-number rule (AGENTS.md § Code Style).

**6. Specimen/gallery (AC8) — a Miri-ignored golden, not an `examples/` binary.** Render the
full variant/size/state matrix to a single wgpu snapshot via the **already-present**
`egui_kittest` + `egui-wgpu` dev-deps `[measured: crates/render/Cargo.toml [dev-dependencies]
egui_kittest{wgpu,snapshot}, egui-wgpu]`, mirroring `placeholder.rs`'s `golden_guard`. Chosen
over a shippable `examples/` binary because that would add **`eframe`** as a new gp-render
dev-dependency solely to open a window `[measured: grep -r eframe crates/render/Cargo.toml → none; only gp-game depends on eframe]` and could not run headless in CI. The gallery lives as an
**in-crate `#[cfg(test)]` module** so it can drive the crate-private `paint` layer with
**forced** visual states (rest/hover/press/active/disabled) in one static frame — no input
simulation. It is `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan ICD")]`
(a red workspace Miri blocks merge), uses `SnapshotOptions::new().threshold(0.0)
.failed_pixel_count_threshold(0)` (both required for exact compare — the 0.6 default is a
trap), and asserts the resolved wgpu adapter is a CPU/software device, exactly as
`golden_guard` does `[measured: placeholder.rs:384-520; golden-tests memory note]`.
Interactive hover/press verification is deferred to `gp-game` integration (out of scope).

### Per-widget prop surface (AC6: `.d.ts` → Rust)

Common mappings for all: `children` → the widget's label/body; `onClick` → `Response::clicked()`
(no stored closure); `style?: CSSProperties` → **dropped**; React `type`/`aria-*` → dropped or
mapped as noted. `[measured: all five .d.ts read in full]`

- **Button** (`Button.d.ts`): `variant: primary|secondary|ghost|danger`, `size: sm|md|lg`,
  `disabled` → `enabled: bool`, `iconLeft`/`iconRight` → `Option<&TextureHandle>`, `fullWidth`,
  `children` → `label: &str`; `type`/`onClick`/`style` dropped/mapped.
- **IconButton** (`IconButton.d.ts`): glyph `children` → `icon: &TextureHandle` (required),
  `label: &str` → `Response::on_hover_text` (tooltip + a11y), `variant: secondary|ghost`,
  `size: sm|md|lg`, `active`, `disabled` → `enabled`.
- **Badge** (`Badge.d.ts`): `tone: neutral|accent|ok|warn|danger`, `solid: bool`,
  `children` → `label: &str`. Non-interactive; `show` allocates the pill + returns a
  `Response` (`Sense::hover()`) for layout uniformity.
- **Tag** (`Tag.d.ts`): `color?: string|null` → `Option<Color32>` (the leading dot),
  `onRemove` → a `bool show_remove` flag; `show` returns a small `TagResponse { response,
  remove_clicked }` so the caller learns which affordance was clicked. `selected: bool`,
  `children` → `label: &str`.
- **Card** (`Card.d.ts`): `title`/`eyebrow` → `Option<&str>`, `right` → an optional
  header-right closure `impl FnOnce(&mut Ui)` (egui container idiom), `grid: bool`,
  `selected: bool`, `elevation: 0|1|2|3` → an `Elevation` enum (default 1), `padding?: string`
  → `f32` (default `spacing::SPACE_5`), body `children` → `add_contents: impl FnOnce(&mut Ui)`;
  `onClick` → `Response::clicked()` on the whole-card `Sense::click()` response.

### Style-mapping ground truth (from the `.jsx`, cross-checked to tokens)

All colors/metrics below already exist as `crate::tokens` consts unless flagged in decision 5.
`[measured: the five .jsx files + tokens/{color,spacing,typography,effects}.rs]`

- **Button** bg = `pressed ? bgActive : (hovered ? bgHover : bgRest)`; border `bw-1` solid;
  radius-2; `pressed` (enabled) → `SHADOW_INSET` + 1-pt downward content nudge; `disabled` →
  all resolved colors `gamma_multiply(DISABLED_OPACITY=0.45)`, `Sense::hover()` (no click).
  Per-variant table = spec's Button table (primary/secondary/ghost/danger). Sizes: sm
  `CONTROL_H_SM`/pad-x 12/`FS_SM`/gap 6; md `CONTROL_H_MD`/16/`FS_BODY`/gap 8; lg
  `CONTROL_H_LG`/22/`FS_TITLE`/gap 10. Font = Onest SemiBold (`Name(ONEST_SEMIBOLD)`).
- **IconButton** square dim = sm 30/md 38/lg 46; bg/fg/border/pressBg per `IconButton.jsx`
  (active→`GRAPHITE_900` fill + `PAPER_0` fg + `GRAPHITE_900` border; ghost→transparent +
  `GHOST_*_OVERLAY`; secondary→`PAPER_0/2/3` + `BORDER_STRONG`); radius-2; press → `SHADOW_INSET`.
- **Badge** height 20, pad-x 8, `FS_XS`, `FW_MEDIUM`, `LS_MONO`, radius-pill; solid → fg
  `PAPER_0` + `solidBg` + transparent border; tinted → per-tone `bg`/`fg`/`bd` (ok/warn fg =
  the two local consts). Font = JetBrains Mono Medium (`Name(JETBRAINS_MONO_MEDIUM)`).
- **Tag** height 26, pad-x 10, `FS_SM`, mono, `TEXT_INK`, radius-0; rest `PAPER_0` + `bw-hair`
  `BORDER_HAIRLINE`; selected `PAPER_2` + `bw-1` `BORDER_STRONG`. Dot 10×10 circle + `bw-1`
  `GRAPHITE_900` ring. Remove ×: 16×16, hover bg `PAPER_3`, `TEXT_MUTED`, radius-1.
- **Card** `SURFACE_CARD` fill; border `bw-hair` `BORDER_HAIRLINE`, selected → `bw-2`
  `BORDER_STRONG`; radius-2; elevation 0/1/2/3 → `SHADOW_0/1/2/3` painted via
  `epaint::Shadow::as_shape(rect, radius)` `[measured: epaint-0.35.0/src/shadow.rs:48]`.
  Eyebrow: mono `FS_XS` uppercase `LS_LABEL` `TEXT_MUTED`. Title: Onest `FS_TITLE` SemiBold
  `TEXT_INK` `LH_SNUG`. Grid watermark: `effects::{BG_GRID_RULING_WIDTH, BG_GRID_COLOR,
  BG_DOTS_RADIUS, BG_DOTS_COLOR}` at pitch `spacing::CELL`, clipped to the card rect, each
  color `gamma_multiply(GRID_WATERMARK_OPACITY=0.5)` (the `.jsx`'s `opacity:0.5`). The ruling
  + dot draw reuses `placeholder.rs`'s `draw_grid` shape (a precedent, not shared code).

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Scaffold: `pub mod widgets` in `lib.rs`; `widgets/mod.rs` (module doc + `mod common`); `widgets/common.rs` with the shared `Size {Sm,Md,Lg}` enum, `GHOST_HOVER_OVERLAY`/`GHOST_PRESS_OVERLAY`/`DISABLED_OPACITY`/`GRID_WATERMARK_OPACITY` consts, and a shared `paint_surface` helper (rounded-rect fill + border via `CornerRadius::from`, optional `SHADOW_INSET` inner band, `gamma_multiply` opacity), + unit tests pinning the two overlay alpha values (15/31). | `crates/render/src/lib.rs`, `crates/render/src/widgets/mod.rs`, `crates/render/src/widgets/common.rs` | — |
| 2 | **Badge** (stateless, simplest first): `BadgeProps` builder, `BadgeStyle`, `const fn resolve(tone, solid)`, `BADGE_OK_FG`/`BADGE_WARN_FG` consts, `paint`, `show`; resolve unit tests (tone → token, solid → fg/bg/border). Register `pub mod badge; pub use badge::Badge;` in `mod.rs`. | `crates/render/src/widgets/badge.rs`, `.../widgets/mod.rs` | 1 |
| 3 | **Button**: `ButtonProps`, `ButtonStyle`, `const fn resolve(variant, size, hovered, pressed)`, `paint` (bg/border/radius/inset-shadow+nudge, icon slots via `icons::draw_icon`, label text), `show` (icon slots `Option<&TextureHandle>`, `full_width`, `enabled`); resolve tests (AC7: variant→color, size→height, pressed→`SHADOW_INSET`). Register in `mod.rs`. | `crates/render/src/widgets/button.rs`, `.../widgets/mod.rs` | 1 |
| 4 | **IconButton**: props, `IconButtonStyle`, `const fn resolve(variant, active, hovered, pressed)`, `paint` (square dim, centered 18px icon, inset-shadow), `show` (`icon: &TextureHandle`, `label`→`on_hover_text`, `active`, `enabled`); resolve tests. Register in `mod.rs`. | `crates/render/src/widgets/icon_button.rs`, `.../widgets/mod.rs` | 1 |
| 5 | **Tag**: props, `TagStyle`, `const fn resolve(selected)`, `paint` (square radius-0, dot, label, remove-× hit area), `show` → `TagResponse { response, remove_clicked }`; resolve tests. Register in `mod.rs`. | `crates/render/src/widgets/tag.rs`, `.../widgets/mod.rs` | 1 |
| 6 | **Card**: props (`Elevation` enum, `padding` f32, `grid`, `selected`, title/eyebrow, optional right-slot closure), `CardStyle`, `const fn resolve(selected, elevation)`, `paint` (shadow via `Shadow::as_shape`, fill, border, clipped grid watermark, eyebrow+title header), `show(self, ui, add_contents)`; resolve tests (elevation→shadow, selected→border). Register in `mod.rs`. | `crates/render/src/widgets/card.rs`, `.../widgets/mod.rs` | 1 |
| 7 | **Gallery golden (AC8)**: in-crate `#[cfg(test)]` module (`widgets/gallery.rs`, `#[cfg(test)]`) rendering the full variant/size/state matrix via `egui_kittest`+wgpu with forced states through the private `paint` layer; installs `fonts::definitions()` + an `IconSet`; `#[cfg_attr(miri, ignore)]`; `SnapshotOptions` threshold 0.0 + failed-pixel 0; CPU-adapter assertion; mint the golden PNG. Wire `#[cfg(test)] mod gallery;` in `mod.rs`. | `crates/render/src/widgets/gallery.rs`, `.../widgets/mod.rs`, `crates/render/tests/snapshots/` (minted PNG) | 2, 3, 4, 5, 6 |

M = 7 (≤ 15 — no issue split needed).

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping (a)–(h). This is a **single
change-type** task — every subtask edits **code** (`*.rs`, plus the minted golden PNG which is
a test artifact of a code subtask), so all subtasks route to one implementor model.

- **(a)/(c)/(e)/(f)** — **Group A** (terminal, and the only group) — **code** change-type,
  implementor **`sonnet`** (sonnet-5), effort **`medium` (pinned in `code-writer` frontmatter)**,
  1M-token window, via the `code-writer` subagent — subtasks **1–7**. All seven subtasks are
  the same change-type (code) with a dependency chain that runs cleanly in source order
  (1 → {2,3,4,5,6} → 7), so they cluster into the **fewest possible groups = 1** (no
  interleaving with any other change-type exists to force a boundary).
- **(b)** group size = 7, within the `≤ 10` maximum (no size-cap split needed).
- **(d)** terminal group size = 7, within `1..=10`.
- **(g)** marked: code group → `code-writer`, `sonnet` / effort `medium` (pinned) — no inline
  `model=`/effort override. The `design`/`design-review`/`self-review` Opus gates are unchanged.
- **(h)** 1 group ≤ the default max of 4 — no user gating needed.
- **Handoff into Group A:** at the start of Group A, spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) — the single
  group runs Step 8 in its own `/context-reset` subagent. No inter-group handoff (only one group).

## Risks

- **Pure-`resolve` const-fn requirement misread as optional.** `resolve` is const-eligible;
  nursery `missing_const_for_fn` (deny) FORCES `const fn`. A non-const `resolve` fails the
  gate. Mitigation: each `resolve` is pure selection referencing existing consts, returns raw
  `f32` radius (defers `CornerRadius::from` to paint). — `[measured: Cargo.toml nursery=deny]` `[derived → cargo clippy --workspace --all-targets -- -D warnings]`
- **`arithmetic_side_effects` (deny) trips on `Pos2 + Vec2` layout math.** The paint layer
  positions icons/text/dots. `placeholder.rs` finding 8: the operator-overload `Pos2 + Vec2`
  trips the deny even though raw `f32 +` does not. Mitigation: build all positions field-wise
  as `Pos2::new(a.x + dx, a.y + dy)` from raw-`f32` sums, never via `Pos2`/`Vec2` operators.
  — `[measured: placeholder.rs:120-124 geometry() builds Pos2 from field-wise f32 sums with the finding-8 comment]` `[derived → cargo clippy]`
- **Any test that lays out real text aborts Miri.** Drawing text rasterises glyphs via
  `vello_cpu`'s checked `u8→u32` cast, which panics under Miri's 1-byte alignment (distinct
  from the wgpu/`dlopen` abort). Mitigation: AC7 coverage lives entirely in `resolve` tests
  (no context, no text → Miri-clean); the only text-drawing test is the gallery golden, which
  is `#[cfg_attr(miri, ignore)]` for the wgpu reason anyway. — `[measured: placeholder.rs:299-304 tessellation_smoke miri ignore; icons.rs:336-348 IconSet miri ignore]`
- **Golden requires a CPU/lavapipe wgpu adapter on CI.** Same premise `golden_guard` already
  relies on. Mitigation: assert `device_type == Cpu` with the install-lavapipe hint, and use
  `RendererOptions::PREDICTABLE` + `.renderer(..)` explicitly (the `.renderer` path bypasses
  the default `PREDICTABLE`). — `[measured: placeholder.rs:394-405, 428; golden-tests memory note]`
- **`IconSet::new` bakes `settings.svg`, which panics under Miri.** Only matters under Miri;
  the gallery (its sole widget-side consumer) is Miri-ignored, so never reached under Miri.
  — `[measured: icons.rs:336-348]`
- **Fonts precondition.** Every text draw resolves a `FontFamily::Name(..)`, which epaint
  cannot lay out unless `fonts::definitions()` is installed first — a **caller** precondition
  (the widgets, like `draw_placeholder`, do not install fonts). Mitigation: document it on
  each `show`; the gallery installs fonts itself (frame-1-install / frame-2-draw pattern).
  — `[measured: placeholder.rs:170-178 # Panics; fonts.rs module doc]`
- **File size.** Largest file (card.rs, ~300 lines incl. tests) stays under the soft 500/800
  limit. — `[derived → file-size review + cargo clippy too_many_lines]`

## Test Design

**AC7 — pure `resolve` unit tests (Miri-clean, the backbone).** One `#[cfg(test)] mod tests`
per widget file, calling `resolve` directly (no `egui::Context`).
- Location: each `widgets/<widget>.rs` `#[cfg(test)] mod tests`.
- Entry point: that widget's `resolve` fn.
- Scenarios per the AC7 checklist:
  - Button: `resolve(Primary, Md, hovered=false, pressed=false).bg == color::ACCENT`;
    `…(Primary, _, true, _).bg == ACCENT_HOVER`; `…(_, Lg, …).height == spacing::CONTROL_H_LG`;
    `…(_, _, _, pressed=true).press_shadow == Some(effects::SHADOW_INSET)`; danger
    hover fg flip (`DANGER` → `TEXT_ON_ACCENT`); ghost hover/press bg = `GHOST_*_OVERLAY`.
  - IconButton: `active` → bg/fg/border = `GRAPHITE_900`/`PAPER_0`/`GRAPHITE_900`; ghost vs
    secondary rest/hover/press; size → dim 30/38/46; pressed → `SHADOW_INSET`.
  - Badge: each of 5 tones × {solid, tinted} → expected `bg`/`fg`/`bd`; ok/warn tinted fg =
    `BADGE_OK_FG`/`BADGE_WARN_FG`; solid fg = `PAPER_0`, transparent border.
  - Tag: rest vs selected → bg (`PAPER_0`/`PAPER_2`), border width (`BW_HAIR`/`BW_1`) + color
    (`BORDER_HAIRLINE`/`BORDER_STRONG`); radius-0.
  - Card: elevation 0/1/2/3 → `SHADOW_0/1/2/3`; selected → `BW_2` + `BORDER_STRONG` else
    `BW_HAIR` + `BORDER_HAIRLINE`; radius-2.
- Fixtures: none — plain enum/bool inputs, `Color32`/`f32` equality. Integer `Color32`-field
  asserts stay naked `assert_eq!` (no `float_cmp` fires on `u8` fields). Fractional-`f32`
  metric asserts (e.g. Tag border `BW_1 = 1.5`, and `LH_SNUG = 1.2`, `LS_LABEL = 0.06`,
  `LS_MONO = 0.02` if asserted) MUST go through `crate::tokens::css::assert_f32(label, got,
  want)` — the crate's single `#[allow(clippy::float_cmp)]` site — because a naked
  `assert_eq!(metric, FRACTIONAL_CONST)` trips `clippy::float_cmp` under the crate's
  deny-pedantic config and reds AC9. It is reachable from every `widgets/<w>.rs`
  `#[cfg(test)] mod tests`.
  `[measured: crates/render/src/tokens/mod.rs:58-60 #[cfg(test)] pub(crate) mod css; assert_f32 at :105 is the crate's sole clippy::float_cmp allow (must NOT be named *_eq/eq_*), reachable from widget test modules]`

**`common.rs` unit test.** Pin `GHOST_HOVER_OVERLAY`/`GHOST_PRESS_OVERLAY` stored alpha to
`15`/`31` (`round(0.06·255)`, `round(0.12·255)`) so the non-token overlay values are a tested
contract, not a comment. — `[derived → cargo test -p gp-render]`

**AC8 — gallery golden (Miri-ignored wgpu snapshot).**
- Location: `crates/render/src/widgets/gallery.rs` `#[cfg(test)]` (in-crate, to reach the
  private `paint` layer with forced states).
- Entry point: the gallery render fn drawing the full matrix (Buttons: 4 variants × {rest,
  hover, press, disabled} + 3 sizes; IconButtons: secondary/ghost × {rest, hover, press,
  active, disabled} with baked glyphs; Badges: 5 tones × {solid, tinted}; Tags: rest,
  selected, with-dot, with-remove; Cards: elevations 0–3, selected, grid-watermark).
- Scenario: one `egui_kittest::Harness` frame, `with_pixels_per_point(1.0)`,
  `with_theme(Light)`, `RendererOptions::PREDICTABLE`, CPU-adapter assertion, fonts installed
  (frame-1-install/frame-2-draw), an `IconSet::new(ctx)` for the icon cells, then
  `try_image_snapshot_options(&image, "widget_gallery", threshold 0.0 + failed_pixel 0)`.
  Mint the golden PNG on first run (image-check verification of the minted golden happens at
  mint time per the code-writer/image-check flow).
- `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)")]`.
- **Selected-border width — expect the per-component mapping, not the AC prose.** The spec's
  AC4/AC5 prose both say "2-pt" selected border, but the per-component `.jsx` mapping (which
  the spec designates as the port ground-truth) differs: Tag `selected` = `BW_1 = 1.5px`,
  Card `selected` = `BW_2 = 2.0px`. The design follows the detailed mapping (Tag = 1.5,
  Card = 2.0); the by-eye gallery check must therefore expect the **1.5px** Tag border and
  the 2.0px Card border, and not treat the thinner Tag border as a regression. No AC change.
- — `[measured: placeholder.rs golden_guard is the structural precedent for every line above]`

**Optional (not required for any AC):** Miri-clean `run_ui` shape-inspection tests on the
non-text paint output (e.g. Tag's dot circle, Card's border rect fill color) in the style of
`icons.rs draw_icon_emits_tinted_textured_mesh` — only for paint helpers that draw **no**
text (text-drawing shape tests would need Miri-ignoring, buying little over the golden).

## Open questions

None blocking. The three spec Open questions are resolved above: interaction model
(decision 1 — confirm interactive `Response` widgets), #88 icon-handle type (decision 4 —
`Option<&egui::TextureHandle>` from a caller-owned `IconSet`), specimen form (decision 6 —
in-crate Miri-ignored wgpu golden). All three were flagged in the spec as design's call
("via a Design Amendment"), so resolving them here is a design decision, not a spec change.

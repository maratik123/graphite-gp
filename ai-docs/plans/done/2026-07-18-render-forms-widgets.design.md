# Design: gp-render forms widgets — Slider, Switch, SegmentedControl, Stepper

**Issue:** #14
**Date:** 2026-07-18
**Spec:** `ai-docs/plans/2026-07-18-render-forms-widgets.spec.md`
**Precedent:** #13 (render-core-widgets), merged — spec/design at
`ai-docs/plans/done/2026-07-18-render-core-widgets.{spec,design}.md`; live code under
`crates/render/src/widgets/` (`mod.rs`, `common.rs`, `button.rs`, `icon_button.rs`, `badge.rs`,
`tag.rs`, `card.rs`, `gallery.rs`)
`[measured: ls crates/render/src/widgets/ → 8 files; git log → cafc2a7 …#94 merged, #13 landed]`.

## Approach

Port the four design-system **forms** components to native `gp-render` widgets under the existing
`crates/render/src/widgets/` submodule, reusing #13's three-layer architecture **verbatim**. Each new
widget is one file (`slider.rs`, `switch.rs`, `segmented_control.rs`, `stepper.rs`) holding a `Copy`
props builder + a `…Style` struct + the three layers + a `#[cfg(test)] mod tests`:

1. **Pure style-resolution layer (AC7)** — a `const fn resolve(state…) -> …Style` mapping widget state
   to a plain `Copy` struct of `Color32` + `f32`. No `egui::Ui`, no text, no allocation → Miri-clean,
   unit-tested. `resolve` is pure selection over `crate::tokens` consts + a struct literal, so the
   nursery `clippy::missing_const_for_fn` (deny) **FORCES `const fn`** on it, exactly as it does on
   `Button::resolve`/`Card::resolve`/`Tag::resolve`
   `[measured: clippy-driver missing_const_for_fn on a pure-selection fn → "this could be a const fn"; Cargo.toml:49-50 pedantic+nursery deny]`.
2. **Paint layer** — a `pub(crate) fn paint(painter, rect, &style, …forced-state…)` drawing the
   resolved style with `egui::Painter` primitives (+ text via `painter.text`), taking every visual
   state **already resolved / already computed** so the AC8 golden can force any state without pointer
   input, mirroring `gallery.rs`.
3. **Interaction shell** — a `pub fn show(self, ui) -> …Response` (egui builder idiom: `new(..)` +
   chainable `const fn` setters) that allocates a rect + `Response`, reads live pointer input, computes
   the new value, calls `resolve` then `paint`, and returns a small per-widget response struct carrying
   the up-to-date value + a `changed` flag.

**Value logic** (Slider snap/clamp + fraction, Stepper step/clamp + bound-disabled, SegmentedControl
single-selection, Switch toggle) lives in **separate pure fns** reachable from the widget module (not
folded into `resolve`, which returns a *style*, not a value), each unit-tested deterministically and
Miri-clean. Their const-ness is **decided by the lint, not by fiat** — see Key decision 2.

**Style comes only from `crate::tokens`.** Every widget-specific numeric literal with semantic meaning
(track height, thumb/knob diameters, the Stepper `34` button, the Switch `40×22`/`16` geometry) becomes
a module-level `const SCREAMING_SNAKE_CASE` with a `.jsx`-source comment, per the magic-number rule and
the `tag.rs`/`button.rs` precedent (which already lift `HEIGHT`/`PAD_X`/`DOT_DIAMETER`/`GAP_LG`, etc.).

**Rejected top-level alternative — one generic `FormWidget`/`Control` trait or a shared generic
`Response<T>`.** #13 deliberately uses one concrete file + one concrete response type per widget
(`TagResponse`), no trait. A trait/generic buys nothing for four independent widgets with different
value types and different paint shapes, and adds abstraction the AGENTS.md YAGNI rule forbids. Keep it
per-widget and concrete. `[measured: widgets/mod.rs re-exports Badge/Button/Card/IconButton/Tag +
TagResponse — no shared widget trait]`

### Key decisions (resolving the spec's flagged questions)

**1. Value types — CONFIRM the spec defaults.** Slider = `f32`, Stepper = `i32`. Floats are permitted
in `gp-render` (the deterministic-integer rule scopes only to `geom`/`sim` per `docs/design.md` §3a;
#14 touches neither) `[measured: spec Technical constraints; docs/design.md §3a scope]`.

**2. `resolve` / value-logic const-ness is set by the lint per callee const-stability (measured, not
assumed).** The nursery `missing_const_for_fn` deny is a *binding constraint* whose firing depends on
what the body **calls**, so I measured each shape on the pinned toolchain (`rustc 1.97.1`
`[measured: rustc --version]`):

| Fn | Body | `missing_const_for_fn` fires? | ⇒ signature |
|---|---|---|---|
| all four `resolve` | pure `match`/`if` over `color::*`/`spacing::*` + struct literal | **yes** | `pub const fn` (FORCED) |
| `Switch::toggled(checked) -> !checked` | trivial bool | **yes** | `const fn` (FORCED) |
| Stepper `dec_disabled`/`inc_disabled` (`value <= min` / `>= max`) | integer compare | **yes** | `const fn` (FORCED) |
| Stepper `stepped` (saturating add/sub + clamp) | `i32::saturating_{add,sub}` + **manual** `if` compares | **yes** | `const fn` (FORCED) |
| Slider `snap_clamp` (`f32::clamp`/`round`/`mul_add`) | float methods | **no** | plain `fn` (lint declines) |
| Slider `fraction` (`(v-min)/(max-min)`) | float arithmetic | **no** | plain `fn` (lint declines) |
| SegmentedControl `selected_index` (`&str` `==`) | `PartialEq<str>` (not const) | **no** | plain `fn` (lint declines) |

`[measured: clippy-driver --edition 2021, #![warn(missing_const_for_fn)] → FIRES on toggled/dec_disabled/stepped(int); does NOT fire on fraction/snap(float)/selected_index(str==)]`

Two non-obvious traps this table encodes, each of which would red the gate or the tests:

- **Integer clamp cannot use `.max()`/`.min()`/`.clamp()` inside a `const fn`.** On 1.97 those route
  through `<i32 as Ord>` which is *not yet a const-stable trait* → hard `E0658` ("`Ord` is not yet
  stable as a const trait"). Stepper's forced-`const` `stepped` therefore clamps with **manual `if`
  comparisons** (`if v < min { min } else if v > max { max } else { v }`), not `Ord::clamp`. `f32::clamp`
  is a const-callable *inherent* method (fine for the non-const float fns anyway).
  `[measured: rustc --edition 2021 → const fn calling i32::max/min errors E0658 "cannot call conditionally-const method <i32 as Ord>::max"; f32::clamp compiles in const fn]`
- **Float value-logic stays plain `fn` — do NOT "helpfully" mark it `const`.** The lint declines on
  `snap_clamp`/`fraction`, so a plain `fn` is the lint-sanctioned form (the same "the lint decides"
  posture that keeps `color::car_color` and `Rect::index` deliberately non-const). This is the correct
  YAGNI-neutral outcome, not a lint miss.

**3. `onChange` → return mechanism: per-widget response struct carrying the new value + `changed`,
builder stays `Copy`.** Following #13's "caller owns the value, passed **by value**; the changed value
is surfaced through `show`'s return" (`TagResponse` precedent), each `show` returns a small `#[derive(
Debug)]` struct (`Response` impls `Debug`, as `TagResponse` proves):

| Widget | `show` returns | Field semantics |
|---|---|---|
| Slider | `SliderResponse { response: Response, value: f32, changed: bool }` | `value` = snapped+clamped current value (updated while dragged); `changed` when it moved |
| Switch | `SwitchResponse { response: Response, checked: bool, changed: bool }` | `checked` = post-click state (`!checked` on click); `changed` when toggled |
| SegmentedControl | `SegmentedControlResponse { response: Response, selected: Option<usize>, changed: bool }` | `selected` = index selected after this frame (clicked index, else the index matching input `value`, else `None`); `changed` when a click moved it |
| Stepper | `StepperResponse { response: Response, value: i32, changed: bool }` | `value` = stepped+clamped current value (updated on +/− click); `changed` when it moved |

`changed` is an **explicit** field (mirroring `TagResponse::remove_clicked`), not derived from egui's
change-tracking — no dependence on `Response::mark_changed`. Rejected `&mut value` mutation: it is
egui's own idiom but breaks the `Copy` by-value builder chain #13 standardised (all #13 builders are
`Copy`; a stored `&mut` or the mutation contract is the opposite of "caller owns the value"). Rejected a
plain `Response` (no struct) for Slider/Stepper/Segmented: the new value is **not** derivable from
`Response` alone (Slider value depends on pointer-x; the widget owns the +/− and per-segment hit areas),
so it must be returned explicitly. `[measured: tag.rs:41-47 TagResponse{response,remove_clicked} #[derive(Debug)]; button.rs builder is #[derive(Clone,Copy)]; egui-0.35.0/src/response.rs:529 interact_pointer_pos, :575 is_pointer_button_down_on, :605 mark_changed all present]`

**4. Slider `format` is a `show` parameter, not a stored field (keeps the builder `Copy`).** A stored
`Fn(f32) -> String` closure is not `Copy`, which would break the `Copy` builder. Mirror `Card::show`,
which takes its `right`/`add_contents` closures as `show` parameters precisely to keep the `Card`
builder `Copy`: `Slider::show(self, ui, format: impl Fn(f32) -> String) -> SliderResponse`. `show_value`
(a stored `bool`) gates whether the readout is drawn; when `false`, `format` is never invoked. The
**paint** layer takes the already-formatted `value_text: Option<&str>` (never the closure), so paint
stays closure-free and the golden forces `Some("T 0.35")` directly. `[measured: card.rs:220-225 show(self, ui, right: Option<impl FnOnce>, add_contents: impl FnOnce) keeps Card #[derive(Copy)]]`

**5. SegmentedControl `options` shape = `&'a [&'a str]` (each entry is value **and** label).** The
`.d.ts` union `(string | SegmentOption)[]` collapses to plain strings in every real usage (difficulty
`['Rookie','Pro','Ace']`, mode, shape) and in the spec's own demo; the spec **explicitly lists
`&[&str]` as an acceptable choice** and notes "The demo uses plain strings". `value: &'a str` selects by
matching; `size: Size` (reuses `common::Size`, already exported). `show` returns the selected **index**
(`Option<usize>`), which the caller maps to `options[i]` — cleaner than returning a lifetime-entangled
`&str`. If a future usage needs `value ≠ label`, upgrade the element type to a `SegmentOption {value,
label}` struct then (clean break — no API-stability contract per AGENTS.md § API Stability). AC5 prop
names (`options`/`value`/`size`) are mirrored; AC3 "surfaces the selected option's value" is satisfied
via the returned index. A `value` matching no option ⇒ `selected_index` returns `None` (all segments
render unselected) — a defined, non-panicking case, per the zero-panic posture.
`[measured: SegmentedControl.d.ts:3-15 options:(string|SegmentOption)[], value:string, size, NO disabled prop; spec Key decisions "plain &[&str] … design's call … demo uses plain strings"]`

**6. Golden specimen = a NEW `forms_gallery.rs` with its own minted golden, NOT an extension of the #13
`widget_gallery`.** A separate in-crate `#[cfg(test)] mod forms_gallery` golden (a) avoids re-minting the
merged #13 `widget_gallery` PNG (pure churn + regression risk on unrelated widgets), (b) mirrors the
established separate-golden precedent (`placeholder`'s golden and `widget_gallery` are distinct), and
(c) tracks `forms.card.html` as its own design-system card. It reuses `gallery.rs`'s structure exactly:
one wgpu frame, CPU-adapter assertion, `RendererOptions::PREDICTABLE` via `.renderer(..)`,
frame-1-install/frame-2-draw fonts, forced states through each widget's `paint`, `#[cfg_attr(miri,
ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)")]`. Because it is **text-heavy**
(labels, readouts, segment labels, Stepper `−`/value/`+`), it uses the same
`SnapshotOptions::new().threshold(1.0).failed_pixel_count_threshold(0)` as `widget_gallery` (design #13
Amendment 2 — the raised per-pixel color threshold absorbs cross-renderer text-AA; a real color
regression still fails). `image-check` re-verifies the minted golden against the drawing code at mint
time (code-writer flow) — never in CI. `[measured: gallery.rs:296-363 widget_gallery golden with threshold(1.0)+failed_pixel_count_threshold(0), CPU-adapter assert, frame-1/2 font dance, miri-ignore]`

**7. Module split — one file per widget + re-exports; a shared 3-site const in `common.rs`.** New
`slider.rs`/`switch.rs`/`segmented_control.rs`/`stepper.rs`, each ~200–320 lines (props + `…Style` + 3
layers + value-logic fns + tests) — under the soft 500/800 limit, not one-struct-per-file. The
disabled-opacity multiplier `0.5` (design-system `opacity: disabled ? 0.5 : 1`) appears in **3 files**
(Slider, Switch, Stepper — SegmentedControl has **no** `disabled` prop), distinct from the core widgets'
`DISABLED_OPACITY = 0.45`; per the shared-const precedent (`GHOST_*_OVERLAY`, `DISABLED_OPACITY`,
`GRID_WATERMARK_OPACITY` all live in `common.rs`) it becomes one module const `FORMS_DISABLED_OPACITY:
f32 = 0.5` in `common.rs`, not three copies. (The ≥3-*crate* shared-crate rule does not apply — this is
one crate, one module.) `[measured: Slider.jsx:23, Switch.jsx:22, Stepper.jsx:30 all "opacity: disabled ? 0.5 : 1"; SegmentedControl.jsx:8 props have no disabled; common.rs:35 DISABLED_OPACITY=0.45 for core widgets]`

### Per-widget prop surface (AC5: `.d.ts` → Rust) & style ground truth (from the `.jsx`)

Common: `disabled` → `enabled: bool` (inverted, as #13); `onChange` → the response struct above;
`style?: CSSProperties` → **dropped** (no Rust analog, as #13). All colors/metrics below are
`crate::tokens` consts unless flagged as a named local const. Fonts resolve
`FontFamily::Name(fonts::<X>.into())`; every needed family (`ONEST_REGULAR`, `ONEST_MEDIUM`,
`JETBRAINS_MONO_REGULAR`, `JETBRAINS_MONO_MEDIUM`) is registered by `fonts::definitions()`
`[measured: fonts.rs:76-133 definitions() inserts all four named families]`.

- **Slider** (`Slider.d.ts`: `value/min/max/step/label/showValue/format/disabled`). Builder: `value:
  f32`, `min: f32` (default 0), `max: f32` (default 100), `step: f32` (default 1), `label: Option<&str>`,
  `show_value: bool` (default true), `enabled: bool`. `resolve() -> SliderStyle` is **stateless** (the
  Slider's palette does not vary with state; disabled is a paint-time opacity uniform) returning:
  track bg `PAPER_3` (height `4`, radius `RADIUS_PILL`), fill `ACCENT` (same height/radius), thumb
  `PAPER_0` fill + `GRAPHITE_900` `BW_2` ring (diameter `18`) + a `SHADOW_1` thumb drop shadow
  (`Slider.jsx:45` `boxShadow: var(--shadow-1)`; token `tokens::effects::SHADOW_1`, drawn by `paint`, see
  the Track row below), plus the row/readout metrics. Named local
  consts: `TRACK_H = 4.0`, `THUMB_D = 18.0`, `ROW_H = 20.0`, `READOUT_GAP = SPACE_2` (`.jsx`
  marginBottom 8). Readout row (`Slider.jsx:24-38`): label mono `JETBRAINS_MONO_REGULAR` `FS_XS`
  uppercase `LS_LABEL` `TEXT_MUTED` (left); value mono `JETBRAINS_MONO_MEDIUM` `FS_SM` `TEXT_INK`
  (right, drawn iff `show_value`). Track row (`:39-46`): thumb at `left = fraction·track_w − THUMB_D/2`;
  `paint` draws the thumb's `SHADOW_1` drop shadow FIRST (below the thumb circle) via
  `painter.add(SHADOW_1.as_shape(thumb_rect, thumb_radius))` — mirroring `card.rs:168-169`'s guarded
  `painter.add(style.shadow.as_shape(rect, corner_radius))` shadow draw (`Slider.jsx:45`
  `boxShadow: var(--shadow-1)`) `[measured: card.rs:169 painter.add(style.shadow.as_shape(rect, corner_radius)); tokens/effects.rs:32 SHADOW_1 = --shadow-1]`.
  `enabled == false` → all draws `gamma_multiply(common::FORMS_DISABLED_OPACITY)` (the thumb shadow dims
  via `Shadow { color: SHADOW_1.color.gamma_multiply(common::FORMS_DISABLED_OPACITY), ..SHADOW_1 }`, since
  a `Shadow` shape has no per-primitive gamma path).
  Value logic: `fraction(value,min,max)`, `snap_clamp(value,min,max,step)` (both plain `fn`, per Key
  decision 2). `paint(painter, rect, &style, fraction, label, value_text, enabled)` (7 args — clean
  without an `#[allow]`: `too_many_arguments` fires at > 7 and there is no threshold override, exactly
  as `card.rs`'s 7-arg `paint` compiles allow-free; `tag.rs`'s `#[allow]` on its own 7-arg `paint` is
  defensive/inert). `[measured: clippy.toml sets only stack/array thresholds — no too-many-arguments override → default 7; card.rs:157 pub(crate) fn paint has 7 args and NO too_many_arguments allow; button.rs:233 8-arg paint DOES carry the allow]` `show(self, ui, format: impl
  Fn(f32) -> String) -> SliderResponse`: allocate full-width row (`ui.available_width()`),
  `Sense::click_and_drag()`, read `response.interact_pointer_pos()` for the drag-x → `new =
  snap_clamp(value_at(px), …)`, `changed = new.to_bits() != value.to_bits()` (f32 **bit-equality** — a
  `u32 != u32`, lint-clean, total, and exactly "the snapped value moved"; a bare `new != value` on two
  `f32` would trip the pedantic `clippy::float_cmp` deny, which `button.rs:443` documents "fires only on
  `==`/`!=`", and `tokens/mod.rs` `assert_f32` is the crate's SOLE `float_cmp` allow site — NOT reachable
  from `show`) `[measured: button.rs:443 "that lint fires only on ==/!="; tokens/mod.rs:105-107 assert_f32 = sole float_cmp allow site]`.
- **Switch** (`Switch.d.ts`: `checked/label/disabled`). Builder: `checked: bool`, `label:
  Option<&str>`, `enabled: bool`. `resolve(checked) -> SwitchStyle`: track = `checked ? ACCENT :
  PAPER_3` (AC2), knob fill `PAPER_0`, knob ring `GRAPHITE_900`, track border `GRAPHITE_900`. Named
  consts (`Switch.jsx:27-36`): `TRACK_W = 40.0`, `TRACK_H = 22.0`, `KNOB_D = 16.0`, `KNOB_INSET = 2.0`,
  `KNOB_ON_X = 20.0` (`.jsx:33` `left: checked ? 20 : 2` — the checked knob x is hard-coded `20`, NOT the
  symmetric `TRACK_W − KNOB_INSET − KNOB_D = 22`), track radius `RADIUS_PILL`, track border `BW_1`, knob
  ring `1.5` (= `BW_1`), label gap `10` (a local `LABEL_GAP = 10.0`). Knob x = `checked ? KNOB_ON_X :
  KNOB_INSET` (= on `20`, off `2` — matches `Switch.jsx:33`'s asymmetric `left: checked ? 20 : 2`; the
  earlier symmetric `TRACK_W − KNOB_INSET − KNOB_D = 22` was WRONG vs ground truth, so the specimen
  matches `forms.card.html`) (paint reads `checked`) `[measured: Switch.jsx:33 left: checked ? 20 : 2]`. Label UI `ONEST_REGULAR` `FS_BODY` `TEXT_BODY`. `toggled(checked) -> !checked` (`const
  fn`). `paint(painter, rect, &style, checked, label, enabled)`. `show` → `SwitchResponse`
  (`Sense::click()`; `checked = if clicked { !self.checked } else { self.checked }`).
- **SegmentedControl** (`SegmentedControl.d.ts`: `options/value/size`). Builder: `options: &'a
  [&'a str]`, `value: &'a str`, `size: Size`. `resolve(selected: bool, size) -> SegmentStyle`: selected
  → bg `GRAPHITE_900` + fg `PAPER_0`; else bg `Color32::TRANSPARENT` + fg `TEXT_BODY` (AC3); height
  `CONTROL_H_{SM,MD,LG}`, font `FS_SM` (sm) else `FS_BODY`, family `ONEST_MEDIUM` (`.jsx` fw-medium).
  Outer chrome (`SegmentedControl.jsx:12-19`): border `GRAPHITE_900` `BW_1`, radius `RADIUS_2`, bg
  `PAPER_0`; inter-segment left divider `GRAPHITE_900` `BW_HAIR` (`:32`); per-segment padding `0 14px`
  (local `SEG_PAD_X = 14.0`, `:31`). `selected_index(options, value) -> Option<usize>` (plain `fn`).
  `paint(painter, rect, options, selected, size, ...)` loops `resolve(i == selected, size)` per segment,
  measuring each label to place it (deterministic, no pointer). `show` → `SegmentedControlResponse`
  (allocate the measured total width; per-segment `ui.interact(seg_rect, id.with(i), Sense::click())`;
  `selected = clicked_index.or(selected_index(options, value))`; `changed = clicked_index.is_some()`).
- **Stepper** (`Stepper.d.ts`: `value/min/max/step/label/disabled`). Builder: `value: i32`, `min: i32`
  (default 0), `max: i32` (default 99), `step: i32` (default 1), `label: Option<&str>`, `enabled: bool`.
  `resolve(dec_disabled, inc_disabled) -> StepperStyle`: `dec_fg`/`inc_fg` = `TEXT_FAINT` when that
  affordance is disabled else `TEXT_INK` (`Stepper.jsx:17`); container border `GRAPHITE_900` `BW_1`,
  radius `RADIUS_2`, bg `PAPER_0`, value fg `TEXT_INK`, cell dividers `BORDER_HAIRLINE` `BW_HAIR`
  (`:36`). Named consts: `BTN_SIZE = 34.0` (`:16`), box height `CONTROL_H_MD` (`:28`), `VALUE_MIN_W =
  40.0` (`:34`), `BTN_FS = 18.0` (`:18`), label gap `SPACE_2` (`:23`). Label mono
  `JETBRAINS_MONO_REGULAR` `FS_XS` uppercase `LS_LABEL` `TEXT_MUTED`; value mono
  `JETBRAINS_MONO_MEDIUM` `FS_TITLE` `TEXT_INK`; `−`/`+` mono `JETBRAINS_MONO_REGULAR` `BTN_FS`. Value
  logic (`const fn`): `dec_disabled(value,min) = value <= min`, `inc_disabled(value,max) = value >= max`
  (AC4), `stepped(value, step, min, max, StepDir) = value.saturating_{add,sub}(step)` then manual
  `if`-clamp into `[min,max]`. `paint(painter, rect, &style, value_text, label, enabled)`. `show` →
  `StepperResponse` (allocate label + box column; two `ui.interact` hit rects for `−`/`+` gated by
  `enabled && !dec/inc_disabled`; on click, `value = stepped(...)`, `changed = true`).

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Shared infra + module doc.** Add `FORMS_DISABLED_OPACITY: f32 = 0.5` (doc: `.jsx` `opacity: disabled ? 0.5 : 1`, 3-site) to `common.rs` + a unit test pinning it; update `widgets/mod.rs` module doc to cover the four forms widgets. | `crates/render/src/widgets/common.rs`, `.../widgets/mod.rs` | — |
| 2 | **Switch** (simplest first): `Switch` builder (`checked`/`label`/`enabled`), `SwitchStyle`, `SwitchResponse`, `const fn resolve(checked)` + `const fn toggled`, named geometry consts, `paint`, `show`; resolve tests (checked→track `ACCENT`/`PAPER_3`, AC2) + toggle test. Register `pub mod switch; pub use switch::{Switch, SwitchResponse};` in `mod.rs`. | `crates/render/src/widgets/switch.rs`, `.../widgets/mod.rs` | 1 |
| 3 | **Slider**: `Slider` builder, `SliderStyle`, `SliderResponse`, `const fn resolve()` (stateless palette/metrics), plain `fn snap_clamp`/`fn fraction`, named consts, `paint(…, fraction, label, value_text, enabled)`, `show(self, ui, format)`; resolve tests + AC1 value-logic tests (tolerant compare — see Test Design). Register in `mod.rs`. | `crates/render/src/widgets/slider.rs`, `.../widgets/mod.rs` | 1 |
| 4 | **Stepper**: `Stepper` builder, `StepperStyle`, `StepperResponse`, `const fn resolve(dec_disabled, inc_disabled)` + `const fn dec_disabled`/`inc_disabled`/`stepped` (manual-clamp, saturating), named consts, `paint`, `show`; resolve tests + AC4 bound tests. Register in `mod.rs`. | `crates/render/src/widgets/stepper.rs`, `.../widgets/mod.rs` | 1 |
| 5 | **SegmentedControl**: `SegmentedControl` builder (`options: &[&str]`/`value`/`size`), `SegmentStyle`, `SegmentedControlResponse`, `const fn resolve(selected, size)` + plain `fn selected_index`, named consts, multi-segment `paint`, `show`; resolve tests + AC3 single-selection tests. Register in `mod.rs`. | `crates/render/src/widgets/segmented_control.rs`, `.../widgets/mod.rs` | — |
| 6 | **Forms gallery golden (AC8)**: in-crate `#[cfg(test)] mod forms_gallery` rendering the four-widget matrix via `egui_kittest`+wgpu through the private `paint` layers with forced states; `#[cfg_attr(miri, ignore)]`; `SnapshotOptions` `threshold(1.0)` + `failed_pixel_count_threshold(0)`; CPU-adapter assert; frame-1-install/frame-2-draw fonts; mint the `forms_gallery` PNG. Wire `#[cfg(test)] mod forms_gallery;` in `mod.rs`. | `crates/render/src/widgets/forms_gallery.rs`, `.../widgets/mod.rs`, `crates/render/tests/snapshots/` (minted PNG) | 2, 3, 4, 5 |

M = 6 (≤ 15 — no issue split needed). Dependencies: 2/3/4 use `FORMS_DISABLED_OPACITY` → depend on 1;
5 has no `disabled` prop → independent of 1; 6 drives all four `paint` layers → depends on 2–5.

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping (a)–(h). This is a **single change-type** task
— every subtask edits **code** (`*.rs`, plus the minted golden PNG which is a test artifact of a code
subtask, subtask 6), so all subtasks route to one implementor model, exactly as #13's handoff plan.

- **(a) grouping required (M ≥ 1):** yes — one `## Handoff plan` group below.
- **Group A** (terminal, and the only group) — **code** change-type, implementor **`sonnet`**
  (sonnet-5), effort **`medium` (pinned in `code-writer` frontmatter)**, 1M-token window, via the
  `code-writer` subagent — subtasks **1–6**. All six are the same change-type (code) with a dependency
  chain that runs cleanly in source order (1 → {2,3,4}; 5 independent; {2,3,4,5} → 6), so they cluster
  into the **fewest possible groups = 1** — no other change-type exists to force a boundary.
- **(b) size cap:** group size = 6, within the `≤ 10` maximum (no size-cap split).
- **(c) handoff destination:** at the start of Group A, spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) — the single group runs Step 8
  in its own `/context-reset` subagent. No inter-group handoff (only one group).
- **(d) terminal-group sizing:** terminal group size = 6, within `1..=10`.
- **(e) change-type homogeneity:** homogeneous — all code (`*.rs` + the golden PNG test artifact); no
  instructions/harness (`*.md`/`.claude/**`) files touched.
- **(f) group-minimization:** 1 group is already the minimum for a single change-type; no interleaving.
- **(g) per-group model + effort marking:** Group A → `code-writer`, `sonnet` / effort `medium` (pinned)
  — no inline `model=`/effort override. The `design`/`design-review`/`self-review` Opus gates are
  unchanged.
- **(h) max-groups:** 1 group ≤ the default max of 4 — no user gating needed.

## Risks

- **`resolve` const-fn requirement misread as optional.** Each `resolve` is const-eligible (pure
  selection over consts + struct literal); nursery `missing_const_for_fn` (deny) FORCES `const fn`. A
  non-const `resolve` reds the gate. Mitigation: write all four `resolve` as `pub const fn`, radius
  returned raw `f32` (defer `CornerRadius::from` to paint, the `button.rs`/`card.rs` pattern). —
  `[measured: clippy-driver missing_const_for_fn FIRES on pure-selection fn; Cargo.toml:49-50 nursery=deny]` `[derived → cargo clippy --workspace --all-targets -- -D warnings]`
- **Integer/bool value-logic must be `const`, but `.max()`/`.min()`/`.clamp()` are not const-callable.**
  `stepped`/`toggled`/`dec_disabled`/`inc_disabled` are all flagged const-eligible → FORCED const; yet
  `<i32 as Ord>::max/min/clamp` is `E0658` inside a `const fn` on 1.97. Mitigation: `stepped` clamps with
  **manual `if` comparisons** and steps with `saturating_add`/`saturating_sub` (also removes the `-step`
  overflow at `i32::MIN`); `dec/inc_disabled` are plain integer compares. —
  `[measured: rustc 1.97.1 → const fn calling i32::max/min = E0658 "Ord is not yet stable as a const trait"; manual-if int clamp + saturating_add compile const; clippy FIRES missing_const_for_fn on the manual-if stepped]` `[derived → cargo clippy + cargo test -p gp-render]`
- **Float value-logic must NOT be force-marked `const` (and needs no `#[allow]`).** `missing_const_for_fn`
  declines on `snap_clamp`/`fraction` (float methods), so plain `fn` is the sanctioned form; float
  `-`/`/`/`round`/`mul_add` (runtime divisors included) do **not** trip `arithmetic_side_effects` either.
  Mitigation: keep them plain `fn`; guard `max <= min → fraction 0.0` (no NaN from `0/0` in thumb/fill
  positioning). — `[measured: clippy-driver → missing_const_for_fn does NOT fire on snap/fraction; arithmetic_side_effects (deny) does NOT fire on float -,/,round,mul_add with runtime divisor]` `[derived → cargo clippy]`
- **Slider `changed` must not `!=`-compare two `f32` (float_cmp deny — gate-affecting).**
  `SliderResponse.changed` means "the snapped value moved", but a bare `new != value` on two `f32` trips
  the pedantic `clippy::float_cmp` deny (crate-wide), which `button.rs:443` documents "fires only on
  `==`/`!=`"; `tokens/mod.rs` `assert_f32` is the crate's SOLE `float_cmp` allow site (not reachable from
  `show`). Mitigation: `changed = new.to_bits() != value.to_bits()` — a `u32 != u32` bit-equality that is
  lint-clean, total, and deterministic. — `[measured: button.rs:443 "that lint fires only on ==/!="; tokens/mod.rs:105-107 assert_f32 = sole float_cmp allow]` `[derived → cargo clippy --workspace --all-targets -- -D warnings]`
- **Slider value tests will flake under exact `assert_f32`.** `crate::tokens::css::assert_f32` is an
  **exact** `assert_eq!` (for token-vs-CSS bit-identical decimals); a *computed* snap result is not
  bit-identical to a decimal literal for every input — e.g. `snap_clamp(0.9, 0, 1, 0.05) = 0.900000036 ≠
  0.9f32`. Mitigation: Slider `f32` value assertions use a **tolerant** compare
  (`(got - want).abs() < 1e-5`, strict `<`, which does NOT trip `clippy::float_cmp` — the #13 darkness
  metric already relies on `<` being exempt), NOT `assert_f32`. —
  `[measured: rustc -O batch → snap(0.9,0,1,0.05)=0.900000036, exact ==0.9f32 FALSE (1 of 10 cases), all pass abs<1e-5; tokens/mod.rs:105-107 assert_f32 = exact assert_eq!]` `[derived → cargo test -p gp-render]`
- **`arithmetic_side_effects` (deny) on `Pos2 + Vec2` layout math.** The paint layers position thumbs,
  knobs, segment cells, buttons. Per `placeholder.rs` finding 8 + the merged `button.rs`/`gallery.rs`
  pattern, the operator-overloaded `Pos2 + Vec2` trips the deny even though raw `f32 +` does not.
  Mitigation: build every position field-wise as `Pos2::new(a.x + dx, a.y + dy)` from raw-`f32` sums
  (and `mul_add` for the `col·w + x0` grid math), never via `Pos2`/`Vec2` operators. —
  `[measured: button.rs:265-278 field-wise Pos2 from f32 sums; gallery.rs:34-42 cell() uses mul_add]` `[derived → cargo clippy]`
- **Any test that lays out real text aborts Miri.** Drawing text rasterises glyphs via `vello_cpu`'s
  checked `u8→u32` cast (panics under Miri's 1-byte alignment). Mitigation: all AC1–AC4/AC7 coverage is
  in `resolve` + value-logic tests (no `Context`, no text → Miri-clean, run under the workspace Miri
  job); the only text-drawing test is `forms_gallery`, already `#[cfg_attr(miri, ignore)]` for the wgpu
  reason. — `[measured: placeholder.rs tessellation_smoke miri-ignore; gallery.rs:303-306 miri-ignore]`
- **Golden needs a CPU/lavapipe wgpu adapter on CI.** Same premise `widget_gallery` already relies on.
  Mitigation: assert `device_type == Cpu` with the install-lavapipe hint; `RendererOptions::PREDICTABLE`
  via explicit `.renderer(..)`. — `[measured: gallery.rs:309-320 CPU-adapter assert + PREDICTABLE]`
- **Fonts precondition.** Every text draw resolves a `FontFamily::Name(..)`, which epaint cannot lay
  out unless `fonts::definitions()` was installed first — a caller precondition. Mitigation: document a
  `# Panics` line on each `show`/`paint` (as `button.rs`/`card.rs` do); `forms_gallery` installs fonts
  itself (frame-1-install/frame-2-draw). All needed families are registered by `definitions()`. —
  `[measured: fonts.rs:76-133 registers ONEST_REGULAR/MEDIUM/SEMIBOLD + JETBRAINS_MONO_REGULAR/MEDIUM; button.rs:228-232 # Panics precedent]`
- **Golden cross-renderer text-AA non-reproducibility.** `forms_gallery` is text-heavy, so exact
  compare would fail on CI like the pre-Amendment-2 `widget_gallery`. Mitigation: `threshold(1.0)` +
  `failed_pixel_count_threshold(0)` (design #13 Amendment 2's measured value — 2× the observed 1-level
  rounding ceiling, ≪ any real color regression). — `[measured: gallery.rs:349-357 threshold(1.0) rationale comment + SnapshotOptions]` `[derived → AC10 CI green after mint]`
- **Zero production panics.** No `unwrap`/`expect`/`panic!`/indexing panic in any `resolve`/`paint`/
  `show`/value-logic; `selected_index` returns `Option`, `stepped` saturates, `fraction` guards
  `max<=min`. The caller-supplied `format` closure is the caller's surface, not ours. Every
  `expect`/`panic!` stays in `#[cfg(test)]` (`forms_gallery`). — `[measured: spec AC9; gp-core panic-index posture carried into gp-render tests]` `[derived → grep for unwrap/expect outside #[cfg(test)] + cargo clippy]`
- **File size.** Largest new file (`segmented_control.rs` or `slider.rs`, ~320 lines incl. tests) stays
  under the soft 500/800 limit. — `[derived → file-size review + cargo clippy too_many_lines]`

## Test Design

**AC7 / AC2 / AC3 / AC4 — pure `resolve` unit tests (Miri-clean, per widget file).** One `#[cfg(test)]
mod tests` per widget, calling `resolve` directly (no `egui::Context`). `Color32`/integer-field asserts
stay naked `assert_eq!` (`u8`/`i32`/`bool` — no `float_cmp`); fractional-`f32` **metric** asserts (track
heights, radii, geometry consts) route through `crate::tokens::css::assert_f32` (the crate's sole
`float_cmp` allow, reachable from widget test modules), *except* computed Slider values (see below).
- **Slider** `resolve()`: track bg `PAPER_3`, fill `ACCENT`, thumb fill `PAPER_0` + ring `GRAPHITE_900`;
  `assert_f32` on `TRACK_H = 4`, `THUMB_D = 18`, radius `RADIUS_PILL`.
- **Switch** `resolve(true).track == ACCENT`, `resolve(false).track == PAPER_3` (AC2); knob fill
  `PAPER_0`, ring `GRAPHITE_900`; `toggled(true) == false`, `toggled(false) == true`.
- **SegmentedControl** `resolve(true, Md)` → bg `GRAPHITE_900` + fg `PAPER_0`; `resolve(false, _)` → bg
  `TRANSPARENT` + fg `TEXT_BODY`; size → height/font (`Sm`→`CONTROL_H_SM`+`FS_SM`, `Md`→`CONTROL_H_MD`+
  `FS_BODY`). **AC3 single-selection:** `selected_index(["Rookie","Pro","Ace"], "Pro") == Some(1)`;
  assert exactly one index matches (loop counts `== 1`); `selected_index(opts, "Nope") == None`.
- **Stepper** `resolve(dec_disabled=true, _).dec_fg == TEXT_FAINT`, `resolve(false,_).dec_fg ==
  TEXT_INK` (same for `inc`); container border `GRAPHITE_900`, dividers `BORDER_HAIRLINE`.

**AC1 — Slider value logic (Miri-clean, tolerant compare).** Entry points `snap_clamp` + `fraction`.
Scenarios (all via `(got - want).abs() < 1e-5`, strict `<`, no `float_cmp` — NOT `assert_f32`):
- fractional step: `snap_clamp(0.37,0,1,0.05) ≈ 0.35`; `snap_clamp(0.9,0,1,0.05) ≈ 0.90` (the ULP case);
  V_target style `snap_clamp(7.4,2,12,1) ≈ 7.0`.
- out-of-range both ends: `snap_clamp(-0.3,0,1,0.05) == 0.0`, `snap_clamp(1.5,0,1,0.05) == 1.0` (clamp
  ends land on exact bounds → these two MAY use exact, but tolerant is uniform).
- step boundaries: `snap_clamp(0.025,0,1,0.05)` rounds to a step multiple; `step <= 0 → returns clamp`.
- `fraction`: `fraction(6,2,12) ≈ 0.4`; guard `fraction(v, 5, 5) == 0.0` (no NaN when `max<=min`).
- Fixtures: none — plain `f32` inputs.

**AC4 — Stepper value logic (Miri-clean).** Entry points `stepped`/`dec_disabled`/`inc_disabled`
(integer → naked `assert_eq!`). Scenarios: `stepped(5, 1, 2, 6, Up) == 6`; at ceiling `stepped(6,1,2,6,
Up) == 6` (clamp, no overflow); `stepped(2,1,2,6,Down) == 2`; `dec_disabled(2,2) == true`,
`dec_disabled(3,2) == false`, `inc_disabled(6,6) == true`; `i32::MIN` down-step saturates (no panic).

**`common.rs` unit test.** Pin `FORMS_DISABLED_OPACITY == 0.5` (via `assert_f32`) so the 3-site
disabled-opacity value is a tested contract, not a comment. — `[derived → cargo test -p gp-render]`

**AC8 — `forms_gallery` golden (Miri-ignored wgpu snapshot).**
- Location: `crates/render/src/widgets/forms_gallery.rs` `#[cfg(test)]` (in-crate, to reach the private
  `paint` layers).
- Entry point: a gallery render fn drawing the matrix into a fixed canvas (~720×620 logical pts at ppp
  1.0 — exact dimensions the implementor's, sized to fit): **Sliders** (2 rows — labeled+readout at two
  fractions, one disabled; each renders the thumb `SHADOW_1` drop shadow); **Switches** (on/off/disabled, with labels); **SegmentedControls** (sizes
  sm/md/lg with different selected indices); **Stepper** (mid-range, at-min with `−` disabled, at-max
  with `+` disabled). Each cell drives the widget's `paint` with forced state (forced `fraction`,
  `checked`, `selected`, `value_text`, `dec/inc_disabled`) via a `cell()` helper like `gallery.rs`'s.
- Scenario: one `egui_kittest::Harness` frame, `with_pixels_per_point(1.0)`, `with_theme(Light)`,
  `RendererOptions::PREDICTABLE` via `.renderer(..)`, CPU-adapter assertion, fonts installed
  (frame-1/frame-2), then `try_image_snapshot_options(&image, "forms_gallery",
  threshold(1.0)+failed_pixel_count_threshold(0))`. `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the
  Vulkan ICD (no FFI under Miri)")]`. Mint the golden PNG; `image-check` verifies it against the drawing
  code at mint time (never CI). — `[measured: gallery.rs is the structural precedent for every line above]`

## Open questions

None blocking. Every spec-flagged ambiguity is resolved in **Key decisions**: value types (1), the
`onChange`→response mapping (3), the Slider `format` param placement (4), the SegmentedControl `options`
shape (5), golden placement — new `forms_gallery` (6), and the module split + the 3-site
`FORMS_DISABLED_OPACITY` const (7). All were explicitly delegated to `design` by the spec.

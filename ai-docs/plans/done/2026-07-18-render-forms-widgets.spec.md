# gp-render forms widgets — Slider, Switch, SegmentedControl, Stepper

**Source:** issue #14
**Date:** 2026-07-18
**Tracked in:** #14

## Scope

Port the four design-system **forms** components to native `egui` widgets in the
`gp_render::widgets` module, styled entirely from `crate::tokens`. Direct sibling
of the merged #13 (render-core-widgets); reuse that task's established
architecture verbatim.

1. **Slider** — grid-aligned range control for continuous params (pilot
   temperature, V_target, corridor width). Ground truth:
   `docs/design-system/components/forms/Slider.{d.ts,jsx}`.
2. **Switch** — boolean toggle for overlay/option flags (speed heatmap,
   fastest-lap, grid). Ground truth: `.../Switch.{d.ts,jsx}`.
3. **SegmentedControl** — row of mutually exclusive labeled options (difficulty,
   mode, shape). Ground truth: `.../SegmentedControl.{d.ts,jsx}`.
4. **Stepper** — integer +/- control for discrete counts (cars `m`, lap target,
   seed). Ground truth: `.../Stepper.{d.ts,jsx}`.

Each widget follows the #13 **three-layer** pattern (see `crates/render/src/widgets/mod.rs`):

1. a pure `const fn resolve(state…) -> …Style` style-resolution layer
   (Miri-clean — no `egui::Context`, no text, no allocation);
2. a crate-visible `paint(painter, rect, &style, …)` layer;
3. a public `show(self, ui) -> Response`-style interaction shell (egui builder
   idiom: `new(..)` constructor + chainable `const fn` setters).

Value logic (Slider snap/clamp, Stepper bound-clamp, SegmentedControl single
selection, Switch toggle) lives in / is reachable from the pure layer and is
unit-tested deterministically. A wgpu golden **specimen** renders the four
widgets in representative states/values for visual check against
`forms.card.html`, mirroring the existing `widgets/gallery.rs` golden.

## Out of scope

- Wiring these widgets into any game HUD / setup / settings panel — a later Block-2
  task consumes them; #14 delivers the reusable widgets + specimen only.
- Track / asphalt / overlay rendering (`docs/design.md` §4 draw layers).
- Window / event-loop ownership — `gp-render` stays **draw-only** (`gp-game` owns
  the window per the #11 render-backend decision); this task adds none.
- Full ARIA / keyboard-accessibility parity with the `.jsx` DOM semantics
  (`role`, `tabIndex`, `aria-*`, key handlers). Port the **value logic + visual
  style**; interaction focus/keyboard is whatever egui provides idiomatically.
- The React-only `style?: CSSProperties` prop on every component — no Rust analog;
  intentionally dropped (as #13 dropped it).

## Deferred

- (none anticipated — the design phase owns any decomposition-time deferral)

## Key decisions

| Question | Decision |
|---|---|
| Slider value type | `f32` — the design-system uses fractional steps (temperature `step 0.05`); floats are permitted in the render layer (see Technical constraints). Design may refine the exact numeric type. |
| Stepper value type | `i32` — the `.jsx` doc-comment scopes Stepper to integer counts (cars `m`, laps, seed). Design may refine. |
| `onChange` / mutation shape | Follow #13's stateless builder idiom: caller owns the value; the changed value / changed-flag is surfaced through `show`'s return. Exact port of the React `onChange` callback (`&mut value` vs. new-value-in-response vs. a custom response struct like `TagResponse`) is the **design** subagent's call. |
| Slider `format` prop | A caller-supplied value formatter (a closure `Fn(f32) -> String` or equivalent), replacing the React `format?: (v) => ReactNode`. Exact type is design's call. `showValue` gates whether the readout is drawn. |
| SegmentedControl `options` union | The `.d.ts` allows `(string \| { value, label })[]`. The Rust shape that mirrors it (plain `&[&str]`, value/label pairs, or an enum) is design's call; must satisfy AC5. The demo uses plain strings (`['Rookie','Pro','Ace']`). |
| SegmentedControl `size` prop | Reuse the existing `widgets::common::Size` (`Sm`/`Md`/`Lg` → `CONTROL_H_{SM,MD,LG}` + font-size), matching the `.jsx` `size` → `control-h-*` mapping. |
| Golden specimen placement | A wgpu golden covering the four forms widgets, drawn through each widget's `paint` layer with forced states (the `gallery.rs` precedent). Whether it extends the existing `widget_gallery` golden or adds a new `forms_gallery` file is design's call. |
| Module layout | New per-widget files under `crates/render/src/widgets/` (e.g. `slider.rs`, `switch.rs`, `segmented_control.rs`, `stepper.rs`) with public re-exports added to `widgets/mod.rs`; design confirms exact split. |

## Technical constraints

- **egui / egui_kittest 0.35** (already the crate's pinned deps); wgpu golden
  infra (`egui_kittest` + `egui-wgpu` + `image` dev-deps, `SnapshotOptions`)
  is already established by `placeholder.rs` and `gallery.rs`.
- **Three-layer architecture** per widget, exactly as #13: pure `const fn
  resolve` (Miri-clean; the nursery `missing_const_for_fn` lint forces `const
  fn`, so any corner-radius returns a raw `f32` and defers `CornerRadius::from`
  to the paint site) / crate-visible `paint` / public `show`.
- **Style only from `crate::tokens`.** Any widget-specific numeric literal with
  semantic meaning (track heights, thumb/knob diameters, gap/padding values not
  present as a `spacing` token — e.g. the Slider thumb `18` / track `4`, the
  Switch `40×22` track / `16` knob, the Stepper `34` button) → a module-level
  `const SCREAMING_SNAKE_CASE` with a `.jsx`-source comment, per the
  magic-number rule and the Button/Tag precedent.
- Widget value types are `f32` (Slider) and `i32` (Stepper). Floats are allowed
  in this crate.
- The deterministic-integer rule of `docs/design.md` §3a scopes only to the
  physics crates (`geom` / `sim`); #14 touches neither.
- **Zero production panics** — every `expect` / `panic!` / `unwrap` stays inside
  `#[cfg(test)]` code (the crate-wide invariant carried through #11/#13). The
  golden's fonts must be installed via the frame-1-install / frame-2-draw dance
  (see `gallery.rs`); `paint` layers document their layout-time font panic.
- Golden test carries `#[cfg_attr(miri, ignore = "…")]` — it drives wgpu, which
  `dlopen`s the Vulkan ICD (no FFI under Miri); a red workspace Miri blocks merge.
- **Miri gate for the pure layers:** the `resolve` unit tests must be Miri-clean
  (no `Context`/text/alloc), so they run under the workspace Miri job.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | **Slider** emits values **clamped to `[min, max]`** and **snapped to `step`**, and exposes a caller-supplied value formatter for the readout. Snap + clamp are deterministic and unit-tested (incl. fractional steps like `0.05`, out-of-range inputs on both ends, and step boundaries). |
| AC2 | **Switch** toggles a boolean, and its resolved style reflects `checked`: the track is the accent color when on and `paper-3` when off (per `Switch.jsx`); the knob position/state maps to `checked`. Toggle + checked→style are unit-tested. |
| AC3 | **SegmentedControl** selects one of N labeled options and surfaces the selected option's value; **exactly one** segment resolves to the selected style at a time (the selected segment fills `graphite-900` with `paper-0` text per `SegmentedControl.jsx`). Single-selection is unit-tested. |
| AC4 | **Stepper** increments / decrements by `step`, clamping the result into `[min, max]`; the decrement affordance is disabled at `value <= min` and increment at `value >= max` (per `Stepper.jsx`). Bound behavior is unit-tested. |
| AC5 | Each widget's public prop surface **mirrors its `.d.ts` contract** — Slider: `value/min/max/step/label/showValue/format/disabled`; Switch: `checked/label/disabled`; SegmentedControl: `options/value/size`; Stepper: `value/min/max/step/label/disabled`. The React `onChange` maps to `show`'s return mechanism; `style?: CSSProperties` is dropped (no Rust analog). |
| AC6 | All color / size / spacing style values come from `crate::tokens`; every remaining widget-specific magic number is a module-level named `const` with a `.jsx`-sourced doc comment. |
| AC7 | Each widget has a pure `const fn resolve(state…) -> …Style` returning a style struct deterministically, with no `egui::Context`, text, or allocation (Miri-clean), unit-tested for its state→style mapping. |
| AC8 | A wgpu golden **specimen** renders the four forms widgets in representative states/values, driven through each widget's `paint` layer with forced states, and matches a minted golden (flat regions exact; AA edge pixels exempt per the `placeholder.rs`/`gallery.rs` precedent). Test is `#[cfg_attr(miri, ignore)]`. |
| AC9 | `gp-render` keeps **zero production panics** (all `expect`/`panic!`/`unwrap` in `#[cfg(test)]`) and adds no window/event-loop ownership. |
| AC10 | Full gate green: `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`, and the workspace Miri job. |

## Open questions

- (none) — every ambiguity has a defensible default recorded in **Key decisions**
  or is delegated to the `design` subagent (API surface / internal data shapes /
  golden placement / module split).

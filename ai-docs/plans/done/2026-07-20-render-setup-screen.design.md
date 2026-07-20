# Design: gp-render SetupScreen (cars / laps / difficulty / V_target → emit config)

**Issue:** #19
**Date:** 2026-07-20

## Approach

Compose the already-shipped forms widgets into the `SetupScreen` from
`docs/design-system/ui_kits/game/Screens.jsx`, emitting an assembled
`RaceConfig` when the primary button is pressed. `gp-render` stays draw-only —
the screen renders and returns data, never owning window/state/generation
(spec § Technical constraints).

### Module placement — a new `screens` module (not `widgets/`)

Add `crates/render/src/screens/` with:

- `screens/mod.rs` — the shared screen-config types `RaceConfig` + `Difficulty`,
  the module scaffold, and re-exports. `Screens.jsx` defines four screens
  (Setup/Race/Lab/Results) that will land as siblings here; the spec's Out-of-scope
  names `RaceScreen` as a separate issue that also takes a `cfg`. A `screens`
  module is the natural home for these config types and the incoming screens —
  not YAGNI overreach, but the obvious shared parent. The test-only widget
  *galleries* (`game_gallery.rs`, `forms_gallery.rs`) live under `widgets/`
  because they are `#[cfg(test)]` specimens of widgets; a real screen is
  production `pub` code and belongs in its own module.
- `screens/setup.rs` — `SetupScreen`, `SetupResponse`, the pure `assemble`
  function, and `show`.
- `screens/setup_gallery.rs` — the `#[cfg(test)]` wgpu golden (mirrors
  `widgets/game_gallery.rs`'s in-crate placement).

`lib.rs` gains `pub mod screens;` and a `pub use screens::{...}` re-export.

### Config type — `gp-render`-local, single consumer

`RaceConfig`/`Difficulty` live in `gp-render`. Its only current consumer is
`gp-render` itself (emit-only); `gp-render` does **not** depend on `gp-gen` or
`gp-ai` today `[measured: sed -n Cargo.toml → deps are gp-core, egui, resvg,
tiny-skia, thiserror, strum, enum-map — no gp-gen/gp-ai]`. Siting the type here
now avoids pulling either crate in for a value they do not yet consume
(`gp_gen::generate` and `gp_ai::policy_action` are both `todo!` stubs — spec §
Out of scope). Per AGENTS.md § "≥3-site duplication", a shared workspace crate
is warranted only once the type is *replicated* across ≥3 crates; today it is a
**single** definition with a single consumer, so no shared crate — revisit when
the downstream `gp-gen`/`gp-ai`/`gp-game` consumers land (downstream, out of
scope). Call-site count recorded here for auditability: **1**.

### Config field types

- `cars: u32` — matches `gp_gen::GenParams.cars: u32` `[measured: rg 'struct
  GenParams|cars|v_ceiling' crates/gen/src/lib.rs → cars: u32, v_ceiling: i32]`.
- `laps: u32` — a non-negative integer (bounded [1, 9]).
- `v_target: i32` — whole cells/turn, honoring the integer-only physics/generation
  domain (`docs/design.md` §3a). `V_target` is the **design input [D3]**
  `[measured: rg 'V_target|D3' docs/design.md → line 34 "V_target — design-скорость …
  Вход генерации … [D3]"; line 35 explicitly forbids conflating it with V_ceil]` —
  **not** `V_ceil`/`GenParams.v_ceiling` (the oracle's floating BFS bound). The
  `Slider` is `f32`-based; the config carries the snapped **integer**.
- `difficulty: Difficulty` — an enum `{ Rookie, Pro, Ace }` with a pure
  `→ temperature` mapping (below). Stored as the enum (lossless, preserves the
  player's choice); the `f32` temperature is *derived* on demand, keeping a
  single source of truth for the mapping.

### `Difficulty → temperature`

`Difficulty::temperature(self) -> f32`, a pure mapping with **Ace = lowest,
Rookie = highest** (`docs/design.md` §5: temperature is the softmax skill dial,
low = strong/smooth pilot, high = noisy) `[measured: rg 'temperature|softmax'
crates/ai/src/lib.rs → policy_action(_features, _mask, _temperature: f32); "low =
a strong, smooth pilot; high = a noisy…"]`. Placeholder values
(spec Key decisions, tunable): **Rookie 1.5, Pro 1.0, Ace 0.6**. The
temperature type is `f32`, matching `gp_ai::policy_action`'s `_temperature: f32`.

**Const-ness (binding lint).** `Difficulty::temperature`, `::label`,
`::from_index`, `::to_index`, and `RaceConfig::temperature` are all pure bodies
of `match` over `f32`/`&str`/`usize` literals with no non-const-stable calls →
`clippy::missing_const_for_fn` (nursery = deny) FORCES `const fn` on each
`[derived → cargo clippy --workspace --all-targets -- -D warnings]`. The pure
`assemble` function is **NOT** const-eligible — it calls `u32::try_from` /
`i32::try_from` (`TryFrom` is not const-stable) and `f32::round`/`clamp` (not
const-stable) → the lint correctly declines; it stays a plain `fn`
`[derived → clippy gate]`.

**`DIFFICULTY_LABELS` drift guard (design-review recommendation).**
`DIFFICULTY_LABELS: [&str; 3]` stays an explicit const (it feeds
`SegmentedControl::new`), and its correspondence to `Difficulty::label()` is
pinned by a test: for every variant `DIFFICULTY_LABELS[v.to_index()] ==
v.label()`. **Disposition:** the reviewer judged the existing label↔index
round-trip adequate and left the choice open; this keeps that round-trip **and**
adds the array⇄`label()` equality assertion as a cheap drift guard, rather than
deriving the array from `label()` at const time (equivalent safety, simpler — the
array must exist as a `&[&str]` slice for `SegmentedControl` regardless).
`[derived → cargo test -p gp-render screens::]`

### Emission mechanism — a response struct, mirroring the widget idiom

Every widget returns a response struct carrying the updated value + a change
flag (`StepperResponse { response, value, changed }`,
`SegmentedControlResponse { response, selected, changed }`). The screen mirrors
this:

```rust
pub struct SetupScreen { config: RaceConfig }          // builder, holds live state
impl SetupScreen {
    pub const fn new(config: RaceConfig) -> Self { … }  // const-eligible: struct literal
    pub fn show(self, ui: &mut Ui) -> SetupResponse { … }
}
pub struct SetupResponse {
    pub response: Response,    // the primary "Generate track" button's row Response
    pub config: RaceConfig,   // the live-updated values this frame (always present)
    pub generated: bool,      // true iff "Generate track" was clicked THIS frame
}
```

`SetupResponse` carries the primary button's `pub response: egui::Response`,
mirroring the crate's widget-response idiom — every widget response is
`{ response: Response, value/selected, changed }` `[measured: rg 'pub struct
.*Response' + fields crates/render/src/widgets/{stepper,slider,segmented_control}.rs
→ each has `pub response: Response`]`. Beyond idiom-consistency (the caller can
read `response.hovered()` for its own feedback), it gives the AC6 interaction
test (§ *Interaction test*) a **deterministic click target**: `resp.response.rect`
is the button's screen rect, so the test clicks its center rather than
hand-computing coordinates. Adding a non-`Copy` `Response` does not regress
anything — `SetupResponse` was never `Copy` (only `RaceConfig` is).

`RaceConfig` is `Copy` (all-`Copy` fields) and serves as **both** the live
editable state (in) and the emitted value (out) — exactly `Screens.jsx`'s `cfg`
object, which is simultaneously the state and the `onGenerate` payload. The
caller (`gp-game`, later) owns a `RaceConfig`, calls `show` each frame, adopts
`resp.config`, and transitions when `resp.generated` (AC6 — "emitting nothing
until the button is pressed" = `generated` is `false` until the click).

*Rejected — `Option<RaceConfig>` as the sole return:* loses the live-edit values
on non-generate frames, so the caller could not reflect widget edits back into
its state. *Rejected — a stored callback:* not the crate idiom (no widget takes
one) and would break `Copy`.

### `show` composition (top → bottom), snapped to the 4px lattice

The screen centers a fixed-width content column (`CONTENT_MAX_W = 560.0`, the
`Screens.jsx` `maxWidth: 560` — a container bound, not an inter-widget gap, so
not subject to AC7's token rule; recorded as a documented module const). Within
it:

1. **Wordmark block** (centered): the accent dot (`16×16` = `SPACE_4` circle,
   `ACCENT` fill, `GRAPHITE_900` `BW_2` ring) + gap `SPACE_3` (12) + the
   two-tone wordmark — `"GRAPHITE "` in `TEXT_INK` then `"GP"` in `ACCENT`, both
   in the **display face** `FontFamily::Name(fonts::ONEST_BOLD)` at `FS_H1` (40,
   `Screens.jsx` `fontSize: 40, fontWeight: 700`). Two `painter.text` calls, the
   second offset by the measured advance of the first (`layout_no_wrap`), since
   egui has no rich-text run. Then, below (gap `SPACE_2`), the mono uppercase
   subtitle `"GRID VECTOR RACING"` (`.to_uppercase()`) in
   `fonts::JETBRAINS_MONO_REGULAR` at `FS_XS`, `TEXT_MUTED`.
2. **`Card`** — built `.eyebrow("New race").title("Set up the grid").grid(true)
   .padding(SPACE_6)`, then invoked as
   `card.show(ui, None::<fn(&mut Ui)>, |ui| { … })`. `Card::show` takes a
   **mandatory** `right` header-right closure argument between `ui` and
   `add_contents` `[measured: card.rs:220-225 → pub fn show(self, ui: &mut Ui,
   right: Option<impl FnOnce(&mut Ui)>, add_contents: impl FnOnce(&mut Ui)) ->
   Response]`; the setup card has no header-right content, so `right = None`,
   written with the turbofish `None::<fn(&mut Ui)>` (a bare `None` cannot infer
   the `impl FnOnce(&mut Ui)` type parameter — a function-pointer type satisfies
   the `FnOnce(&mut Ui)` bound and pins it). Its `add_contents` closure lays out,
   vertically (gap `SPACE_6`):
   - a horizontal row (gap `SPACE_8`) of two `Stepper`s: `Cars (m)` `.min(2)
     .max(6)`, `Laps` `.min(1).max(9)`;
   - the difficulty block — a mono uppercase label `"Difficulty (pilot
     temperature)"` (`FS_XS`, `TEXT_MUTED`), gap `SPACE_2`, then
     `SegmentedControl::new(&DIFFICULTY_LABELS, current.difficulty.label())`;
   - the `Slider` `.min(3.0).max(10.0).step(1.0).label("V_target (design
     speed)")`, shown with `format = |v| format!("{} cells/turn", v as i32)`.
3. **Primary button** (centered, gap `SPACE_6` above):
   `Button::new("Generate track").variant(Primary).size(Lg)` → `.show(ui)
   .clicked()` sets `generated`.
4. **Footer** (centered, gap `SPACE_3` above): mono
   `"Procedural · closed loop · valid by construction"` (`FS_XS`, `TEXT_FAINT`).

**Spacing map (`Screens.jsx` px → token).** AC7 requires every gap/padding be a
4-multiple sourced from `crate::tokens::spacing`. Several `.jsx` values are
off-lattice or tokenless; each is snapped to the nearest spacing token:

| Site | `.jsx` px | Token | Value |
|---|---|---|---|
| outer padding (vert / horiz) | 48 / 24 | `SPACE_12` / `SPACE_6` | 48 / 24 |
| header → card | 36 | `SPACE_8` | 32 |
| dot → wordmark gap | 12 | `SPACE_3` | 12 |
| wordmark → subtitle | 10 | `SPACE_2` | 8 |
| card padding | 24 (`--space-6`) | `SPACE_6` | 24 |
| card vertical gap | 24 | `SPACE_6` | 24 |
| cars/laps row gap | 32 | `SPACE_8` | 32 |
| difficulty label → control | 8 | `SPACE_2` | 8 |
| card → button row | 24 | `SPACE_6` | 24 |
| button → footer | 14 | `SPACE_3` | 12 |

Every resulting value is a 4-multiple sourced from a named token (AC7).

### Deviations from the reference

- **Shuffle icon omitted.** `Screens.jsx`'s button has `iconLeft={shuffle}`, but
  the vendored `Icon` set is exactly `{Play, Pause, Grid3x3, ZoomIn, Settings}`
  — no `shuffle` `[measured: ls crates/render/icons/ → grid-3x3, pause, play,
  settings, zoom-in]`. No AC requires the icon (spec item 3 + AC list are
  icon-silent), so the button is text-only. Vendoring `shuffle.svg` (new `Icon`
  variant + `svg_bytes` arm + byte-size/`COUNT`/`IntoStaticStr` test updates + a
  baked `TextureHandle` at render time) is out of scope — see Open questions.
- **Letter-spacing dropped.** `painter.text` has no letter-spacing parameter;
  the `.jsx`'s `0.14em`/`-0.02em` tracking is not applied, consistent with every
  existing widget (`Card` eyebrow, `Stepper` label, etc.).

### Golden test — drive the real `show` in `egui_kittest`

Unlike the widget galleries (which force per-widget states through the private
`paint` layer), the screen golden drives the **real `show`** inside an
`egui_kittest::Harness` with a fixed `RaceConfig` and **no pointer input** →
every widget renders at rest, deterministically, through the actual layout.
This tests the true composed layout and avoids maintaining a parallel
manual-layout `paint` path that could silently drift from `show` — a real risk
for production screen code (the galleries accept a separate path only because
they are throwaway test specimens forcing hover/press/disabled states the screen
never needs). Harness setup mirrors `game_gallery.rs`'s **structure** —
CPU-device assert, frame-1-install-fonts / frame-2-draw flag,
`with_pixels_per_point(1.0)`, `with_theme(Light)`, `run_steps(1)`, `render()`,
`SnapshotOptions::new().threshold(1.0).failed_pixel_count_threshold(0)`, snapshot
key `setup_screen` — **except the canvas size**. `game_gallery` uses
`with_size(640×420)` `[measured: rg 'CANVAS_RECT|with_size'
crates/render/src/widgets/game_gallery.rs → CANVAS_RECT max Pos2::new(640.0,
420.0); .with_size(CANVAS_RECT.size())]`, but the stacked SetupScreen (48px top
pad + wordmark + `SPACE_8` + card{header + 2 steppers + difficulty + slider} +
`SPACE_6` + `lg` button + `SPACE_3` + footer + 48px bottom pad) is taller than
420 and would clip. The `SetupScreen` golden uses an **explicit** canvas sized to
fit the whole screen — **start `640.0 × 760.0`** (width holds the 560 content
column + `SPACE_6` side pads; height fits the full vertical stack) and confirm at
mint that nothing clips, raising the height if the footer is cut — **not**
`game_gallery`'s 640×420 `[derived → the minted `setup_screen.png` at subtask 3;
if the footer clips, enlarge the canvas height and re-mint]`.
`#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under
Miri)")]` per the golden discipline (spec § Golden test discipline; AGENTS.md —
a red workspace Miri blocks merge).

### Interaction test — closing the `show`-plumbing gap (AC6)

The golden supplies **no pointer input**, so it renders the rest state and never
exercises (a) the `generated` flag's `.clicked()` wiring or (b) the value
plumbing (`StepperResponse.value` / `SliderResponse.value` /
`SegmentedControlResponse.selected` → `Difficulty::from_index` → `assemble` →
`resp.config`) — a bug in either would pass every other test. A dedicated
`egui_kittest` interaction test (subtask 4) closes this gap; the feasibility was
investigated before committing to it.

**Kittest-driveability (investigated).** AccessKit label-querying is **not**
available: every widget draws its label with `painter.text` / `paint_form_label`
and none calls `Response::widget_info`, so no accessible node carries "Generate
track" and `Harness::get_by_label(..)`/`Node::click()` cannot target the button
`[measured: rg 'accesskit|widget_info' crates/render/src/widgets/ → no
widget_info/accesskit registration; labels are painter-drawn]`. **Position-based**
pointer injection **is** available and needs no AccessKit tree:
`Harness::hover_at(pos)`, `drag_at(pos)` (PointerButton pressed) and
`drop_at(pos)` (PointerButton released + PointerGone) inject raw events at a
screen point `[measured: rg 'pub fn (hover_at|drag_at|drop_at|input_mut)'
~/.cargo/.../egui_kittest-0.35.0/src/lib.rs → hover_at/drag_at/drop_at take
egui::Pos2; input_mut() -> &mut RawInput]`. Deterministic targeting comes from
the button's own rect: `SetupResponse.response.rect.center()` (§ *Emission
mechanism* exposes the button `Response`), so the test clicks exactly there
rather than hand-computing coordinates.

**Test flow (default, non-wgpu harness — no `render()`, no Vulkan `dlopen`).**
Build the `SetupScreen` with a fixed config in a `build_ui` closure that captures
each frame's `SetupResponse` into a shared cell; install fonts frame 1, draw
frame 2. (1) **Rest frame:** assert `generated == false` (AC6 "emits nothing
until pressed") and `resp.config == fixed_config` (the value plumbing round-trips
the widget values through the real `show`/`assemble`). (2) **Click:** `hover_at`
→ `drag_at` → `drop_at` at `resp.response.rect.center()` across frames, then
assert `generated == true` on the release frame and `resp.config` still equals
the fixed config. **Miri:** the test never calls `render()`, so it does not
`dlopen` the Vulkan ICD (the golden's abort cause is absent here); whether egui's
CPU tessellation path still aborts under Miri is confirmed at implementation and
the test is `#[cfg_attr(miri, ignore)]`d **iff** it does `[derived →
MIRIFLAGS=-Zmiri-tree-borrows cargo miri test -p gp-render setup at
implementation — gate iff it aborts]`.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | `screens/mod.rs`: `Difficulty` enum (FORCED `const fn` `temperature`/`label`/`from_index`/`to_index`, `DIFFICULTY_LABELS: [&str; 3]`), `RaceConfig` struct (+ `const fn temperature` delegate), module scaffold (`pub mod setup;`, `#[cfg(test)] mod setup_gallery;`), re-exports; wire `pub mod screens;` + `pub use` in `lib.rs`. Unit tests: AC3 temperature ordering (`Ace < Pro < Rookie`) + exact placeholder values; label↔index round-trip. | `crates/render/src/screens/mod.rs`, `crates/render/src/lib.rs` | — |
| 2 | `screens/setup.rs`: `SetupScreen` builder, `SetupResponse`, pure `assemble(cars: i32, laps: i32, v_target: f32, difficulty: Difficulty) -> RaceConfig` (defensive clamp + total conversions), `show` composing wordmark + `Card` + 2×`Stepper` + `SegmentedControl` + `Slider` + `Button` + footer per the spacing map. Unit tests: AC8a pure assemble (values → config), AC1/AC2/AC4 bound clamps, AC3 difficulty→temperature via assembled config. | `crates/render/src/screens/setup.rs` | 1 |
| 3 | `screens/setup_gallery.rs`: `#[cfg(test)]` wgpu golden driving `SetupScreen::show` in `egui_kittest` (fixed config, no input), explicit SetupScreen canvas (start 640×760, confirm at mint), Miri-ignored, snapshot `setup_screen`. Mint `setup_screen.png` (implementor mints + spawns `image-check`). | `crates/render/src/screens/setup_gallery.rs`, `crates/render/tests/snapshots/setup_screen.png` | 2 |
| 4 | `screens/setup_gallery.rs`: `egui_kittest` **interaction** test driving `SetupScreen::show` with injected pointer events (default non-wgpu harness, no snapshot, no `render()`). Rest frame → assert `generated == false` + `resp.config == fixed`; click at `resp.response.rect.center()` → assert `generated == true` + config unchanged. Covers **AC6** (`generated` flag + `StepperResponse`/`SliderResponse`/`SegmentedControlResponse` → `from_index` → `assemble` → `resp.config` plumbing). `#[cfg_attr(miri, ignore)]`d iff CPU tessellation aborts (verify at impl). | `crates/render/src/screens/setup_gallery.rs` | 2, 3 |

M = 4, all code (`*.rs` + a minted `.png` asset produced by the golden code path).
Subtasks 3 and 4 share `setup_gallery.rs`; 4 is ordered after 3 (needs the file +
`SetupResponse.response` from subtask 2).

## Handoff plan

Per `.claude/skills/task/SKILL.md` Step 8 + `.claude/agents/design.md` §
handoff-grouping. Grouping is required for **every M ≥ 1** (a); groups are
capped at **≤ 10** consecutive subtasks (b); each boundary hands off to
`/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction
recovery (re-entry) (c); the terminal group is sized `1..=10` (d); each group is
**change-type-homogeneous** (e); same-change-type subtasks are clustered into
the **fewest groups** (f); groups are **marked** with implementor model + effort
(g); default max **4** groups (h).

All four subtasks are the **code** change-type (Rust `*.rs`, plus the golden's
minted `.png` produced by the golden code) — one homogeneous group, no
change-type switch, well under the size cap.

- **Handoff into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) —
  the entry into the first group is also a handoff boundary.
- **Group A** — **code** group → `code-writer` subagent (`model: sonnet`
  (sonnet-5), effort **`medium` (pinned in frontmatter)**), 1M-token window —
  subtasks **1, 2, 3, 4**. Terminal group (4 subtasks; within `1..=10`). No
  inline `model=`/effort override — `code-writer`'s frontmatter pins both. The
  `code-writer` mints the golden and spawns `image-check` at subtask 3 (memory:
  image-check is spawned by code-writer at mint, never in CI). The `design` /
  `design-review` / `self-review` Opus gates are unaffected by this marker.

One group total — within the default max of 4.

## Risks

- **`f32 → i32` cast for `v_target` in `assemble`.** `f32` has no `TryFrom<i32>`
  inverse; the snapped value must cast. Mitigation: clamp+round first
  (`v.round().clamp(3.0, 10.0)`), then `as i32` under a justified
  `#[allow(clippy::cast_possible_truncation, reason = "clamped to [3,10] and
  rounded — a small finite integer-valued f32")]` — the crate's established
  pattern (`icons.rs` carries the same class of justified cast allow).
  `cast_sign_loss` does not apply (target `i32` is signed). — `[derived → cargo
  clippy --workspace --all-targets -- -D warnings]`
- **`arithmetic_side_effects` (deny) on the layout/`assemble` float math.** Does
  not fire: it lints integer operators, not `f32` — `slider.rs::snap_clamp`
  already does `(clamped - min) / step` on `f32` with **no** allow `[measured:
  Read slider.rs:328-335 → f32 `-`,`/` operators, zero `#[allow]`]`. The `u32`/
  `i32` `try_from` conversions involve no arithmetic operator. — `[derived →
  clippy gate]`
- **`missing_const_for_fn` (deny) mis-fires / mis-declines.** The pure enum/
  config accessors are FORCED `const fn`; `assemble` and `SetupScreen::show`
  are NOT const-eligible (`try_from`, `f32` methods, `egui::Ui` mutation). Getting
  const-ness wrong on any is a hard clippy error. Mitigation: mark each per the
  Approach; the gate discharges it. — `[derived → clippy gate]`
- **File size.** `setup.rs` (builder + `assemble` + wordmark/layout `show` +
  unit tests) risks the soft 500/800 limit. The golden lives in its **own**
  `setup_gallery.rs` (subtask 3) — the same golden-in-own-file split the widget
  galleries use — keeping `setup.rs`'s production+unit-test size down.
  Mitigation: if `setup.rs` still crosses soft-500 excl. tests, extract the
  wordmark/footer text drawing into a private `header`/`chrome` helper. —
  `[derived → wc -l crates/render/src/screens/setup.rs at implementation]`
- **Golden nondeterminism from live pointer state.** `show` reads hover/press;
  a stray pointer would render the button non-rest. Mitigation: the harness
  supplies no input, so `response.hovered()`/`is_pointer_button_down_on()` are
  `false` → rest state, deterministic (same guarantee the galleries rely on). —
  `[derived → the minted golden + image-check verification at subtask 3]`
- **Slider width inside the card.** `Slider::show` uses `ui.available_width()`
  `[measured: Read slider.rs:262 → let width = ui.available_width()]`; an
  unbounded parent would make it stretch unpredictably. Mitigation: the screen
  constrains the content column to `CONTENT_MAX_W` before the card, so available
  width is bounded and the golden is stable. — `[derived → golden at subtask 3]`
- **Interaction-test click targeting (subtask 4).** No AccessKit label is emitted
  by the painter-drawn button, so a label query cannot find it; the test clicks
  `resp.response.rect.center()` from the captured rest-frame `SetupResponse`
  instead. This is deterministic because the layout is input-independent (the
  rest frame that produced the rect had no pointer input), and egui's
  `Response::clicked()` fires on a press-then-release both landing on the button
  rect — exactly what `hover_at`→`drag_at`→`drop_at` at the center produce.
  Mitigation if a single-frame press+release does not register a click: split
  press and release across `run_steps` frames (the standard egui_kittest
  drag/drop choreography). — `[derived → the interaction test at subtask 4]`
- **Interaction test under Miri.** The test drives egui layout/tessellation but
  never `render()`s, so it does not `dlopen` the Vulkan ICD; if egui's CPU
  tessellation nonetheless aborts under Miri it is `#[cfg_attr(miri, ignore)]`d,
  keeping the workspace Miri job green (a red Miri blocks merge). —
  `[derived → MIRIFLAGS=-Zmiri-tree-borrows cargo miri test -p gp-render setup at
  implementation]`

## Test Design

**Subtask 1 — `screens/mod.rs` `#[cfg(test)] mod tests`** (Miri-clean; no egui,
no wgpu):
- Entry points: `Difficulty::temperature`, `::label`, `::from_index`,
  `RaceConfig::temperature`.
- Scenarios: AC3 ordering `Ace.temperature() < Pro.temperature() <
  Rookie.temperature()`; exact placeholder values (1.5 / 1.0 / 0.6) via
  `test_util::assert_f32`; `label`↔`from_index` round-trip for all three;
  `from_index(3)` / `from_index(usize::MAX)` → `None` (total, non-panicking);
  `DIFFICULTY_LABELS` equals `["Rookie","Pro","Ace"]` (drives the
  `SegmentedControl` options + AC3's "exactly Rookie/Pro/Ace"); **drift guard** —
  `DIFFICULTY_LABELS[v.to_index()] == v.label()` for every variant (design-review
  recommendation: pins the array to `label()` so the two cannot silently
  diverge). — `[derived → cargo test -p gp-render screens::]`

**Subtask 2 — `screens/setup.rs` `#[cfg(test)] mod tests`** (Miri-clean; pure
`assemble`, no egui):
- Entry point: `assemble(cars, laps, v_target, difficulty)`.
- Scenarios (AC8a / AC1 / AC2 / AC4): happy path — mid-range widget values
  map through to `RaceConfig` with matching `cars`/`laps`/`v_target` and
  `config.difficulty.temperature()` == the expected placeholder; **AC1** cars
  below 2 / above 6 clamp to `[2, 6]`; **AC2** laps below 1 / above 9 clamp to
  `[1, 9]`; **AC4** `v_target` `2.4`→`3`, `10.6`→`10`, `7.0`→`7` (round + clamp
  to `[3, 10]`, integer result); type-conversion totality — no panic at the
  bounds. — `[derived → cargo test -p gp-render screens::setup]`

**Subtask 3 — `screens/setup_gallery.rs` `#[cfg(test)]`** (wgpu; Miri-ignored):
- Entry point: `SetupScreen::new(fixed_config).show(ui)` inside
  `egui_kittest::Harness`.
- Scenario (AC8b / AC5 / AC7): one wgpu frame renders the whole screen — accent
  dot + two-tone `GRAPHITE GP` wordmark (display face, `GP` in `ACCENT`) + mono
  subtitle + the gridded card with the four inputs + primary button + footer —
  and matches the minted `setup_screen.png` exactly (flat regions; AA edges
  exempt via `threshold(1.0)` + `failed_pixel_count_threshold(0)`, the
  `widget_gallery` precedent). Fixed config e.g. `{ cars: 4, laps: 3, v_target:
  5, difficulty: Pro }`.
- Fixtures: `crate::fonts::definitions()` installed in the frame-1 closure;
  `WgpuTestRenderer` on a CPU device (assert as in `game_gallery.rs`). The
  minted PNG is verified by an `image-check` spawn (code-writer, at mint). —
  `[derived → cargo test -p gp-render setup_gallery + image-check]`

**Subtask 4 — `screens/setup_gallery.rs` `egui_kittest` interaction test**
(default non-wgpu harness; no snapshot, no `render()`):
- Entry point: `SetupScreen::new(fixed_config).show(ui)` captured into a shared
  cell across frames, plus `Harness::hover_at`/`drag_at`/`drop_at` pointer
  injection.
- Scenarios (**AC6**, the gap the golden cannot reach): **rest frame** — no input
  → `resp.generated == false` (AC6 "emits nothing until pressed") and
  `resp.config == fixed_config` (value plumbing round-trips widget values through
  the real `show`/`assemble`); **click frame** — `hover_at` → `drag_at` →
  `drop_at` at `resp.response.rect.center()` → `resp.generated == true` on the
  release frame and `resp.config` still equals `fixed_config` (a stray widget
  edit would be caught).
- Fixtures: `crate::fonts::definitions()` installed frame 1 (same
  frame-1-install / frame-2-draw flag as the golden); a `Cell`/`RefCell` holding
  the latest `SetupResponse` fields the closure needs to surface. No wgpu render
  state — the test asserts on the returned struct, not pixels. `#[cfg_attr(miri,
  ignore)]` applied **iff** egui's CPU tessellation aborts under Miri (the golden
  aborts only because it `render()`s; this test does not). — `[derived → cargo
  test -p gp-render setup_gallery; MIRIFLAGS=-Zmiri-tree-borrows cargo miri test
  -p gp-render setup at implementation]`

No integration (`tests/`) file is added — the golden is in-crate `#[cfg(test)]`
for consistency with `game_gallery`/`forms_gallery` and to share the
`tests/snapshots/` sink.

## Open questions

- **Vendor a `shuffle` icon for the Generate button?** The reference has a
  leading shuffle glyph; the vendored set lacks it and no AC requires it, so the
  button ships text-only. If the product owner wants the icon, it is a small
  follow-up (vendor `shuffle.svg` + `Icon::Shuffle` + the icons.rs test-pin
  updates + bake a `TextureHandle` in the screen/harness).
- **Final `Rookie / Pro / Ace` temperature values** — placeholders (1.5 / 1.0 /
  0.6) now; the real values are empirical, set once `gp-ai` (Block 4) exists
  (spec Deferred / Open questions). The `Difficulty::temperature` mapping is the
  single edit site when they are tuned.
- **`RaceConfig` / `Difficulty` home once downstream consumers land** — sited in
  `gp-render` for this emit-only issue; the product owner may prefer a shared
  location once `gp-game`/`gp-gen`/`gp-ai` consume it (spec Open questions).

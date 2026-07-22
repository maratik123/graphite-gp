# Design: Reuse a single Galley for text shaping in gp-render

**Issue:** #96
**Date:** 2026-07-22

## Approach

### The exact `Painter::text` equivalence (the whole correctness argument)

`egui-0.35`'s `Painter::text` is a three-line function
`[measured: read ~/.cargo/.../egui-0.35.0/src/painter.rs:469-481]`:

```rust
pub fn text(&self, pos, anchor: Align2, text, font_id, text_color) -> Rect {
    let galley = self.layout_no_wrap(text.to_string(), font_id, text_color); // shape
    let rect = anchor.anchor_size(pos, galley.size());                       // anchor
    self.galley(rect.min, galley, text_color);                              // draw at top-left
    rect                                                                     // return anchored rect
}
```

So the byte-identical hand-rolled form of any current `painter.text(pos, anchor, text, font, color)` call whose galley was already built via `layout_no_wrap(text, font, color)` is:

```rust
let rect = anchor.anchor_size(pos, galley.size()); // Align2::anchor_size is pub [measured: emath-0.35.0/src/align.rs:220]
painter.galley(rect.min, galley, color);           // draws galley top-left at rect.min
// `rect` is exactly what painter.text returned
```

Two facts make this a *pure factoring* with no pixel change:

- `Galley::size() == self.rect.size()` `[measured: epaint-0.35.0/src/text/text_layout_types.rs:1010]`, so the crate's current width/height reads (`galley.rect.width()` / `.rect.height()`) are identical to `galley.size().x` / `.y`. Anchoring off `galley.size()` reproduces `Painter::text` exactly.
- `Painter::galley(rect.min, galley, color)` draws the galley's *baked* glyph colors (the `color` arg only fills `Color32::PLACEHOLDER` sections, of which a `layout_no_wrap`-built plain run has none). Passing the **same** color the galley was baked with is therefore identical to `Painter::text`.

**The two shared draw helpers** live in a new crate-root module `crate::text` (rationale below):

```rust
/// Static-color sites: mirrors Painter::text exactly (fallback path).
pub(crate) fn paint_galley(painter, pos: Pos2, anchor: Align2, galley: Arc<Galley>, color: Color32) -> Rect {
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}
/// Dynamic-color sites: overrides the galley's baked color at draw time.
pub(crate) fn paint_galley_override(painter, pos: Pos2, anchor: Align2, galley: Arc<Galley>, color: Color32) -> Rect {
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley_with_override_text_color(rect.min, galley, color);
    rect
}
```

**Why the override variant is also byte-identical.** For a *dynamic-color* run (button hover/press/disabled, per-segment selected fg) the final draw color is only known **after** the run must already have been shaped for allocation width. Width/glyph-geometry is color-independent, so the run is shaped once with a throwaway measure color and re-colored at draw via `galley_with_override_text_color`. The tessellator sets every **glyph** vertex to the override color when `override_text_color` is `Some`, and keeps the baked color (falling back only for `PLACEHOLDER`) when it is `None` `[measured: epaint-0.35.0/src/tessellator.rs:2073-2080]`. For a plain single-format run (all these runs), both paths therefore yield the same glyph-vertex colors at the same positions → identical mesh → identical pixels. Disabled-opacity is baked into the color we pass (`gamma_multiply` applied before the call, exactly as the current `painter.text(tint(fg))`), never via `TextShape::opacity_factor`, so both paths keep `opacity_factor == 1.0`.

### Shared helper vs per-widget inlining — decision: shared crate-root module

The draw operation "anchor a pre-built galley and return its rect" recurs at **~30 sites** across `widgets/`, `screens/`, and `app.rs`. That is far past the ≥3-site duplication threshold (AGENTS.md § Rules), so a single shared helper is mandated over per-site inlining. It cannot live in `widgets/common.rs`: `common` is a **private** `mod common;` inside `widgets` `[measured: crates/render/src/widgets/mod.rs:22]` and is unreachable from `screens`/`app` (the existing `setup.rs` `draw_mono_label` doc already records "screens cannot reach that widgets-private helper"). A new crate-root `mod text;` (private module, `pub(crate) fn` items) is visible crate-wide — reachable from `widgets/*`, `screens/*`, and `app.rs` as `crate::text::paint_galley[_override]`. This is a single crate, so the AGENTS.md shared-*crate* rule does not trigger; a shared *module* is the right unit.

For **building** galleys, each show-measured widget builds in `show` and threads the `Arc<Galley>` into `paint` (AC3). Where the identical build logic would repeat at **≥3 sites** (a widget's `show` plus multiple direct `Widget::paint` gallery-harness callers — telemetry: 5 sites; tag: 5 sites), extract a `pub(crate) fn <widget>_galleys(...) -> …` builder so `show`, `paint`'s callers, and the gallery agree by construction; inline the build at 1–2-site widgets.

### The three site shapes found by the survey

Survey command `[measured: rg -n 'layout_no_wrap' crates/render/src → 28 sites across 12 files]` plus per-file `.text(`/`.galley(` reads (all 12 files covered by decomposition subtasks 2–13). `Painter::galley` is currently called **nowhere** `[measured: grep 'painter.galley' crates/render/src → none]`.

**A. Static-color, measured in `show`, re-shaped in `paint`** — build once in `show`, thread `Arc<Galley>` into `paint`, draw via `paint_galley` (same color): `telemetry`, `badge`, `tag`, `car_chip`.

**B. Static-color, measured **and** drawn only in `paint`** — build once inside `paint`, no signature change: `lap_meter`.

**C. Dynamic-color, measured in `show`, re-shaped in `paint`** — build once in `show`, thread into `paint`, draw via `paint_galley_override` with the paint-time color: `button` (fg flips on hover/press for Danger; disabled tint), `switch` (disabled tint), `segmented_control` (per-segment selected fg, selection resolved post-allocation).

**D. Screen / app-shell inline free functions** — build once locally, allocate, reuse the handle in the same function: `setup::draw_wordmark`+`draw_footer`, `results::draw_header`, `lab::draw_header`, `app::draw_wordmark` (static, `paint_galley`); `app::nav_item` (dynamic active/disabled color → `paint_galley_override`).

**Explicitly NOT targets — already single-shape (draw-only, no separate measurement).** `stepper`, `slider`, `movepad`, `card`, `setup::draw_mono_label`, and `common::paint_form_label` draw via `painter.text` with **no** preceding `layout_no_wrap` `[measured: rg layout_no_wrap → none of stepper/slider/movepad/card]`. Their single internal shape already satisfies AC1; converting them would be pure churn with golden-drift risk. Leave them. `icon_button` renders no text.

### Gallery-harness callers of `paint` MUST be updated in lockstep (signature changes)

The golden harnesses call `Widget::paint(...)` **directly** with `&str`, so every `paint` signature that changes forces its caller updates in the **same** subtask `[measured: grep '<Widget>::paint(' crates/render/src]`:

| Widget (signature change) | Direct `::paint` callers to update |
|---|---|
| `telemetry` | `game_gallery.rs:72,87,94,109` (4) |
| `badge` | `gallery.rs:155` (1) |
| `tag` | `gallery.rs:167,178,189,200` (4) |
| `car_chip` | `game_gallery.rs:161` (1) |
| `button` | `gallery.rs:74,93` (2) |
| `switch` | `forms_gallery.rs:80` (1) |
| `segmented_control` | `forms_gallery.rs:102` (1) + `segment_widths` at `:96` |
| `lap_meter` | **none** — internal-only fix, signature unchanged (`game_gallery.rs:130` unaffected) |

### Per-site anchor + downstream-rect table (the golden-drift surface)

Each row: the run, its `Align2`, the draw pos, and any downstream rect read. All rects come from the `paint_galley[_override]` return (`anchor.anchor_size(pos, galley.size())`), identical to today's `painter.text` return.

| Site | Run(s) | Align2 | Downstream rect read |
|---|---|---|---|
| `telemetry::paint` | label / value / unit | LEFT_TOP or RIGHT_TOP (label,value); LEFT/RIGHT_BOTTOM (unit) | `label_rect.max.y`→value_top; `value_rect.max.x`/`.min.x`→unit_x; `value_rect.max.y`→unit_y |
| `badge::paint` | label | CENTER_CENTER | none |
| `tag::paint` | label | LEFT_CENTER | none |
| `car_chip::paint` | name / pill | LEFT_CENTER (name); CENTER_CENTER (pill) | `name` width→cursor; `pill` width→pill_rect |
| `lap_meter::paint` | label / done / total | LEFT_TOP | widths→`readout_left`, `done_width`→total pos (all already local) |
| `button::paint` | label | LEFT_CENTER (override) | returned `rect.size().x`→icon_right x |
| `switch::paint` | label | LEFT_CENTER (override) | none |
| `segmented_control::paint` | per-segment label | CENTER_CENTER (override; via the `clipped` painter, not base) | widths→`seg_rect_at` |
| `setup::draw_wordmark` | "GRAPHITE " / "GP" / subtitle | LEFT_TOP | `graphite_rect.max.x`→"GP" pos |
| `setup::draw_footer` | footer | LEFT_TOP | none |
| `results::draw_header` | eyebrow / prefix / suffix | LEFT_TOP | `prefix_rect.max.x`→suffix pos |
| `lab::draw_header` | "Track lab" | LEFT_CENTER | none |
| `app::draw_wordmark` | "GRAPHITE " / "GP" | LEFT_TOP | `graphite_rect.max.x`→"GP" pos |
| `app::nav_item` | label | CENTER_CENTER (override) | none |

### Notes on specific threads

- **`telemetry`**: today shapes label **4×**, value **4×** (each measured for width *and* height separately in `show`, then re-shaped in `paint`) and unit 2× `[measured: telemetry.rs layout_no_wrap at :271,:279,:285,:292,:296 + painter.text at :197,:210,:230]`. `show` builds label (`.to_uppercase()` once, not thrice)/value/unit galleys once; `paint` reads width **and** height off the same handles and draws them. `paint` keeps `style` (to pass the baked color as the identical fallback) + `align`.
- **`car_chip`**: build the pill galley in `show` with its **final** `tag.fg` (available via `style.tag` in `show`), so `paint` draws it plainly with `paint_galley` — no override needed. `name` is `TEXT_INK` (static) at both measure and draw. `rank` stays a draw-only `painter.text` (not measured — leave it).
- **`segmented_control`**: replace `pub(crate) fn segment_widths(...) -> Vec<f32>` with `pub(crate) fn segment_galleys(painter, options, size) -> Vec<Arc<Galley>>`; `paint` takes `&[Arc<Galley>]` and draws each centered via `paint_galley_override(style.fg)`; padded segment width is `SEG_PAD_X.mul_add(2.0, galley.size().x)`. `show` and `forms_gallery::draw_segmented_controls` each build galleys once and thread them into `paint` (removing their independent `segment_widths` call). Old `segment_widths` is removed (clean break — AGENTS.md § API Stability). **Painter to pass:** the per-segment label draw currently goes through `clipped = painter.with_clip_rect(rect)`, NOT the base `painter` (`segmented_control.rs:117,171`). `paint_galley_override(...)` MUST receive that `clipped` painter, mirroring current behavior — do **not** pass the base `painter`. (Byte-identity holds either way here since each label sits inside `seg_rect ⊂ rect`, but the implementer must preserve the clip-painter call to keep the factoring pure.)
- **`button`**: `show` builds the label galley with `metrics.fg` for width; `paint` draws via `paint_galley_override(tint(style.fg))` and takes `rect.size().x` off the returned rect for `icon_right`.

### Rejected alternatives

- **Helper in `widgets/common.rs`** — rejected: private module, unreachable from `screens`/`app`; would force duplication or a re-export hack.
- **Keep `paint` taking `&str`, shape once *inside* `paint`** — rejected: `show` still needs the width to allocate, so it would shape a second time; that is exactly the "run already built earlier in the same draw" AC1 forbids. Threading the handle is required (and AC3-mandated).
- **`egui::text::LayoutJob` two-tone for the wordmarks** — out of scope; the sequential-`text` idiom is preserved verbatim, only its shaping is deduplicated.
- **Universal override at every site** — rejected: static sites use `paint_galley` (fallback) to mirror `Painter::text` on the most obviously-correct path; override is reserved for the genuinely dynamic-color runs.

## Decomposition

All subtasks are behavior-preserving; each widget subtask includes its gallery-harness caller updates. Every subtask 2–13 depends on the shared helper (subtask 1) and is otherwise independent.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `crate::text` module: `paint_galley` + `paint_galley_override` (+ Miri-gated rect-equivalence unit test); declare `mod text;` in `lib.rs` | `crates/render/src/text.rs` (new), `crates/render/src/lib.rs` | — |
| 2 | `telemetry`: `pub(crate) fn telemetry_galleys` built in `show`; `paint` takes 3 galley handles; reads w+h off them; draw via `paint_galley`; update 4 `game_gallery` callers | `widgets/telemetry.rs`, `widgets/game_gallery.rs` | 1 |
| 3 | `badge`: `show` builds label galley; `paint` takes it; draw via `paint_galley`; update `gallery.rs` caller | `widgets/badge.rs`, `widgets/gallery.rs` | 1 |
| 4 | `tag`: `pub(crate) fn tag_label_galley` built in `show`; `paint` takes it; draw via `paint_galley`; update 4 `gallery.rs` callers | `widgets/tag.rs`, `widgets/gallery.rs` | 1 |
| 5 | `car_chip`: `show` builds name (`TEXT_INK`) + pill (`tag.fg`) galleys; `paint` takes them; draw via `paint_galley`; update `game_gallery` caller | `widgets/car_chip.rs`, `widgets/game_gallery.rs` | 1 |
| 6 | `lap_meter`: build label/done/total galleys **once inside `paint`**, read widths, draw via `paint_galley`; **no** signature change | `widgets/lap_meter.rs` | 1 |
| 7 | `button`: `show` builds label galley (`metrics.fg`); `paint` draws via `paint_galley_override(tint(style.fg))`, reads returned rect for `icon_right`; update 2 `gallery.rs` callers | `widgets/button.rs`, `widgets/gallery.rs` | 1 |
| 8 | `switch`: `show` builds label galley; `paint` draws via `paint_galley_override(tint(TEXT_BODY))`; update `forms_gallery` caller | `widgets/switch.rs`, `widgets/forms_gallery.rs` | 1 |
| 9 | `segmented_control`: `segment_widths`→`segment_galleys`; `paint` takes `&[Arc<Galley>]`, draws via `paint_galley_override(style.fg)`; update `show` + `forms_gallery` | `widgets/segmented_control.rs`, `widgets/forms_gallery.rs` | 1 |
| 10 | `setup` screen: `draw_wordmark` (GRAPHITE/GP/subtitle) + `draw_footer` build-once + `paint_galley` | `screens/setup.rs` | 1 |
| 11 | `results` screen: `draw_header` (eyebrow/prefix/suffix) build-once + `paint_galley` | `screens/results.rs` | 1 |
| 12 | `lab` screen: `draw_header` ("Track lab") build-once + `paint_galley` | `screens/lab.rs` | 1 |
| 13 | `app` shell: `draw_wordmark` (GRAPHITE/GP) build-once + `paint_galley`; `nav_item` build-once + `paint_galley_override(fg.gamma_multiply(opacity))` | `app.rs` | 1 |

Scope: 13 subtasks (< 15). No split needed.

## Handoff plan

Per `.claude/skills/task/SKILL.md` Step 8 and `.claude/agents/design.md` § Rules (handoff-grouping). All 13 subtasks are the **code** change-type (`*.rs`) → every group routes to `code-writer` (`model: sonnet`, effort `medium` pinned in frontmatter — no inline override), 1M-token window. `M = 13`, so **grouping is required** (M ≥ 1). Max group size is **10**; 13 > 10 forces exactly **2** groups, which is the minimum possible (a change-type switch or dependency chain never forces more here — subtask 1 leads Group A and every dependent follows it in-order). 2 groups ≤ the default max of 4. A `/context-reset` handoff is spawned at the **start of every group**, including the first.

- **Handoff into Group A:** spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
- **Group A** — `code-writer`, model `sonnet`, effort `medium` (pinned), 1M-token window — subtasks **1–10** (10 subtasks = size cap; homogeneous code). Subtask 1 (the shared helper) is first, so subtasks 2–10 have their sole dependency satisfied in-group and in-order.
- **Handoff after Group A:** spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). Parent `/task` resumes in Group B with fresh context.
- **Group B** — `code-writer`, model `sonnet`, effort `medium` (pinned), 1M-token window — subtasks **11–13** (screens/app; each depends only on subtask 1, completed in Group A). Terminal group (3 subtasks; within `1..=10`).

## Risks

- **Hand-rolled anchoring drifts from `Painter::text` → golden pixel diff (primary risk).** Mitigation: the helper *is* `Painter::text`'s exact body minus the shaping (`anchor.anchor_size(pos, galley.size())` then `galley(rect.min, …)`), verified line-by-line — `[measured: painter.rs:469-481; Galley::size==rect.size epaint text_layout_types.rs:1010; anchor_size pub emath align.rs:220]`. Discharged by every affected exact-compare golden — `[derived → AC4: cargo test -p gp-render, no snapshot regeneration]`.
- **Override path ≠ fallback path pixels for dynamic-color runs (button/switch/segmented_control/nav_item).** Mitigation: tessellator sets glyph vertices to the override color, identical to a baked-then-fallback run for a plain single-format run; positions are color-independent — `[measured: epaint-0.35.0/src/tessellator.rs:2073-2080]`. Discharged by the button/switch/segmented/nav goldens — `[derived → AC4]`.
- **A changed `paint` signature leaves a gallery-harness caller un-updated → build break.** Mitigation: each widget subtask lists its exact `::paint` callers (table above, from `[measured: grep '<Widget>::paint(' crates/render/src]`); folded into the same subtask so the crate always compiles. Discharged by `[derived → AC6: cargo build / clippy --workspace --all-targets]`.
- **`-D warnings` masks later same-class sites** (e.g. a newly-unused `segment_widths`, a stale import, or a fresh `clippy::too_many_arguments` after adding a galley param). Mitigation: after subtasks land, re-run `cargo clippy --workspace --all-targets -- -D warnings` and surface any newly-revealed class — `[derived → AC6]`. Note: `telemetry`/`car_chip` `paint` keep their existing arg counts (galley params replace `&str` params) so their existing `#[allow(clippy::too_many_arguments, …)]` still applies unchanged.
- **Miri red from a new Context-constructing test** (subtask 1's rect-equivalence test builds an `egui::Context` + installs fonts). Mitigation: gate it `#[cfg_attr(miri, ignore = "constructs egui::Context to lay out real galleys — interpreted wall-clock cost, no UB signal")]` per AGENTS.md § Rust Test Conventions (gp-render Context/painter gate) — `[derived → workspace Miri job]`.
- **No behavioral edit to any `resolve` layer.** All `resolve`/`ink_colors`/pure-fn unit tests are untouched — `[derived → AC5: existing resolve tests pass unchanged]`.

## Test Design

**No golden is minted, regenerated, or re-thresholded.** This is behavior-preserving; the existing exact-compare wgpu goldens (`widget_gallery`, `forms_gallery`, `game_gallery`, `setup_screen`, `results_screen`, `lab_screen`, `race_screen`, `app_shell`, `app_shell_race`, `app_shell_lab` — `[measured: ls crates/render/tests/snapshots/*.png]`) are the safety net and must pass **byte-identical** (AC4). The design-agent "text-golden threshold" rule does not apply — no new text golden is created; existing thresholds are untouched.

New test (subtask 1 only):
- **Location:** `crates/render/src/text.rs` `#[cfg(test)] mod tests`.
- **Entry point:** `paint_galley` / `paint_galley_override`.
- **Scenario:** for each of `LEFT_TOP`, `RIGHT_TOP`, `LEFT_CENTER`, `CENTER_CENTER`, `LEFT_BOTTOM`, `RIGHT_BOTTOM` at a fixed `pos`, assert the `Rect` returned by `paint_galley(painter, pos, anchor, galley.clone(), color)` **equals** the `Rect` returned by `painter.text(pos, anchor, text, font, color)` for the same text/font/color (regression guard on the anchor math; the galleys are the same run). This is an exact `Rect` `assert_eq!` (`Rect: PartialEq`), not an image snapshot.
- **Fixtures:** an `egui::Context` with `crate::fonts::definitions()` installed to produce real galleys via `layout_no_wrap`; a `Painter` from a background `LayerId`.
- **Miri gate:** `#[cfg_attr(miri, ignore = "constructs egui::Context to lay out real galleys — interpreted wall-clock cost, no UB signal")]` (mechanical trigger: constructs a `Context`/painter — AGENTS.md § Rust Test Conventions).

All existing `resolve`/pure-fn unit tests across the touched files remain unchanged (AC5) — the refactor edits only `show`/`paint` draw paths and `paint` signatures, never a `resolve` body.

## Open questions

- None. Q1 (scope) resolved to whole `gp-render` in the spec; the survey confirms the target set and the gallery-harness caller updates required by the signature changes.

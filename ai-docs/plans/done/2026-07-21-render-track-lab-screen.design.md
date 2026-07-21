# Design: gp-render Track lab screen (LabScreen)

**Issue:** #20
**Date:** 2026-07-21

## Approach

Port `Screens.jsx`'s `LabScreen` as the fourth full screen in `gp-render`,
following the exact draw-only / caller-supplies-data idiom `SetupScreen`
established (`crates/render/src/screens/setup.rs`)
`[measured: read setup.rs → SetupScreen{config} builder + show(ui)->SetupResponse{response,config,generated}]`.
No `gp-gen`/`gp-ai` dependency; compose only existing building blocks. **One
small `gp-core` change** (round-2 spec amendment): add a `width_min: u32` field
to `TrackArtifact` and an S/F-width getter — the four oracle-report tiles are now
sourced directly off the `TrackArtifact` the screen already holds, with **no**
gp-render-local report struct (spec Key decision, round-2). `TrackMetrics` is
unchanged.

The screen is a `Copy` builder holding its per-frame draw data by value/ref,
with a single `show(ui) -> LabResponse` entry that installs an `Order::Middle`
layer (required before any `Card` chrome — `Card::show` paints its fill on
`LayerId::background()`
`[measured: read card.rs:250-253 → layer_painter(LayerId::background())]`,
so the caller layer must outrank Background, exactly as `SetupScreen::show`
documents `[measured: setup.rs:92-99]`), lays out two columns, draws them, and
returns which action buttons fired this frame.

### Key decisions (shapes the spec delegated to design)

**1. Oracle-report tiles — sourced off the `TrackArtifact`, NO local struct**
(round-2 amendment; reverses the round-1 `OracleReport` struct, which is
**dropped entirely**). The screen already holds a `&TrackArtifact` for the canvas
(AC1/AC6), so all four tiles read straight off it — the amendment makes
propagation automatic (once Block-1 fills the artifact, the screen shows the real
values with zero screen-side change):

| Tile | Source on `&TrackArtifact` | Type → display |
|---|---|---|
| Vmax | `track.metrics.vmax_attain` | `Option<i32>` → `None` renders `"—"` |
| Tempo | `track.metrics.tempo` | `Option<f32>` → `None` renders `"—"` (else `"{:.2}"`, Telemetry Accent tone) |
| Width min | `track.width_min` (new field) | `u32` → **always** a real number, never `"—"` |
| S/F width | `track.sf.width()` (new getter) | `usize` → **always** a real number, never `"—"` |

`[measured: core/src/track.rs:304-306 → vmax_attain: Option<i32>, tempo: Option<f32>]`.
The tile strings are produced by one pure, Context-free free function in `lab.rs`,
`fn oracle_tile_strings(track: &TrackArtifact) -> [String; 4]` (order: Vmax,
Tempo, Width min, S/F width), which `show` calls to fill the four `Telemetry`
tiles and AC2 calls directly. The `None → "—"` path applies to **Vmax and Tempo
only**; Width min / S/F width always format a real number via `.to_string()`.
Helpers: `const PLACEHOLDER: &str = "—";` (em-dash); Vmax → `map_or(PLACEHOLDER
.to_owned(), |v| v.to_string())`; Tempo → `map_or(PLACEHOLDER.to_owned(), |v|
format!("{v:.2}"))`. Not `const fn` (`String`/`format!` allocate — `missing_const
_for_fn` does not fire) `[derived → cargo clippy --workspace --all-targets -D
warnings]`. Rejected: re-introducing a local report struct (the amendment
explicitly removes it — 3 of 4 values already live on the artifact, and copying
them into a struct would break the automatic-propagation goal).

**1a. gp-core change (a): `TrackArtifact.width_min: u32`.** Add exactly one
**non-`Option`, unsigned** field to `gp_core::track::TrackArtifact`
(`crates/core/src/track.rs:320`), with a one-line `///`. It is a Ф4
static-validation geometry output (`docs/design.md` §2; computed by #27, stored
at assembly by #34). Non-`Option` because a validated exported `TrackArtifact`
(§2, Ф7) always has a measured min width `≥ n = ⌈m/2⌉ ≥ 1` — no genuine "absent"
state; `u32` because it is a **count** of lattice points across a cross-section
(non-negative), matching the width-floor domain `GenParams::min_width()` /
`start_finish_width()` (both `u32`)
`[measured: grep gen/src/lib.rs → min_width/start_finish_width both -> u32]`. Do
**not** add it to `TrackMetrics` (that is §3 speed data; this is §2 geometry). It
is hand-populated with a concrete `u32` in fixtures until Block-1 generation is
real, exactly as `metrics` already is.

  `TrackArtifact` derives `Clone, Debug` with **no `Default`**
  `[measured: track.rs:319 → #[derive(Clone, Debug)]]`, so it is built with struct
  literals — a non-`Default` field means **every** `TrackArtifact { … }` literal
  MUST add `width_min: <u32>`. The full workspace enumeration
  `[measured: rg -Un 'TrackArtifact\s*\{' crates → 3 literal sites + 1 struct def
  + 1 todo! body]`:

  | Site | What it is | Action |
  |---|---|---|
  | `crates/core/src/track.rs:320` | the struct **definition** | ADD the `width_min: u32` field + `///` |
  | `crates/core/src/track.rs:610` | gp-core test `track_artifact_carries_all_eight_members` literal | add `width_min: <u32>` (e.g. `1`); test name → `…_all_nine_members` (now 9 members) |
  | `crates/render/src/track/mod.rs:159` | `fixture_track()` test-helper literal | add `width_min: <u32>` |
  | `crates/render/src/track/golden.rs:44` | `scene_track()` golden fixture literal | add `width_min: <u32>` (a value matching the AC6 golden, e.g. `3`) |

  **Non-literal (inherit `width_min` from a base literal — NO field edit):**
  `crates/render/src/track/mod.rs:242` `fixture_track_with_metrics()` and
  `crates/render/src/track/golden.rs:116` `scene_track_with_metrics()` both do
  `let mut track = <base>(); track.metrics = …` `[measured: mod.rs:242-243,
  golden.rs:116-119]` — they carry the base's `width_min` forward; they MAY
  reassign `track.width_min` if the golden wants a specific value, but need no
  literal change. **Non-construction:** `crates/gen/src/lib.rs:52` `generate` is
  a `todo!()` body, not a struct literal `[measured: gen/src/lib.rs:52-54]` — no
  edit. No `TrackArtifact` literal exists in `gp-game`/`gp-ai`
  `[measured: rg found only crates/{core,gen,render}]`.

**1b. gp-core change (b): S/F-width getter.** Add
`impl StartFinish { pub fn width(&self) -> usize { self.chord.len() } }` in
`crates/core/src/track.rs` (S/F width is a property of `StartFinish`, the natural
home; the chord is the `Vec<Point>` across the corridor
`[measured: track.rs:81-88 → StartFinish{chord: Vec<Point>, orient, gate}]`), with
a one-line `///`. **FORCED `const fn`?** — `Vec::len` is `const`-stable, and the
body is a pure accessor, so `clippy::missing_const_for_fn` (nursery = deny) will
require `pub const fn width`. It is NOT the `Rect::index` counter-example (no
`.then()`/conditionally-const call) `[measured: design.md § binding-constraint —
missing_const_for_fn forces const on const-eligible pure fns; Cargo.toml nursery
= deny]`. The screen and every test use `track.sf.width()`, **never** raw
`.sf.chord.len()`. AC7: a gp-core unit test asserts `width() == chord.len()`
including the **empty-chord `0`** case.

**2. Phase-status type** — `enum PhaseStatus { Ok, Repair }` plus a fixed
`[PhaseStatus; 7]` caller-supplied array; the phase **ids** and **names** are
`gp-render`-local const tables, since Ф1–Ф7 and their labels are fixed
generation-pipeline stages (`docs/design.md` §2), not per-frame data — only the
status varies `[measured: Screens.jsx:153-161 → 7 rows, ids Ф1..Ф7, fixed
names, per-row status ok/warn]`:

```
const PHASE_IDS:   [&str; 7] = ["Ф1", "Ф2", "Ф3", "Ф4", "Ф5", "Ф6", "Ф7"];
const PHASE_NAMES: [&str; 7] = ["Coarse ring (infield-first)", "Rasterize to points D",
    "Start / finish + grid", "Static validation", "Passability oracle",
    "Local repair", "Output artifact"];
```

`enum PhaseStatus { Ok, Repair }` derives `#[derive(Clone, Copy, Debug,
PartialEq, Eq)]`; its variants are unit, so `Eq` is safe here (`assert_eq!` in
the AC3 test relies on `PartialEq` + `Debug`). `Copy`/`Clone` are already forced
by `PhaseStatus` being a `[PhaseStatus; 7]` field of the `Copy` `LabScreen`, but
they are listed explicitly for parity with the other `Copy` types.

The `[PhaseStatus; 7]` array type-enforces AC3's "exactly 7 rows". A pure
`const fn phase_badge(PhaseStatus) -> (BadgeTone, &'static str)` maps
`Ok → (Tone::Ok, "✓")`, `Repair → (Tone::Warn, "repair")` (spec Key decision —
reusing existing `badge::Tone::{Ok,Warn}`
`[measured: badge.rs:11-23 → Tone{Neutral,Accent,Ok,Warn,Danger}]`). This is
**FORCED `const fn`** — a pure `match` over `Copy` values is const-eligible, so
`clippy::missing_const_for_fn` (nursery = deny) requires it, matching every
widget `resolve` in-crate `[measured: Cargo.toml workspace.lints.clippy →
nursery = deny; badge.rs:82 pub const fn resolve]`. Rejected: a
`Phase{id,name,status}` row list (duplicates fixed labels every frame; allows
≠7 rows — weaker than the array's type guarantee).

**3. `LabResponse` shape** — mirrors `SetupResponse` (bool flag + `Response`)
scaled to three buttons `[measured: setup.rs:53-63 → SetupResponse{response,
config, generated}]`:

```
pub struct LabResponse {
    pub regenerate: bool, pub test_lap: bool, pub menu: bool,
    pub regenerate_response: egui::Response,
    pub test_lap_response: egui::Response,
    pub menu_response: egui::Response,
}
```

Each `*_response` is the button's row `Response`; each bool is
`response.clicked()`. The three `Response`s are required by the AC4 interaction
test, which synthesizes clicks at each button's `rect.center()` — these are
hand-painted buttons with no AccessKit label to query, the exact reason
`SetupResponse` carries its `Response` `[measured: setup_gallery.rs:105-108,175
→ captured rest-frame resp.response.rect.center()]`. Not `Copy`/`Debug`
(`egui::Response` is neither), consistent with `SetupResponse`.

**4. Icon handling** — **caller-supplied `Option<&TextureHandle>`**, applied
conditionally to the `Button` builder. `Button::icon_left` takes a **non-`Option`**
`&TextureHandle` and sets the slot to `Some` internally `[measured: button.rs:117
→ pub const fn icon_left(mut self, icon: &'a TextureHandle) -> Self { self.icon_left
= Some(icon); … }]`, so a caller-supplied `Option` is **not** a direct
pass-through: the screen constructs `Button::new(label)` (whose default
`icon_left = None` renders a text-only button `[measured: button.rs:90-99,327]`)
and applies the icon only when the caller passes `Some`, e.g. `let mut btn =
Button::new("Regenerate").variant(Primary); if let Some(h) = self.regenerate_icon
{ btn = btn.icon_left(h); }`. Same pattern for the Test-lap button. This is the
load-bearing finding the spec's "reuse
… icons (shuffle/play/menu)" wording glossed over: **the vendored `Icon` set has
no `shuffle` and no `menu` glyph**
`[measured: ls crates/render/icons/ → grid-3x3, pause, play, settings,
zoom-in .svg only; icons.rs:41-54 → enum Icon{Play,Pause,Grid3x3,ZoomIn,Settings}]`.
Resolution per button:
  - **Menu**: text-only ghost/sm "← Menu" — the JSX Menu button has **no**
    `iconLeft` `[measured: Screens.jsx:169 → <Button variant="ghost" size="sm">←
    Menu</Button>]`, so no icon is needed regardless.
  - **Test lap**: `play` exists → caller passes `Some(icons.get(Icon::Play))`.
  - **Regenerate**: `shuffle` is absent. Rather than vendor a new SVG + widen
    the `Icon` enum (its own concern: new asset, byte-size pin test, Miri-bake
    behaviour — outside "port the screen"), the builder accepts an
    `Option<&TextureHandle>` and renders **text-only** when the caller passes
    `None`. gp-game can supply a shuffle handle in a later follow-up without any
    signature change.

  Icons are therefore per-frame texture data (like `CarRender`), carried as two
  `Copy` builder fields: `regenerate_icon`/`test_lap_icon: Option<&'a
  TextureHandle>`. Rejected: taking `&IconSet` (can only supply `Play`, not
  `shuffle`; forces every test to bake the full set — `IconSet::new` rasterizes
  `settings.svg`, which panics under Miri `[measured: icons.rs:336-348 →
  #[cfg_attr(miri, ignore)] on icon_set_bakes_all_five]`); vendoring `shuffle`
  now (scope creep beyond the screen port).

**5. `LabScreen` builder** — `Copy`, lifetime-parameterised, `derive(Clone,
Copy)` **only** (no `Debug` — holds `Option<&TextureHandle>`, and
`TextureHandle` has no `Debug`, exactly why `Button` omits it
`[measured: button.rs:66-67 → "Not Debug: egui::TextureHandle … has no Debug"]`):

```
pub struct LabScreen<'a> {
    track: &'a TrackArtifact,   // canvas fixture + all 4 oracle tiles (caller-supplied)
    phases: [PhaseStatus; 7],
    valid: bool,                // header Badge tone
    seed: i32,                  // header Tag "seed <N>"
    regenerate_icon: Option<&'a TextureHandle>,
    test_lap_icon: Option<&'a TextureHandle>,
}
```

No `report` field — the four oracle tiles read off `self.track` (Key decision 1).
`const fn new(track, phases, valid, seed)` + `const fn` setters for the
two icons (FORCED `const fn` per the same nursery lint — pure struct-literal
returns, matching `Button`'s setters `[measured: button.rs:102-141 → pub const
fn variant/size/icon_left…]`). The canvas draws with an **empty** car slice —
the JSX passes `cars={[]}` `[measured: Screens.jsx:172 → <TrackCanvas cars={[]}
…>]` — via `crate::render_frame(painter, canvas_rect, self.track, &[], false,
LAB_OVERLAYS)`.

**6. Canvas overlays const** — `const LAB_OVERLAYS: Overlays = Overlays {
speed_heatmap: true, fastest_lap: true, grid: true };` `[measured: Screens.jsx:172
→ showGrid showHeatmap showFastestLap all true]`. Exposed as a `pub(crate)`
const so AC1 can assert its fields directly (`Overlays` derives no `PartialEq`
`[measured: lib.rs:30 → derive(Clone, Copy, Debug, Default)]`, so the test
asserts the three bool fields, not `assert_eq`).

### Module / file layout

Mirrors `setup.rs` + `setup_gallery.rs` exactly:
- `crates/render/src/screens/lab.rs` — screen + `PhaseStatus` + `LabResponse` +
  const tables + `oracle_tile_strings` + pure logic + Context-free unit tests.
- `crates/render/src/screens/lab_gallery.rs` — `#[cfg(test)]`-only: the AC4
  interaction test + the AC6 wgpu golden (both Miri-gated).
- `crates/render/src/screens/mod.rs` — `pub mod lab;`, `#[cfg(test)] mod
  lab_gallery;`, `pub use lab::{LabScreen, LabResponse, PhaseStatus};`.
- `crates/render/src/lib.rs` — extend the crate-root re-export
  (`pub use screens::{Difficulty, RaceConfig …}`) with the three lab types, for
  discoverability parity with `RaceConfig` `[measured: lib.rs:26]`.
- `crates/core/src/track.rs` — the `width_min` field (§1a) + `StartFinish::width`
  getter + its unit test (§1b).

`PhaseStatus`/`LabResponse` live in `lab.rs` (single consumer = LabScreen),
**not** in `mod.rs` — `RaceConfig`/`Difficulty` sit in `mod.rs` only because
they are genuinely shared config `[measured: screens/mod.rs:1-11 doc]`; the lab
types are lab-only (YAGNI).

### Two-column layout algorithm (the load-bearing draw detail)

egui has no flexbox, so the "canvas fills remaining height" (JSX `flex: 1`
`[measured: Screens.jsx:171]`) is done with explicit rect math off
`ui.max_rect()` after the `Order::Middle` child install:
1. `full = ui.max_rect()`; inset by `PAD_OUTER (20)` on all sides.
2. Split horizontally: right column fixed `COL_RIGHT_W (320)` at the right,
   `COL_GAP (20)` gutter, left column = the remainder.
3. Left column (top→bottom, `COL_LEFT_GAP (14)` between bands): **header band**
   at top (fixed height = max control height in the row), **action band**
   reserved at the bottom (Button md height), **canvas band** = everything
   between. Draw the canvas border (rounded-rect stroke `BW_1 (1.5)`
   `color::GRAPHITE_900`, `RADIUS_2`, clip inside) then `render_frame` into the
   inset inner rect.
4. Right column: two `Card`s stacked with `COL_RIGHT_GAP (16)`.

Header/action bands are drawn via child `Ui`s (`ui.child_ui`/`allocate_ui_at_rect`
with the computed band rect) so the existing `Button`/`Badge`/`Tag`/`Telemetry`
`show(ui)` builders compose unchanged. The golden (AC6) is the visual check on
the exact geometry — the design fixes the algorithm, not every pixel.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **gp-core + fixtures.** Add `width_min: u32` field (+ `///`) to `TrackArtifact` (§1a) and add `width_min: <u32>` to **all three** literal sites: `core/src/track.rs:610` (rename test `…_all_eight_members → …_all_nine_members`), `render/src/track/mod.rs:159` (`fixture_track`), `render/src/track/golden.rs:44` (`scene_track`). Add `StartFinish::width` — FORCED `pub const fn width(&self) -> usize { self.chord.len() }` (+ `///`) (§1b) and its gp-core AC7 unit test (`width() == chord.len()`, incl. empty-chord `0`). Run `cargo build --workspace` to confirm every literal compiles. | `crates/core/src/track.rs`, `crates/render/src/track/mod.rs`, `crates/render/src/track/golden.rs` | — |
| 2 | `lab.rs` pure core: `PhaseStatus` + `const fn phase_badge`, `PHASE_IDS`/`PHASE_NAMES`, `oracle_tile_strings(&TrackArtifact) -> [String;4]` (+ `PLACEHOLDER`, Vmax/Tempo `Option` helpers), `LabResponse`, `LabScreen` builder (`new` + icon setters), `LAB_OVERLAYS`, module consts. Context-free unit tests: AC1 (overlays const fields), AC2 (`oracle_tile_strings` over a fixture: Vmax/Tempo `Some`/`None→"—"`, Width min / S/F width always a real number), AC3 (7 phases + `Ok→Ok`/`Repair→Warn` tones). | `crates/render/src/screens/lab.rs` | 1 |
| 3 | `LabScreen::show`: `Order::Middle` install, two-column rect layout, header (title/Badge/Tag/Menu), canvas border + `render_frame(LAB_OVERLAYS)`, action row (Regenerate/Test lap), right column (oracle 2×2 `Telemetry` fed by `oracle_tile_strings` + phases 7-row `Card`s); assemble `LabResponse`. | `crates/render/src/screens/lab.rs` | 2 |
| 4 | Module wiring: `pub mod lab;` + `#[cfg(test)] mod lab_gallery;` + `pub use lab::{LabScreen, LabResponse, PhaseStatus};` in `mod.rs`; crate-root re-export in `lib.rs`. | `crates/render/src/screens/mod.rs`, `crates/render/src/lib.rs` | 3 |
| 5 | `lab_gallery.rs`: AC4 interaction test (drive click on each of the 3 buttons via captured rects, assert flags flip; Miri-gated — `Harness::builder` `getcwd`), AC6 wgpu golden of the whole screen against a hand-built `TrackArtifact` fixture with hand-populated `TrackMetrics` **and** `width_min` (Miri-gated — wgpu `dlopen`). Mint `lab_screen.png`; spawn `image-check` — its spawn prompt MUST explicitly require confirming the `Ok`-badge checkmark `✓` renders as a real glyph (not a tofu/missing-glyph box) in `JETBRAINS_MONO_MEDIUM`. If tofu, apply the text-const fallback (§ Risks) — `"✓" → "ok"` in `phase_badge`, no structural churn — and re-mint. | `crates/render/src/screens/lab_gallery.rs`, `crates/render/tests/snapshots/lab_screen.png` | 4 |

## Handoff plan

Per `.claude/agents/design.md` § Rules (handoff-grouping) and
`.claude/skills/task/SKILL.md` Step 8. `M = 5`. Every subtask is **code**
change-type (Rust `*.rs` — gp-core + gp-render — plus one binary golden `.png`
asset; no `*.md`/`.claude`/`AGENTS.md`/`ai-docs` edit), so all five cluster into
ONE homogeneous group, minimizing group count (§ handoff-grouping (e)(f)). The
dependency chain (1 → 2 → 3 → 4 → 5) is a within-group ordering, not a group
boundary (boundaries are forced only by a change-type switch or the size cap,
neither of which applies). Default max 4 groups respected (1 ≤ 4, (h)).

- **Entry / Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned in
  `code-writer` frontmatter)**, 1M-token window, via the `code-writer` subagent
  (code change-type → `subagent_type="code-writer"`, no inline model/effort
  override) — subtasks **1, 2, 3, 4, 5**. **Handoff at entry:** spawn
  `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction
  recovery (re-entry) at the start of this group (every-group handoff contract,
  (a)(c)). Terminal group (5 subtasks; within `1..=10`, (b)(d)). No inter-group
  handoff — the single group completes Step 8 in its own `/context-reset`
  subagent.

The `design`/`design-review`/`self-review` quality gates stay on Opus
regardless of this marker (§ handoff-grouping (g)).

## Risks

- **Canvas "fill remaining height" has no egui flexbox primitive** — mitigated
  by the explicit rect-subtraction algorithm above (header band + reserved
  action band → canvas = the middle), then the AC6 golden pins the result. The
  crate already does explicit rect math for goldens `[measured: track/golden.rs:21-24
  → const CANVAS_RECT hand-built; setup.rs:113 → margin = (available_width -
  CONTENT_MAX_W)/2]`.
- **`✓` (U+2713) glyph coverage in JetBrains Mono** — the `Ok` phase badge label
  is a checkmark drawn through `FontFamily::Name(JETBRAINS_MONO_MEDIUM)`
  `[measured: badge.rs:141-143]`; if the vendored subset lacks the glyph the
  golden mints tofu. Mitigation: the `image-check` subagent at mint verifies the
  drawn frame against the code; if the glyph is missing, fall back to a text
  label (e.g. "ok") — a mint-time decision, not a code-structure change
  `[derived → subtask 4 image-check PASS/FAIL]`.
- **f32 layout arithmetic under `clippy::arithmetic_side_effects` (deny)** — not
  a risk: the lint targets integer overflow; existing screens/widgets do f32
  `-`/`/`/`mul_add` layout math with no `#[allow]`
  `[measured: setup.rs:113 → (ui.available_width() - CONTENT_MAX_W) / 2.0, no
  allow; clippy green in-tree]`.
- **`missing_const_for_fn` forcing** — `phase_badge`, `LabScreen::new`, and the
  icon setters are const-eligible pure functions and MUST be `const fn` (nursery
  = deny), matching every widget `resolve`/setter; `*_display` methods are
  correctly non-const (`String`/`format!` allocate)
  `[derived → cargo clippy --workspace --all-targets -- -D warnings]`.
- **Golden mint needs a software Vulkan ICD** — the wgpu golden asserts a
  CPU/software adapter (lavapipe), matching CI; identical constraint to every
  existing wgpu golden `[measured: track/golden.rs:194-199, setup_gallery.rs:53-58]`.
- **Zero-production-panics invariant** — the screen + gp-core additions add no
  `unwrap`/`expect`/`panic!`/panic-index entry; Vmax/Tempo `Option`s are handled
  totally via `map_or`, `width_min` (`u32`, non-`Option`) and `sf.width()`
  (`usize`) format via total `.to_string()`, and layout is pure f32/rect math
  `[measured: icons.rs:8-9 → "gp-render stays at zero production panics"; core
  targets zero production panics — panic-index has no gp-core entries, and this
  task adds none]`. `StartFinish::width` is a
  total `Vec::len` accessor (no index, no arithmetic). The documented layout-time
  panic (fonts not installed) is the same precondition every widget's `paint`
  documents `[measured: badge.rs:120-124]` — a `# Panics` doc note, not a new
  failure mode.
- **`TrackArtifact { … }` literal migration (non-`Default` new field)** — adding
  a non-`Default` `width_min` breaks every struct-literal site until each adds the
  field; `cargo build` reports each missing field as its own `E0063` (not
  fail-fast-masked). Mitigation: subtask 1 enumerates all **3** literal sites
  (§1a table) and ends with `cargo build --workspace` to surface any site the
  enumeration missed `[measured: rg -Un 'TrackArtifact\s*\{' crates → 3 literals
  + 1 def + 1 todo! body; gp-game/gp-ai have none]`.

## Test Design

- **AC1 — overlays const** (`lab.rs` `#[cfg(test)]`, Context-free, un-gated):
  `LAB_OVERLAYS.speed_heatmap && .fastest_lap && .grid` all true. Entry:
  `LAB_OVERLAYS`. No `egui::Context` → no Miri gate `[derived → the AGENTS.md §
  Rust Test Conventions grep audit finds no `Context::default`/painter in this
  test]`.
- **AC2 — oracle tiles off the artifact** (`lab.rs`, Context-free, un-gated):
  `oracle_tile_strings(&track)` returns `[Vmax, Tempo, Width min, S/F width]`
  strings tracking their source on the `TrackArtifact`. Fixtures: (i) a
  fully-populated artifact — `metrics.vmax_attain=Some(7)`, `metrics.tempo
  =Some(0.87)`, `width_min=3`, `sf.chord` of length `4` → `["7","0.87","3","4"]`
  (the JSX exemplars `[measured: Screens.jsx:182-185]`); (ii) an absent-metrics
  artifact — `vmax_attain=None`, `tempo=None`, `width_min=3`, chord length `4` →
  `["—","—","3","4"]` (the `None→"—"` path is Vmax/Tempo **only**; Width min /
  S/F width still render real numbers). Reuses the `render/src/track` fixture
  pattern to build the artifact (now with `width_min`). Entry:
  `oracle_tile_strings`. No `egui::Context` → no Miri gate.
- **AC3 — phase table + tone mapping** (`lab.rs`, Context-free, un-gated):
  `PHASE_IDS.len() == 7`, `PHASE_NAMES.len() == 7`; `phase_badge(PhaseStatus::Ok)
  == (BadgeTone::Ok, "✓")`, `phase_badge(PhaseStatus::Repair) == (BadgeTone::Warn,
  "repair")`. `BadgeTone` derives `PartialEq, Eq, Debug` → `assert_eq!` works
  `[measured: badge.rs:11]`.
- **AC7 — S/F-width getter** (`core/src/track.rs` `#[cfg(test)]`, Context-free,
  un-gated — pure `Vec::len`): `StartFinish { chord: vec![p0,p1,p2], … }.width()
  == 3` and `chord.len()`, and the **empty-chord** case `chord: vec![]` →
  `width() == 0`. No `egui`/Miri concern (spec: "pure integer/`Vec::len` logic").
- **AC4 — click signals** (`lab_gallery.rs`, `egui_kittest::Harness`, no
  `render()`): rest frame → all three flags `false`; then drive
  `hover_at`/`drag_at`/`drop_at` at each captured button `rect.center()` (3
  separate `Rc<Cell<Option<Rect>>>` + `Rc<Cell<bool>>` OR-accumulators, the
  setup precedent `[measured: setup_gallery.rs:137-159]`) → the corresponding
  flag flips `true`. Icons passed `None` (text-only buttons stay clickable →
  no `IconSet` needed in the test). **Miri-gated** `#[cfg_attr(miri, ignore =
  "Harness::builder() calls getcwd via egui_kittest's kittest.toml lookup,
  unsupported under Miri isolation (no render() here)")]` — the setup interaction
  test's exact reason `[measured: setup_gallery.rs:119-124]`.
- **AC6 — whole-screen golden** (`lab_gallery.rs`, wgpu `Harness`): frame-1 font
  install + frame-2 `LabScreen::show`, compared to `lab_screen.png` with
  `threshold(1.0)` + `failed_pixel_count_threshold(0)` (the text-bearing-screen
  setting `[measured: setup_gallery.rs:89-91]`). Fixture: a hand-built
  `TrackArtifact` with hand-populated `TrackMetrics` (`speed_heatmap` +
  `fastest_lap`) **and** a concrete `width_min` (+ an `sf.chord` whose `.width()`
  is the S/F-width tile value) — the `scene_track_with_metrics` pattern extended
  for `width_min` `[measured: track/golden.rs:82-120]`; icons `None` (smoke test
  — no `IconSet` bake).
  **Miri-gated** `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan ICD
  (no FFI under Miri)")]` `[measured: setup_gallery.rs:43-46]`. Mint locally
  (software Vulkan ICD), then spawn `image-check` on
  `(lab.rs draw code, lab_screen.png)` per the golden-mint contract. The
  `image-check` spawn prompt MUST explicitly require confirming the `Ok`-badge
  checkmark `✓` renders as a **real glyph** — not a tofu / missing-glyph box —
  in `JETBRAINS_MONO_MEDIUM`. If it is tofu, the fallback is **text-const-only**:
  change `phase_badge`'s `Ok` label from `"✓"` to `"ok"` (and the AC3 test's
  expected string to match), then re-mint. No structural churn — one string
  const.

All Context/painter/wgpu tests here match the crate's mechanical Miri-gate
triggers `[measured: AGENTS.md § Rust Test Conventions → "constructs a
Context/painter" + wgpu-golden gates]`; the three pure `lab.rs` tests construct
no `Context` and stay un-gated (grep-checkable:
`rg -Un 'egui::Context::default|Harness::builder' crates/render/src/screens/lab.rs`
returns empty).

## Open questions

None. The spec's two round-1 questions are resolved in it; the icon-availability
gap (shuffle/menu absent from the vendored set) is resolved here (Key decision 4
— `Option<&TextureHandle>`, text-only fallback, no `Icon` enum change), and the
`✓`-glyph coverage question is a mint-time `image-check` verification (Risks),
not a blocking design question.

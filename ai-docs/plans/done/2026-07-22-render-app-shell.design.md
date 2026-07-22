# Design: gp-render app shell — top bar + screen router

**Issue:** #23
**Date:** 2026-07-22

## Approach

Add one new module `crates/render/src/app.rs` holding an **`AppShell`** — the
draw-only router. It owns the minimal cross-transition state the spec fixes
(`Screen` cursor + `RaceConfig` + `Overlays` + a `has_generated` latch), borrows
all externally-sourced session data per frame, draws the top bar + the current
screen, and applies navigation intents derived from the screen `*Response`s and
the nav-bar clicks. A sibling `#[cfg(test)] mod` file `app_gallery.rs` carries
the AC3 wgpu golden and the AC4 `egui_kittest` click-through, mirroring the
established per-screen split (`setup_gallery.rs` drives the real `SetupScreen`
inside a `Harness` — `[measured: sed setup_gallery.rs → build_ui(move |ui| … SetupScreen::new(FIXED_CONFIG).show(ui))]`).

**Separation of pure logic from drawing (the testability spine).** The router's
transition logic is a pure `AppShell::apply(&mut self, Nav)` method plus a pure
`can_nav(&self, Screen) -> bool` guard — **no `egui` in either**. This is what
lets AC1/AC2/AC5/AC6/AC7 be plain state-machine unit tests with **no
`egui::Context`**, so they stay **un-Miri-gated** (the gate's trigger is
*constructs a Context/painter* — AGENTS.md § Rust Test Conventions; these
construct neither). Only AC3 (golden) and AC4 (click-through) touch a `Harness`
and carry the Miri gate.

**Navigation intent enum.** The shell maps each frame's screen `*Response` +
nav-bar click into one `Nav` value and feeds it to `apply`:

| `Nav` | Source | Effect on `screen` | Latch |
|---|---|---|---|
| `Generate` | `SetupResponse.generated` | → `Lab` | sets `has_generated = true` |
| `TestLap` | `LabResponse.test_lap` | → `Race` | — |
| `Menu` | `LabResponse.menu` **or** `ResultsResponse.menu` | → `Setup` | — |
| `Regenerate` | `LabResponse.regenerate` | no-op (stays `Lab`) — AC5 | — |
| `Finish` | `RaceResponse.finish` | → `Results` | — |
| `Again` | `ResultsResponse.again` | → `Race` | — |
| `JumpTo(Screen)` | top-bar nav click | → target **iff `can_nav`** (AC6/AC7) | — |

`can_nav(t)` = `matches!(t, Screen::Setup) || self.has_generated` — `New race`
always enabled, `Race`/`Track lab` gated on the latch (AC7). At most one `Nav`
is applied per frame; a nav-bar click takes precedence only when no screen
intent fired (the screens' own controls and the top bar are disjoint hit
regions, so simultaneous fire is not reachable in one frame, but pinning an
order keeps `apply` single-valued).

**Owned vs borrowed (spec Key decisions).** `AppShell` owns only
`{ screen: Screen, config: RaceConfig, overlays: Overlays, has_generated: bool }`
— all `Copy`, all `gp-render`-local. Everything sourced from
`gp-gen`/`gp-ai`/`gp-core::sim` is borrowed per frame through one
**`ShellSession<'a>`** bundle. The shell owns `screen`, so the caller cannot
know which screen will draw; it supplies the whole session bundle every frame
and the shell selects the fields the current screen needs, building
`SetupScreen::new(self.config)` / `LabInput` / `RaceInput { scene: Scene { … } }`
/ `ResultsInput` on the fly. `config` is refreshed from `SetupResponse.config`
every Setup frame; `overlays` from `RaceResponse.overlays` every Race frame — so
both persist across transitions purely by being owned fields (AC2).

**The binary hand-builds the fixture track (matches the corrected spec, 2026-07-22).**
The corrected spec **prescribes** the hand-built fixture track — `gp_core::geom::{Corridor, walls_from_boundary}`
+ a hand-populated `TrackMetrics` — naming `gp_gen::generate`'s `todo!()` stub
as the reason; spec and this design now **agree** (no deviation). `generate` is
an unimplemented stub `[measured: sed -n 52,54p crates/gen/src/lib.rs → pub fn generate(_params: GenParams) -> TrackArtifact { todo!("track generation pipeline (design doc §2)") }]`,
so calling it would panic on `cargo run -p gp-game`, breaking both AC8 and the
"zero production panics maintained" constraint. Instead `gp-game` constructs a
`TrackArtifact` **by hand** — the exact JS-mock "physics/AI faked" posture the
spec endorses — reusing the public `gp_core::geom::{Corridor, walls_from_boundary}`
API the way `track/mod.rs`'s own `fixture_track` does
`[measured: rg walls_from_boundary crates/core/src/geom/graph.rs:308 → pub fn walls_from_boundary]`.
The render path is proven safe on such a minimal artifact — `track/mod.rs`'s
`fixture_track_with_metrics` (a 3×3 ring + `walls_from_boundary`, with
hand-populated `speed_heatmap`/`fastest_lap`) renders under **all 8 overlay
combinations without panicking**
`[measured: rg -n "all_overlay_combinations_render_without_panic|fixture_track_with_metrics" crates/render/src/track/mod.rs → lines 338-339: the 8-combo test uses fixture_track_with_metrics]`,
which covers Lab's all-on `LAB_OVERLAYS`. `gp-game` hand-populates
`metrics.speed_heatmap`/`fastest_lap` over the ring's cells (mirroring
`fixture_track_with_metrics`) so the Lab/Race canvases show a non-empty heatmap
and ideal line.

**Nav icons are text-only.** The mock's nav glyphs (`flag`, `gamepad-2`,
`flask-conical`) are **absent** from the vendored Lucide set
`[measured: ls crates/render/icons/ → grid-3x3, pause, play, settings, zoom-in (+LICENSE)]`
— the same absence `lab.rs` already handles by rendering text-only. The three
nav items render label-only (`New race` / `Race` / `Track lab`); the
display-only right-hand `Settings` icon-button + `{cars} cars · {laps} laps`
readout are out of scope (spec § Out of scope) and omitted.

**Module placement — top-level `crates/render/src/app.rs` (decided).** The shell
is the composition **root above** the screen layer — it imports `SetupScreen`/
`LabScreen`/`RaceScreen`/`ResultsScreen` from `screens/` and routes between them,
so nesting it *inside* `screens/` would invert that dependency. Top level keeps
it a sibling of the other cross-screen infrastructure modules (`track`,
`widgets`) and of `placeholder` — the module it replaces
`[measured: rg -n "pub mod|mod " crates/render/src/lib.rs → placeholder, screens, track, widgets all top-level]`.
Declaration is `pub mod app;` in `lib.rs` (subtask 1) with
`pub use app::{AppShell, Screen, ShellSession, ShellResponse};` re-exported from
`lib.rs` (subtask 3); the test sibling is `#[cfg(test)] mod app_gallery;` in
`lib.rs` (subtask 4). The per-screen split (`setup.rs` + `setup_gallery.rs`)
lives under `screens/`; the app split (`app.rs` + `app_gallery.rs`) lives at top
level for the composition-root reason above — the Decomposition file paths are
authoritative.

**Placeholder-scaffold removal (Scope 7 / AC9) — delete `placeholder.rs`
wholesale, relocate the canary into `app_gallery.rs` (decided).** Subtask 5
already took `gp-game` off `draw_placeholder` (`main.rs` drives `AppShell` now)
`[measured: rg -n "AppShell|draw_placeholder" crates/game/src/main.rs → line 77 AppShell::new; zero draw_placeholder]`,
so the scaffold has **zero production callers**. Every item in `placeholder.rs`
— `draw_placeholder`, `geometry`/`PlaceholderGeometry`, `grid_lines`/`draw_grid`,
`pixel_at`, `CANVAS_RECT`, and the palette/sample consts — is consumed **only
inside `placeholder.rs` itself** (by `draw_placeholder`, the `golden_guard`
golden, and the `tessellation_smoke` canary); the sole cross-file import is that
module's own `#[cfg(test)]` `use super::…`
`[measured: rg -n "use .*placeholder|crate::placeholder|placeholder::" crates/ --type rust → only placeholder.rs:265 use super::{CANVAS_RECT, draw_placeholder, geometry}]`.
The `CANVAS_RECT`/`grid_lines`/`pixel_at` names that appear in
`movepad_gallery`/`game_gallery`/`forms_gallery`/`gallery`/`track/golden.rs`/
`widgets/card.rs` are each **that file's own local copy**, and the
`icons.rs`/`card.rs`/`track/golden.rs`/`track/walls.rs`/`gallery.rs` mentions of
`placeholder.rs::…` are **prose precedent-citations in comments**, not code
references
`[measured: rg -Un "pixel_at|grid_lines|CANVAS_RECT|draw_grid" crates/render/src → cross-file hits are own-local consts/fns or reason=/doc comments; golden.rs:186 + walls.rs:79 cite placeholder.rs::pixel_at in reason= strings]`.
So removing `draw_placeholder` + `golden_guard` and repointing the canary off
`draw_placeholder` leaves **nothing** in `placeholder.rs` used → the module is
**deleted wholesale**, not trimmed: a trimmed shell holding one canary that draws
*app-shell* code (not placeholder art) would be dead scaffolding misfiled away
from the shell it now exercises.

**Canary → `app_gallery.rs`, driving the real `AppShell::show`.**
`tessellation_smoke` relocates into `app_gallery.rs`'s `#[cfg(test)] mod tests`,
the app-shell test sibling that already holds the `ShellSession` fixtures the
canary needs (`fixture_track` / `fixture_standings` / `FIXED_SUMMARY` /
`FIXED_CONFIG`, module-level so `mod tests` reuses them)
`[measured: read crates/render/src/app_gallery.rs:35-107 → fixture_track/fixture_standings fns + FIXED_SUMMARY/FIXED_CONFIG consts]`.
It keeps its **bare `egui::Context::default()` + `run_ui` single-pass** shape
(**not** a `Harness`, so it introduces neither the wgpu-`dlopen` nor the `getcwd`
Miri cause) and swaps its draw payload from
`draw_placeholder(ui.painter(), CANVAS_RECT)` to
`AppShell::new(FIXED_CONFIG).show(ui, session)` on a fresh shell (Setup). That
real path rasterises the top-bar `GRAPHITE GP` wordmark + three nav labels + the
`SetupScreen` body — **strictly more** glyph geometry than the old synthetic
3-row sample — so the existing `!primitives.is_empty()` + `vertex_count > 0` +
`index_count > 0` assertions hold unchanged, now over the **production**
tessellation path. Its `#[cfg_attr(miri, ignore = "…vello_cpu checked u8→u32
cast…")]` gate and reason are **unchanged** (drawing text still hits the same
abort site; spec AC9). `set_fonts(definitions())` before `run_ui` stays
load-bearing (wordmark + `SetupScreen` resolve `FontFamily::Name`; a single pass
suffices — design D11). Alongside: delete
`crates/render/tests/snapshots/placeholder.png`; drop `pub mod placeholder;` from
`lib.rs`; and rephrase `lib.rs`'s one **live** doc-comment mention of
`draw_placeholder` (line 82) so no live code names the deleted fn
`[measured: sed -n 78,86p crates/render/src/lib.rs → "…a pure function of (rect, scene) — the same precedent draw_placeholder sets"]`.

**Rejected alternatives.**
- *A free `Router` struct separate from `AppShell`* — rejected: the router state
  IS the shell state; a second struct duplicates the four fields and their
  accessors for no gain (YAGNI). One struct with pure `apply`/`can_nav` methods
  gives identical testability.
- *Reuse `Tag`/`Button` for nav items* — rejected: `Tag`'s selected look is a
  `PAPER_2` fill + `BORDER_STRONG` border
  `[measured: sed tag.rs Tag::resolve → selected ⇒ bg PAPER_2, border BORDER_STRONG]`,
  not the mock's active pill (`GRAPHITE_900` fill + `PAPER_0` text). A tiny
  `nav_item` helper matches the mock exactly and carries the disabled state the
  guard needs; `Button` has no active/selected concept.
- *Return the full screen `*Response` enum from `show`* — rejected: only the AC4
  click-through needs a click target, and only the *forward* control per screen.
  `show` returns just `ShellResponse { screen, advance_rect }` (minimal surface;
  the binary ignores it).

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Router core + state machine.** `Screen`/`Nav` enums, `AppShell` owned-state struct, `new()` (const), `apply(&mut self, Nav)`, `can_nav(&self, Screen)` (const), config/overlays setters. `#[cfg(test)] mod tests` covering AC1/AC2/AC5/AC6/AC7 (pure, no `egui::Context`, un-gated). Declare `pub mod app;` in `lib.rs`. TDD — tests first. | `crates/render/src/app.rs`, `crates/render/src/lib.rs` | — |
| 2 | **Top bar.** `TOP_BAR_H` + nav consts; `draw_top_bar` (page + header chrome, small wordmark, 3 `nav_item`s with active/disabled states) + `nav_item` helper returning `(Response, clicked)`; returns the clicked target `Option<Screen>`. | `crates/render/src/app.rs` | 1 |
| 3 | **Shell composition + dispatch.** `ShellSession<'a>` bundle, `ShellResponse { screen, advance_rect }`, `AppShell::show(&mut self, ui, session) -> ShellResponse` — draws top bar + current-screen body (child `Ui` at the body rect), builds each screen's input from owned + borrowed state, threads `config`/`overlays`, derives `Nav`, calls `apply`, surfaces the forward control's rect. Re-export `AppShell`/`Screen`/`ShellSession`/`ShellResponse` from `lib.rs`. | `crates/render/src/app.rs`, `crates/render/src/lib.rs` | 1, 2 |
| 4 | **Golden + click-through.** `app_gallery.rs`: AC3 wgpu golden `app_shell.png` (fresh shell on Setup — `New race` active, `Race`/`Track lab` disabled), `threshold(1.0)`+`failed_pixel_count_threshold(0)`, Miri-gated, minted + `image-check`-verified; AC4 `egui_kittest` click-through smoke driving Setup→Lab→Race→Results→Menu via `advance_rect`, asserting `resp.screen` at each step, Miri-gated. Declare `#[cfg(test)] mod app_gallery;` in `lib.rs`. | `crates/render/src/app_gallery.rs`, `crates/render/src/lib.rs` | 3 |
| 5 | **gp-game wiring (AC8).** Replace `draw_placeholder` with an `AppShell`-driven `eframe::App`: own the shell + a hand-built fixture `TrackArtifact` (`Corridor` + `walls_from_boundary` + hand-populated metrics) + fixture `CarState`s/trails/standings/summary/phases; rebuild the borrowed `ShellSession` each `ui()` frame and call `shell.show`. `cargo build -p gp-game` green, `draw_placeholder` call gone. | `crates/game/src/main.rs` | 3 |
| 6 | **Remove the `draw_placeholder` scaffold (Scope 7 / AC9).** Delete `placeholder.rs` (whole module) and its `pub mod placeholder;` in `lib.rs`; **relocate** `tessellation_smoke` into `app_gallery.rs`'s `#[cfg(test)] mod tests`, repointed onto `AppShell::new(FIXED_CONFIG).show(ui, session)` (bare `Context`, reusing the module's `fixture_track`/`fixture_standings`/`FIXED_SUMMARY` fixtures), keeping its `#[cfg_attr(miri, ignore = "…")]` gate + reason verbatim; delete `crates/render/tests/snapshots/placeholder.png`; rephrase `lib.rs`'s stale `draw_placeholder` doc-comment (line 82). Verify: `cargo test -p gp-render tessellation_smoke` green + `rg 'draw_placeholder' crates/` clean. | `crates/render/src/placeholder.rs` (delete), `crates/render/src/app_gallery.rs`, `crates/render/src/lib.rs`, `crates/render/tests/snapshots/placeholder.png` (delete) | 5 |

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping, mandatory for every
`M ≥ 1` (here M = 6 after the AC9 amendment). All six subtasks change **code**
(`*.rs` under `crates/render/**` + `crates/game/**`, plus subtask 6's
`placeholder.png` snapshot **deletion** — a test asset, code change-type, not
instructions) — a single homogeneous change-type — so they cluster into the
**fewest groups possible**: **one** group. Subtask 6 depends on subtask 5
(`main.rs` must already be off `draw_placeholder` — verified: it is
`[measured: rg -n draw_placeholder crates/game/src/main.rs → no hits]`), so it
appends to Group A in dependency order. Group size 6 is `≤ 10` (size cap) and the
group is terminal (`1..=10`). Max-groups = 1 (≤ 4 default; no user gate needed).

- **Group A** — model `sonnet` (sonnet-5), effort `medium` (pinned in the
  `code-writer` frontmatter), 1M-token window, via the `code-writer` subagent —
  subtasks 1–6 (code change-type: `*.rs` + one snapshot deletion). Terminal group
  (6 subtasks; within the `1..=10` range). No inline `model=`/effort override at
  spawn — `code-writer` pins both. The Opus quality gates (`design`,
  `design-review`, `self-review`, `image-check` at mint) review its output.
  **Subtask 6 mints no golden** — the relocated `tessellation_smoke` asserts mesh
  vertex/index counts, not pixels, and the `placeholder.png` change is a
  **removal** (mints nothing) — so **no `image-check` is spawned for subtask 6**.
  (Subtask 4's `app_shell.png` golden still gets `image-check` at mint, unchanged.)
- **Handoff into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) at the
  start of the group (every-group handoff contract — the first group included).
  The single group completes /task Step 8 in its own `/context-reset` subagent.

## Risks

- **`gp_gen::generate` panics (`todo!()`).** The binary must **not** call it; it
  hand-builds the fixture `TrackArtifact` instead — exactly as the corrected spec
  (2026-07-22) prescribes. Mitigation: subtask 5 uses `Corridor`/`walls_from_boundary`
  + hand-populated metrics, the render-safe pattern `track/mod.rs` already
  exercises. —
  `[measured: sed -n 52,54p crates/gen/src/lib.rs → todo!("track generation pipeline …")]`
- **Zero-production-panics invariant** (spec § Technical constraints; gp-render +
  gp-game). No new `.expect`/`unwrap`/`panic!`/panicking index in either crate's
  production path. Session-data access is total: `active` index falls back via
  the screens' existing `CarState::default` paths; out-of-range car/standings
  indices already fall back (`CAR_NAMES.get(..).unwrap_or`). The shell adds no
  index arithmetic. — `[derived → cargo clippy --workspace --all-targets -- -D warnings + code review; ai-docs/panic-index.md stays empty]`
- **`missing_const_for_fn` (nursery = deny) FORCES `const fn` on const-eligible
  pure fns.** `AppShell::new` (struct literal) and `can_nav` (pure `&self`
  `matches!` + `||` over `bool`) are const-eligible → must be `const fn`.
  `apply(&mut self)` const-eligibility (assignment through `&mut self`) is
  toolchain-dependent; the implementor lets clippy decide — mark `const` iff the
  lint fires, else plain `fn`. Do **not** pre-declare non-const as YAGNI. —
  `[measured: rg -n "nursery|pedantic" Cargo.toml → both level=deny]` /
  `[derived → cargo clippy --workspace --all-targets -- -D warnings]`
- **Screen-layer collision.** Each screen installs its own `Order::Middle` layer
  keyed on `ui.id().with("…_screen")`. The shell draws the top bar on the parent
  layer and dispatches the body through a child `Ui` with a distinct id, so the
  screen's keyed layer does not collide with the top bar. — `[derived → AC3 golden + AC4 click-through render the composed frame]`
- **Font precondition (`# Panics` on `RaceScreen::show`/`LabScreen::show`).** Both
  screens panic at layout time if `crate::fonts::definitions()` was not installed
  first — a pre-existing gp-render contract
  `[measured: rg -n "# Panics|not installed" crates/render/src/screens/race.rs crates/render/src/screens/lab.rs → race.rs:171, lab.rs:210]`.
  Subtask 5's `gp-game` `eframe::App` must install fonts **once** before the first
  `shell.show` call so the zero-production-panics constraint holds in the binary;
  `main.rs` already does this (`.set_fonts(gp_render::fonts::definitions())`) —
  keep that install when replacing `draw_placeholder`, do not drop it
  `[measured: rg -n "set_fonts|definitions" crates/game/src/main.rs → line 38: .set_fonts(gp_render::fonts::definitions())]`.
- **`placeholder.rs` deleted wholesale (Scope 7 / AC9) — no compile breakage,
  only stale prose.** Every `placeholder.rs` item is consumed only within that
  file (the only cross-file import is its own `#[cfg(test)] use super::…`), so
  deleting the module breaks no build. The sole fallout is prose
  precedent-citation comments in sibling files that name `placeholder.rs` /
  `golden_guard` (`icons.rs`, `widgets/card.rs`, `widgets/gallery.rs`,
  `track/golden.rs`, `track/walls.rs`) — readable but referencing a deleted
  file. These are **ACCEPTED cosmetic staleness** — outside AC9's `crates/**`
  grep scope (which targets `draw_placeholder`, not `placeholder.rs`/`golden_guard`
  prose citations in comments), left as-is, **not** absorbed into subtask 6 and
  **not** tracked as an open item. —
  `[measured: rg -n "use .*placeholder|crate::placeholder|placeholder::" crates/ --type rust → only placeholder.rs:265 use super::{…}; no external import]`
- **Miri-gate inventory — the two AGENTS.md-cited gates both stay covered.**
  AGENTS.md § Rust Test Conventions names the two gp-render Miri gates by cause:
  `golden_guard` (wgpu `dlopen`, FFI) and `tessellation_smoke` (vello_cpu
  checked-cast, no FFI). Subtask 6 **deletes `golden_guard`**, but the
  FFI-`dlopen` abort *class* is preserved by the surviving wgpu goldens
  (`app_gallery.rs::app_shell_matches_golden` + the per-screen/track goldens),
  and the vello_cpu-cast gate is preserved by the **relocated** `tessellation_smoke`
  with its `ignore` reason unchanged — so **both documented causes remain gated**.
  What goes stale is only AGENTS.md's *by-name* example (`golden_guard` line 308)
  and `ai-docs/context.md`'s "Both `placeholder.rs` tests are Miri-ignored" note
  (`placeholder.rs` deleted). Both are **instructions/harness** change-type — not
  subtask 6's code group (homogeneity). **RESOLVED (2026-07-22, user-approved):**
  TWO **paired** orchestrator **IN-THREAD** edits in THIS PR (Propagation Rule —
  agent-doc siblings), both **sequenced after subtask 6** deletes
  `golden_guard`/`placeholder.rs`: **(i)** AGENTS.md:308's `golden_guard` by-name
  example is **repointed** to a surviving wgpu golden (e.g. `app_shell`/
  `setup_screen`); **(ii)** `ai-docs/context.md`'s stale placeholder/`golden_guard`
  Miri-gate description lines (the "Both `placeholder.rs` tests are Miri-ignored…"
  note naming `draw_placeholder`/`placeholder.rs`/`golden_guard`) are updated to
  reflect the module deletion + canary relocation into `app_gallery.rs`. Both
  target protected/agent-doc files whose edits fail closed under a background
  subagent (AGENTS.md § Workflow), so both are authored in-thread and add **no**
  code subtask; **neither** is part of Group A. The Miri-gate rule itself is
  unchanged — only its stale examples move. See § Open questions #1. —
  `[measured: rg -n golden_guard AGENTS.md → line 308; rg -rn golden_guard ai-docs/context.md → "Both placeholder.rs tests are Miri-ignored, for *different* reasons"]`

## Test Design

**Subtask 1 — router state machine** (`crates/render/src/app.rs` `#[cfg(test)] mod tests`; **no `egui::Context` → un-Miri-gated**, no `#[cfg_attr(miri, ignore)]`):
- Entry points: `AppShell::new`, `apply`, `can_nav`, config/overlays setters.
- AC1 `linear_flow_setup_lab_race_results_menu`: fresh shell (`Setup`); `apply(Generate)`⇒`Lab`; `apply(TestLap)`⇒`Race`; `apply(Finish)`⇒`Results`; `apply(Menu)`⇒`Setup`; and from `Lab`, `apply(Menu)`⇒`Setup`.
- AC2 `config_and_overlays_persist_across_transitions`: set a non-default `RaceConfig`; `apply(Generate)`/`TestLap`/`Finish`; assert `config` unchanged on Lab/Race/Results. Set `Overlays` on Race; assert it persists while `screen` stays in the race sub-flow.
- AC5 `regenerate_does_not_change_screen`: from `Lab`, `apply(Regenerate)`⇒ still `Lab`.
- AC6 `nav_jump_from_arbitrary_screen`: with `has_generated == true`, `apply(JumpTo(Race))` from `Setup`⇒`Race`, `apply(JumpTo(Lab))` from `Results`⇒`Lab`, `apply(JumpTo(Setup))` from `Race`⇒`Setup`.
- AC7 `nav_guard_before_first_generate`: fresh shell (`has_generated == false`): `apply(JumpTo(Race))` and `apply(JumpTo(Lab))` are no-ops (stays `Setup`); `apply(JumpTo(Setup))` allowed. After `apply(Generate)`: `has_generated == true`, all three jumps succeed.
- Fixtures: a `const FIXED_CONFIG` (cars 4, laps 5, v_target 7, Pro — the mock startup default); `Overlays` literals. No harness.

**Subtask 4 — golden (AC3)** (`crates/render/src/app_gallery.rs`; **Miri-gated** — drives wgpu):
- `app_shell_matches_golden`: `Harness::builder().with_size(…).with_pixels_per_point(1.0).with_theme(Light).renderer(PREDICTABLE-wgpu)`, frame-1-install-fonts / frame-2-draw, `run_steps(1)`, `render()`, compare `"app_shell"` with `SnapshotOptions::new().threshold(1.0).failed_pixel_count_threshold(0)`. **`threshold(1.0)` pinned AT DESIGN TIME** — the frame is text-bearing (wordmark + three nav labels + the Setup body), the settled content class for `threshold(1.0)`+`failed_pixel_count_threshold(0)` per the `setup_gallery` precedent; `threshold(0.0)` would schedule a wasted red-CI round. Canvas size confirmed at mint. Fresh shell on `Setup` (no session track needed — `SetupScreen` needs only `config`); shows `New race` active, `Race`/`Track lab` disabled. Minted golden **must** be `image-check`-verified against the drawing code at mint.
- `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)")]` — the golden's own cause.

**Subtask 4 — click-through (AC4)** (`crates/render/src/app_gallery.rs`; **Miri-gated** — `Harness::builder()` calls `getcwd`):
- `click_through_setup_to_menu`: default (non-wgpu) `Harness`, no `render()`. Holds `&mut AppShell` in the closure; each frame captures the returned `ShellResponse { screen, advance_rect }` into `Rc<Cell<…>>` (rects/`Screen` are `Copy`). Loop: rest-frame → read `advance_rect` → `hover_at`/`drag_at`/`drop_at` at its `.center()` (3 `step()`s, the `setup_gallery` click idiom — no AccessKit label to query) → assert the OR-accumulated `screen` reached the expected next screen. Sequence: `Setup` →(Generate)→ `Lab` →(Test lap)→ `Race` →(Finish)→ `Results` →(Menu)→ `Setup`.
- Fixtures: the hand-built `TrackArtifact` (`Corridor` + `walls_from_boundary`) + a `CarState`/trail slice + `StandingEntry`s + `RaceSummary` + `[PhaseStatus; 7]`, assembled into a `ShellSession`. `saw_*` `Cell`s OR-accumulate one-frame click pulses (egui runs several internal passes per `step()` — the `setup_gallery` note).
- `#[cfg_attr(miri, ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml lookup, unsupported under Miri isolation (not the golden's Vulkan-dlopen cause — this test never calls render())")]` — the reason `setup_gallery.rs` already documents for the identical no-`render()` harness case.

**Subtask 5 — binary (AC8)**: covered by `cargo build -p gp-game` (green, `draw_placeholder` call removed); the flow itself is exercised by AC4's `gp-render` click-through, so no separate binary test (spec AC8). — `[derived → cargo build -p gp-game]`

**Subtask 6 — relocated `tessellation_smoke` canary (AC9)** (`crates/render/src/app_gallery.rs` `#[cfg(test)] mod tests`; **Miri-gated**, reason unchanged; **no golden / no `image-check`**):
- Entry point: `AppShell::show`, driven through a bare `egui::Context::default()` + `ctx.run_ui` **single pass** (NOT a `Harness`), then `ctx.tessellate(output.shapes, output.pixels_per_point)` — the exact shape the old canary used, only the draw payload changes.
- Fixtures: `AppShell::new(FIXED_CONFIG)` (fresh → `Setup`) + a `ShellSession` assembled from the module's existing `fixture_track()` / `fixture_standings()` / `FIXED_SUMMARY` + `[PhaseStatus::Ok; 7]` (the same fixtures AC3/AC4 build — reused, not duplicated). `ctx.set_fonts(crate::fonts::definitions())` **before** `run_ui` (the wordmark + `SetupScreen` body resolve `FontFamily::Name`; one pass suffices — design D11).
- Assertions (unchanged from the deleted placeholder canary, now over the **production** shell draw path): `!primitives.is_empty()`; folded `vertex_count > 0`; `index_count > 0`. Setup draws the `GRAPHITE GP` wordmark + three nav labels + the `SetupScreen` body — strictly more glyph geometry than the old 3-row sample, so the counts remain positive.
- `#[cfg_attr(miri, ignore = "drawing text rasterises glyphs via vello_cpu, whose checked u8->u32 pixmap cast panics under Miri's 1-byte allocator alignment (TargetAlignmentGreaterAndInputNotAligned)")]` — carried over **verbatim** from `placeholder.rs::tessellation_smoke`; the abort cause is identical (text still rasterised) so the reason must not change (spec AC9; AGENTS.md § Rust Test Conventions "write the reason for that test's own abort").
- No image is rendered and `placeholder.png` is deleted (mints nothing), so **subtask 6 spawns no `image-check`**. AC9's grep verification is **scoped to live `crates/**` production code** (`rg 'draw_placeholder' crates/` returns **no hits**), **NOT** workspace-wide — frozen `ai-docs/plans/done/*` records and the append-only `learnings.md` legitimately retain the historical name and are excluded (resolved 2026-07-22; § Open questions #2). — `[derived → cargo test -p gp-render tessellation_smoke green + rg 'draw_placeholder' crates/ empty]`

## Open questions

None — both resolved 2026-07-22 (user-approved this turn).

The round-1 `gp_gen::generate` question was resolved earlier by the corrected
spec (2026-07-22), which itself prescribes the hand-built fixture track — spec
and design agree. The two AC9-amendment items are now both resolved:

1. **AGENTS.md's `golden_guard` by-name example — RESOLVED: fixed in THIS PR as
   an orchestrator IN-THREAD edit** (NOT a delegated `code-writer`/`general-purpose`
   group). AGENTS.md § Rust Test Conventions names `golden_guard` as one of "the
   two in-tree gates"; subtask 6 deletes it, so the stale by-name example is
   **repointed** to a surviving wgpu golden (e.g. `app_shell`/`setup_screen`).
   AGENTS.md is a protected instruction file, so a background subagent's edit
   fails closed (AGENTS.md § Workflow — "protected-file edit fails closed
   regardless of `Edit` allow-lists — apply those in-thread"); the orchestrator
   authors the repoint in-thread, **sequenced after subtask 6 deletes
   `golden_guard`**. It is **not** part of Group A (which stays subtasks 1–6, all
   `*.rs`/snapshot code) and adds **no** code subtask. The Miri-gate *rule* itself
   is unchanged — only its stale example moves; the FFI-`dlopen` abort class it
   documents survives in `app_shell_matches_golden` + the per-screen/track
   goldens. **Paired agent-doc edit (Propagation Rule), PINNED into THIS PR:** in
   the **SAME in-thread orchestrator pass**, `ai-docs/context.md`'s stale "Both
   `placeholder.rs` tests are Miri-ignored…" note (naming `draw_placeholder`/
   `placeholder.rs`/`golden_guard`) is updated to reflect the module deletion +
   canary relocation into `app_gallery.rs` — **NOT** left as a discretionary
   follow-up. Both edits are orchestrator in-thread (protected/agent-doc), **NOT**
   part of code Group A.
   `[measured: rg -n golden_guard AGENTS.md → line 308; rg -rn "placeholder.rs tests are Miri-ignored" ai-docs/context.md → the stale note naming draw_placeholder/placeholder.rs/golden_guard]`
2. **AC9 grep scope — RESOLVED: scoped to live `crates/**` production code.** The
   spec's AC9 was re-scoped this turn so the verification is `rg 'draw_placeholder'
   crates/` returns no hits (NOT workspace-wide). Frozen completed-plan records
   (`ai-docs/plans/done/*.{spec,design}.md`) and the append-only `learnings.md`
   legitimately retain `draw_placeholder` in the historical record and are
   **excluded**; narrative docs (`README.md`, `ai-docs/{context,key-decisions}.md`,
   `plans/INDEX.md`) are likewise outside the live-code scope. Subtask 6 clears
   every live `crates/**` hit
   `[measured: rg -n draw_placeholder crates/ → only placeholder.rs (deleted) + lib.rs:82 (rephrased); both handled by subtask 6]`.

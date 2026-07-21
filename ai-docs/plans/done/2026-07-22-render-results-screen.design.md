# Design: gp-render Results screen — final standings, fastest lap, crashes

**Issue:** #22
**Date:** 2026-07-22

## Approach

Port `Screens.jsx`'s `ResultsScreen` (`docs/design-system/ui_kits/game/Screens.jsx:205-235`)
as a new `screens::results` module, following the exact draw-only,
caller-supplies-data shape of the three shipped siblings. `SetupScreen` is the
governing precedent: Results is a **single centered content column**
(`Screens.jsx:207` `maxWidth: 560, margin: '0 auto', padding: '48px 24px'`),
identical in structure to `SetupScreen`'s centered column
(`setup.rs` `CONTENT_MAX_W = 560`, `margin = ((available_width - CONTENT_MAX_W)/2).max(SPACE_6)`)
[measured: `grep -n CONTENT_MAX_W crates/render/src/screens/setup.rs` → `const CONTENT_MAX_W: f32 = 560.0;`].

The screen composes only already-shipped widgets — `Card`, `CarChip`,
`Telemetry`, `Button` — plus raw painter text; it draws **no** track canvas
(no `render_frame` call), unlike `race.rs`/`lab.rs`. It performs no ranking,
timing, or counting: the caller hands it already-ranked standings and the
summary metrics, and the screen renders them in slice order.

### Resolving the Open question — outcome input type

Chosen shape (the spec's defensible default, made concrete):

- **`StandingEntry`** — one per car, in caller-supplied rank order:
  - `car_index: usize` — the car's **stable identity** (0 = the player's car),
    which resolves *name* via `screens::race::CAR_NAMES` and *color* via
    `tokens::color::car_color`, honoring spec Key-decision "Reuse the existing
    `CAR_NAMES` table and the car color ramp rather than duplicating a new
    table." Identity is deliberately decoupled from `rank` — a real player can
    finish P3 with `car_index == 0`.
  - `kind: CarKind` — explicit `You`/`Ai` (matches `CarChip`'s own prop and
    drives the player-position derivation below).
  - `rank: u32` — finishing rank (`CarChip.rank`).
  - `finish_time: f32` — seconds; formatted at draw time.
- **`RaceSummary`** — the three summary-tile values, numeric:
  `fastest_lap: f32`, `tempo: f32`, `crashes: u32`. Formatted at draw time
  (`{:.1}` / `{:.2}` / `to_string`), mirroring `lab.rs::oracle_tile_strings`'
  numeric-in / string-at-draw contract [measured: `grep -n 'fn oracle_tile_strings' crates/render/src/screens/lab.rs` → `pub fn oracle_tile_strings(track: &TrackArtifact) -> [String; 4]`].

Both are plain **public-field `Copy` structs with no constructor**, mirroring
`RaceConfig` (`screens/mod.rs` — public fields, struct-literal construction, no
`new`). `StandingEntry`/`RaceSummary` derive `Clone, Copy, Debug, PartialEq`
(all fields are `Copy`; `f32` blocks `Eq`), enabling `assert_eq!` in tests.

Rejected alternatives:

- **One flat struct** carrying standings inline + summary + position: fails the
  atomic per-car iteration `CarChip` needs, and forces a fixed car count.
- **Explicit `name: &str` + `color: Color32` per entry**: rejected — the spec
  Key-decision directs *reuse* of `CAR_NAMES`/ramp, and explicit strings/colors
  would duplicate that table at every call site.
- **Explicit `player_position` field on the summary**: rejected — it duplicates
  data already present as the `You` entry's `rank`, and a redundant field can
  disagree with the standings. Instead the position is **derived** (single
  source of truth) by `player_position(&[StandingEntry]) -> Option<u32>`, which
  returns the rank of the `kind == You` entry (total: `None` if absent — the
  header then renders a `P—` placeholder, never a panic).

### Icon handling (`rotate-ccw`)

The `rotate-ccw` leading icon (`Screens.jsx:230`) is **not** in the vendored
Lucide set [measured: `ls crates/render/icons/` → `grid-3x3.svg pause.svg play.svg settings.svg zoom-in.svg`; `grep -rin rotate crates/render/src/icons.rs crates/render/icons/` → no match], exactly like `lab.rs`'s absent shuffle glyph.
Follow the `lab.rs` precedent verbatim: `ResultsScreen` carries an optional
`again_icon: Option<&'a TextureHandle>` set via a builder method; when `None`
the "Race again" `Button` renders **text-only**. This holds `Option<&TextureHandle>`,
so `ResultsScreen` derives `Clone, Copy` but **not `Debug`** (TextureHandle has
no `Debug`), identical to `LabScreen` [measured: `grep -n 'derive(Clone, Copy)' crates/render/src/screens/lab.rs` → `#[derive(Clone, Copy)]` on `LabScreen`].

### Layout (port map)

`show(ui) -> ResultsResponse`, mirroring the sibling opening: install an
`Order::Middle` child layer first (so `Card::show`'s background-layer fill
renders behind the screen content — the documented reason in every sibling)
[measured: `grep -n 'layer_painter(LayerId::background' crates/render/src/widgets/card.rs` → `card.rs:251` region draws card chrome on `LayerId::background()`].

1. `add_space(SPACE_12)` top pad (JSX:207 `padding: '48px …'`).
2. Centered column, `set_width(560)`, `margin = ((available_width - 560)/2).max(SPACE_6)`
   (JSX:207 `maxWidth: 560, margin: '0 auto'`, side gutter 24 = `SPACE_6`).
3. **Header** (centered): mono uppercase eyebrow `"RACE COMPLETE"` (`TEXT_MUTED`),
   then a display-face title built as a two-section `egui::text::LayoutJob` —
   `"You finished "` in `TEXT_INK` + `"P<n>"` in `ACCENT` — centered. `<n>` from
   `player_position`. (egui carries no letter-spacing; the eyebrow renders
   uppercase mono without tracking, consistent with `Card::paint`'s eyebrow.)
   Then `add_space(HEADER_GAP=28)` (JSX:208 `marginBottom: 28`).
4. **Final-standings `Card`** — `Card::new().title("Final standings").grid(true).padding(SPACE_6)`
   (JSX:214 `grid padding="var(--space-6)"`). Body:
   - One row per `StandingEntry` (`STANDINGS_ROW_GAP=10` between rows, JSX:215):
     a `CarChip` (name/color/kind/rank resolved by `standings_rows`) on the
     left, and the mono right-aligned finish-time label `"{:.1}s"` at `FS_SM`
     (=13, JSX:219 `fontSize: 13`) `TEXT_MUTED`.
   - A hairline divider: `SPACE_5(20)` above, a `BW_HAIR` `BORDER_HAIRLINE`
     `hline`, `SPACE_4(16)` below (JSX:223 `marginTop:20 paddingTop:16 borderTop:1px hairline`).
   - Summary `Telemetry` row (`gap = SPACE_6(24)`, JSX:223): `Fastest lap`
     (`tone Accent`, `unit "s"`), `Tempo` (default tone), `Crashes`
     (`tone Danger`), values from `summary_tiles`.
5. **Action row** — `add_space(SPACE_6)` (JSX:229 `marginTop:24`), centered,
   `gap = SPACE_3(12)`: primary "Race again" `Button` (`again_icon` if set,
   else text-only) + secondary "Menu" `Button`. Each click → a distinct flag on
   `ResultsResponse`.

`ResultsResponse { again: bool, menu: bool, again_response: Response, menu_response: Response }`
— not `Copy`/`Debug` (carries `Response`), mirroring `LabResponse`/`RaceResponse`.

### Pure seams (testable, mirror `race.rs`)

Three public pure fns, each with a `#[cfg(test)]` in-module test — the exact
`race.rs::standings_entry` / `lab.rs::oracle_tile_strings` pattern:

- `player_position(&[StandingEntry]) -> Option<u32>` — rank of the `You` entry.
- `standings_rows(&[StandingEntry]) -> Vec<StandingRow>` — one `StandingRow`
  (`name: &'static str`, `color: Color32`, `kind: CarKind`, `rank: u32`,
  `finish_time: String`) per entry, in slice order; `show`'s draw loop consumes
  the **same** fn so the test binds to real behavior. Name via
  `CAR_NAMES.get(i).copied().unwrap_or("Car")`, color via
  `car_color(i).unwrap_or(CAR_COLORS[0])` — where **`i` is `entry.car_index`
  (the car's stable identity), NOT the iteration/enumerate index** of the row
  within the slice. Identity is decoupled from rank/position by design (a player
  can finish P3 with `car_index == 0`), so resolving name/color from the loop
  index would be a regression, not an equivalent. Both lookups are total,
  matching `CarRender::color` [measured: `sed -n '77,84p' crates/render/src/track/car.rs` → `car_color(self.color_index).unwrap_or(crate::tokens::color::CAR_COLORS[0])`].
- `summary_tiles(RaceSummary) -> [String; 3]` — `[fastest_lap {:.1}, tempo {:.2}, crashes]`.

`StandingRow` derives `Clone, Debug, PartialEq`. All three fns are plain (non-`const`):
`player_position` uses `<[_]>::iter().find` (not const-stable), `standings_rows`
allocates a `Vec` + calls `format!`/`car_color` (`car_color` is documented
non-`const` — `<[T]>::get` hits `E0658`) [measured: `sed -n '165,174p' crates/render/src/tokens/color.rs` → `Deliberately not const fn: <[T]>::get is not yet const-stable (attempting const fn here hits E0658)`], and `summary_tiles` calls `format!`. `missing_const_for_fn`
therefore does not fire — same class as `standings_entry`/`car_color`. The
builder setters (`ResultsScreen::new`, `again_icon`) ARE `const fn` (pure struct
literals over `Copy`/refs) — forced by `missing_const_for_fn` (nursery = deny)
[measured: `grep -n 'nursery' Cargo.toml` → `nursery = { level = "deny", priority = -1 }`], mirroring `RaceScreen::new`/`reduced_motion`.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Define `StandingEntry`, `RaceSummary`, `StandingRow`, `ResultsResponse`; implement pure helpers `player_position`, `standings_rows`, `summary_tiles`; add their `#[cfg(test)]` unit tests (AC2/AC3/AC4 data paths). | `crates/render/src/screens/results.rs` | — |
| 2 | Implement `ResultsScreen` builder (`new`, `again_icon`) + `show`: centered column, header (two-section `LayoutJob`), standings `Card` (rows + hairline divider + summary `Telemetry` row), action row; return `ResultsResponse`. Module doc + `# Panics` (font precondition) on `show`. | `crates/render/src/screens/results.rs` | 1 |
| 3 | Wire the module into `screens/mod.rs`: `pub mod results;`, `#[cfg(test)] mod results_gallery;`, and `pub use results::{RaceSummary, ResultsResponse, ResultsScreen, StandingEntry};`. | `crates/render/src/screens/mod.rs` | 2 |
| 4 | Golden gallery + interaction test; mint `results_screen.png` (code-writer runs `image-check` at mint). AC5 (button intents) + AC6 (exact-compare golden, Miri-gated). | `crates/render/src/screens/results_gallery.rs` | 3 |

M = 4 subtasks, all **code** (`*.rs`).

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping (every M ≥ 1). All four
subtasks share the **code** change-type (`*.rs`), have no change-type switch, and
fit within the size cap (≤ 10), so they form **one** group (minimized) which is
also **terminal** (4 ∈ `1..=10`). Default max of 4 groups is not exceeded (1 group).

- **Entry into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
- **Group A** — code group → routes to `subagent_type="code-writer"`, model
  `sonnet` (sonnet-5), effort **`medium` (pinned in frontmatter)**, 1M-token
  window — subtasks 1–4 (code change-type: `*.rs`). Terminal group (4 subtasks;
  within the `1..=10` range).
- No inter-group handoff (single group); Group A completes `/task` Step 8 in its
  own `/context-reset` subagent.

## Risks

- **`rotate-ccw` icon absent from the vendored set** → optional `again_icon`
  handle, text-only default (the `lab.rs::regenerate_icon` precedent). The
  golden renders text-only. — `[measured: ls crates/render/icons/ → 5 svgs, no rotate-ccw; grep -rin rotate → no match]`
- **Text-bearing golden must not be minted at `threshold(0.0)`** (eyebrow,
  title, ranks, names, finish times, telemetry, buttons are all glyphs). Mint at
  `.threshold(1.0).failed_pixel_count_threshold(0)` — the settled text-content
  class, mandated at design time (§ *Read before designing* → text-golden rule).
  — `[derived → the AC6 golden gate discharges exact-pixel fidelity]`
- **Card fill on the background layer** would paint over screen content unless
  the screen installs its own `Order::Middle` layer first — done in `show`,
  mirroring all three siblings. — `[measured: card.rs ~:251 layer_painter(LayerId::background())]`
- **Per-frame `String`/`Vec` allocation** in `standings_rows`/`summary_tiles`
  is acceptable — `lab.rs::oracle_tile_strings` and `race.rs::hud_readouts`
  allocate per frame by the same contract. — `[measured: lab.rs oracle_tile_strings returns [String;4]; race.rs hud_readouts returns (String,String,String)]`
- **Shared-boundary fill/stroke consistency is not applicable here** — the
  screen draws only `Card`/`CarChip`/`Telemetry`/`Button` (each fills and
  strokes its *own* rect, self-consistent) plus raw text and one hairline
  `hline`; it has **no** per-cell fill beneath a separately-smoothed stroke (no
  `render_frame`/track canvas, unlike `race.rs`/`lab.rs`). The enumerated draw
  operations contain no cross-layer shared boundary. — `[derived → clippy + the AC6 exact-compare golden discharge any layout/paint disagreement]`
- **Player has no `You` entry (defensive)** → `player_position` returns `None`;
  header renders `P—` (placeholder), never a panic — consistent with the
  crate's total-fallback posture (`car_color`/`entry name` `unwrap_or`). — `[derived → unit test player_position_none_when_no_you_entry]`

## Test Design

### Subtask 1 — pure helpers (`results.rs` `#[cfg(test)] mod tests`)

Un-gated (no `egui::Context`/painter constructed — same as `race.rs`'s helper
tests). — `[derived → the crate Miri job stays green; these tests build no Context, matching race.rs::tests]`

- **Entry point `player_position`** (AC2):
  - You entry at rank 3 (in a 4-entry rank-ordered fixture) → `Some(3)`.
  - No `You` entry → `None`. Empty slice → `None`.
- **Entry point `standings_rows`** (AC3):
  - Fixture of 4 rank-ordered entries → returned `Vec` length == 4 (chip count
    == car count).
  - Ranks in the returned rows are strictly ascending (`rows.windows(2).all(|w| w[0].rank < w[1].rank)`).
  - **Decoupling assertion (the headline property under test):** the `You` car
    sits at **slice position 2** yet carries `car_index == 0`. Assert
    `rows[2].name == "You"` **and** `rows[2].color == car_color(0).unwrap_or(CAR_COLORS[0])`
    — i.e. name/color track `entry.car_index` (0), NOT the loop/enumerate index
    (2). A regression to positional lookup would resolve `rows[2]` from index 2
    (`"Rival Green"` / `car_color(2)`), so this assert fails loudly on that bug —
    which the old in-order fixture (`car_index == slice position`) could not
    distinguish.
  - `car_index == 2` (slice pos 0) → `"Rival Green"`; out-of-range `car_index`
    (`9`, slice pos 1) → name `"Car"` (total fallback, no panic).
  - `finish_time` formatting: `38.0 → "38.0s"`, `39.6 → "39.6s"`.
- **Entry point `summary_tiles`** (AC4):
  - `RaceSummary { fastest_lap: 12.4, tempo: 0.87, crashes: 1 }` →
    `["12.4", "0.87", "1"]` (values reflect supplied data). A companion assert
    on the three label consts (`"Fastest lap"`, `"Tempo"`, `"Crashes"`) covers
    "labels present".
- Fixtures: a `fn fixture_standings() -> [StandingEntry; 4]` that **deliberately
  decouples `car_index` from slice position** so the AC3 test can prove identity
  resolves from `entry.car_index` and never from the iteration index. In rank
  order (finish_time `38.0 + k*1.6`, `k` = slice position → `38.0/39.6/41.2/42.8`):
  - pos 0 → `rank 1`, `car_index 2`, kind `Ai` (→ `"Rival Green"`),
  - pos 1 → `rank 2`, `car_index 9`, kind `Ai` (out-of-range → `"Car"` fallback),
  - pos 2 → `rank 3`, `car_index 0`, kind `You` (→ `"You"`) — the player finishes
    **P3** with the identity-0 car, at a **non-first** slice position (the exact
    positional-vs-identity discriminator Note 1 requires),
  - pos 3 → `rank 4`, `car_index 1`, kind `Ai`.

  Plus a `RaceSummary` literal — the JSX exemplar values, reused by the gallery.

### Subtask 4 — `results_gallery.rs`

Mirrors `race_gallery.rs` structure (frame-1 install fonts / frame-2 draw;
`Rc<Cell<…>>` click-rect capture).

- **Golden `results_screen_matches_golden`** (AC6): one wgpu frame renders the
  whole `ResultsScreen`; asserts CPU/software adapter (the `race_gallery`
  guard); exact-compare against minted `results_screen.png` with
  `SnapshotOptions::new().threshold(1.0).failed_pixel_count_threshold(0)`
  (text-bearing — mandated above). Canvas ≈ **640 wide** (fits the 560 column +
  ≥ `SPACE_6` side margins) × tall enough to **fully contain** header + standings
  `Card` (4 rows + hairline divider + telemetry row) + action row. **Mint
  guidance:** before running `image-check`, the implementor picks the canvas
  height so every element (down through the action row) is inside the frame — a
  clipped bottom row would still exact-compare green against a clipped golden,
  masking a layout defect, so verify visual completeness at mint time. The
  results canvas is **taller** than `setup_gallery`'s `640×620` (same 560 column,
  but Results stacks more vertical content); the implementor sets the exact size
  when minting. — `[derived → the golden is minted this subtask; image-check verifies the frame against the drawing code before commit]`
  - Miri-gate reason (drives wgpu / `dlopen`s the Vulkan ICD): reuse
    `race_gallery`'s golden reason string verbatim in
    `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)")]`.
- **Interaction `results_again_and_menu_emit_intents`** (AC5): default harness
  (no `render()`). Rest frame → `again == false && menu == false`. Click the
  captured `again_response.rect` center → `again == true`, `menu` unchanged;
  reset, click `menu_response.rect` center → `menu == true`. Uses the same
  `hover/drag/drop` click helper as `race_gallery`.
  - Miri-gate reason (`Harness::builder()` `getcwd`, no `render()` here): reuse
    `race_gallery`'s interaction-test reason string verbatim.
- Fixture reuses subtask-1's `fixture_standings()` + summary (JSX exemplar:
  4 cars, finish times 38.0/39.6/41.2/42.8, fastest 12.4 / tempo 0.87 /
  crashes 1), rendered `again_icon = None` (text-only, since `rotate-ccw` is
  unvendored). — `[measured: icons dir has no rotate-ccw.svg]`

### Cross-cutting (AC7)

Subtask 4 closes with the full gate run: `cargo build`, `cargo test`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`,
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`, and the workspace
Miri job (every new Context/painter-driving test carries `#[cfg_attr(miri, ignore)]`).
Every new public item (`StandingEntry`, `RaceSummary`, `ResultsScreen`,
`ResultsResponse`, `StandingRow`, the three helpers, all setters) gets a `///`.
A `-D warnings` gate aborts on first failure — re-run clippy/doc after the first
clean pass to surface any masked second-class site (§ Rules). — `[derived → the AC7 gate commands discharge these on green]`

## Open questions

- (none) — the Open question in the spec (outcome input-type shape) is resolved
  above (`StandingEntry` slice + `RaceSummary`, `car_index`-indexed identity,
  numeric values formatted at draw, player position derived from the `You`
  entry). The screen is self-contained given the shipped `Card`/`CarChip`/
  `Telemetry`/`Button` widgets.

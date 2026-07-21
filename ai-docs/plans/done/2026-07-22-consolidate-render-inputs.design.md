# Design: Consolidate gp-render frame-immutable inputs into cohesive input struct(s)

**Issue:** #111
**Date:** 2026-07-22

## Approach

A pure plumbing refactor of `gp-render`: replace the loose required-positional
argument lists on `render_frame` and the three screen builders with cohesive
input structs. **No behavior change** — every value fed to the drawing code is
byte-identical; only the calling convention changes.

### The central decision — shape of the bundle

The spec defers the one real decision (AC4 of the issue): **one shared
"scene/frame input" struct** vs. **per-screen input structs**. I investigated
what each surface's required inputs actually *are*
`[measured: rg -Un 'pub fn new|pub fn render_frame' crates/render/src/lib.rs crates/render/src/screens/{lab,race,results}.rs]`:

| Surface | Required frame-immutable inputs | Canvas? |
|---|---|---|
| `render_frame` (`lib.rs:68`) | `track`, `cars`, `reduced_motion`, `overlays` | is the canvas |
| `LabScreen::new` (`lab.rs:170`) | `track`, `phases`, `valid`, `seed` | draws `render_frame(track, &[], false, LAB_OVERLAYS)` — cars/overlays/reduced_motion are **lab-fixed constants, not caller inputs** (`lab.rs:329`) |
| `RaceScreen::new` (`race.rs:151`) | `track`, `cars`, `active`, `overlays`, `laps_done`, `total_laps` (+ `reduced_motion` setter) | draws `render_frame(track, cars, reduced_motion, overlays)` — **all four scene inputs are genuine caller data** (`race.rs:378`) |
| `ResultsScreen::new` (`results.rs:190`) | `standings`, `summary` | **no canvas** — never calls `render_frame` `[measured: rg -Un render_frame crates/render/src/screens/results.rs → (no matches)]` |

The three screens' required inputs genuinely diverge (Lab: phases/valid/seed;
Race: scene + active + laps; Results: standings/summary). Only **one** true
"frame-immutable scene" concept exists — the `render_frame` canvas input
`{track, cars, reduced_motion, overlays}` — and exactly **two** surfaces consume
it: `render_frame` and `RaceScreen`'s canvas.

**Chosen: a hybrid.** Introduce one shared `Scene<'a>` struct (the canvas
input, consumed by `render_frame` **and embedded** in `RaceScreen`), plus three
per-screen required-input structs (`LabInput`, `RaceInput`, `ResultsInput`).
Each screen's `::new` takes exactly one input struct in place of its required
positional list; **optional** inputs (icon handles) stay as `const fn` setters on
the screen builder, matching the spec default and every existing widget builder.

```
Scene<'a>        { track: &TrackArtifact, cars: &[CarRender], reduced_motion: bool, overlays: Overlays }
LabInput<'a>     { track: &TrackArtifact, phases: [PhaseStatus; 7], valid: bool, seed: i32 }
RaceInput<'a>    { scene: Scene<'a>, active: usize, laps_done: i32, total_laps: i32 }
ResultsInput<'a> { standings: &[StandingEntry], summary: RaceSummary }

render_frame(painter: &Painter, rect: Rect, scene: Scene<'_>)
LabScreen::new(input: LabInput<'a>)      + .regenerate_icon / .test_lap_icon setters
RaceScreen::new(input: RaceInput<'a>)    (no setters — reduced_motion folds into Scene)
ResultsScreen::new(input: ResultsInput<'a>) + .again_icon setter
```

**`reduced_motion` folds into `Scene`** (moving it from RaceScreen's optional
setter to a required `Scene` field). The spec permits this ("Design may fold
them in if it produces a cleaner router interface"): `reduced_motion` is a
literal positional arg of `render_frame` and a genuine per-frame scene property,
not a rare tweak like an icon handle; the #23 router assembles one `Scene` value
and hands it to both `render_frame` and `RaceScreen`. No in-tree caller uses the
`.reduced_motion(true)` setter, so removing it is a clean break (AGENTS.md § API
Stability — no compat shim) with zero byte impact
`[measured: rg -Un '\.reduced_motion\(' crates → (no matches)]`.

### Rejected alternatives

- **One god-struct spanning all four surfaces.** A single struct would carry
  Lab's `phases/valid/seed`, Race's `active/laps`, Results' `standings/summary`,
  and the scene fields together — each screen ignoring most of them. Non-cohesive,
  and forcing a screen to ignore a supplied field (e.g. Lab ignoring caller
  `overlays` when it must draw `LAB_OVERLAYS`) is exactly where a byte-identity
  regression hides. Rejected.
- **Fully independent per-surface structs, no sharing.** `render_frame` and
  `RaceInput` would each redefine the same 4-field scene bundle — duplicating the
  precise concept the spec wants consolidated. Embedding a shared `Scene` in
  `RaceInput` eliminates the duplication and is the DRY win. Rejected in favor of
  the hybrid.

### Precedent + constraints honored

- Mirrors the `SetupScreen::new(config: RaceConfig)` precedent — one cohesive
  required-input struct passed by value, public fields, struct-literal
  construction (`screens/mod.rs:120`, `screens/setup.rs:68`).
- **`Copy` preserved.** All three screen builders derive `Clone, Copy` today
  `[measured: rg -Un 'derive\(Clone, Copy\)' crates/render/src/screens/{lab,race,results}.rs → lab.rs:154, race.rs:134, results.rs:177]`.
  `Scene`/`LabInput`/`RaceInput`/`ResultsInput` hold only shared refs + `Copy`
  scalars (`Overlays`, `RaceSummary`, `PhaseStatus`, `CarState` are all `Copy`),
  so all four derive `Clone, Copy` and the screens stay `Copy` (spec § Technical
  constraints).
- **`Debug` added** on all four input structs — every member type is `Debug`
  (`TrackArtifact` `[measured: rg -Un 'pub struct TrackArtifact' -B1 crates/core/src/track.rs → :328 #[derive(Clone, Debug)]]`,
  `CarRender`/`StandingEntry`/`RaceSummary` `[measured: rg -Un 'derive' crates/render/src/track/car.rs crates/render/src/screens/results.rs → car.rs:42 Clone,Copy,Debug; results.rs:49,66 Clone,Copy,Debug,PartialEq]`,
  `Overlays`/`PhaseStatus` derive `Debug`). Free, matches the `RaceConfig`
  precedent, and useful for test diagnostics. (The screen *builders* stay
  non-`Debug` — they hold `TextureHandle`, which the input structs do not.)
- **`const fn` forced.** Each new `::new(input)` is a struct-literal move over
  `Copy` values → const-eligible → `clippy::missing_const_for_fn` (nursery =
  deny) **forces** `const fn`
  `[measured: rg -Un 'nursery|pedantic' Cargo.toml → :62 pedantic deny, :63 nursery deny]`.
  All new `::new` and the retained icon setters are `pub const fn`.
- **Draw-only contract untouched:** no new dependency, no `gp-core`/`gp-gen`/
  `gp-ai` edit; only grouping around existing types changes (spec § Out of scope).
- **`draw_frame` (`track/mod.rs:74`, `pub(crate)`) stays positional** — it is not
  a public multi-arg surface the spec targets, and its only caller is
  `render_frame` `[measured: rg -Un 'draw_frame\(' crates/render/src → track/mod.rs:74 (def), lib.rs:76 (sole call)]`.
  `render_frame` destructures `Scene` into the existing `draw_frame` call. Keeps
  `track/mod.rs` internals unchanged — minimal churn.

## Decomposition

Each subtask is atomic: a signature change plus **every** call site of that
signature, in one commit, leaving the crate compiling and `cargo test -p
gp-render` byte-identical green. All call sites verified against source
`[measured: rg -Un 'render_frame|(Lab|Race|Results)Screen::new' crates/render/src]`.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add `Scene<'a>` to `lib.rs` (next to `Overlays`); change `render_frame(painter, rect, scene: Scene<'_>)`, destructuring into the existing `draw_frame` call; re-export `Scene`. Update all **4** `render_frame` call sites to construct `Scene { .. }`: `lab.rs:329`, `race.rs:378`, `golden.rs:156`, `track/mod.rs:196` (the `render_shapes` test helper). | `src/lib.rs`, `src/screens/lab.rs`, `src/screens/race.rs`, `src/track/golden.rs`, `src/track/mod.rs` | — |
| 2 | Add `LabInput<'a>` in `lab.rs`; change `LabScreen::new(input: LabInput<'a>)` (`const fn`); `LabScreen` holds `{ input, regenerate_icon, test_lap_icon }`; rewrite `show` to read `self.input.{track,phases,valid,seed}`; keep icon setters. Update **2** call sites `lab_gallery.rs:144,197`. Re-export `LabInput` (`mod.rs`, `lib.rs`). | `src/screens/lab.rs`, `src/screens/lab_gallery.rs`, `src/screens/mod.rs`, `src/lib.rs` | 1 |
| 3 | Add `RaceInput<'a> { scene: Scene<'a>, active, laps_done, total_laps }` in `race.rs`; change `RaceScreen::new(input: RaceInput<'a>)` (`const fn`); `RaceScreen` holds `{ input }`; **remove** the `reduced_motion` setter (folded into `Scene`). Rewrite `show` to read `self.input.scene.{track,cars,reduced_motion}` and `self.input.{active,laps_done,total_laps}` in place of every current `self.{track,cars,reduced_motion,active,laps_done,total_laps}` read (`race.rs:227,232,233,241,242,243,247,257`), and feed `draw_toolbar` from `self.input.scene.overlays` (input side, `race.rs:236`). **Canvas overlays threading — mandatory:** `draw_toolbar` returns the live-toggled overlays (`race.rs:236`), which the canvas must use *this frame* (`race.rs:245` feeds the toolbar return, NOT `self.overlays`, into `draw_canvas`). Change `draw_canvas` to take a `Scene` and, at the `show` call site, reconstruct it as `draw_canvas(ui, canvas_rect, Scene { overlays, ..self.input.scene })` — `track`/`cars`/`reduced_motion` come from `self.input.scene` unchanged; **`overlays` stays the `draw_toolbar` return value**, never `self.input.scene.overlays`. Do NOT forward `self.input.scene` wholesale. Update **2** call sites `race_gallery.rs:178,245` to `RaceInput { scene: Scene { .., reduced_motion: false, .. }, .. }`. Re-export `RaceInput`. | `src/screens/race.rs`, `src/screens/race_gallery.rs`, `src/screens/mod.rs`, `src/lib.rs` | 1 |
| 4 | Add `ResultsInput<'a> { standings, summary }` in `results.rs`; change `ResultsScreen::new(input: ResultsInput<'a>)` (`const fn`); `ResultsScreen` holds `{ input, again_icon }`; rewrite `show` to read `self.input.{standings,summary}`; keep `again_icon` setter. Update **2** call sites `results_gallery.rs:102,154`. Re-export `ResultsInput`. | `src/screens/results.rs`, `src/screens/results_gallery.rs`, `src/screens/mod.rs`, `src/lib.rs` | 1 |
| 5 | Full-workspace gate sweep + AC verification: `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings` (re-run after each clean, per the first-failure-masks-later rule), `cargo test -p gp-render` (byte-identical goldens), `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` (AC5), and confirm `git diff --stat Cargo.toml Cargo.lock` is **empty** (AC3). Fix any residual missing `///` / clippy site surfaced. | (verification; any `.rs` needing a doc/clippy touch) | 2, 3, 4 |

Each of subtasks 1–4 carries the doc lines (AC5) for the struct + fields + changed
signature it introduces — docs are written with the code, not deferred to 5.

## Handoff plan

- **(a)/(c)** Grouping is required (M = 5 ≥ 1). The group is entered via a
  `/context-reset` handoff per `.claude/skills/context-reset/SKILL.md`
  § Compaction recovery (re-entry).
- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)**, via the
  `code-writer` subagent, 1M-token window — subtasks **1–5**. Change-type:
  **code** (`*.rs` only) — homogeneous **(e)**. Terminal group; 5 subtasks ∈
  `1..=10` **(d)**. Size ≤ 10 **(b)**.
- **(f) Minimization:** all 5 subtasks are one change-type → **one** group is the
  fewest possible. No handoff between groups (single group).
- **(h) Max-groups:** 1 group ≤ 4 default — no user gate needed.

Marker → routing: **code** group → `subagent_type="code-writer"` (its `model:
sonnet` + `effort: medium` are frontmatter-pinned; no inline override). The
`design`/`design-review`/`self-review` gates stay on Opus.

## Risks

- **Golden byte-identity (AC2).** The refactor feeds `render_frame`/`draw_frame`
  the *identical* values — only the wrapper changes. Goldens are byte-identical by
  construction. Guard: the existing wgpu screen goldens already assert exact compare
  `[measured: rg -Un 'threshold|failed_pixel_count_threshold' crates/render/src/screens/race_gallery.rs → :194 .threshold(1.0), :195 .failed_pixel_count_threshold(0)]`
  and re-run under subtask 5. No new golden is minted, so no new threshold is
  chosen. — `[derived → cargo test -p gp-render green with unchanged golden PNGs]`
- **Interactive-toggle path is gate-invisible — reconstruction of the canvas
  `Scene` is mandatory (`race.rs` only).** The `RaceScreen` canvas is drawn with
  the **`draw_toolbar` RETURN value** `overlays` (`race.rs:236` shadows
  `self.overlays`; `race.rs:245` feeds the return into `draw_canvas`), NOT the
  `Scene`/`self.input.scene.overlays` field — so a live toolbar toggle updates the
  canvas within the same frame. Forwarding `self.input.scene` wholesale would drop
  that threading and render the *pre-toggle* overlays. Static goldens **cannot**
  catch this (no golden toggles a switch ⇒ `draw_toolbar` returns its input
  unchanged ⇒ byte-identical either way), and spec § Out of scope forbids **any**
  rendering-behavior change, not just golden drift. Subtask 3 therefore
  reconstructs `Scene { overlays, ..self.input.scene }` at the `draw_canvas` call
  site (with `draw_canvas` taking `Scene`). Golden byte-identity does **not**
  discharge this path; no automated gate covers it — the reconstruction is a
  design contract, verified by reading the subtask-3 diff, not by a test.
  **Confirmed scoped to `race.rs` only:** `LabScreen` draws with the
  `LAB_OVERLAYS` **constant** (no toolbar, no shadow — `lab.rs:329`) and
  `ResultsScreen` has no canvas, so the shadowing pattern exists nowhere else.
  `[measured: rg -Un 'draw_toolbar|draw_canvas|LAB_OVERLAYS|render_frame' crates/render/src/screens/{race,lab,results}.rs → race.rs:236 (toolbar shadow), :245 (return→canvas); lab.rs:329 render_frame(.., LAB_OVERLAYS); results.rs → no render_frame]`
- **`reduced_motion` fold changes RaceScreen's public API.** The `.reduced_motion()`
  setter is removed; 2 gallery call sites gain `reduced_motion: false` inside
  `Scene`. Clean break, no compat shim (AGENTS.md § API Stability). Zero byte
  impact — no caller passed `true`
  `[measured: rg -Un '\.reduced_motion\(' crates → (no matches)]`. — `[derived → cargo build + cargo test -p gp-render green]`
- **Signature change breaks compile until every call site is updated.** Each
  subtask bundles its signature change with all its call sites in one commit; the
  4 `render_frame` sites + 2-per-screen builder sites are enumerated above from
  source. — `[measured: rg -Un 'render_frame|(Lab|Race|Results)Screen::new' crates/render/src → 4 render_frame calls (lab:329, race:378, golden:156, mod:196) + 6 builder calls (lab_gallery:144,197; race_gallery:178,245; results_gallery:102,154)]`
- **`-D warnings` aborts on first failure, masking later ones.** New `const fn` /
  missing-`///` sites may surface only after earlier ones clear. Subtask 5
  re-runs clippy + doc gates to exhaustion; any out-of-contract class surfaces to
  the orchestrator. — `[derived → clippy + rustdoc gates re-run clean in subtask 5]`
- **No external caller breakage.** `gp-game` does not yet use any of these
  surfaces (#23 is the future consumer). — `[measured: rg -Un 'render_frame|LabScreen|RaceScreen|ResultsScreen' crates/game/src → (no references)]`

## Test Design

This is a refactor with **no behavior change**; the existing suite is the
regression guard and no new test is required.

- **Regression guard:** the full `gp-render` unit suite (`oracle_tile_strings`,
  `hud_readouts`, `active_legal_mask`, `overlays_from_switches`,
  `standings_rows`, `player_position`, `layer_order_is_documented`,
  `render_frame_draws_without_panicking`, …) + the wgpu screen/track goldens
  (`race_screen`, `lab_screen`, `results_screen`, track per-overlay goldens),
  run via `cargo test -p gp-render`. Byte-identical goldens discharge AC2.
- **Miri gates unchanged.** The refactor edits call sites *inside* already-gated
  tests (`render_frame_draws_without_panicking` is `#[cfg_attr(miri, ignore)]`,
  `track/mod.rs:205`) and adds **no** new `egui::Context`/painter test, so the two
  AGENTS.md § Rust Test Conventions Miri triggers impose no new gate. — `[derived → cargo test -p gp-render green + no new #[test] added]`
- **No text-bearing golden is minted** (no new golden at all), so the design-time
  text-threshold rule adds no work; the existing screen goldens keep
  `.threshold(1.0).failed_pixel_count_threshold(0)` untouched.

## Open questions

- **`SetupScreen` symmetry (spec Open question).** Out of scope by default — it
  already takes one cohesive `RaceConfig` (`setup.rs:68`), so it is not a "many
  loose params" surface. This design does **not** touch it. Folding it into a
  `SetupInput` for router symmetry would be a Design Amendment the product owner
  can request without blocking this refactor.

# gp-game: game loop — turn/round order, multi-car resolution, lap/win, replay, full wiring

**Source:** issue #43
**Date:** 2026-07-28
**Tracked in:** #43

Block 3b (design doc §3b, §6). The last build-order item (41/41): the runnable
game loop that wires generation (block 1) → the physics core (block 3a) →
rendering (block 2) through the #42 controller seam. Today `crates/game/src/main.rs`
draws a **hand-built fixture** track with fixture cars/standings and never calls
`gp_gen::generate`, never steps physics, and never polls a controller; `crates/game/src/lib.rs`
exposes the controller seam but the binary does not consume it. This task replaces
the fixture wiring with a real race.

**Round-5 scope expansion (owner directive).** Cross-crate changes to `gp-render`,
`gp-gen`, `gp-core` and any other crate are authorised, as are arbitrary public-API
breaks. Every previously-deferred / out-of-scope / open item is dispositioned in
§ *Scope-expansion dispositions* — nothing is silently dropped.

**API-break posture.** The owner's blanket authorisation lines up with AGENTS.md
§ *API Stability*, which governs here and applies silently: rename / remove /
restructure public API freely, and update every call site in the same PR.

## Scope

### A. The game loop (`gp-game`)

1. **Race state** — an owned, per-race structure holding the generated
   `TrackArtifact`, its `BakedTrackGeometry`, and per-car state: `gp_core::sim::CarState`,
   a `gp_core::sim::LapCounter`, the pending scrub-tick flag from a crash, a
   position trail for `CarRender`, the turn/round counters the Results screen needs,
   and a finished/rank marker.
2. **Round loop** — rounds of turns in a deterministic multi-car turn order.
   Per car, per turn:
   a. compute the action mask — `gp_core::sim::legal_mask` normally, or
      `gp_core::sim::CrashOutcome::action_mask` while that car's scrub tick is
      pending;
   b. **empty mask ⇒ crash**: route straight to `gp_core::sim::resolve_crash`
      **without** polling any controller (the `PollContext` precondition in
      `crates/game/src/controller/mod.rs` — `legal` is documented non-empty);
   c. otherwise poll the car's seat via `Roster::poll` with a `PollContext`
      carrying track / state / mask / this frame's `FrameInput`; a `None` answer
      means "not decided yet, ask again next frame" and the round does not advance;
   d. on `Some(a)`: `gp_core::sim::step`, then register the S/F crossing via
      `LapCounter::register_move(&track.sf, from, to)`.
3. **Crossing-before-collision ordering** — every car's crossing for the round is
   registered from its own `step` chord (2d), **before** any collision resolution
   runs (design §3 "Счётчик кругов": ≤1 per turn, fixed by the move chord, counted
   before collisions; a collision/crash teleport never touches the counter).
4. **Collision resolution** — one `resolve_collisions(&corridor, &mut cars, &mut rng)`
   pass per round over all cars' post-step positions, on a single stream built once
   per race (§ Seed policy).
5. **Win detection and race end** — a car *finishes* when `LapCounter::laps()`
   reaches the configured lap count via a **valid finish** (legal move — guaranteed,
   since only mask-member actions are stepped — *and* a chord crossing S/F in
   `race_dir`), evaluated on the step-1 crossings, never on post-collision
   positions. The race then **plays out the current round** so every car takes an
   equal number of turns, and stops at that round's end. Same-round finishers rank
   by turn order; earlier-round finishers outrank later-round ones.
6. **Background generation worker with cancellation** — `gp_gen::generate` runs off
   the main thread; the UI keeps painting and requests repaints while a job is in
   flight. A superseded or abandoned job is **cooperatively cancelled** (§ B) rather
   than left to run to completion.
7. **Screen wiring** — Setup → Lab → Race → Results with real data, including
   Regenerate (re-runs generation), Race-again (fresh race on the same track), and
   the Results transition on race end.
8. **Replay: in-memory record + on-disk persistence, in TWO playback modes** — a
   per-race record holding the resolved seeds, the resolved race configuration, and
   **every seat's** per-turn action; an in-process replay driver; a **human-readable
   text** format with an explicit version field; `--record <PATH>` /
   `--replay <PATH>` / `--replay-mode <MODE>` CLI flags (§ Replay CLI); and **both**
   playback modes (owner ruling R5-Q2):
   - **headless** — re-runs the recorded race with no window, prints the final
     standings, exits non-zero on divergence. This is the CI-testable path;
   - **gui** — opens the window and plays the recorded race back on screen,
     advancing one turn per fixed interval (§ Playback pacing).
9. **Player-only roster end-to-end** — all of the above runs with a roster of
   `PlayerController` seats only; `gp-game` gains no `gp-ai` edge.

### B. `gp-gen` changes

10. **Cancellation + phase observation on `generate`.** Today
    `generate(params: GenParams) -> Result<TrackArtifact, GenerationError>` runs an
    outer `for _ in 0..seed_budget` loop (Ф1 `phase1_coarse_ring` → Ф2
    `phase2_rasterize` → Ф3 `phase3_start_finish`) wrapping an inner
    `for _ in 0..repair_budget` loop (Ф4 `phase4_static_checks`, Ф5
    `oracle_liveness_v1` / `phase5_full_oracle` / `phase5_runout_checks`, Ф6
    `phase6_local_repair`), with Ф7 = `build_artifact` on the accept path. Two
    additions, both at existing loop boundaries:
    - a **cancellation check** at the top of each seed iteration and each repair
      iteration, returning a distinct `GenerationError` variant when tripped;
    - a **phase-observation hook** reporting per-phase outcomes as the pipeline
      runs, sufficient to drive the Lab screen's seven badges under the
      **aggregate-worst** semantics (owner ruling R5-Q3, § Phase-status ordering).
      Aggregate-worst means the hook must report **every** attempt and repair
      iteration, not just the accepting one — a hook that only fires on the accept
      path cannot satisfy it.
    Note Ф5 is genuinely *conditional*: `should_run_oracle(&issues, liveness)` gates
    `phase5_full_oracle`, so "skipped" is an observed outcome of the real pipeline,
    not an invented status. `oracle_liveness_v1` runs unconditionally each repair
    iteration, so the Ф5 badge aggregates its liveness / full-oracle / run-out
    sub-steps. Ф7 never runs at all on a budget-exhausted run.
    The signature changes; every call site updates in the same PR.

### C. `gp-render` changes

11. **Pending state** — a first-class representation of the window between a
    generation request and the artifact landing, so `gp-game` **never fabricates a
    placeholder `TrackArtifact`**. Today `ShellSession::track` is a non-optional
    `&TrackArtifact`.
12. **Setup-screen error slot** — `SetupResponse` today carries `response` /
    `config` / `generated` and no error channel; a `GenerationError` from the worker
    must be renderable there.
13. **Regenerate intent on `ShellResponse`** — `AppShell::show` consumes
    `LabResponse::regenerate` internally and `Nav::Regenerate` changes no screen, so
    the click is currently invisible to `gp-game`; `ShellResponse` returns only
    `screen` / `advance_rect` / `action`. Surface it.
14. **Seed widened to `u64`** — owner directive. `ShellSession::seed` (`app.rs:379`),
    `LabInput::seed` (`lab.rs:165`), `draw_header`'s parameter (`lab.rs:276`) and the
    `format!("seed {seed}")` label (`lab.rs:309`) all move from `i32` to `u64`, plus
    the pass-through at `app.rs:296` and the fixtures at `lab_gallery.rs:96,157` and
    `app_gallery.rs:134`. The displayed seed is now expected to round-trip into
    `--seed`, which is already a `u64`.
15. **Per-phase status** — the phase-status type gains the variants the real
    observation hook needs (today it is exactly `Ok` | `Repair`, which cannot express
    pending, skipped, or failed) **and a total order**, so "worst across attempts" is
    well-defined (§ Phase-status ordering). A variant alone is not sufficient — it is
    only useful paired with §B's hook. `phase_badge` gains an arm per new variant.
16. **Short-grid notice** — an explicit "seated N of M requested" surface, taken
    rather than the floor, per the pre-authorised escalation.
17. **Results time relabel** — the Results screen's time fields carry **turn counts**
    (owner ruling R2-Q3), so the "seconds" presentation is corrected: the
    `format!("{:.1}s", …)` finish-time string (`results.rs:133`), the `.unit("s")`
    summary tile (`results.rs:383`), and the `SUMMARY_LABELS` entries
    (`results.rs:95`) are updated to a turn-based label, along with the numeric
    formats where a turn count reads better as an integer.

### D. `gp-core` changes

18. **Whatever replay persistence requires, and nothing more.** A persisted record
    contains `Action`, `CarState` and the resolved seed values, all `gp-core` types.
    Either those types gain serialization support in `gp-core` or `gp-game` defines
    mirror types and conversions; the choice (and its dependency implication) is the
    design phase's, but it is authorised either way. `gp-core`'s physics semantics
    do **not** change — serialization support only, no new behaviour.

## Scope-expansion dispositions

Every row from the round-4 `## Deferred` table, `## Out of scope` list and
`## Open questions` section, with its disposition. Nothing dropped silently.

| Prior item | Disposition |
|---|---|
| Replay persistence (format + `--record` / `--replay` flags) | **ABSORBED** — Scope 8 |
| Replay format versioning | **ABSORBED** — Scope 8 carries an explicit version field |
| Widening `ShellSession::seed` past `i32` | **ABSORBED** — Scope 14, owner directive 3 |
| Cooperative cancellation of an in-flight generation run | **ABSORBED** — Scope 10 |
| Real per-phase status on the Lab screen | **ABSORBED** — Scope 10 + 15 (hook *and* variants; the variant alone was never enough) |
| Relabelling Results time fields away from "seconds" | **ABSORBED** — Scope 17 |
| Short-grid notice (the escalation clause) | **ABSORBED** — Scope 16, explicit notice rather than the floor |
| Scope 7c (exposing the Regenerate intent) | **ABSORBED** — Scope 13, now ordinary scope, no longer a flagged entailment |
| Open question: trail length | **CLOSED** — decided (Key decisions) |
| Open question: worker thread count | **CLOSED** — decided (Key decisions) |
| Open question: Generate-while-pending affordance | **CLOSED** — decided (Key decisions) |
| Open question: the § Seed policy | **CLOSED** — stands as decided; design-review may still challenge it |
| Fixed-point bot features (design §5 [M3]) | **DECLINED — belongs to a tracked open issue.** It only has meaning once bot actions exist, i.e. #158 (verified OPEN: "gp-game: AI controller — gp-ai policy sampling at the configured temperature"). Absorbing it would swallow another issue's scope. |
| The AI controller seat / `gp-ai` wiring | **DECLINED — separately tracked.** #158, verified OPEN. Raised as Q-none; not absorbed per the standing constraint. |
| Spectating / free-camera during another seat's turn | **DECLINED on merit.** The roster this task ships is hot-seat: every seat is a human at the *same* screen, so there is no vantage point to spectate *from* — the "other player's turn" is already fully visible to everyone in the room. The feature only acquires meaning with AI opponents (#158) or networked play (untracked). Re-file if either lands. |
| Auto-retry on generation failure / on a short grid | **DECLINED — standing product ruling.** Rejected by the owner at R2-Q2 and R3-Q3; "expand scope" is not a reversal of a product decision. |
| Real-time pacing / animation timers | **PARTIALLY ABSORBED** (round 6). *In:* the replay **playback** advance — one turn per fixed interval on the `gui` replay path (§ Playback pacing). *Still out:* every form of pacing the **interactive** race would need — no turn clock, no per-move timer, no countdown, no auto-advance; the interactive race still advances only when a controller answers. Also still out: animation beyond what `RaceScreen` already performs, and playback transport controls. **This row was missing from the round-5 table — recorded here rather than left dropped.** |
| Changes to the `Controller` / `PollContext` / `Roster` seam (#42) | **STILL OUT.** The widened scope gives no reason to touch it; the seam already accepts an AI seat unchanged. |
| Changes to `gp-core` physics semantics | **STILL OUT.** `gp-core` is touched only for replay serialization support (Scope 18) — no behaviour change. |
| "No `gp-render` change beyond the three authorised surfaces" | **SUPERSEDED** by the round-5 expansion: six `gp-render` surfaces are now in scope (Scopes 11–17). The bound is not removed, only widened — `gp-render` work outside Scopes 11–17 is still out. |
| `gp-gen`'s exclusion from the workspace Miri gate | **DECLINED — separately tracked.** #134, verified OPEN. Referenced by this spec's Miri constraint; not this task's to close. |

## Risks / sizing

**ACCEPTED RISK — the owner chose One PR (R5-Q1) after reading this section and the
four-way split it proposed.** That is an informed override, not an oversight, and it
is not re-litigated here. The section stays because design-review should see the
blast radius it is signing off on, and because the mitigations below are now
*requirements* rather than arguments for splitting.

The round-4 spec was already an `L` with 19 ACs confined to
`gp-game` plus two additive `gp-render` surfaces. The expansion adds: a signature
change to `gp_gen::generate` threaded through every call site; six `gp-render`
surfaces, one of which (Scope 17) **repaints an existing golden**; a serialization
format with CLI flags; and a possible `gp-core` type-level change. Concretely:

- **Golden blast radius is no longer zero.** Scopes 11–16 stay additive and leave
  existing goldens byte-identical — verified: the only rendered seed values are
  `FIXED_SEED = 42` (`lab_gallery.rs:29`) and `seed: 7` (`app_gallery.rs:134`), both
  of which format identically as `u64`. But **Scope 17 changes visible text** in
  `results_screen.png` and any gallery scene drawing the summary row, so those
  goldens must be regenerated through the mint-time verification flow. The
  round-4 "no golden regeneration" guarantee is therefore **narrowed, not kept**.
- **`gp_gen::generate`'s signature change is the widest blast radius** — it is the
  block-1 capstone, called from tests across `gp-gen` and now from `gp-game`.
- **The file-size rule bites.** The loop, the worker, the replay record, the
  persistence layer, the CLI surface and the standings computation cannot share a
  module under the 500/1000-line rule.

**Mitigations, now required rather than proposed.** Since it ships as one PR, the
design phase must make it reviewable from the inside:

- **Sequence the commits along the old split lines** so the history is readable even
  though the PR is not: (i) loop + worker + in-memory replay on today's `gp-render`
  (Scopes 1–7, 9, 11–13); (ii) `gp-gen` cancellation + phase observation + Lab
  per-phase status (Scopes 10, 15); (iii) persistence + both playback modes + CLI +
  `gp-core` support (Scopes 8, 18); (iv) presentation (Scopes 14, 16, 17). Group (i)
  is the one that makes the game runnable and should land first so every later group
  is exercised against a working race.
- **Keep the golden repaint in its own commit** (group iv), so the one commit that
  legitimately changes pixels is trivially separable from the ~24 ACs that must not.
- **`.progress.md` group boundaries follow the same four groups**, giving
  `/context-reset` natural handoff points on a task this size.

## Replay CLI

Settled from the live CLI surface, not invented. [measured: 2026-07-28] `crates/game/src/config/cli.rs`
declares a flat `#[derive(Parser)] struct Cli` under `#[command(name = "graphite-gp", version)]`
with **no subcommands**; every flag is `long`-only; optional flags are already
`Option<u64>` (`--seed-collision`, `--seed-generation`, …); and `--difficulty` is the
in-tree precedent for a **label-parsed enum flag** (`value_parser = parse_difficulty`
over `DIFFICULTY_LABELS`, case-insensitive).

- `--record <PATH>` — `Option<PathBuf>`, mirroring the existing `Option<_>` flag idiom.
- `--replay <PATH>` — `Option<PathBuf>`.
- `--replay-mode <MODE>` — `headless` | `gui`, **default `gui`**, parsed
  case-insensitively in the `--difficulty` style. The binary is a GUI app
  (`eframe::run_native`), so `gui` is the least surprising default; CI passes
  `--replay-mode headless` explicitly.

**A subcommand is deliberately not used**: the CLI has none today, and introducing
one would restructure every existing flag path and its tests for no gain.

**Cross-field errors reuse the existing mechanism.** `config::error::ConfigError`
already carries a cross-field variant (`--block-size` below `⌈cars/2⌉`) rendered
through `Cli::command().error(..)` so it matches `clap`'s own diagnostics. Two new
cross-field rejections join it: `--replay-mode` without `--replay`, and `--record`
together with `--replay`.

## Playback pacing

The GUI playback mode advances the recorded race **one turn per fixed interval**, a
module-level constant, with the shell repainting between advances. Transport
controls (pause / step / scrub / speed) are **not** in scope — each would be a further
`gp-render` surface, and the owner's ruling asked for a playback *mode*, not a
player. Flagged for design-review as the natural first follow-up if playback proves
too fast or too slow to watch.

## Phase-status ordering

"Aggregate all — worst status each phase reached" (owner ruling R5-Q3) requires a
total order. Declare the widened status enum in ascending severity so a derived
`Ord` *is* the ordering (no hand-written `cmp` to drift):

`Pending` < `Skipped` < `Ok` < `Repair` < `Failed`

- **`Pending`** — the run is still in flight and this phase has not been reached yet.
- **`Skipped`** — the run (or attempt) finished without ever executing this phase.
  Real, not hypothetical: Ф5's expensive oracle is gated by `should_run_oracle`, and
  Ф7 never executes on a budget-exhausted run.
- **`Ok`** / **`Repair`** — today's two variants, unchanged in meaning.
- **`Failed`** — the phase produced a blocking issue on some attempt.

The badge for phase `i` is `max` over every attempt and repair iteration. Two
consequences the owner's choice accepts deliberately: a phase that failed on an
early seed and succeeded on the accepted one still shows `Failed`, and `Ok` outranks
`Skipped` because "ran cleanly at least once" is the more informative signal.

## Seed policy

`gp_core::rng::Seeds::from_master(master)` seeds one `Xoshiro256PlusPlus` via
`seed_from_u64(master)` and takes four successive `next_u64()` draws — `collision`,
`generation`, `ai_learning`, `ai_inference`, in that order (verified in
`crates/core/src/rng.rs`). `seed_from_u64` runs its input through SplitMix64, so
adjacent inputs yield decorrelated streams; `wrapping_add(1)` is a sound "next seed"
step, and no OS entropy is involved.

1. **Generation attempts.** Request `k` uses `M_k = configured_master.wrapping_add(k)`
   (`k = 0` for the first Generate, `+1` per Regenerate or later Generate). This is
   what makes Regenerate produce a *different* track — re-running identical
   `GenParams` yields a byte-identical artifact by `gp-gen`'s own determinism tests.
2. **Races on one track.** Race `r` on a given track uses
   `Seeds::from_master(M_k).collision.wrapping_add(r)` (`r = 0` first race, `+1` per
   `Nav::Again`), making "Race again" fresh rather than a rerun.
3. **What the record stores.** The two **resolved `u64` seed values** actually used
   (`generation`, `collision`), so a replay is self-contained and needs no session
   button-press history; the configured master is stored for provenance, and — now
   that the seed display is `u64` — the displayed effective master round-trips into
   `--seed`.

## Key decisions

| Question | Decision |
|---|---|
| Turn order within a round | Fixed roster index order (`0..m-1`), identical every round. Deterministic and independent of the collision stream. |
| Race end (R1-Q2) | **Play out the round**, then stop. Same-round finishers tie-break by turn order. |
| A car that has already finished | Keeps taking its turns to round end; its rank is fixed at the finishing move. |
| Replay storage (R1-Q1, extended round 5) | In-memory record **and** on-disk persistence, with an explicit format version field. |
| What the replay records | **Every seat's** action, uniformly. The issue's "bot-action recording is dead-simple (empty) on a player-only roster" is inaccurate as a *record* rule: on a player-only roster the human actions are the sole non-deterministic input, so an empty record reproduces nothing. |
| Which seeds the record stores | The two resolved `u64` values (§ Seed policy 3), not the configured master alone — Regenerate and Race-again both move the race off it by design. |
| Generation threading (R1-Q3) | Background worker; main thread keeps painting and calls `request_repaint`. |
| In-flight cancellation | **Cooperative**, checked at the two existing loop boundaries in `generate` (per seed iteration, per repair iteration). Supersede and navigate-away both cancel. A distinct `GenerationError` variant reports it; a cancelled job's result is never installed. |
| Pending-window presentation (R2-Q1) | First-class pending state; `gp-game` never fabricates a `TrackArtifact`. |
| Generation-failure presentation (R2-Q2) | Setup-screen error slot; text is the `GenerationError`'s own `Display`. |
| Regenerate (R3-Q1) | Re-runs generation at the next effective master (§ Seed policy 1), reusing the pending state. |
| Race again (R3-Q2) | Fresh race on the same track: re-seat, reset counters, advance the collision stream, discard the previous in-memory record; the track and baked geometry are reused. |
| Short start grid (R3-Q3) | **Seat fewer and race** — `min(cars, positions.len())`; never fail, never retry. Reachable in practice: `gp-gen`'s own `start_grid_degrades_gracefully_when_d_cannot_host_m_cells` (`crates/gen/src/phase3.rs`) pins it. Now surfaced by an explicit "seated N of M" notice (Scope 16). |
| Results time units (R2-Q3) | **Turn counts**: `finish_time` = the finishing turn index; `fastest_lap` = fewest turns any car spent on one lap; `tempo` = `Centerline::length` ÷ turns spent (design §3 "темп круга"); `crashes` = total `resolve_crash` calls. Now **relabelled** to match (Scope 17) rather than shipping a knowingly-wrong "s" suffix. |
| Seed display | `u64` end-to-end (Scope 14); the shown value round-trips into `--seed`. The round-4 "lossy and stays lossy" constraint is **withdrawn**. |
| Where crossings are registered | Immediately after each car's own `step`, inside the per-car turn — ordering vs collisions is structural, not a comment. |
| Crash path never polls a controller | Per the `PollContext` precondition. Empty mask → `resolve_crash` → `scrub` flag → `CrashOutcome::action_mask` → `consume_scrub`. |
| Collision RNG lifetime | One stream per race, threaded through every round; re-deriving per round would replay one shuffle forever. |
| Lap counter initialisation | `LapCounter::new()` (init `-1`) per design §3; `laps()` already clamps at 0. |
| `CarKind` for a player-only roster | Every seat renders as `CarKind::You` — a per-seat property, not a hard-coded index-0 rule. |
| Standings for cars that never finish | After every finisher, ordered by lap counter, then `SField::scalar_at` progress, then car index. Total and deterministic. |
| Replay format + modes (R5-Q2) | **Human-readable text**, and **both** playback modes ship: `headless` (CI-testable re-run, prints standings, non-zero on divergence) and `gui` (on-screen playback). Neither is a follow-up. |
| Replay CLI shape | `--record <PATH>` / `--replay <PATH>` / `--replay-mode <headless\|gui>`, default `gui`, flat flags in the existing idiom — no subcommand. Full rationale and the two new cross-field rejections: § Replay CLI. |
| Playback pacing | One turn per fixed interval on the `gui` path only; no transport controls (§ Playback pacing). The interactive race keeps advancing solely on controller decisions. |
| Phase-status aggregation (R5-Q3) | **Worst across every attempt and repair iteration**, over the total order `Pending < Skipped < Ok < Repair < Failed` supplied by declaration-order `Ord` (§ Phase-status ordering). |
| Trail length *(was open)* | Keep the full per-car trail. Bounded by turns taken, and races are ≤9 laps; no cap until a measurement says otherwise. |
| Worker thread count *(was open)* | Spawn-per-request. Generation is user-initiated and rare; spawn cost is irrelevant beside a multi-second run. |
| Generate-while-pending *(was open)* | The pending state **disables** the controls that raise a request. The supersede-by-generation-id rule remains as the internal invariant for any path that still raises one, and now also cancels the superseded job. |

## Technical constraints

- **Determinism.** `gp-core` is integer-only. Collision and generation streams both
  resolve per § Seed policy.
- **`gp_gen::generate` is not instantaneous** — the measurement behind the worker and
  the cancellation. [measured: 2026-07-28, this machine]
  `cargo test -p gp-gen --lib generate_e2e_cheap_default_suite_has_a_non_empty_centerline`
  runs **two** `generate` calls at the CLI default budgets (`seed_budget = 1`,
  `repair_budget = 8`, `block_size = 6`) in **7.56 s** wall in *debug* (≈3.8 s per
  call); the `#[ignore]`d heavy case is documented at **~467 s debug** for
  `seed_budget = 64` / `repair_budget = 32`. Cancellation granularity is bounded by
  one repair iteration, which at large budgets is not instant.
- **Worker feasibility.** `GenParams` is plain `Copy` data and `TrackArtifact` plain
  owned data (`Corridor` / `Vec<Point>` / `Vec<Wall>` / enums) — both cross a thread
  boundary with no new bounds.
- **Goldens — narrowed guarantee.** `crates/render/tests/snapshots/` holds 15 PNGs.
  Scopes 11–16 are additive and must leave every one byte-identical; the `u64` seed
  widening is verified safe because the only rendered seeds are `42`
  (`lab_gallery.rs:29`) and `7` (`app_gallery.rs:134`). **Scope 17 deliberately
  changes rendered text**, so the Results-drawing goldens (`results_screen.png`, plus
  any gallery scene drawing the summary row — confirm at implementation time) are
  regenerated through the mint-time verification flow, never accepted on sight. The
  in-tree compare setting for a text-bearing frame is
  `.threshold(1.0).failed_pixel_count_threshold(0)` (`crates/render/src/app_gallery.rs`),
  which exempts AA edges only.
- **Miri gate on new `gp-render` tests — mechanical, per [`ai-docs/miri-gate.md`](../miri-gate.md).**
  Any new `gp-render` unit test constructing an `egui::Context` or driving a painter
  (`run_ui` / `layer_painter`, directly or via `render_shapes` /
  `painted_shape_count` / `painted_meshes`) MUST carry
  `#[cfg_attr(miri, ignore = "<why>")]` in the same commit, as must any new wgpu
  golden. The reason names **that test's own** cause; the Context/painter group is a
  wall-clock *cost* gate whose reason must make no abort claim. New `gp-gen` tests
  need no gate while #134 keeps that crate excluded.
- **New dependency edges.** Persistence (Scope 8) and any `gp-core` serialization
  support (Scope 18) may introduce direct dependency edges. Verified starting point:
  no workspace `Cargo.toml` declares `serde` today (`grep -rn serde --include='Cargo.toml' .`
  → no hits), though `cargo tree --invert serde` shows it reachable transitively via
  `eframe` → `egui-winit` → `accesskit_winit` → `accesskit_unix`. Any version added
  follows AGENTS.md § *Dependency Versions* (`0.x` / `x`, no `~`, no patch pin),
  looked up live at implementation time — this spec pins no version. Re-check Miri
  after a new dependency class lands.
- **`legal_mask` empty is the crash predicate.** `resolve_crash`'s doc marks the
  respawn `Coast` guaranteed legal, so `CrashOutcome::action_mask` is never empty.
- **File size.** Soft 500 / hard 1000 lines excluding `#[cfg(test)]` — see § Risks.
- **No `unwrap()` in production** without justification; `expect("reason")` preferred.
  `-D warnings` including `nursery`-level `missing_const_for_fn`.
- **Thread-panic posture.** A panicking worker must not deadlock the UI: join/recv
  errors are handled as a generation failure, never `unwrap`ped.
- **Propagation.** Public-API changes in `gp-render`, `gp-gen` and `gp-core` update
  every call site in the same PR, with no compat aliases.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Rounds run in fixed roster-index turn order; every move comes from `Roster::poll` and is applied with `step`. A test asserts polled seat indices across ≥2 rounds are exactly `0..m-1, 0..m-1`. |
| AC2 | Every action passed to `step` is a member of the mask the seat was given. A test asserts `legal_move(&corridor, state_before, action)` for every applied move of a scripted race. |
| AC3 | Crossings are registered from each car's own move chord **before** `resolve_collisions` for that round. A test teleports a car across S/F via collision resolution and asserts its `LapCounter` is unchanged. |
| AC4 | An empty-mask car is never polled: it routes to `resolve_crash`, its next mask comes from `CrashOutcome::action_mask`, and the scrub tick is consumed once. A test asserts `poll` is not called on the crash turn and the scrub mask is the `Coast` singleton. |
| AC5 | A car finishes exactly when `laps()` reaches the lap count on a legal S/F-crossing move — not a turn early, not on a post-collision position. A test asserts no finish at `N-1`, a finish on the crossing move reaching `N`, and no finish from a same-round collision teleport across S/F. |
| AC6 | Race end plays out the round. A test asserts total turns are a multiple of the seated-car count, a last-turn finish adds no extra round, same-round finishers rank by turn order, and an earlier-round finisher outranks a later-round one. |
| AC7 | Generation runs off the main thread: the request frame returns without waiting, and the artifact installs on a later frame. A headless test asserts spawn-then-poll reports pending, then the artifact, and that a superseded generation id's result is discarded. |
| AC8 | `generate` accepts a cancellation signal and honours it at both loop boundaries, returning the dedicated `GenerationError` variant. A test cancels a long-budget run and asserts it returns cancelled without producing an artifact, and that an uncancelled run is unaffected. |
| AC9 | `generate` reports per-phase outcomes through its observation hook on **every** attempt and repair iteration (not only the accepting one), and the Lab screen renders the aggregate. A test asserts the hook fires for Ф1–Ф7 on both an accepting and a budget-exhausting run, and that a phase which failed on an early seed but succeeded on the accepted one still aggregates to `Failed`. |
| AC9b | The phase-status type's total order is `Pending < Skipped < Ok < Repair < Failed` and comes from declaration-order `Ord`, not a hand-written comparison. A test asserts the ordering pairwise and that `max` over a mixed attempt sequence yields the documented winner, including `Ok > Skipped`. |
| AC10 | `gp-render` renders the request→artifact window **without** the caller supplying a `TrackArtifact`. A test drives a pending frame and a landed frame with no placeholder artifact constructed anywhere in `gp-game`. |
| AC11 | A `GenerationError` from the worker renders in the Setup error slot and clears once a later generation succeeds. |
| AC12 | The Lab Regenerate click reaches `gp-game` through `ShellResponse`, raises a request at `M_{k+1}`, and yields a **different** artifact; the same `k` reproduces the same artifact. |
| AC13 | Race-again starts a fresh race on the same track: cars re-seated, counters reset, collision seed advanced per § Seed policy 2, prior record discarded, `TrackArtifact` reused not regenerated. A test asserts the second race's collision stream differs while the track is identical by value. |
| AC14 | A short start grid seats `min(cars, positions.len())`, the race still runs to a finisher, an explicit "seated N of M" notice is rendered, and no error path is taken. |
| AC15 | `ShellSession::seed`, `LabInput::seed` and `draw_header` are `u64` end-to-end; a seed exceeding `i32::MAX` displays exactly and round-trips into `--seed`. A test asserts the rendered label for such a seed. |
| AC16 | The Results screen presents turn counts with turn-based labels — no "s" suffix on a turn count. A test asserts the formatted strings; the Results-drawing goldens are regenerated through the mint-time verification flow. |
| AC17 | Every golden **other than** the Results-drawing scenes still matches without regeneration, and the full `gp-render` suite is green. |
| AC18 | The loop drives `AppShell` Setup → Lab → Race → Results with `GenParams` from the live `RaceConfig` + CLI budgets, real artifacts on Lab and Race, and `Nav::Finish` into Results with real standings. A headless test exercises the full sequence. |
| AC19 | Results carry real values: `finish_time` the finishing turn index, `fastest_lap` the fewest turns for one lap, `tempo` centerline length over turns spent, `crashes` the `resolve_crash` count — each asserted against hand-computed values on a scripted race. |
| AC20 | In-memory replay: re-running a record (resolved seeds + per-seat actions + race config) reproduces identical final car states, lap counters, standings and summary. The test constructs the record, drops the source race, and replays from the record alone. |
| AC21 | Persisted replay round-trips **headless**: `--record <PATH>` writes a race, `--replay <PATH> --replay-mode headless` reproduces the identical final state in a **separate process**, prints the final standings, and exits zero; a tampered record that diverges exits non-zero. A test asserts the round-trip and that the written file carries an explicit format version. |
| AC21b | The record file is human-readable text: a test asserts the written bytes are valid UTF-8 and that a hand-inspectable field (the format version) is greppable from the raw file. |
| AC21c | `--replay <PATH> --replay-mode gui` plays the recorded race back on screen, advancing one turn per fixed interval and reaching the same final state as the headless mode over the same record. The advance logic is tested headlessly (interval elapsed ⇒ exactly one turn advances); no golden is required for playback. |
| AC21d | CLI surface: `--replay-mode` defaults to `gui`; `--replay-mode` without `--replay` and `--record` together with `--replay` are both rejected as cross-field errors rendered through `Cli::command().error(..)`, matching the existing `--block-size` cross-field diagnostic's style. |
| AC22 | A record whose version field is unrecognised is rejected with a clear error rather than mis-parsed, in both replay modes. |
| AC23 | A player-only roster (`m` `PlayerController` seats) runs end-to-end from generation to Results; `crates/game/Cargo.toml` declares no `gp-ai` dependency. |
| AC24 | `main.rs`'s `fixture_track` / `fixture_cars` / `fixture_standings` / `FIXTURE_SEED` / `FIXTURE_CAR_COUNT` are removed; rendered data comes from generation and the live race, and `--cars` is honoured up to grid capacity (closing #41's known inconsistency). |
| AC25 | Gates: `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` green; every new `gp-render` test matching a mechanical Miri trigger carries `#[cfg_attr(miri, ignore = "<why>")]` in the same commit. |

## Open questions

All three round-5 questions are answered and closed: PR splitting (**One PR**, an
accepted risk — § Risks / sizing), replay format and modes (**text, both modes** —
§ Replay CLI), and phase-status semantics (**aggregate-worst** — § Phase-status
ordering). Nothing on this list is blocking; each is a shape choice inside an
already-settled requirement.

- **`gp-core` serialization shape.** Derive on `gp-core` types versus mirror types in
  `gp-game` (Scope 18). Design's call; both are authorised, and the dependency
  consequence differs between them.
- **Playback interval value.** § Playback pacing fixes the *mechanism* (one turn per
  fixed interval) but not the constant. Design picks a watchable default; transport
  controls stay out.
- **Where the "seated N of M" notice lives** (Scope 16) — Lab status channel versus a
  Race-screen line. Either satisfies AC14.
- Everything else previously open is closed — see § Scope-expansion dispositions.

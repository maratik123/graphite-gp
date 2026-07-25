# gp-game: controller abstraction + player input controller

**Source:** issue #42
**Date:** 2026-07-25
**Tracked in:** #42

## Scope

1. **A controller abstraction in `gp-game`** — the single seam through which every
   car's per-turn `gp_core::sim::Action` is produced. **Poll-shaped** (owner
   ruling R1-Q1): one method returning `Option<Action>`, where `None` means
   *"no answer yet — ask again next frame"*. The player seat returns `None` on
   every frame before it has input; a future AI seat (#158) always returns
   `Some`. Every `Some(a)` it yields is legal by construction, i.e. `a` is a
   member of the `gp_core::sim::legal_mask` for the state it was asked about.
   The abstraction must be implementable by an AI seat with **no change to the
   interface**.

2. **The player controller** — the concrete implementation, fed by two input
   sources: the on-screen MovePad and the keyboard. It never yields an action
   outside the legal mask.

3. **Keyboard input, implemented in `gp-game`** (owner ruling R1-Q3).
   `gp-game` reads arrow keys / WASD / space from egui's per-frame input and
   maps them to `Action`, masked against `legal_mask`. **`gp-render` gains no
   keyboard handling and no knowledge of controllers.**

4. **The non-empty-mask precondition and the `{Coast}`-singleton case**
   (owner ruling R1-Q2). Controllers carry a **documented precondition that the
   legal mask passed to them is non-empty**; the game loop (#43) calls
   `legal_mask` first and routes an empty mask straight to
   `gp_core::sim::resolve_crash` **without asking any controller**. The issue's
   "`Coast` fallback" therefore means exactly one thing: *a singleton `{Coast}`
   mask — including the `CrashOutcome::action_mask` scrub tick — resolves to
   `Some(Action::Coast)` automatically, without demanding a MovePad click or a
   keypress.* It does **not** mean "return `Coast` when nothing else is chosen"
   (see Key decisions: `Coast` is not unconditionally legal).

5. **A heterogeneous roster shape** — the controller collection must hold a mix
   of seats (player now, AI later) and be driven through one uniform call site,
   with no seat-kind branching in the caller.

6. **One non-drawing `gp-render` plumbing edit** (owner ruling R2-Q1): add a
   documented `action: Option<Action>` field to `gp_render::app::ShellResponse`
   and forward `RaceResponse.action` from `AppShell::show_body`'s `Screen::Race`
   arm (every other arm yields `None`). This is the MovePad path's only route
   out of the shell — verified absent today. **No pixels move**, so every wgpu
   golden stays byte-identical.

7. **Remove the declared, unreferenced `gp-ai` edge** from
   `crates/game/Cargo.toml` (owner ruling R2-Q3). #158 re-declares it when it
   first calls the policy. The workspace-root `[workspace.dependencies]`
   `gp-ai = { path = "crates/ai" }` entry **stays** (cargo does not warn on an
   unconsumed workspace-dependency entry, and #158 needs it), and `crates/ai`
   remains an explicit `[workspace] members` entry, so `gp-ai` is still built
   and tested by `cargo build` / `cargo test --workspace`.

## Out of scope

- Policy inference, temperature sampling, feature extraction — anything touching
  `gp-ai` (that is #158). `gp_ai::extract_features` / `gp_ai::policy_action` are
  `todo!()` stubs today; calling either would panic.
- The game loop itself: turn/round order, multi-car resolution, S/F scoring,
  crash resolution, collision resolution, win detection, replay recording,
  Setup→Lab→Race→Results driving. That is #43, which *consumes* this
  abstraction — including the empty-mask pre-check assigned to it by R1-Q2.
- **Any change to `crates/game/src/main.rs`'s behaviour** (owner ruling R2-Q2 —
  "test-only seam"). The fixture track, fixture cars, and their static trails
  stay exactly as they are; `gp_core::sim::step` is still never called from the
  binary, and nothing visibly moves when you run it. Consuming
  `ShellResponse.action` in the binary is #43's.
- Replacing `main.rs`'s hand-built fixture with real `gp_gen::generate` output
  (also #43).
- Any change to the `gp-core` physics API.
- Any change to `gp-render`'s **drawing** code — widgets, screen layout, design
  tokens, or golden PNGs. Scope 6 is a non-drawing plumbing edit only.

## Deferred

| What | Why | Separate issue needed? |
|---|---|---|
| AI controller behind the same trait | Block 4 (`gp-ai`) is stubbed | Already filed — #158 |
| Re-declaring `gp-ai` in `crates/game/Cargo.toml` | Only #158 needs it | Already filed — #158 |
| Consuming `ShellResponse.action` in the binary; the live turn loop and its empty-mask pre-check | Needs turn order, lap scoring, crash/collision resolution | Already filed — #43 |

## Key decisions

| Question | Decision |
|---|---|
| **Per-turn shape** (R1-Q1) | **Poll → `Option<Action>`.** `None` = "no answer yet, ask again next frame"; `Some(a)` = a decided, legal action. Mirrors the existing `RaceResponse.action: Option<Action>` shape. The caller must tolerate a no-progress tick. |
| **`None` is unambiguous** | Because R1-Q2 gives the empty-mask case to the loop's pre-check, `None` carries exactly one meaning (*pending*) and is never overloaded with *"no legal move exists"*. |
| **Empty legal mask** (R1-Q2) | A genuine crash, and **not the controller's problem**. `gp_core::sim::resolve_crash`'s documented precondition is exactly `s.pos() ∈ D` **and** `legal_mask(d, s)` empty. The loop (#43) pre-checks and routes there. This task documents the precondition on the seam and pins it by test. |
| **What "`Coast` fallback" means** | A singleton `{Coast}` mask resolves automatically to `Some(Action::Coast)` without player input. The issue's literal wording ("`Coast` fallback when the legal mask leaves no other choice") is **verified wrong** as a general rule and is restated accordingly — see the next row. |
| Is `Action::Coast` always legal? | **No.** `legal_move` checks the coast chord like any other action: from `v = (3, 0)` two cells from a wall, `Coast` is illegal while `West` (brake) is legal. A blanket "fall back to `Coast`" rule would emit an illegal action and violate AC1. |
| Scrub tick | `gp_core::sim::CrashOutcome::action_mask` already returns `BitFlags::from(Action::Coast)` while `scrub` holds — precisely a singleton `{Coast}` mask, covered by the rule above with **no new `gp-game`-side rule**. |
| Where legality comes from | `gp_core::sim::legal_mask` / `legal_move` only — the same functions `gp-render`'s `RaceScreen` already calls via `active_legal_mask`. The controller layer computes **no** legality of its own; state advance only via `gp_core::sim::step`. No controller-side rule bending, no relaxation for any seat kind (AC7). |
| Which `Action` type | `gp_core::sim::Action` (the 5-variant `#[bitflags]` enum `Coast/East/West/North/South`). No `gp-game`-local action type. `BitFlags` is re-exported from `gp_core::sim`, so `gp-game` needs no direct `enumflags2` dependency. |
| **Key map** (R1-Q3; orientation verified, no flip needed) | `↑`/`W` → `North`; `↓`/`S` → `South`; `←`/`A` → `West`; `→`/`D` → `East`; `Space` → `Coast`. `crates/render/src/track/transform.rs` documents and implements a `y`-flip (lattice `y` increases northward, egui screen `y` downward), and `movepad.rs`'s cell table places `Action::North` at the pad's **top** row with the `↑` glyph — so `North` is visually up on both the track canvas and the pad. |
| Keeping the key map unit-testable | Prefer a pure `egui::Key → Option<Action>` mapping function, separate from the frame-input read, so the AC4 table test needs no egui context and stays Miri-clean. Design may choose otherwise, but must keep AC4 testable without a GPU/kittest harness. |
| Illegal keypresses | Masked exactly like MovePad cells: a key naming an action outside `legal_mask` is a **no-op** — it does not decide the turn (AC3). |
| Reuse of `gp-render`'s existing masking | `MovePad::show` gives illegal cells no `Sense::click`, so clicking one is already a structural no-op (`crates/render/src/widgets/movepad.rs:251`), and `RaceScreen::show` computes the mask itself via `active_legal_mask`. The player controller **consumes** that masked result; it does not re-implement masking. |
| **`ShellResponse.action` has no in-binary consumer this task — accepted** (reconciles R2-Q1 with R2-Q2) | This is a **#43-facing seam, proven by test, not by the binary**, and it raises no lint problem: `gp-render` is a **lib-only crate** (no `[[bin]]`/`[lib]` section in `crates/render/Cargo.toml`) and `ShellResponse` is `pub` and re-exported at `crates/render/src/lib.rs:31`, so a new `pub` field is reachable from the crate root and `dead_code` does not fire. The field is **populated on every code path** (the `Screen::Race` arm forwards `resp.action`; the other three arms yield `None`), so no "never constructed / never read" analysis applies. The workspace's `missing_docs = "deny"` means the new field needs a `///`. AC10 makes the forwarding itself verifiable, so the field is never merely decorative. |
| Same-frame input precedence | Deterministic and documented: at most one action is decided per frame. `gp-render`'s existing precedence is preserved as-is (a Coast-button click wins over a MovePad change, `race.rs:252`), and the `gp-game` keyboard read is resolved against it in one documented order, pinned by a test. The exact ordering is the `design` Subagent's call. |
| Determinism | No RNG on the player path (issue *Test notes*). The player controller is a pure function of the mask plus the frame's input. |
| Module placement / lib-vs-bin target | The `design` Subagent's call. **The bin-only constraint does not block the test plan:** `crates/game` has a single `[[bin]]` target and no lib target, so `crates/game/tests/*.rs` cannot `use` in-crate items — but in-crate `#[cfg(test)] mod tests` compiles and runs inside the bin target, and **42 such tests already exist and run** under `crates/game/src/` (verified 2026-07-25: `grep -rn '#\[test\]' crates/game/src` → 42). Every `gp-game` AC below is therefore reachable from in-crate tests; `tests/cli.rs` stays a process-level spawn harness. Design may still propose a lib target, but is not required to. |
| Naming | Design's call (`Controller`/`Pilot`/`Driver`, `poll`/`decide`/`choose_action`). AGENTS.md § *API Stability* permits free renaming later. |

## Technical constraints

- **`gp-core` surface actually available** (read from `crates/core/src/sim/mod.rs`,
  2026-07-25): `CarState { x, y, vx, vy }`; `Action` + `Action::accel()`;
  `legal_move(&Corridor, CarState, Action) -> bool`;
  `legal_mask(&Corridor, CarState) -> BitFlags<Action>`;
  `step(CarState, Action) -> CarState`; `resolve_crash(&Corridor, CarState) ->
  CrashOutcome`; `CrashOutcome::{action_mask, consume_scrub}`;
  `resolve_collisions`; `LapCounter`.
- **`step` performs no legality check.** Passing an illegal action is documented
  as unsupported. This is why "legal by construction" must hold at the seam.
- **The player's answer is frame-asynchronous.** `gp-game` owns the window and
  the `eframe` loop (`main.rs`, design §6); `RaceScreen::show` yields
  `action: Option<Action>` **per frame** and is `None` on every frame the player
  has not clicked. This is what the poll shape accommodates.
- **The MovePad route out of the shell does not exist yet** (verified
  2026-07-25 — the reason for Scope 6). `crates/render/src/app.rs`'s
  `Screen::Race` arm consumes only `resp.overlays` and `resp.finish` and
  **discards `resp.action`**; `ShellResponse` is `{ screen, advance_rect }`. The
  workspace's only consumer of `RaceResponse.action` is `gp-render`'s own
  `race_gallery.rs:230`. `main.rs` discards the shell's return entirely
  (`let _ = self.shell.show(…)`) and, per R2-Q2, continues to.
- **No keyboard input exists in `gp-render`**: `movepad.rs` and `race.rs` contain
  no `egui::Key` / `egui::Event` handling (verified 2026-07-25). Per R1-Q3 none
  is added there — the reads live in `gp-game`.
- **Removing the `gp-ai` edge cannot break the build**: no `gp_ai` path is
  referenced under `crates/game/src` or `crates/game/tests` (verified
  2026-07-25: `grep -rn 'gp_ai' crates/game/src crates/game/tests` → no match),
  and `gp-ai`'s only dependency is `gp-core`, which `gp-game` already declares
  directly — so no transitively-required crate leaves `gp-game`'s graph.
- **Workspace lint posture** (root `Cargo.toml`): `missing_docs = "deny"`,
  `rustdoc::broken_intra_doc_links = "deny"`, clippy `pedantic` + `nursery`
  denied as groups, plus `arithmetic_side_effects = "deny"`. Every new public
  item **and the new `ShellResponse` field** needs a `///`.
- **AGENTS.md gates apply**: `cargo clippy --workspace --all-targets -D warnings`,
  `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc`, `thiserror` for
  any new error type, no `unwrap()` in production code without justification,
  file-size soft caps 500/800.
- **Miri**: the controller, mask, and key-map tests must be Miri-clean and must
  **not** be gated. Only a test that spawns a process or touches FFI/GPU takes a
  per-test `#[cfg_attr(miri, ignore = "<its own cause>")]` — e.g. if AC10 is
  discharged with an `egui_kittest` interaction test, it inherits the same
  `getcwd`/wgpu gating the existing `app.rs` click-through test carries.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | A single controller abstraction in `gp-game` yields `Option<Action>` per car per poll from a `CarState` + track context: `None` = undecided this tick, `Some(a)` = decided. Every `Some(a)` satisfies `gp_core::sim::legal_move` for the state it was asked about. |
| AC2 | The player controller never yields a `Some(a)` outside `legal_mask(&track.corridor, state)`. Verified across a table of `CarState`s including at least: zero velocity mid-corridor; wall-adjacent with a restricted mask; a state whose mask **excludes** `Coast` (fast approach to a wall); and a singleton-`{Coast}` mask. |
| AC3 | Illegal inputs are no-ops on both input paths: an illegal MovePad cell is not selectable, and a key naming an action outside the legal mask does not decide the turn (the poll still returns `None`). |
| AC4 | The keyboard map is implemented **in `gp-game`** and pinned by a table test that needs no GPU/kittest harness: `↑`/`W`→`North`, `↓`/`S`→`South`, `←`/`A`→`West`, `→`/`D`→`East`, `Space`→`Coast`, each gated by the legal mask. |
| AC5 | A singleton `{Coast}` mask — including the one produced by `CrashOutcome::action_mask` during a scrub tick — resolves to `Some(Action::Coast)` on the first poll, with no MovePad click and no keypress. Pinned by test. |
| AC6 | The seam's **non-empty-mask precondition** is documented on the public item(s) and pinned by test: the empty-mask (crash) case is the caller's to pre-check and route to `resolve_crash`; the controller is never asked. |
| AC7 | The controller layer contains no independent legality or physics logic: legality comes only from `gp_core::sim::legal_mask` / `legal_move`, state advance only from `gp_core::sim::step`. No controller-side rule bending, and no relaxation for any seat kind. |
| AC8 | The controller collection admits a heterogeneous roster: a test constructs a roster mixing the player controller with a second, non-player stub implementation of the same abstraction and drives both through one uniform call site with no seat-kind branching. |
| AC9 | Deterministic: no RNG on the player path. Replaying the same sequence of player inputs against the same states yields an identical action sequence. |
| AC10 | `gp_render::app::ShellResponse` carries a documented `action: Option<Action>`; a test proves the `Screen::Race` arm forwards `RaceResponse.action` into it (a MovePad selection reaches the shell's caller) and that a non-Race screen yields `None`. |
| AC11 | **No `gp-render` drawing change:** `git diff --stat` on the branch shows no edit to `crates/render/src/widgets/**`, `crates/render/src/screens/**`, `crates/render/src/tokens/**`, or any `*.png`; every existing wgpu golden is byte-identical and the golden tests are green. |
| AC12 | **No `main.rs` behaviour change** (R2-Q2): the fixture track/cars/trails are unchanged, `gp_core::sim::step` is still never called from the binary, and `crates/game/tests/cli.rs`'s three process-level tests pass unmodified. |
| AC13 | **`gp-game` carries no AI dependency**, proven mechanically rather than by running a race: (a) `cargo tree -p gp-game` contains no `gp-ai` line; (b) `grep -rn 'gp_ai' crates/game/` returns no match; (c) `cargo build -p gp-game` succeeds and `cargo test -p gp-game` passes. *(This replaces the issue's "a player-only race runs with no AI dependency" — under the R2-Q2 test-only-seam ruling the binary runs no race at all, so the original wording is not verifiable by this task; the live race is #43's AC.)* |
| AC14 | Gates green: `cargo build`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`. |

## Open questions

None blocking design. Remaining latitude, all explicitly the `design` Subagent's
to settle:

- **Roster ownership** — whether the roster type lives in the controller module
  or is assembled by #43's loop. Either satisfies AC8.
- **Same-frame input precedence order** — a deterministic, documented order is
  required (Key decisions); which order is design's call.
- **How AC10 is discharged** — a Miri-clean structural test over the Race arm's
  mapping, or an `egui_kittest` interaction test (which then inherits the
  existing click-through test's Miri gating).
- **Module placement and naming** — including whether `gp-game` gains a lib
  target; not required, since in-crate bin tests are proven reachable.

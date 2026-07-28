# Design: gp-game game loop — turn/round order, multi-car resolution, lap/win, replay, full wiring

**Issue:** [#43](https://github.com/maratik123/graphite-gp/issues/43)
**Spec:** [`ai-docs/plans/2026-07-28-game-loop-orchestration.spec.md`](2026-07-28-game-loop-orchestration.spec.md)
**Date:** 2026-07-28

All owner rulings (R1-Q1 … R5-Q3) are treated as settled. This design proposes
mechanisms *inside* them; it reverses none, and raises no design-blocking STOP.

---

## Approach

### The central structural decision — one loop, two controller kinds

The interactive race and both replay playback modes drive **the same** round
loop through **the same** `Controller` seam (#42, untouched). A replay is a
roster of `ReplayController` seats that answer from a recorded action stream;
an interactive race is a roster of `PlayerController` seats that answer from
`FrameInput`. Nothing else differs.

This is what makes AC20 ("replaying reproduces identical final car states, lap
counters, standings and summary") true *by construction* rather than by a
parallel re-implementation that must be kept in sync. It also means Scope 8's
`gui` and `headless` playback modes share their entire simulation path and
differ only in **what advances the loop** (a `PlaybackClock` vs. `run_native`'s
frame pump) and **whether a window is opened**.

Rejected alternative: a separate `replay::simulate()` re-runner. It is the
shape the spec's wording most directly suggests, and it is the shape that
silently diverges the first time the live loop's ordering changes — exactly the
"bots play the same game" hazard `docs/design.md` §3a exists to prevent.

### The loop is a per-frame state machine, one seat per frame

`Controller::poll` returns `Option<Action>` where `None` means "ask again next
frame" (`crates/game/src/controller/mod.rs:66-74`, read 2026-07-28). A round
therefore cannot be a `for` loop over seats inside one frame: seat 0 may answer
on frame 12 and seat 1 on frame 40.

`race::round::TurnCursor` advances at most one seat per call:

1. take seat `cursor`; compute its mask — `legal_mask` normally, or
   `CrashOutcome::action_mask` while its scrub tick is pending (Scope 2a);
2. **empty mask ⇒ crash**: call `resolve_crash` directly, bump the crash
   counter, store the `CrashOutcome`, advance the cursor — **no `Roster::poll`**
   (Scope 2b; the `PollContext` non-empty-`legal` precondition,
   `controller/mod.rs:39-46`);
3. otherwise `Roster::poll` with a `PollContext` carrying track/state/mask/this
   frame's `FrameInput`; `None` ⇒ return `Pending`, cursor unchanged (Scope 2c);
4. `Some(a)` ⇒ apply, then **immediately** `LapCounter::register_move(&track.sf,
   from, to)` for that car's own chord, then record the action, then advance the
   cursor (Scope 2d + Scope 3).

When the cursor wraps past the last seat: run **one**
`resolve_collisions(&corridor, &mut states, &mut race_rng)` over all seats
(Scope 4), increment the round, then evaluate finishes. Because every crossing
in step 4 was registered before any collision ran, AC3 holds *structurally* —
there is no ordering comment to drift (spec § Key decisions, "Where crossings
are registered").

Race end (Scope 5): a car finishes when `laps()` reaches `total_laps` **at step
4**, on its own legal chord. The round then plays out to the cursor wrap and
the race stops there, so every seat has taken an equal number of turns (AC6).

**Scrub turns use `consume_scrub`, not a bare `step`.** The Key decision names
the chain `resolve_crash → scrub flag → CrashOutcome::action_mask →
consume_scrub` explicitly, so the design follows it: on a scrub turn the mask is
the `{Coast}` singleton, `PlayerController::decide` auto-resolves it
(`player.rs:29-31`, read 2026-07-28), and the outcome is applied via
`CrashOutcome::consume_scrub()` — whose body is `step(state, Action::Coast)`
(`sim/mod.rs:406`, read 2026-07-28), so AC2's "every action passed to `step`
is a mask member" holds for it too. `Action::Coast` is what gets recorded for
that turn, so replay reproduces it identically.

### `gp-gen`'s new signature — one observer trait, not two closures

```rust
pub trait GenObserver {
    fn is_cancelled(&self) -> bool { false }
    fn on_phase(&mut self, event: PhaseEvent) {}
}
impl GenObserver for () {}

pub fn generate(params: GenParams, obs: &mut dyn GenObserver)
    -> Result<TrackArtifact, GenerationError>;
```

One parameter instead of two; `&mut ()` is the no-op call for every existing
test. Rejected: two separate `&dyn Fn` / `&mut dyn FnMut` parameters (a
three-argument `generate` with two closure types is noisier at every call site
and cannot grow a third hook without another break); and a `GenHooks<'a>`
struct (same arity, but the borrow split between an immutable `cancel` and a
mutable `on_phase` forces callers to split their own state).

**The blast radius is one file, not "tests across `gp-gen`".** Every call site
of `generate(` in the workspace is inside `crates/gen/src/generate.rs`'s own
`#[cfg(test)]` module.
[measured: 2026-07-28, `rg -n "generate\(" --glob '*.rs' crates/ | grep -v "fn generate|phase|racing"` → 7 hits, all `crates/gen/src/generate.rs:{237,246,304,305,341,342,373}`, plus one false positive `crates/render/src/app.rs:661` (`fn nav_guard_before_first_generate`)]
This narrows — it does not contradict — the spec's § Risks framing.

### Per-phase outcome table (Scope 10, AC9)

`gp-gen` gains `Phase { F1..F7 }` and `PhaseOutcome { Skipped, Ok, Repair,
Failed }` (no `Pending` — the pipeline never *reports* pending; `Pending` is the
Lab-side initial value). Emission, per seed attempt `k` / repair iteration `j`:

| Phase | When emitted | Outcome |
|---|---|---|
| Ф1 `phase1_coarse_ring` | once per seed attempt | `Ok` |
| Ф2 `phase2_rasterize` | once per seed attempt | `Ok` |
| Ф3 `phase3_start_finish` | once per seed attempt | `Ok` |
| Ф4 `phase4_static_checks` | every repair iteration | `issues.is_empty()` → `Ok`, else `Failed` |
| Ф5 (liveness / oracle / run-out) | every repair iteration | `!liveness` → `Failed`; `should_run_oracle == false` with `liveness == true` → `Skipped`; `NotLappable` → `Failed`; `Lappable` + clean run-out → `Ok`; `Lappable` + dirty run-out → `Repair` |
| Ф6 `phase6_local_repair` | every repair iteration | not reached (accept path) → `Skipped`; `Repaired` → `Repair`; `Failed` → `Failed` |
| Ф7 `build_artifact` | once per run | accept path → `Ok`; `Err(SeedBudgetExhausted)` return → `Skipped` |

**Terminal rule — a phase that emitted nothing resolves to `Skipped`, never
`Pending`.** The table above emits no event for Ф4/Ф5/Ф6 when the repair loop
body never executes (`repair_budget == 0`) nor for Ф1–Ф6 when the seed loop
never executes (`seed_budget == 0`) — `crates/gen/src/generate.rs:110,116` are
plain `for _ in 0..budget` loops, so a `0` budget skips the body entirely
[measured: 2026-07-28, read `crates/gen/src/generate.rs:110-158`]. Leaving those
phases at `Pending` would contradict spec § Phase-status ordering, which defines
`Pending` as "the run is **still in flight**" and `Skipped` as "the run (or
attempt) **finished** without ever executing this phase". The rule is therefore
pinned **on the `gp-game` side**: when a run terminates (either arm), every
phase whose aggregate is still `Pending` becomes `Skipped`. Chosen over a
terminal `Skipped` sweep inside `generate` because it needs no
which-phases-have-I-emitted bookkeeping in `gp-gen`, it covers the `Ok` and
`Err` terminations uniformly, and it stays total for any phase added later
without an emission site. `gp-gen`'s raw event stream is left purely factual —
it reports what ran, and nothing about what did not. Note the CLI cannot reach
the `0` case at all (`SEED_BUDGET_MIN` / `REPAIR_BUDGET_MIN` are both `1`,
`crates/game/src/config/mod.rs:68,74` [measured: 2026-07-28]), so this rule is
the defined answer for a library-level call and a future-proofing guarantee, not
a shipped code path.

Ф5's shape is grounded in the real pipeline, not invented: `should_run_oracle(&issues, liveness)`
gates `phase5_full_oracle`, and `oracle_liveness_v1` runs unconditionally each
repair iteration
[measured: 2026-07-28, read `crates/gen/src/generate.rs:110-158`]. Ф7's
`Skipped` on the failing return is what makes the spec's "Ф7 never runs at all
on a budget-exhausted run" an *observed* status rather than a stuck `Pending`.

Aggregation (`max` over every event) lives in **`gp-game`**, not `gp-gen`:
`gp-gen` reports raw events; `gp-render` supplies the ordered `PhaseStatus`;
`gp-game` is the only crate that sees both. `gp-gen` gains no `gp-render` edge.

### Cancellation, the worker, and the pending window

`gen_worker::Worker` is spawn-per-request (Key decision — generation is
user-initiated and rare). Each request carries a monotonic `GenerationId`, an
`Arc<AtomicBool>` cancel flag, and an `mpsc::Sender<WorkerMsg>`. The worker
thread's `GenObserver` impl reads the flag in `is_cancelled` and folds
`on_phase` events into a **local** `[PhaseStatus; 7]` aggregate, sending
`WorkerMsg::Phases(snapshot)` **only when the aggregate changes**. That bounds
channel traffic to ≤ 35 messages per run (7 phases × 5 monotonically-increasing
statuses) instead of ~4·`seed_budget`·`repair_budget` — up to ≈4.2 M at the
CLI's `1024`/`1024` ceilings
[measured: 2026-07-28, `SEED_BUDGET_MAX`/`REPAIR_BUDGET_MAX` = 1024 each,
`crates/game/src/config/mod.rs:72,76`].

Supersede and navigate-away both set the cancel flag; a result whose
`GenerationId` is not the session's current one is dropped (AC7). A panicking
worker drops its `Sender`, so the main thread observes `Disconnected` without a
`Done` and reports it as a generation failure — never `join().unwrap()`
(spec § Technical constraints, thread-panic posture).

*Recorded post-Group-A, descriptive only — this documents what A6 shipped, and
adds no obligation beyond the posture sentence above.* A6 satisfies that posture
with a **stronger** mechanism than the minimum it requires: the worker thread
body wraps the call as
`std::panic::catch_unwind(AssertUnwindSafe(|| gp_gen::generate(params)))` and
maps `Err(_panic)` to `GenerationFailure::WorkerLost`, which is then `send`
normally (`crates/game/src/gen_worker.rs:102-110`, read 2026-07-28). A panic
therefore arrives as an ordinary typed failure message rather than as a dropped
`Sender`, which makes the `Disconnected`-without-a-`Done` handling above the
residual fallback (a thread lost some other way, or a dropped receiver) instead
of the primary panic path. Both are consistent with the posture — no
`join().unwrap()`, every failure mode surfaced as a generation failure — and the
`send` itself is `let _ = …`, so a dropped receiver cannot panic the worker
either.

`ShellSession::track` becomes:

```rust
pub enum TrackView<'a> {
    Pending,
    Ready { track: &'a TrackArtifact, geometry: &'a BakedTrackGeometry },
}
```

Folding `geometry` into `Ready` removes the possibility of a
present-track/absent-geometry pair, which the current two-field shape allows.
`AppShell::show_body`'s Lab and Race arms draw a private `draw_pending` body
(centered "Generating track…" card) on `Pending`, returning that card's rect as
`advance_rect`. **No new public screen module and no new golden**: AC10 needs a
driven pending frame, not a snapshot, and AC17 protects the existing 15 PNGs.

**Generate-while-pending needs no new `gp-render` surface.** The Key decision
says the pending state "disables the controls that raise a request". The pending
body *replaces* Lab's action row, so Regenerate and Test-lap are structurally
absent while pending — the disable is achieved by not drawing them. The Setup
path stays reachable via the top bar, and a second Generate there is handled by
the supersede-by-generation-id rule the same Key decision explicitly retains
("remains as the internal invariant for any path that still raises one, and now
also cancels the superseded job"). This keeps `gp-render` inside Scopes 11–17;
a `SetupScreen` disabled-button affordance would be a **seventh** surface, which
§ Scope-expansion dispositions still rules out.

### `ShellResponse` surfaces the whole navigation intent, not just Regenerate

Scope 13 asks for the Regenerate click to reach `gp-game`. The design adds
`pub nav: Option<Nav>` rather than `pub regenerate: bool`, because the loop
independently needs `Nav::Generate` (raise a request), `Nav::Again` (fresh race
per § Seed policy 2, AC13) and `Nav::Menu` (navigate-away cancels). One field
answers all of them; a bool would be joined by three more within the same PR.

### Replay format, and why `gp-core` gets two derives and nothing else

**Resolves spec Open question 1.** `gp-core` gains
`#[derive(strum::Display, strum::EnumString)]` on `sim::Action`. That is the
whole of Scope 18. `CarState` (four `pub i32`) and `Seeds` (four `pub u64`) are
written field-by-field by `gp-game`'s own line formatter and need no `gp-core`
change at all.

- **No new dependency edge anywhere.** `strum` is already a direct `gp-core`
  dependency with the `derive` feature
  [measured: 2026-07-28, `crates/core/Cargo.toml` `[dependencies] strum = { workspace = true }`;
  root `Cargo.toml` `strum = { version = "0.28", features = ["derive"] }`], and
  `strum` 0.28 re-exports `Display`/`EnumString` under that feature
  [measured: 2026-07-28, `~/.cargo/registry/src/index.crates.io-*/strum-0.28.0/src/lib.rs:234-235`
  → `#[cfg(feature = "derive")] pub use strum_macros::*;`, with `Display` and
  `EnumString` named in the `DocumentMacroRexports!` list at `:253,:259`].
- **It compiles and lints clean under the workspace posture, including
  alongside `#[bitflags]`.**
  [measured: 2026-07-28, probe — added both derives to `Action`, then
  `cargo build -p gp-core` → `PROBE-GREEN` and
  `cargo clippy -p gp-core --all-targets -- -D warnings` → `PROBE-CLIPPY-GREEN`;
  edit reverted via cp-backup, `git status --short` → clean]
- **No existing `Display`/`FromStr` impl to collide with.**
  [measured: 2026-07-28, `rg -n "for Action\b" --glob '*.rs' crates/` → no hits,
  corroborated by a raw read of `crates/core/src/sim/mod.rs:33-80` (the `Action`
  declaration plus its single `impl Action { accel }` block) and of the file's
  full `^pub` outline, which lists no trait impl on `Action`]

Rejected — **`serde` derives on `gp-core` types**: `serde` is a direct
dependency of no workspace crate today
[measured: 2026-07-28, spec § Technical constraints, re-confirmed
`rg -n '^serde' --glob 'Cargo.toml' .` → no hits], so this adds a real edge to
the physics core plus a proc-macro compile cost, in exchange for a derive-driven
format the design does not use (the record is a hand-written line format, chosen
for AC21b's greppability). It would also require re-checking the Miri gate for a
new dependency class (AGENTS.md § Rust Test Conventions).

Rejected — **mirror types in `gp-game`**: duplicating `Action`'s five variants
creates exactly the drift class `DIFFICULTY_LABELS`' in-tree drift-guard test
exists to prevent (`crates/render/src/screens/mod.rs:188-196`), and costs a
conversion plus its own test — more code than one derive.

**Record shape** (human-readable text, AC21b; `\n`-separated, one directive per
line, `#`-prefixed comments allowed):

```
graphite-gp-replay 1
master <u64>
seed-generation <u64>
seed-collision <u64>
cars <u32>  laps <u32>  v-target <i32>  difficulty <Rookie|Pro|Ace>
min-straight <i32>  block-size <i32>  seed-budget <u32>  repair-budget <u32>
seats <usize>
processed <u32>
turn <round> <seat> <Coast|East|West|North|South>
...
final <seat> <x> <y> <vx> <vy> <lap-raw>
```

The track is **regenerated** from `seed-generation` + the `GenParams` fields,
not persisted — `ai-docs/code-style.md` § Deterministic collections states the
contract directly ("a replay stores only the seed and *regenerates* its track",
`docs/design.md` §2 `[N4]`), and `gp-gen`'s own determinism tests pin it
(`generate_e2e_cheap_default_suite_has_a_non_empty_centerline`,
`crates/gen/src/generate.rs:338-356`).

**`processed <u32>` — a design gap the implementation caught (C4, added
post-Group-C).** The round-1 grammar sketch above had no such line, and that was
wrong: `turns.len()` counts only turns that produced an action, while a **crash
turn consumes a turn and emits no `turn` line** (see the two paragraphs below).
A record capped by an external turn budget rather than ending at `RaceOver` must
therefore replay to the same point and stop; driving it with a `max_turns`
derived from `turns.len()` runs past the last recorded action and registers a
**false divergence**, which is exactly what `crates/game/tests/replay.rs` hit.
`ReplayRecord::total_processed_turns` counts every `Moved` **or** `Crashed`
outcome and is persisted as this line; the headless driver uses it as its exact
bound (`crates/game/src/replay/mod.rs:76-86` + `format.rs:114,154`, read
2026-07-28).

Each `turn` line carries its `<round> <seat>` coordinates, so the replay driver
can assert the next record line matches the turn it is actually about to take.
A tampered or reordered stream is therefore detected as a **divergence** rather
than silently mis-applied. Crash turns emit no `turn` line (they poll no
controller and are recomputed deterministically), so a desync there is caught by
the same coordinate check.

Divergence detection is layered, all non-zero-exit. **Amended post-Step-10 —
see § *Design Amendment 1* immediately below for the trigger.** Each layer names
where it lives, what it compares, and the error it produces:

| Layer | Lives in | Compares | Error |
|---|---|---|---|
| **(a1)** structural | `format::parse_record`, after `parse_turns` | The `turn` block's own well-formedness, with no track and no simulation: `round` **non-decreasing**; within one `round`, `seat` **strictly increasing**; every `seat < seats`. | `ReplayError::TurnSequence { line, reason }` (new variant) |
| **(a2)** positional | the replay drivers, via a shared `RecordCursor` helper over `record.turns` | For every `Advance::Moved`, the `(round, seat, action)` actually taken vs. `record.turns[i]`. The `round` compared is the one read **before** `RaceRound::advance` — see Amendment 1 note 3. | `HeadlessError::TurnMismatch { index, expected, actual }` (new variant) |
| **(b)** legality | `RaceRound::advance`, between `Roster::poll` and `step` | `mask.contains(action)` on the mask already computed for this turn. | new `Advance::Illegal { seat, action }`; drivers map it to `HeadlessError::IllegalRecordedAction { seat, action }` |
| **exhaustion** | `ReplayController::poll` → `Advance::Pending` | A seat polled with an empty recorded stream. | `HeadlessError::Diverged` (as shipped) |
| **(c)** end-state | `run_headless_replay_from_file` → `finals_agree` | Every seat's recomputed final `CarState` + `lap_raw` vs. the file's `final` lines. | non-zero exit with the recorded-vs-recomputed message (as shipped, `playback.rs`'s `finals_agree` and its call site) |

**(a1) is deliberately NOT a "seat cycle" check.** A crash turn consumes a turn
and emits no `turn` line (§ *Replay format*), so the seat sequence within a round
is a *subsequence* of `0..seats`, never the full cycle. Asserting a strict
`0,1,2,0,1,2` pattern would reject every legitimate record containing a crash.
Non-decreasing-round + strictly-increasing-seat-within-round is the strongest
invariant that is true of all legal records, and it is computable at parse time —
before any `generate` call.

**Citation convention for `playback.rs`.** This file is held by an active code
delegate and its line numbers move — `finals_agree`'s call site and definition are
`:171,195` at HEAD but `:175,199` in the working tree
[measured: 2026-07-29, `git show HEAD:crates/game/src/replay/playback.rs` vs. the
working tree]. **Every** `playback.rs` reference in this document — above and below this note —
therefore cites a **symbol**, not a line, so none can go stale
mid-implementation. (The round-4 draft mixed snapshots: the layer-(c) row and the
escape hatch carried HEAD numbers while the rest carried working-tree ones.) Line citations elsewhere in
this document are working-tree-anchored as of 2026-07-29.

**Every `Advance` match site gets a named `Illegal` arm — all four.** Layer (b)
lives in `RaceRound::advance`, so the four production `match` sites only decide
how to *react*; leaving any of them on a catch-all is a hang or a silent stall.
Layer (a2) needs the expected record in hand, so it lands in only **three** of
them (`run_headless_race` has no record — it is also the record-*producer*).

| Site | `Advance::Illegal` arm | Carries (a2)? |
|---|---|---|
| `run_headless_race` (`playback.rs`, its `Advance` match) — **record production only** once `replay_headless` lands; `run_headless_replay_from_file` stops calling it | `return Err(HeadlessError::IllegalRecordedAction { seat, action })` | **No** — it has no record to compare against. |
| **new** `pub fn replay_headless(config: &GameConfig, record: &ReplayRecord) -> Result<(RaceOutcome, ReplayRecord), HeadlessError>` (`playback.rs`) — returns the **driven** record as well as the outcome, because `run_headless_replay_from_file` still needs it for `finals_agree` (layer (c)); `run_headless_replay_from_file` calls **this** instead of `run_headless_race` | `return Err(HeadlessError::IllegalRecordedAction { .. })` | **Yes** |
| `replay_in_process` (`replay/mod.rs:235`) | **Must break and surface, never `=> {}`** — see the hang note below | **Yes** |
| `PlaybackDriver::tick` (`playback.rs`) → `app/mod.rs`'s `let _ = playback.tick(..)` | `tick` already returns `Advance`; **`app/mod.rs` must stop discarding it** (`let _ = playback.tick(..)`) — match it, halt playback, and surface the divergence in the Setup error slot | **Yes** |
| `GameSession::advance_race` (`session.rs:313`) — interactive only | `Advance::Illegal { .. } => {}`, a deliberate defensive no-op | n/a — no record on this path |

**`HeadlessError` templates must not repeat the `"replay diverged: "` prefix.**
`Diverged`'s own `#[error]` string already begins with it *and* its call site wraps
it in `format!("replay diverged: {err}")`, so it prints **doubled** today
[measured: 2026-07-29, `HeadlessError::Diverged`'s `#[error]` attribute vs. the
`report_error` call in `run_headless_replay_from_file`]. That is harmless for
(a1)'s *absence* assertion, but Amendment 1 adds two more variants that hit the
same wrapper: give `TurnMismatch` and `IllegalRecordedAction` bare templates (no
prefix), and fix `Diverged`'s in the same change.

**`run_headless_race` and `replay_headless` share one inner loop.** They differ
only in whether a `RecordCursor` is threaded, so factor the body into a private
`fn drive(.., cursor: Option<&mut RecordCursor>) -> Result<.., HeadlessError>` and
let both call it. Two sites is *below* the ≥3-site shared-crate threshold, so
duplicating would also be defensible — this design chooses the shared fn and says
so explicitly, because the loop is not trivial (pre-`advance` `round_before`
capture, recorder bookkeeping, `processed` accounting, five `Advance` arms) and
silent drift between the record-producing and record-checking loops is precisely
what layer (a2) exists to detect. **One implementation wrinkle:** `Option<&mut
RecordCursor>` is not `Copy`, so it cannot be used across loop iterations
directly — the implementor re-borrows per iteration via `cursor.as_deref_mut()`.
That is the single place this shape is not mechanical.

**`replay_in_process` cannot take a no-op arm — it would hang.** It returns
`(RaceState, RaceRound)` with no `Result`, and its loop is
`while processed < max_turns`. Because `advance` mutates **nothing** when it
returns a non-advancing outcome, a `Advance::Illegal => {}` arm never increments
`processed` and never moves the cursor, so the loop spins **forever** — not until
`max_turns`. It must instead break and surface. *Note, found while specifying
this table:* its **existing** `Advance::Pending => {}` arm (`replay/mod.rs:240`)
already has that exact shape — an exhausted `ReplayController` returns `None`
forever — so this is a latent hang in shipped code today, not one layer (b)
introduces. AC20 does not trip it only because it replays a record produced from
the same race, so the streams cannot run dry early. Give both arms the same
break-and-surface treatment: widen `replay_in_process` to return the divergence
alongside its state (a third `Option<…>` element or a `Result`), so AC20 keeps
its current assertions and a genuine divergence stops being invisible.

**Interactive failure semantics, chosen deliberately.** `Advance::Illegal` not
advancing the cursor means a hypothetical controller bug on the interactive path
becomes a **silent stall** — the seat is re-polled every frame and the race stops
progressing, with no diagnostic. That is the right call here: `gp-core`/`gp-game`
hold a zero-production-panics invariant (`ai-docs/panic-index.md`), so `panic!`
is not available, and this workspace has no logging facility to reach for. The
arm is also unreachable in practice — `PlayerController::decide` only ever returns
mask members. **Do not "fix" this into a `panic!` or an `expect` later**; if it
ever needs a diagnostic, the surface is the Setup error slot (Scope 12), not a
panic.

**(b) lives in the round loop, not in `Roster::poll`.** `Roster`/`Controller`/
`PollContext` are the #42 seam, which spec § Scope-expansion dispositions keeps
**STILL OUT**; adding a legality check there would change it. `RaceRound::advance`
already computes `mask` and passes it into `PollContext`
(`crates/game/src/race/round.rs:189-193`, read 2026-07-29), so the check is a
single bitflag test on an already-computed value — there is **no** interactive-path
cost worth making it replay-only, and making it unconditional is what turns AC2
("every action passed to `step` is a member of the mask the seat was given") from
a controller-good-behaviour convention into an enforced invariant on every path.
`Advance::Illegal` applies nothing and does **not** advance the cursor. The
interactive arm in `app/mod.rs` treats it as a defensive no-op: it is unreachable
for `PlayerController`, whose `decide` only ever returns mask members
(`crates/game/src/controller/player.rs:28-36`).

### Design Amendment 1 — post-Step-10, owner-approved

**Trigger.** Step 10 `self-review` returned REJECT: the round-1 design specified
three divergence layers, but **only (c)** and the exhaustion path shipped.
Verified against the tree on 2026-07-29:

- `RecordedTurn::round` is written (`format.rs:116`) and parsed (`format.rs:299`)
  but **compared by nothing** — `rg -n "\.round\b" crates/game/src/` returns only
  `RaceRound::round()` call sites and that one `writeln!`; there is no reader of
  the parsed field.
- `ReplayController::for_seat` filters on `seat` alone (`replay/mod.rs:167-175`).
- `RaceRound::advance` feeds whatever `Roster::poll` returns straight into `step`
  with no mask test (`round.rs:190-206`), and `Roster::poll` has none either
  (`controller/mod.rs:100-102`) — so a tampered file could drive the physics core
  with an action outside the legal mask, exactly what AC2 asserts cannot happen.

**Ruling.** Implement (a) and (b) as tabled above. No AC changes; the spec is
untouched. This makes the design honest about *how* AC21's "a tampered record
that diverges exits non-zero" is achieved, and closes an AC2 gap on the replay
path.

**Note 1 — the amendment is a follow-up commit set, not new subtasks.** Groups A
and C have shipped; C2's row owns `replay/format.rs` (layer a1), C4's row owns the
drivers (layer a2), and A4's row owns `race/round.rs` (layer b). Those rows are
annotated with a pointer rather than rewritten, since they correctly describe what
already shipped.

**Note 2 — why the gap survived every gate.** Layers (a) and (b) are the only ones
that inspect a *turn*; (c) inspects only the end state. A record whose middle is
tampered but whose end state happens to agree would have passed, and no test
existed to notice, because the design named the layers without pinning where each
lived.

**Note 3 — BOTH (a1) and (a2) depend on the corrected writer, which has now
landed.** The off-by-one `self-review` finding 6 was: both drivers called
`recorder.record(round.round(), seat, action)` **after** `round.advance(..)`, and
`advance_cursor` increments the round on cursor wrap (`round.rs:244-253`), so each
round's final seat was persisted as round N+1. **That fix is no longer in
flight — it has landed in the working tree in both drivers**, each now capturing
`round_before = round.round()` ahead of the `advance` call
(`run_headless_race`'s pre-`advance` `round_before` binding in
`crates/game/src/replay/playback.rs`, and `crates/game/src/app/session.rs:300-310`,
read 2026-07-29), with a pinning test.
*(Citation corrected: the round-3 text cited `replay/mod.rs:324-331` as the second
driver — that is the AC20 **test** body's own recorder loop, not a driver. The
second driver is `session.rs`.)*

The dependency is **stronger than the round-3 text claimed**, which pinned it on
(a2) alone. **(a1) depends on it just as hard.** Against the old, uncorrected
values a perfectly legal 3-seat record reads `0,0,1 / 1,1,2 / 2,2,3 / …` — the
round-wrapping seat carries N+1 — which violates (a1)'s "seat strictly increasing
within a round" the moment two consecutive lines share a round number out of
order. (a1) would therefore reject **every legitimate file**. Consequence for
sequencing: **neither (a1) nor (a2) may merge ahead of the corrected writer.**
Since it has landed, that ordering constraint is already satisfied — but a rebase
that drops it silently re-breaks (a1) into a total-rejection filter.

Finally, the reason the layer matters at all: the off-by-one went undetected
precisely **because** layer (a) was missing. A persisted coordinate that nothing
ever reads cannot be wrong in a way any gate can see.

**Unrecognised version** (AC22) is rejected by the parser before any other line
is interpreted, in both modes — `ReplayError::UnsupportedVersion { found,
expected }` via `thiserror`.

### Design Amendment 2 — post-Step-10 (self-review round 2), owner-approved

**Trigger.** Self-review round 2 returned REJECT on divergence layer **(c)**,
proven by **mutation rather than argument**: inserting `if true { return true; }`
at the top of `finals_agree` leaves the **entire** `gp-game` suite green — 131
tests, exit 0 `[measured: 2026-07-29, self-review round 2 mutation probe]`.
`finals_agree`'s **rejecting** branch had no coverage at all.

The justification for shipping without a (c) tamper case traced to a sentence in
this document's own § Risks escape hatch — *"layer (c) is already covered in-crate
by the shipped `finals_agree` path"* — which the mutation disproves directly. That
false premise had already been inherited verbatim by the Step-11 decisions-log
entry, making it load-bearing in two places. Same defect class as this task's
fabricated "dependency-addition gate": **a claim asserted rather than checked, then
propagated by a downstream reader who trusted it.** The specific lesson, recorded
so it is not repeated: *the existence of a code path is not coverage of its
branches* — "already covered" is a claim about a **test**, and is only ever
established by naming or running one.

The document was also internally inconsistent: two places **required** (c) (§ Risks'
cost table, § *AC21 tamper construction*) while one waived it.

**Ruling.** (c) is **REQUIRED**, not optional. No AC changes; the spec is
untouched.

**Why (c) cannot be waived.** It is the only guard for a class layer (a2)
*structurally cannot reach* — a crash turn emits no `turn` line, so a crash- or
collision-induced desync never advances `RecordCursor` and is invisible to the
positional check. Final-state disagreement is the sole detector for that class.

**Where this amendment's content lives** (it is recorded in place, not duplicated
here):

| Change | Section |
|---|---|
| The false sentence retracted, with the mutation evidence, and the escape hatch withdrawn | § Risks → *Replay wall-clock* |
| The (c) tamper case specified to the same depth as (a1)/(a2)/(b) | § Test Design → *AC21 tamper construction* |
| Four required tamper cases named for a row-only reader | § Test Design → the `AC21` row |
| `generate` cost reconciled at **5**, premise verified against the shipped `LazyLock` fixture | § Risks → *Replay wall-clock* cost table |
| Amendment 1's routing table repaired (an interleaved paragraph had orphaned four rows) | § Approach → *Design Amendment 1* |


### Standings and summary semantics (AC19) — pinning the two ambiguities

- `StandingEntry::finish_turn: Option<u32>` — the global turn index of the
  finishing move; `None` for a car that never finished, rendered as the `—`
  placeholder. `Option`, not a sentinel: a non-finisher displaying "42 turns"
  would be a lie, and `results.rs` already carries the `PLACEHOLDER` idiom for
  absent values (`lab.rs:61`).
- `RaceSummary::fastest_lap: u32` — the fewest turns any car spent on one lap
  (min over per-car consecutive lap-increment turn deltas). `0` when no lap
  completed.
- `RaceSummary::tempo: f32` = `centerline.length / fastest_lap as f32`, i.e.
  **cells per turn on the fastest lap** — `docs/design.md` §3's "темп круга" is a
  *lap* tempo, and this pairs the tile with the `Fastest lap` tile beside it.
  `0.0` when `fastest_lap == 0` (no division by zero). Rejected: a whole-race
  average (`length · laps / total_turns`), which is a race statistic, not a lap
  tempo, and blends crashed and finished cars into one number.
- `RaceSummary::crashes: u32` — the count of `resolve_crash` calls in the race.
- **Non-finisher ordering**: after every finisher, by `LapCounter::laps()`
  descending, then `SField::scalar_at(pos)` descending (the field is a BFS
  distance seeded at the gate's *forward face*, so a larger scalar is further
  around the loop — `crates/core/src/track.rs:131-134,168`), `None` treated as
  `0`, then car index ascending. Total and deterministic.

### Seed policy — how the CLI per-source overrides compose with `M_k`

§ Seed policy 1 says request `k` uses `M_k = configured_master.wrapping_add(k)`.
`GameConfig` today stores only the *resolved* `Seeds`, with any
`--seed-generation` / `--seed-collision` override already applied
(`config/mod.rs:111-117`). The design adds `GameConfig::master: u64` and pins
the composition rule:

- **`(k = 0, r = 0)` uses `config.seeds` verbatim**, so #41's per-source
  override contract is preserved exactly.
- **`k > 0` uses `Seeds::from_master(M_k)` with no overrides applied**, and
  **`r > 0` uses `…collision.wrapping_add(r)`**. Otherwise a pinned
  `--seed-generation` would make Regenerate produce the same track forever and
  a pinned `--seed-collision` would make "Race again" a rerun — defeating the
  very property § Seed policy 1/2 exist to create.

This is a mechanism choice inside a settled ruling, not a reversal; recorded as
a Key Decision below.

### Where the "seated N of M" notice lives — Lab header

**Resolves spec Open question 3.** `LabInput` gains `seated: Option<SeatedGrid
{ seated: u32, requested: u32 }>`, rendered as an extra `Tag` in the Lab header
row beside the existing `seed <N>` tag, and only when `Some` **and**
`seated < requested`.

- The short grid is a property of the generated track paired with the config —
  the same class as the seed, the VALID badge and the oracle report, all of
  which already live on Lab.
- A Race-screen line would compete with the HUD and, more importantly, would be
  drawn in `race_screen.png` and `app_shell_race.png` — a golden change AC17
  forbids. The Lab header's `Option` gate keeps `lab_screen.png` and
  `app_shell_lab.png` byte-identical: the existing fixtures pass `None`, which
  allocates nothing in the `horizontal_centered` row
  [derived → AC17's full `gp-render` suite run at subtask D2].

### Playback interval — 250 ms

**Resolves spec Open question 2.** `PLAYBACK_TURN_INTERVAL: Duration =
Duration::from_millis(250)` (4 turns/second). A representative race is
~10 turns/lap × 5 laps × 4 seats ≈ 200 turns ≈ 50 s of playback — long enough to
follow individual moves, short enough to watch end-to-end. The advance predicate
is a pure `PlaybackClock::tick(&mut self, now: Instant) -> bool`, taking `now`
as a parameter so AC21c's "interval elapsed ⇒ exactly one turn advances" is
tested with a synthetic clock — no sleeping, no `egui::Context`, no Miri gate.
§ Playback pacing already flags tuning this constant as the natural first
follow-up; transport controls stay out.

### Module decomposition (spec § Risks — "the file-size rule bites")

Soft 500 / hard 1000 lines excluding `#[cfg(test)]`. The spec names six
concerns that cannot share a module; the design gives each its own file and
adds two (race state, app glue) that the 500-line cap forces out of their
neighbours. Estimated production lines in parentheses.

```
crates/game/src/lib.rs               crate docs + pub mod decls                  (~40)
crates/game/src/main.rs              CLI parse → dispatch → run_native          (~110)
crates/game/src/config/{mod,cli,     moved bin → lib; +3 flags, +2 cross-field
                error,echo}.rs       variants, +GameConfig::master           (existing)
crates/game/src/controller/**        UNCHANGED — the #42 seam                (existing)
crates/game/src/gen_worker.rs        worker handle, ids, cancel, observer, msgs (~260)
crates/game/src/race/mod.rs          RaceState, per-car record, seating         (~230)
crates/game/src/race/round.rs        the turn/round state machine               (~250)
crates/game/src/race/standings.rs    finishing order, ranks, summary metrics    (~200)
crates/game/src/replay/mod.rs        record, recorder, ReplayController, driver,
                                     + `pub use playback::run_headless_race;`     (~250)
crates/game/src/replay/format.rs     text encode/decode, version, errors        (~280)
crates/game/src/replay/playback.rs   headless runner + PlaybackClock            (~200)
crates/game/src/app/session.rs       GameSession: seeds, gen + race lifecycle   (~300)
crates/game/src/app/mod.rs           eframe::App glue, FrameInput, session lit  (~260)
crates/game/src/test_fixtures.rs     #[cfg(test)] race fixture track          (test-only)
```

The counter-rule is respected — no file holds a single struct: `race/mod.rs`
carries `RaceState` + `CarRecord` + seating; `replay/mod.rs` carries the record,
the recorder, the controller and the driver.

**`config` moves from the bin target into the lib target.** Both `main.rs` and
`app/session.rs` need `GameConfig`, and the lib is where the headless tests can
reach it. `crates/game/src/lib.rs`'s current doc comment ("`src/main.rs` keeps
its own, independent `mod config;` subtree") becomes false at that moment and is
rewritten in the same subtask.

**`app/` splits glue from state.** `app/session.rs` holds the whole game
lifecycle (seed policy, generation requests, race construction, nav handling,
race-end → Results) with **no `egui` type in its signatures** — so AC12, AC13
and AC18 are plain headless tests. `AppShell::apply`/`can_nav` are already
`egui`-free (`crates/render/src/app.rs:15-17,194-219`), so AC18 can drive the
real shell state machine alongside the real session without a `Context`.

---

## Decomposition

21 subtasks (`A1..A9` = 9, `B1..B3` = 3, `C1..C5` = 5, `D1..D4` = 4 — the same `M` the § Handoff plan enumerates, and the `M` `/task` Step 8 records as `subtask N of M complete` in `.progress.md`). **This exceeds the design-agent's 15-task "propose splitting into
multiple issues" threshold, and the split is deliberately not proposed** —
owner ruling R5-Q1 chose One PR as an informed override after reading the
spec's own four-way split proposal, and the spec records the four-group commit
sequence as the *required* mitigation in its place. The § Handoff plan below is
that mitigation.

| # | Task | ACs | Files | Depends on |
|---|------|-----|-------|------------|
| **A1** | `gp-render` Scopes 11–13: add `TrackView` (folding `geometry` into `Ready`), private `draw_pending` body on Lab/Race arms, `SetupScreen::error(Option<&str>)` + `ShellSession::setup_error`, `ShellResponse::nav: Option<Nav>`. Update every in-crate call site and `main.rs`'s session literal so the workspace builds. | AC10, AC11 | `crates/render/src/app.rs`, `crates/render/src/screens/setup.rs`, `crates/render/src/lib.rs`, `crates/render/src/app_gallery.rs`, `crates/game/src/main.rs` | — |
| **A2** | Move `config` from the bin target to the lib target (`pub mod config`, `pub` items, doc-comment rewrite); relocate the existing fixture `eframe::App` verbatim into `src/app/mod.rs`; add empty `race`/`replay`/`gen_worker`/`app/session` module skeletons. No behaviour change. | — | `crates/game/src/lib.rs`, `crates/game/src/main.rs`, `crates/game/src/app/mod.rs`, `crates/game/src/config/**` | A1 |
| **A3** | `race/mod.rs`: `RaceState`, `CarRecord` (state, `LapCounter`, pending `CrashOutcome`, trail, finish marker), seating `min(cars, positions.len())`, one `Xoshiro256PlusPlus` collision stream per race. Plus `src/test_fixtures.rs` — a hand-built ring corridor with a real `StartFinish`/`TimingGate`, a real `SField::from_gate_bfs`, a 4-cell `StartGrid`, and a real `gp_gen::racing_line` centerline. | AC14 (seating) | `crates/game/src/race/mod.rs`, `crates/game/src/test_fixtures.rs`, `crates/game/src/lib.rs` | A2 |
| **A4** | `race/round.rs`: the per-frame turn/round state machine (mask → crash-or-poll → apply → `register_move` → cursor wrap → `resolve_collisions` → finish detection → play-out-the-round end). **See § *Design Amendment 1* (post-Step-10): this row describes what shipped; the amendment adds divergence layer **(b)** — the unconditional `mask.contains(action)` test between `Roster::poll` and `step`, and the `Advance::Illegal` variant.** | AC1–AC6 | `crates/game/src/race/round.rs` | A3 |
| **A5** | `race/standings.rs`: finishing order + ranks + non-finisher ordering. Two types, because `finish_turn` is inherently **per-car** and cannot sit on a race-level struct: `CarOutcome { car_index: usize, rank: u32, finish_turn: Option<u32> }` and `RaceOutcome { standings: Vec<CarOutcome>, fastest_lap: u32, tempo: f32, crashes: u32 }` (`standings` in rank order, rank 1 first; `fastest_lap == 0` when no car completed a lap; `tempo == 0.0` when `fastest_lap == 0`). All computed natively in `u32` turn counts. The `→ StandingEntry`/`RaceSummary` boundary converts to today's `f32` shape (one `as f32` per field, deleted in D3). *(Shape corrected post-Group-A to match what shipped — `crates/game/src/race/standings.rs:12-40` [measured: 2026-07-28]; no AC, constraint or approach change.)* | AC19 | `crates/game/src/race/standings.rs` | A4 |
| **A6** | `gen_worker.rs`: spawn-per-request worker, `GenerationId`, `Arc<AtomicBool>` cancel flag, `mpsc` message protocol, `GenerationFailure { Pipeline(#[from] GenerationError), WorkerLost }`, superseded-result discard. Calls today's **one-argument** `gp_gen::generate(params)` — the two-argument form does not exist until B2 creates it — and **B2 must update this call site** when it lands the widened signature. *(Corrected post-Group-A: the round-1 wording said `generate(params, &mut ())`, which A6 could not have called; the shipped call site is `gp_gen::generate(params)` at `crates/game/src/gen_worker.rs:102` [measured: 2026-07-28].)* | AC7 | `crates/game/src/gen_worker.rs` | A2 |
| **A7** | `app/session.rs`: `GameSession` — seed policy (`M_k`, race `r`, override composition), generation request/install/failure, race construction, `Nav` handling (Generate / Regenerate / TestLap / Again / Menu-cancels), race-end → Results. `egui`-free signatures. | AC12, AC13 | `crates/game/src/app/session.rs` | A5, A6 |
| **A8** | `replay/mod.rs`: `ReplayRecord` (in-memory), `Recorder` (fed from A4's apply step), `ReplayController` (a `Controller` impl over the recorded stream, with a divergence flag), and the in-process driver. Wire the recorder into `GameSession`. | AC20 | `crates/game/src/replay/mod.rs`, `crates/game/src/app/session.rs` | A7 |
| **A9** | `app/mod.rs`: replace the relocated fixture app with the real `eframe::App` — `FrameInput` assembly (`ShellResponse::action` + `keys::keyboard_action`), active-seat, `TrackView` selection, `request_repaint` while pending, `ShellSession` literal. Delete `fixture_track`/`fixture_cars`/`fixture_standings`/`FIXTURE_SEED`/`FIXTURE_CAR_COUNT` (AC24). | AC18, AC23, AC24 | `crates/game/src/app/mod.rs`, `crates/game/src/main.rs` | A8 |
| **B1** | `gp-render` Scope 15: widen `PhaseStatus` to `Pending < Skipped < Ok < Repair < Failed` with declaration-order `#[derive(PartialOrd, Ord)]`; `phase_badge` gains three arms (`Pending`→`(Neutral,"…")`, `Skipped`→`(Neutral,"skip")`, `Failed`→`(Danger,"failed")`). Existing fixtures keep `Ok`/`Repair`. | AC9b | `crates/render/src/screens/lab.rs` | A9 |
| **B2** | `gp-gen` Scope 10: `GenObserver` + `Phase` + `PhaseOutcome` + `PhaseEvent`; `GenerationError::Cancelled`; `generate(params, &mut dyn GenObserver)` with cancel checks at both loop tops and the per-phase emission table above. Update the 7 in-file test call sites to `&mut ()`, **and A6's production call site in `crates/game/src/gen_worker.rs:102`** — the only `generate` caller outside `gp-gen` once Group A has shipped. | AC8, AC9 (hook) | `crates/gen/src/generate.rs`, `crates/gen/src/lib.rs`, `crates/game/src/gen_worker.rs` | B1 |
| **B3** | `gp-game`: `WorkerObserver` (cancel-flag read + change-only aggregate snapshots), `PhaseOutcome → PhaseStatus` mapping, worst-across-attempts aggregation into `[PhaseStatus; 7]`, cancel on supersede and on navigate-away. **B3 also implements the Terminal rule** (§ Approach → *Terminal rule*): when a run terminates on **either** arm, every phase whose aggregate is still `Pending` becomes `Skipped` — `Pending` must never survive a finished run, per spec § Phase-status ordering's own definitions. | AC9 (aggregate + Terminal rule) | `crates/game/src/gen_worker.rs`, `crates/game/src/app/session.rs` | B2 |
| **C1** | `gp-core` Scope 18: `#[derive(strum::Display, strum::EnumString)]` on `sim::Action` + a `Display`/`FromStr` round-trip test over `Action::VARIANTS`. | AC21b (action-token encoding) | `crates/core/src/sim/mod.rs` | B3 |
| **C2** | `replay/format.rs`: `FORMAT_VERSION`, writer, total parser, `ReplayError` (`UnsupportedVersion`, `Malformed`, `UnknownAction`, …) via `thiserror`. No `unwrap`/`expect`/panicking index. **See § *Design Amendment 1* (post-Step-10): this row describes what shipped; the amendment adds divergence layer **(a1)**, the parse-time `turn`-sequence check, plus `ReplayError::TurnSequence`.** Also widens `ReplayRecord` with `finals: Vec<FinalCarState>` — the persisted `final` lines — which the round-1 file list missed. *(Files corrected post-Group-C against `git show --stat a68d50d` [measured: 2026-07-28].)* | AC21b, AC22 | `crates/game/src/replay/format.rs`, `crates/game/src/replay/mod.rs`, `crates/game/src/app/session.rs` | C1 |
| **C3** | CLI: `--record <PATH>` / `--replay <PATH>` / `--replay-mode <headless\|gui>` (default `gui`, `parse_difficulty`-style case-insensitive label parse); two new `ConfigError` cross-field variants rendered via `Cli::command().error(..)`; `GameConfig` carries the three. Update `ac16_help_lists_all_thirteen_flags_and_nine_defaults` → **sixteen flags / *nine* defaults** — the round-1 row said *ten*, which the implementation refuted: `--replay-mode` ships as `Option<ReplayMode>` (no `default_value_t`) so that AC21d's cross-field check can distinguish *explicitly given* from *defaulted*, and clap prints no `[default: ]` for an `Option` — three new flags, **zero** new defaults. The resolution to `gui` happens at the `GameConfig` level, in code. *(Corrected post-Group-C: shipped test is `ac16_help_lists_all_sixteen_flags_and_nine_defaults`, `crates/game/src/config/cli.rs:271`, with the rationale at `:265-268` [measured: 2026-07-28].)* Note `GameConfig` gains a `PathBuf` field here and so is **no longer `Copy`**; the call-site ripple is part of this subtask. | AC21d | `crates/game/src/config/cli.rs`, `crates/game/src/config/error.rs`, `crates/game/src/config/mod.rs`, `crates/game/src/lib.rs`, `crates/game/src/replay/format.rs`, `crates/game/src/app/session.rs` | C2 |
| **C4** | `replay/playback.rs` headless runner — `pub fn run_headless_race(&GameConfig, Roster, max_turns) -> Result<(RaceOutcome, ReplayRecord), HeadlessError>`, re-exported by `replay/mod.rs` as `pub use playback::run_headless_race;` so its one public path is `gp_game::replay::run_headless_race` (regenerate track → drive the loop with `ReplayController` seats → three-layer divergence check → print standings → exit code) + `main.rs` dispatch (`--replay` + `headless` never calls `run_native`; the startup echo is skipped on that path). Wire `--record` into `GameSession`'s race-end. Also adds `ReplayRecord::total_processed_turns` + the format's `processed <u32>` line (§ *Replay format* — the crash-turn undercount the round-1 grammar missed) and **creates `crates/game/tests/replay.rs`**, AC21/AC21b's own § Test Design entry point, which the round-1 file list omitted. **See § *Design Amendment 1* (post-Step-10): this row describes what shipped; the amendment adds divergence layer **(a2)** — the `RecordCursor` positional check in both drivers — the `HeadlessError::TurnMismatch`/`IllegalRecordedAction` variants, and a **doc-comment correction on `run_headless_replay_from_file`**, whose text still says it *"drives [`run_headless_race`] through [`ReplayController`] seats"* — false once it routes to `replay_headless`.** *(Files corrected post-Group-C against `git show --stat 948d32f` [measured: 2026-07-28].)* | AC21 | `crates/game/src/replay/playback.rs`, `crates/game/src/replay/mod.rs`, `crates/game/src/replay/format.rs`, `crates/game/src/main.rs`, `crates/game/src/app/mod.rs`, `crates/game/src/app/session.rs`, `crates/game/tests/replay.rs` | C3 |
| **C5** | GUI playback: `PlaybackClock` + `PLAYBACK_TURN_INTERVAL`; `AppMode { Interactive, Playback }` in `app/mod.rs`; `request_repaint_after(interval)`. *(Files corrected post-Group-C against `git show --stat 114a04b` [measured: 2026-07-28] — C5 touched `race/round.rs`, **not** `tests/replay.rs`, which C4 creates.)* | AC21c | `crates/game/src/replay/playback.rs`, `crates/game/src/app/mod.rs`, `crates/game/src/race/round.rs` | C4 |
| **D1** | Scope 14: `ShellSession::seed`, `LabInput::seed` and `draw_header`'s parameter move `i32` → `u64`; fixtures at `lab_gallery.rs:29` and `app_gallery.rs:134`; `gp-game` pass-through. **Extract the inline `format!("seed {seed}")` at `lab.rs:309` into a new pure `pub fn header_tag_labels(seed: u64) -> Vec<String>`**, which `draw_header` calls and draws one `Tag` per entry — the `oracle_tile_strings` precedent (`lab.rs:111`, a pure `[String; 4]` formatter asserted by ungated unit tests). Without it there is no assertable entry point: `draw_header` is private and takes `&mut Ui` (`lab.rs:276`). | AC15 | `crates/render/src/app.rs`, `crates/render/src/screens/lab.rs`, `crates/render/src/screens/lab_gallery.rs`, `crates/render/src/app_gallery.rs`, `crates/game/src/app/mod.rs` | C5 |
| **D2** | Scope 16: `SeatedGrid`, `LabInput::seated` + `ShellSession::seated`; `gp-game` supplies it from `min(cars, positions.len())`. **Widen D1's formatter to `header_tag_labels(seed: u64, seated: Option<SeatedGrid>) -> Vec<String>`** (a clean break, no shim — AGENTS.md § API Stability), appending `"seated N of M"` only when `Some` **and** `seated < requested`; `draw_header` draws one `Tag` per returned label, so an absent notice allocates nothing. | AC14 (notice) | `crates/render/src/screens/lab.rs`, `crates/render/src/app.rs`, `crates/render/src/lib.rs`, `crates/game/src/app/mod.rs` | D1 |
| **D3** | Scope 17 **code only**: `StandingEntry::finish_time: f32` → `finish_turn: Option<u32>`; `RaceSummary::fastest_lap: f32` → `u32`; `SUMMARY_LABELS` → `["Fastest lap, turns", "Tempo, cells/turn", "Crashes"]`; drop `.unit("s")`; `standings_rows` formats `"{n} turns"` / `"—"`; `summary_tiles` formats `fastest_lap` as an integer. `gp-game` drops A5's `as f32` conversions. **Adds `#[ignore]` to `results_screen_matches_golden`** so this commit's gates stay green. | AC16 | `crates/render/src/screens/results.rs`, `crates/render/src/gallery_support.rs`, `crates/render/src/screens/results_gallery.rs`, `crates/game/src/race/standings.rs` | D2 |
| **D4** | Scope 17 **pixels only**: regenerate `results_screen.png` through the mint-time verification flow, remove D3's `#[ignore]`, and confirm AC17 (every other golden green without regeneration). The `image-check` spawn MUST explicitly check the summary row for **clipping / wrap**, not merely accept the diff: the new labels are materially longer than today's inside a fixed `CONTENT_MAX_W = 560` column with `spacing::SPACE_6` gaps (`results.rs:22,379-390`). `results_screen_matches_golden` keeps `.threshold(1.0).failed_pixel_count_threshold(0)` **verbatim** — it is already correct for a text-bearing frame (`ai-docs/code-style.md` § Golden-image thresholds), and a regen is exactly when that setting gets wrongly "tidied" to `0.0`. | AC16 (golden), AC17 | `crates/render/tests/snapshots/results_screen.png`, `crates/render/src/screens/results_gallery.rs` | D3 |

---

## Handoff plan

Four groups, matching the spec's § Risks / sizing sequence exactly — (i) loop +
worker + in-memory replay, (ii) `gp-gen` cancellation + phase observation + Lab
per-phase status, (iii) persistence + both playback modes + CLI + `gp-core`
support, (iv) presentation. **No deviation from that grouping.** Group A lands
first, so every later group is exercised against a working race.

Every group is **code** change-type (`*.rs`, `Cargo.toml`, one `*.png`), so
every group routes to `subagent_type="code-writer"`, whose `model: sonnet` and
`effort: medium` are frontmatter-pinned — **no inline `model=` or effort
override**. No group mixes in an instructions/harness file: `ai-docs/**` and
`.claude/**` updates (INDEX, context-status, the deferred inbox) are `/task`
orchestrator work outside this design's groups, which is what keeps every group
homogeneous.

Four groups is the **default maximum**; no user gate is required.

- **Group A** — model `sonnet`, effort `medium` (pinned) via the `code-writer`
  subagent, 1M-token window — subtasks A1–A9 (9 subtasks; code change-type:
  `*.rs`). Scopes 1–7, 9, 11–13. Ends at the size/scope boundary, not a
  change-type switch. **Entry handoff:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) —
  the every-group contract binds the *first* group too.
- **Handoff after Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
  Parent `/task` resumes in Group B with fresh context.
- **Group B** — model `sonnet`, effort `medium` (pinned) via the `code-writer`
  subagent, 1M-token window — subtasks B1–B3 (3 subtasks; code change-type:
  `*.rs`). Scopes 10, 15.
- **Handoff after Group B:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
- **Group C** — model `sonnet`, effort `medium` (pinned) via the `code-writer`
  subagent, 1M-token window — subtasks C1–C5 (5 subtasks; code change-type:
  `*.rs`). Scopes 8, 18.
- **Handoff after Group C:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
- **Group D** — model `sonnet`, effort `medium` (pinned) via the `code-writer`
  subagent, 1M-token window — subtasks D1–D4 (4 subtasks; code change-type:
  `*.rs` + one `*.png`). Scopes 14, 16, 17. Terminal group (4 subtasks; within
  the `1..=10` range).

### Keeping the golden repaint separable (spec § Risks mitigation 2)

D3 and D4 exist as two subtasks solely so that the one commit that legitimately
changes pixels is trivially separable from the ~24 ACs that must not. The
`#[ignore]`-then-un-`#[ignore]` split is what lets **both** commits pass their
gates green — a single combined commit would be green too, but `git show --stat`
on it would mix a Rust API break with a binary PNG, and a code-only D3 without
the `#[ignore]` would leave a knowingly-red `cargo test` in the history, which
AGENTS.md's per-subtask gate discipline forbids.
After D4, `git show --stat` names exactly two paths:
`crates/render/tests/snapshots/results_screen.png` and
`crates/render/src/screens/results_gallery.rs` (the one removed attribute).

---

## Key Decisions

| # | Question | Decision | Rationale |
|---|---|---|---|
| **KD1** | `gp-core` serialization shape (spec Open question 1) | `#[derive(strum::Display, strum::EnumString)]` on `sim::Action` only. `CarState`/`Seeds` are written field-by-field by `gp-game`. No `serde`, no mirror types. | Zero new dependency edges — `strum` + `derive` is already a `gp-core` dep [measured, § Approach]. Compile- and clippy-verified against `#[bitflags]` [measured, § Approach]. `serde` would add a real edge to the physics core, a proc-macro cost, and a Miri re-check, for a derive-driven format the hand-written line record does not use. Mirror types would duplicate the variant set and need their own drift guard. |
| **KD2** | Playback interval (spec Open question 2) | `PLAYBACK_TURN_INTERVAL = Duration::from_millis(250)`. | 4 turns/s ⇒ ~50 s for a representative ~200-turn race — followable per-move, watchable end-to-end. § Playback pacing already flags the constant as the first follow-up if it proves wrong. |
| **KD3** | Where "seated N of M" lives (spec Open question 3) | Lab header, an extra `Tag` beside `seed <N>`, `Option`-gated and drawn only when `seated < requested`. | Track-metadata belongs with the seed / VALID badge / oracle report. A Race line would repaint `race_screen.png` + `app_shell_race.png`, which AC17 forbids; the `Option` gate keeps every existing Lab golden byte-identical. |
| **KD4** | Replay execution path | The replay drives the **same** round loop through `ReplayController` seats. | Makes AC20 true by construction; a parallel re-runner is the shape that silently diverges. |
| **KD5** | `gp-gen` hook shape | One `GenObserver` trait, `&mut dyn`, with `impl GenObserver for ()`. | One added parameter; `&mut ()` at all 7 existing call sites; extensible without a further break. |
| **KD6** | Where phase aggregation happens | `gp-game`. `gp-gen` reports raw events; `gp-render` owns the ordered `PhaseStatus`. | `gp-gen` gains no `gp-render` edge; `gp-game` is the only crate that sees both types. |
| **KD7** | Worker→UI phase traffic | The worker folds events locally and sends a snapshot **only on change** (≤35 msgs/run). | A per-event channel is up to ≈4.2 M messages at the CLI's `1024`/`1024` budget ceilings [measured, § Approach]. |
| **KD8** | Seed policy × CLI per-source overrides | Overrides apply at `(k = 0, r = 0)` only; `k > 0` uses pure `Seeds::from_master(M_k)`, `r > 0` uses `…collision.wrapping_add(r)`. | Preserves #41's override contract while keeping Regenerate/Race-again genuinely fresh — a pinned `--seed-generation` would otherwise freeze Regenerate on one track. |
| **KD9** | Generate-while-pending disable | Achieved by the pending body *replacing* Lab's action row; the Setup path is covered by supersede-by-generation-id + cancel. | Avoids a seventh `gp-render` surface, which § Scope-expansion dispositions still rules out. The Key decision itself retains supersede "for any path that still raises one". |
| **KD10** | `ShellResponse` Regenerate exposure | `pub nav: Option<Nav>`, not `pub regenerate: bool`. | The loop independently needs `Generate`/`Again`/`Menu`; one field answers all four intents. |
| **KD11** | `TrackView` shape | An enum with `geometry` folded into `Ready`, replacing the two independent `ShellSession` fields. | Makes a present-track/absent-geometry pair unrepresentable; the current two-field shape allows it. |
| **KD12** | `finish_turn` for a non-finisher | `Option<u32>`, rendered `—`. **D3 declares its own module-private `const PLACEHOLDER: &str = "—"` in `results.rs`** — it does **not** lift `lab.rs`'s. | A sentinel turn count would be a lie. Citation corrected (design-review round 1): the round-1 draft claimed `results.rs` "already carries the `PLACEHOLDER` idiom", which is false — `PLACEHOLDER` is module-private to `lab.rs:61` and read only at `lab.rs:115,119`, with no `results.rs` use. `[measured: 2026-07-28, `rg -n "PLACEHOLDER" crates/render/src/` → `lab.rs:{61,115,119}` and `text.rs:22` (an unrelated doc mention of `egui`'s `Color32::PLACEHOLDER`); no `results.rs` hit]` So `lab.rs` is a **sibling-module idiom to mirror**, not an importable constant. A `pub(crate)` lift is rejected: two independent screens each owning a one-line display constant is cheaper than a cross-module coupling, and the em-dash is a *presentation* choice each screen makes for itself. |
| **KD13** | `tempo` denominator | `centerline.length / fastest_lap` (cells/turn on the fastest lap); `0.0` when `fastest_lap == 0`. | `docs/design.md` §3's "темп круга" is a per-lap rate, and it pairs with the tile beside it. A whole-race average blends crashed and finished cars. |
| **KD14** | `gp-game` race-test fixture | A hand-built ring in `src/test_fixtures.rs` (real `StartFinish`/gate/`SField`/`StartGrid`, real `gp_gen::racing_line` centerline) — **not** a `gp_gen::generate` call. | `generate` costs ≈3.8 s/call in debug [spec § Technical constraints, measured 2026-07-28] and is prohibitive under Miri, which would force a cost gate on nearly every `gp-game` test. A tiny ring also makes AC3/AC5/AC6 lap-boundary cases precisely controllable. `ai-docs/code-style.md` § Golden setup fidelity's reuse-an-approved-fixture rule governs **visual** golden fixtures; these tests render nothing. |
| **KD15** | `config` module target | Moves from the bin target into the lib target, `pub`. | `main.rs` and `app/session.rs` both need `GameConfig`, and the lib is where headless tests reach it. `lib.rs`'s contrary doc comment is rewritten in the same subtask (A2). |

---

## Risks

- **`gp_gen::generate`'s signature change** — narrower than the spec feared: all
  7 call sites are inside `crates/gen/src/generate.rs`'s own test module.
  Mitigation: none needed beyond the mechanical edit.
  `[measured: 2026-07-28, rg -n "generate\(" --glob '*.rs' crates/ → 7 hits, all in crates/gen/src/generate.rs; the 8th (crates/render/src/app.rs:661) is fn nav_guard_before_first_generate]`
- **A `-D warnings` gate aborts on the first failure**, so the subtask that
  first widens `PhaseStatus` (B1) or renames `StandingEntry::finish_time` (D3)
  may reveal call sites beyond those enumerated. Mitigation: each such subtask
  re-runs `cargo clippy --workspace --all-targets -- -D warnings` **after** its
  enumerated fixes clear, and surfaces any newly-revealed out-of-contract class
  to the orchestrator as a blocker rather than absorbing it.
  `[derived → cargo clippy --workspace --all-targets -- -D warnings at B1, D1, D3]`
- **AC17 golden blast radius.** Only `results_screen.png` renders the affected
  strings: `SUMMARY_LABELS`/`summary_tiles`/`standings_rows` reach a wgpu golden
  through exactly one test.
  `[measured: 2026-07-28, rg -n "SUMMARY_LABELS|summary_tiles|standings_rows|FIXED_SUMMARY" --glob '*.rs' crates/ → the only golden-rendering site is crates/render/src/screens/results_gallery.rs:65 (results_screen_matches_golden); app_gallery.rs:136 passes FIXED_SUMMARY but its three goldens render Setup/Lab/Race, never Results]`
  `[derived → the full gp-render suite, run at D4]`
- **Seed-widening golden safety.** The only rendered seed values are `42` and
  `7`, which format identically as `i32` and `u64`.
  `[measured: 2026-07-28, crates/render/src/screens/lab_gallery.rs:29 const FIXED_SEED: i32 = 42; crates/render/src/app_gallery.rs:134 seed: 7]`
  `[derived → the full gp-render suite, run at D1]`
- **`gp-game` tests that invoke `gp_gen::generate` are Miri-cost hazards.**
  `gp-game` is *not* excluded from the workspace Miri job (only `gp-gen` is —
  `ai-docs/miri-gate.md`, read 2026-07-28), so an ungated `generate` call in a
  `gp-game` test would run a multi-second integer sweep under the interpreter.
  Mitigation: KD14 confines `generate` to two tests (AC18 e2e, AC21
  cross-process), **both** carrying `#[cfg_attr(miri, ignore = "…")]` with a
  cost-only reason. `[derived → the per-test attribute, added in the same commit at A9 and C4]`
- **PRE-EXISTING defect (not an amendment artifact) — `replay_in_process` can
  hang, and its own doc comment asserts it cannot.** `RaceRound::advance` returns
  `Advance::Pending` at `round.rs:199` **before** touching `turn`/`cursor`/`round`,
  and `ReplayController::poll` is a `pop_front()` that returns `None` forever once
  drained — so the `Advance::Pending => {}` arm at `replay/mod.rs:240` never
  increments `processed`, and `while processed < max_turns` spins **indefinitely**,
  not until the cap. Worse, the fn's own doc (`replay/mod.rs:212-214`) asserts the
  opposite — *"a replay whose stream is shorter than expected must terminate rather
  than spin — `ReplayController::diverged` signals that case without ever
  panicking"* — while the code **never reads `diverged()`**
  `[measured: 2026-07-29, read `crates/game/src/replay/mod.rs:205-245` and
  `crates/game/src/race/round.rs:190-206`]`. AC20 does not trip it only because it
  replays a record produced from the same race, so the streams cannot run dry
  early. **A doc asserting termination is exactly what a future reviewer trusts
  instead of the code, so the doc must be corrected in the same change as the
  code.** Amendment 1 requires break-and-surface for both this arm and the new
  `Advance::Illegal` arm; the defect is called out here separately so it is not
  mistaken for something layer (b) introduced.
- **Replay wall-clock — FIVE `generate` calls, ≈19.5 s — and this figure is
  CONDITIONAL on a sharing mechanism that the design now PRESCRIBES.** This count
  has been stale three times; each time the arithmetic was fine and an **implicit
  premise** was wrong. The premise is therefore stated first, as a requirement:

  > **REQUIRED (C4) — and now SATISFIED in the tree.** `crates/game/tests/replay.rs`
  > must produce its record **once per test binary**, not once per `#[test]`. The
  > round-4 draft recorded 4 per-test `write_real_record` sites, which with four
  > tamper cases would have reached 7 productions + 4 child calls = **11 ≈ 42.9 s**.
  > The shipped file now holds a `static FIXTURE: LazyLock<Fixture>` carrying **both**
  > the record text **and** the `FirstLegal` observation log (§ *AC21 tamper
  > construction* needs the log from that same race), with each `#[test]` copying the
  > shared text into its own `ScratchFile`; zero `write_real_record` call sites remain
  > `[measured: 2026-07-29, `crates/game/tests/replay.rs:163` `static FIXTURE:
  > LazyLock<Fixture>`; `rg -c "write_real_record\(&scratch"` → 0]`. Production count
  > is therefore **1, independent of test count** — the premise the table below
  > rests on, now verified rather than assumed.

  Given that mechanism, every step that reaches `run_headless_race` pays exactly
  one `generate`, and every step rejected at parse time pays none:

  | AC21 step | Reaches `generate`? | Calls |
  |---|---|---|
  | (1) record production, in-process, **shared via `LazyLock`** | yes, **once for the whole binary** | 1 |
  | (2) clean replay, spawned | yes | 1 |
  | (3)-(a1) structural tamper | **no** — rejected by the parse-time `turn`-sequence check | 0 |
  | (3)-(a2) last-line `<round>` bump | yes — parses clean, diverges mid-replay | 1 |
  | (3)-(b) out-of-mask action tamper | yes — parses clean, diverges mid-replay | 1 |
  | (3)-(c) `final`-`x` tamper — **REQUIRED** (§ *Design Amendment 2*) | yes — parses clean, diverges only at the end | 1 |
  | AC21b UTF-8/version | no — reuses the shared record | 0 |
  | **Total** | | **5** |

  `[measured: 2026-07-28, this machine — a single `generate(params(6, 1, 8))` call
  is 3900 ms; 5 x 3.9 s ≈ 19.5 s]`
  Note (b) costs **no** extra generation: its mask is derived from the shared
  race's own observation log, not from a second generation (§ *AC21 tamper
  construction*).
  History, so the next reader re-derives instead of adjusting: round 1 said
  "three ≈11.7 s"; round 3 kept it while growing step (3) to three tamper cases;
  round 4 corrected the arithmetic to five but still assumed one shared production
  without requiring it. **The premise, not the arithmetic, is what went stale —
  which is why it is now a REQUIRED box above.** § *Design Amendment 2* re-confirms the total
  against the shipped file: the implementor built **4** child calls (omitting (c)
  on the retracted premise), and restoring (c) makes **5** — matching this table,
  which never waived it. The three places that mention (c) — this table, § *AC21
  tamper construction*, and AC21's own § Test Design row — now agree.
  Mitigations: the whole file is Miri-gated so the cost never reaches the
  interpreter; the (a1) case is deliberately shaped to be **structurally** invalid
  so it is rejected before any generation (the one 0-call row).
  **Escape hatch — RETRACTED under § *Design Amendment 2*.** The round-4 text offered
  "drop cross-process cases in this order — (c) first, *layer (c) is already covered
  in-crate by the shipped `finals_agree` path*". **That premise is false and is
  disproved by mutation:** inserting `if true { return true; }` at the top of
  `finals_agree` leaves the **entire** `gp-game` suite green — 131 tests, exit 0
  `[measured: 2026-07-29, self-review round 2 mutation probe]`. `finals_agree`'s
  **rejecting** branch has no test at all, so (c) was the one case that could least
  afford to be dropped. **No case in step (3) is droppable**: each is the sole
  rejecting-branch coverage for its own layer, and waiving one on an unchecked
  "already covered" claim is exactly the defect § *Design Amendment 2* exists to correct. If
  19.5 s is unacceptable in CI, the lever is the per-call cost (`cheap_config`'s
  budgets, `MAX_TURNS`) or the shared-fixture premise — **surface it to the
  orchestrator; never silently drop a layer.** *(If the `LazyLock` requirement is
  ever dropped, every figure here is wrong again — the escape-hatch savings become
  per-test, not per-file.)*
  `[derived → the C4 subtask's own `cargo test -p gp-game --test replay` timing]`
- **`arithmetic_side_effects` is `deny`** (root `Cargo.toml`
  `[workspace.lints.clippy]`, read 2026-07-28). Turn/round/lap counters and the
  seed `+k`/`+r` steps must use `saturating_add`/`wrapping_add`/`checked_add`,
  never bare `+`. `[derived → cargo clippy --workspace --all-targets -- -D warnings per subtask]`
- **`missing_const_for_fn` is `deny`** (nursery). Every new const-eligible pure
  fn (`phase_badge`'s new arms, `PhaseStatus` helpers, `PlaybackClock::new`,
  `SeatedGrid` accessors, `TrackView` predicates) is `const fn` unless its body
  calls something not const-callable on stable. `[derived → the same clippy gate]`
- **Zero production panics.** `ai-docs/panic-index.md` has **no `crates/core/`
  table row** and this design adds none anywhere: the replay parser is total
  (`thiserror` errors, no `unwrap`/`expect`/panicking index), the worker never
  `join().unwrap()`s, and `Roster::poll`/`resolve_crash`/`register_move` are
  already total.
  `[measured: 2026-07-28, ai-docs/panic-index.md — the only "crates/core/" occurrence is the prose invariant at line 5; the table's rows are crates/render (4), crates/gen (2)]`
- **Hot-seat input aliasing.** Every seat is a `PlayerController` reading the
  same `FrameInput`. Because the loop polls exactly **one** seat per frame and a
  shell click is level-triggered for one frame, seat *k*'s answer cannot leak
  into seat *k+1*. `[derived → AC1's polled-index test at A4]`

---

## Test Design

New `gp-render` tests that construct an `egui::Context` or a
`Harness`, and the new cross-process `gp-game` test, carry
`#[cfg_attr(miri, ignore = "<why>")]` **in the same commit**, with a reason
naming that test's own cause (`ai-docs/miri-gate.md`). No new wgpu golden is
minted — D4 regenerates an existing one whose gate already exists. New `gp-gen`
tests need no gate (#134 keeps the crate excluded).

| AC | Location | Entry point | Scenarios / fixtures |
|---|---|---|---|
| AC1 | `race/round.rs` tests | `TurnCursor::advance` | A recording `Roster` of 3 counting stubs over ≥2 rounds; assert polled seat indices are exactly `0,1,2,0,1,2`. Ring fixture (A3). No `Context` ⇒ ungated. |
| AC2 | `race/round.rs` tests | `RaceRound::advance` | A scripted race; for every applied move assert `legal_move(&corridor, state_before, action)`. Includes the scrub turn (applied via `consume_scrub`). **Amendment 1 adds the enforcement half**, on a state with a *proper-subset* mask — the fixture detail matters: the ring's seat-0 grid cell `(2,1)` at `v = (0,0)` has a **full** 5-action mask, so no out-of-mask action exists there and the test would be unwritable. Use seat 1's cell **`(2,0)`**, the top-edge grid position: `South` steps to `(2,-1)`, outside the corridor, so the mask is exactly `{Coast, East, West, North}` and `South` is the out-of-mask action to feed `[measured: 2026-07-29, `crates/game/src/test_fixtures.rs:33-40,64-72` — `Corridor::filled(Point::new(0,0), 11, 11)` minus the `3..8` square hole, grid positions `[(2,1), (2,0), (2,2), (1,1)]`]`. A stub `Controller` returning `South` from `(2,0)` must yield `Advance::Illegal { seat, action }` with the car's state, the cursor, the round, **`turn()` and `crashes()`** all unchanged — the last two are what prove *no side effect at all*, not merely that `step` was skipped (`RaceRound::turn`/`crashes`, `crates/game/src/race/round.rs:104,110`). Until Amendment 1 this AC held only by controller good behaviour, since `advance` fed `Roster::poll`'s answer straight into `step` (`round.rs:190-206`, read 2026-07-29). Pure integer logic ⇒ ungated. |
| AC3 | `race/round.rs` tests | one full round | Two seats forced onto the same final cell adjacent to S/F so `resolve_collisions` teleports one across the gate; assert its `LapCounter::raw()` is unchanged across the collision pass. |
| AC4 | `race/round.rs` tests | `TurnCursor::advance` | A seat seeded at `controller::test_fixtures::crash_prone_state`'s velocity inside the ring; a `PanicOnPoll` stub proves `poll` is not called on the crash turn; assert the next mask `== Actions::from(Action::Coast)` and that the scrub is consumed exactly once. |
| AC5 | `race/round.rs` tests | finish detection | Three scenarios on the ring: no finish at `laps() == N-1`; finish on the crossing move reaching `N`; **no** finish when a same-round collision teleport carries a car across S/F. |
| AC6 | `race/round.rs` tests | race-end | Assert `total_turns % seated == 0`; a last-turn finish adds no extra round; two same-round finishers rank by turn order; an earlier-round finisher outranks a later-round one. |
| AC7 | `gen_worker.rs` tests | `Worker::request` / `Worker::poll` | Spawn-then-poll reports `Pending`, then `Ready(artifact)`; a superseded `GenerationId`'s result is discarded. Uses the cheap `GenParams` triple; `#[cfg_attr(miri, ignore = "runs the gp-gen generation pipeline on a worker thread — a multi-second integer sweep whose interpreted wall-clock is prohibitive")]`. |
| AC8 | `crates/gen/src/generate.rs` tests | `generate` | Fixture pinned to **`params(6, 64, 32)`** — the same helper AC9 uses (`crates/gen/src/generate.rs:169-182`), so `cars: 4`, `min_straight: 3`, `v_ceiling: 5`, `block_size: 6`. Three scenarios: (a) `CancelAfter(0)` — tripped at the **seed**-loop boundary before any phase runs — returns `Err(GenerationError::Cancelled)` with no artifact; (b) `CancelAfter(1)` — tripped at the **repair**-loop boundary, so both documented check sites are exercised, not just the outer one; (c) the uncancelled run at the same params still accepts. Seed 6 accepts on the **first** seed attempt, so the large `64`/`32` budgets never actually iterate — they exist only to make a cancelled run unambiguously distinguishable from a budget-exhausted one — and (c) therefore costs the same as the cheap triple. `[measured: 2026-07-28, this machine — `generate(params(6, 64, 32))` → ok=true, elapsed 3866 ms; cf. `params(6, 1, 8)` at 3900 ms, confirming the outer budget is never consumed]` Cases (a) and (b) return at a loop boundary and are bounded above by that, so the row's whole cost is ≈3.9 s. `gp-gen` ⇒ no Miri gate (#134). |
| AC9 | `crates/gen/src/generate.rs` tests + `gen_worker.rs` tests | `generate` / the aggregator | A `RecordingObserver` asserts events for Ф1–Ф7 on **both** runs, including non-accepting attempts. Accepting run: `params(6, 1, 8)`. Budget-exhausting run: **`params(1, 1, 1)`** — `repair_budget = 1`, so the repair-loop body executes exactly once and Ф4/Ф5/Ф6 genuinely emit before Ф7's `Skipped` on the `Err` return. `repair_budget = 0` would have been wrong: `crates/gen/src/generate.rs:116` is `for _ in 0..params.repair_budget`, so a `0` budget skips the body and Ф4–Ф6 emit nothing at all. `[measured: 2026-07-28, sweep of seeds 1..=8 × repair_budget ∈ {1,2} at seed_budget = 1 → params(1,1,1) returns Err(SeedBudgetExhausted); only seed 6 accepts at these budgets]` `[measured: 2026-07-28, params(1,1,1) → err=true, elapsed 4193 ms; params(6,1,8) → ok=true, elapsed 3900 ms — the two AC9 fixtures together cost ~8.1 s debug on this machine]` Both reuse the existing `params(seed, seed_budget, repair_budget)` helper (`crates/gen/src/generate.rs:169-182` — `cars: 4`, `min_straight: 3`, `v_ceiling: 5`, `block_size: 6`), so the two triples are fully pinned. Both are `gp-gen` tests ⇒ no Miri gate (#134). The `gp-game` half feeds a synthetic event sequence where Ф4 is `Failed` on attempt 0 and `Ok` on attempt 1 and asserts the aggregate is `Failed`, **plus** a run that emits nothing for Ф4–Ф6 and asserts the terminal rule resolves them to `Skipped`, never `Pending` (§ Approach → Terminal rule). |
| AC9b | `crates/render/src/screens/lab.rs` tests | `PhaseStatus` `Ord` | Pairwise `<` across all five variants in declaration order; `max` over a mixed sequence yields the documented winner; explicitly `Ok > Skipped`. Also assert the ordering comes from `#[derive(Ord)]` and not a hand-written `cmp` — a `rg -Un 'impl (Partial)?Ord for PhaseStatus' crates/render/src` scan finds no hand-written impl. Pure `Ord` ⇒ no `Context` ⇒ ungated. |
| AC10 | `crates/render/src/app_gallery.rs` tests + `crates/game/src/app/mod.rs` tests | `AppShell::show` | (a) A `Harness` drives one `TrackView::Pending` frame and one `Ready` frame; assert the pending frame tessellates non-empty output and the `Ready` frame reaches the Lab body. `#[cfg_attr(miri, ignore = "Harness::builder() calls getcwd via egui_kittest's kittest.toml lookup, unsupported under Miri isolation (no render() here, so not the golden's Vulkan-dlopen cause)")]`. (b) A structural scan mirroring `controller_module_calls_no_physics`'s `include_str!` idiom asserts no `gp-game` production source contains a `TrackArtifact {` struct literal. Ungated. |
| AC11 | `crates/render/src/screens/setup.rs` tests | `SetupScreen::error` | A `Harness` frame with `Some("…")` tessellates more shapes than the same frame with `None`; the `gp-game` half asserts `GameSession` clears `setup_error` once a later generation succeeds (headless, ungated). The render half is `Harness`-gated as above. |
| AC12 | `app/session.rs` tests | `GameSession::on_nav(Nav::Regenerate)` | Assert the raised request's params carry `Seeds::from_master(master.wrapping_add(1)).generation`; the same `k` twice yields byte-equal `format!("{artifact:?}")`; `k` and `k+1` differ. Calls `generate` ⇒ `#[cfg_attr(miri, ignore = "…")]` with the same cost reason as AC7. |
| AC13 | `app/session.rs` tests | `GameSession::on_nav(Nav::Again)` | Assert the second race's collision seed is `first.wrapping_add(1)`, the prior in-memory record is dropped, and the `TrackArtifact` is identical **by value** (`format!("{:?}")`) and not regenerated (the generation counter is unchanged). Ring fixture ⇒ ungated. |
| AC14 | `race/mod.rs` tests + `crates/render/src/screens/lab.rs` tests | seating / `header_tag_labels` | A `StartGrid` with 3 positions and `cars = 6` seats 3, the race still runs to a finisher, and no error path is taken (the reachability of a short grid is pinned upstream by `start_grid_degrades_gracefully_when_d_cannot_host_m_cells`, `crates/gen/src/phase3.rs:871` [measured: 2026-07-28]). The render half asserts on **D2's pure formatter**: `header_tag_labels(42, Some(SeatedGrid { seated: 3, requested: 6 })) == ["seed 42", "seated 3 of 6"]`, and that both `None` and `seated == requested` yield a **one**-element vec (the AC17 golden-safety condition — an absent notice allocates nothing). A pure `Vec<String>` formatter, no `Ui`/`Context`/`Harness` ⇒ **ungated**. |
| AC15 | `crates/render/src/screens/lab.rs` tests + `config/cli.rs` tests | `header_tag_labels` / `Cli` | Assert `header_tag_labels(2_147_483_648, None)[0] == "seed 2147483648"` — a value exceeding `i32::MAX` that the pre-D1 `i32` parameter could not represent — and that the same value round-trips through `Cli::try_parse_from(["graphite-gp", "--seed", "2147483648"])` into `cli.seed`. **Asserted on D1's pure formatter, not on a rendered frame**: the label is built inline inside the *private* `draw_header(ui: &mut Ui, …)` (`lab.rs:276`, label at `:309`) [measured: 2026-07-28], so without D1's extraction the only way to observe it would be an `egui::Context`/`Harness` — `ai-docs/miri-gate.md`'s mechanical trigger, which would then force a `#[cfg_attr(miri, ignore)]`. With the extraction: pure formatting + clap ⇒ **ungated**. |
| AC16 | `crates/render/src/screens/results.rs` tests | `standings_rows` / `summary_tiles` / `SUMMARY_LABELS` | Assert `standings_rows(..)[0].finish_turn == "38 turns"`, a `None` entry renders `"—"`, `summary_tiles(..)[0] == "12"`, and no returned string nor any `SUMMARY_LABELS` entry contains a bare `"s"` suffix on a turn count. Pure ⇒ ungated. |
| AC17 | the whole `gp-render` suite | — | Run at D4 **without** `UPDATE_SNAPSHOTS`; all 15 goldens green with only `results_screen.png` regenerated. `[derived → cargo test -p gp-render at D4]` |
| AC18 | `app/session.rs` tests | `GameSession` + `AppShell::apply` | Headless: drive `Setup → Generate → Lab → TestLap → Race → (loop to race end) → Results`, asserting the `GenParams` came from the live `RaceConfig` + CLI budgets, that Lab and Race see a real artifact, and that Results carries real standings. Calls `generate` ⇒ `#[cfg_attr(miri, ignore = "…")]`, cost reason. |
| AC19 | `race/standings.rs` tests | `RaceOutcome::from_race` | A fully scripted race on the ring fixture with hand-computed expectations for `finish_turn`, `fastest_lap`, `tempo` (= `centerline.length / fastest_lap`, asserted via `test_util`-style epsilon compare) and `crashes`. Ungated. |
| AC20 | `replay/mod.rs` tests | `Recorder` → driver | Build the record from a scripted race, **drop the source race**, replay from the record alone, assert identical final `CarState`s, `LapCounter::raw()`s, standings and `RaceOutcome`. Ring fixture ⇒ ungated. |
| AC21 | `crates/game/tests/replay.rs` (new) | `gp_game::replay::run_headless_race` + the built binary | See § *How AC21's record is produced* below the table — the record comes from a **real race on the regenerated track**, not a hand-built file. Steps: (1) build a `Roster` of `m` integration-test-local `FirstLegal` seats, call `run_headless_race(&config, roster, MAX_TURNS)`, then `replay::format::write_record` — the same writer `--record` invokes at race end; (2) `Command::new(env!("CARGO_BIN_EXE_graphite-gp")).args(["--replay", path, "--replay-mode", "headless"])` → exit `0`, stdout contains each seat's standings line; (3) **four layer-discriminating** tamper cases — one each for **(a1)**, **(a2)**, **(b)** and **(c)**, all four REQUIRED (§ *Design Amendment 2*: the round-4 draft waived (c) on the false premise that the shipped `finals_agree` path already covered it — mutation-disproved, `[measured: 2026-07-29]`) — see § *AC21 tamper construction* below the table, which derives each one against the record the test actually produces (`cheap_config` `cars: 2`, `MAX_TURNS = 8`) rather than asserting its realizability in prose. Each asserts a needle unique to its layer in the child's stderr, **not merely a non-zero exit**: a bare exit-code assertion is *shadowed*, because every layer is backstopped by a later one, so deleting (a1) or (a2) entirely would leave an exit-code-only test green — the exact blindness § *Design Amendment 1* Note 2 describes. Needles follow the in-tree `ac6_cross_field_error_names_flags_and_values` precedent (`config/error.rs:99-124`); (4) the raw file contains `graphite-gp-replay 1`. `#[cfg_attr(miri, ignore = "spawns the built binary via std::process::Command; process spawning is unsupported under Miri")]` — the same cause its `tests/cli.rs` siblings already document, so the reason is shared legitimately. **Coverage note:** the `--record` *flag wiring* is covered separately by a C3 CLI-parse test plus an A8/C4 unit test that the race-end path calls `write_record`; the cross-process half of AC21 is the **replay**, which is what the AC's own wording asks for. |
| AC21b | `crates/game/tests/replay.rs` | the written file | `std::str::from_utf8(&bytes)` is `Ok`; the raw bytes contain the literal `graphite-gp-replay 1`. Reuses step (1)'s record — **no additional `generate` call**. Same Miri gate as AC21. |
| AC21c | `replay/playback.rs` tests | `PlaybackClock::tick` | With an injected `Instant`: `tick(t0)` false, `tick(t0 + interval - 1ms)` false, `tick(t0 + interval)` true **once**, and the immediately following `tick` at the same instant false. Plus: driving the same record through the playback driver reaches the identical final state as the headless runner. Synthetic clock, no `Context` ⇒ **ungated**. |
| AC21d | `crates/game/src/config/cli.rs` + `config/error.rs` tests | `parse` / `parse_err` / `rendered` | `--replay-mode` defaults to `gui`; `--replay-mode headless` without `--replay` → the new cross-field variant; `--record a --replay b` → the other; both `rendered(..)` texts contain contiguous flag-plus-value needles the `#[error(..)]` template cannot satisfy alone (the `ac6_cross_field_error_names_flags_and_values` precedent, `config/error.rs:99-124`). Also update `ac16_help_lists_all_thirteen_flags_and_nine_defaults` → **sixteen** flags / **nine** defaults. *(Corrected post-Group-C. The round-1 tag predicted **ten** defaults on the assumption that `--replay-mode` would carry a `default_value_t`; the implementation refuted it by shipping `Option<ReplayMode>` instead — deliberately, so this very AC's cross-field check can tell *explicitly given* from *defaulted* — and clap prints no `[default: ]` for an `Option`. Sixteen flags was right; ten defaults was not. Shipped test: `ac16_help_lists_all_sixteen_flags_and_nine_defaults`, `crates/game/src/config/cli.rs:271`, rationale at `:265-268` `[measured: 2026-07-28, read crates/game/src/config/cli.rs:117-130,265-280]`.)* |
| AC22 | `replay/format.rs` tests | `parse_record` | A record whose first line reads `graphite-gp-replay 2` yields `Err(ReplayError::UnsupportedVersion { found: 2, expected: 1 })`, and the headless runner surfaces it as a non-zero exit with that message. Pure ⇒ ungated. |
| AC23 | `crates/game/src/lib.rs` tests | `include_str!("../Cargo.toml")` | Assert the manifest contains no `gp-ai`; assert a roster of `m` `PlayerController` seats runs the AC18 sequence end-to-end. |
| AC24 | `crates/game/src/app/mod.rs` tests | `include_str!` scan | Assert none of `fixture_track` / `fixture_cars` / `fixture_standings` / `FIXTURE_SEED` / `FIXTURE_CAR_COUNT` appears in `main.rs` or `app/mod.rs`; assert a `--cars 6` config against a 3-position grid seats 3 (the AC14 fixture) and against a 6-position grid seats 6. |
| AC25 | gates | — | `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` per subtask; plus a `rg -Un '#\[test\]' <changed gp-render test files>` sweep confirming each mechanically-triggered test carries its `#[cfg_attr(miri, ignore = "…")]`. |

### How AC21's record is produced

A hand-built record cannot be shown divergence-free: the headless replay
**regenerates** the track from `seed-generation`, and the three-layer divergence
check requires every recorded action to be a member of that *regenerated*
track's live legal mask. Step (2) expects exit `0`, so the record must come from
a real race on that same track. Concretely:

- **Public API — one canonical path.** The runner is *defined* in
  `crates/game/src/replay/playback.rs` and **re-exported** by
  `crates/game/src/replay/mod.rs` as `pub use playback::run_headless_race;`, so
  its single public path is **`gp_game::replay::run_headless_race`**. C4's
  implementor writes the re-export; no caller ever names `::playback::`.
  (A re-export rather than moving the fn, so `playback.rs` keeps the runner next
  to `PlaybackClock` and stays inside the 500-line soft cap.)
  `pub fn run_headless_race(config: &GameConfig, roster: Roster, max_turns: u32)
  -> Result<(RaceOutcome, ReplayRecord), HeadlessError>` — sharing the *same inner
  `drive` loop* `--replay-mode headless` runs, differing only in which seats
  populate the `Roster` and whether a `RecordCursor` is threaded. *(Qualifier
  corrected under § Design Amendment 1: it previously said the same **entry
  point**, which the Amendment's routing table refutes — `run_headless_replay_from_file`
  now calls `replay_headless`, not `run_headless_race`. The test's own use of
  `run_headless_race` stays correct; only the qualifier was stale.)* `Roster`, `Controller`, `PollContext` and `FrameInput` are already
  `pub` (`crates/game/src/controller/mod.rs:27,47,66,79`), so the integration test
  supplies its own seat with no new production type.
- **The seats.** A test-local `FirstLegal` seat implementing the public
  `gp_game::controller::Controller`: pick the first action of `Action::VARIANTS`
  that is in `ctx.legal`, **starting the scan at `turn_index % 5`** rather than
  at index 0. A naive "first legal in declaration order" pilot picks
  `Action::Coast` whenever it is legal, and `Coast` at `v = (0, 0)` never moves
  the car (`Action::Coast.accel() == (0, 0)`, `crates/core/src/sim/mod.rs:68`) —
  the race would never advance. The rotating start makes the pilot actually
  drive while staying fully deterministic.
- **`gp-core` is reachable from the integration test** — it is a `[dependencies]`
  entry of `gp-game`, and Cargo makes `dependencies + dev-dependencies` available
  to test targets.
  `[measured: 2026-07-28, scratch crates/game/tests/zz_dep_reach_probe.rs doing
  "use gp_core::sim::{Action, Actions};" → cargo test -p gp-game --test
  zz_dep_reach_probe → DEP-REACH-GREEN, "test result: ok. 1 passed"; probe file
  deleted, git status --short clean]`
- **Prerequisite: `gp-game` needs its own `strum` dev-dependency.** This
  subsection prescribes `Action::VARIANTS`, which requires
  `strum::VariantArray` in scope — and `gp-game` had **no direct manifest edge**
  to `strum`. A `[workspace.dependencies]` entry is inert until a member opts in,
  and `cargo tree --invert strum` showing `gp-game` reachable transitively via
  `gp-core` does **not** grant `use strum::…` from `gp-game` itself. C3/C4
  therefore add `[dev-dependencies] strum = { workspace = true }` to
  `crates/game/Cargo.toml` — dev-only, since the need is confined to
  `tests/replay.rs`. No version churn: `strum` is already in `Cargo.lock` at the
  workspace-pinned version, so this is not a new dependency class and triggers no
  Miri re-check. *(Recorded post-Group-C — the round-1 subsection prescribed
  `Action::VARIANTS` without noting the edge it needs, and the gap cost a
  workaround; see `crates/game/Cargo.toml`'s `[dev-dependencies]` comment, read
  2026-07-28. Cumulative Group-C lock delta is three added `gp-game` edges —
  `rand`, `rand_xoshiro`, `strum` — with zero version churn.)*
#### AC21 tamper construction — derived, not asserted

Round-3/4 review found both the round-3 tamper cases unrealizable *as worded*,
with one root cause: their realizability was argued in prose instead of derived
against the artifact the test operates on. The fix pattern is the one AC2's row
already uses (deriving `(2,0)`'s mask cell-by-cell). Derived here against the
**generated** track and the record `write_real_record` actually produces —
`cheap_config` is `cars: 2` (`crates/game/tests/replay.rs:27`) and
`MAX_TURNS = 8` (`:86`), read 2026-07-29 — **not** against the ring fixture.

**Record shape — MEASURED, not derived.** The record `write_real_record` actually
produces is, in full:

```
turn 0 0 Coast
turn 0 1 Coast
turn 1 0 East
turn 1 1 East
turn 2 0 West
turn 2 1 West
turn 3 0 North
turn 3 1 North
processed 8
```

with **no crashes** `[measured: 2026-07-29, design-review round 5 probe against
`cheap_config` + `MAX_TURNS = 8`]`. This confirms the derived shape rather than
replacing it: crashes emit no `turn` line, so a crashing race would yield fewer
lines and different coordinates — every construction below is therefore stated to
be **crash-independent**, and none of them depends on this measurement holding.

**(a1) — structural.** Bump the **last** `turn` line's `<round>` *down*, or repeat
a seat within one round. Fires at parse time.

**(a2) — positional. Bump the LAST `turn` line's `<round>` by one.** On the
measured record that is **`turn 3 1 North` → `turn 4 1 North`**; on any record, the
last line's `r` → `r + 1`.
- (a1) passes: rounds stay **non-decreasing**; the old final round loses its last
  member and the new round `r+1` is a singleton, so "seat strictly increasing
  within a round" holds trivially in both; `seat < seats` is untouched.
- (b) cannot fire: the action token is unchanged.
- (a2) fires: at that turn the driver's `round_before` is `r` while the record
  says `r + 1` ⇒ `TurnMismatch`.
- Crash-independent: the argument only uses "it is the last line".

*Superseded:* the round-4 text proposed a **seat swap**. That is unrealizable
here — with `cars: 2`, `seats ∈ {0,1}`, so changing either line's `<seat>` yields
a duplicate (`0,0` / `1,1`) or `seat >= seats`, and **(a1)** fires with
`TurnSequence` instead of (a2). The only (a1)-passing seat tamper needs a round in
which exactly one seat crashed — neither guaranteed nor asserted.

**(b) — legality. The mask must be COMPUTED, never assumed.** A text-only edit
cannot know the mask: the record carries no track. The shipped tamper
(`tests/replay.rs:170-193`) only guarantees the line *changed*, and its comment's
premise — *"Every seated seat's mask is a proper subset of `Actions::all()`"* — is
**false in general**: a start cell interior to a ≥3-wide corridor at rest has a
**full** mask (exactly the `(2,1)` case AC2's row derives), in which case no token
is out-of-mask and (b) silently degrades to (c).

Construction, at **zero** extra `generate` cost: give the test's `FirstLegal`
seat a shared observation log, recording `(seat, ctx.state, ctx.legal)` on every
poll — `PollContext::legal` is already public
(`crates/game/src/controller/mod.rs:57`). The log is filled by the **same single**
`run_headless_race` call that produces the record, and its entries align **1:1 and
in order** with the `turn` lines. That alignment is a *biconditional* and needs
both directions: a `turn` line implies a poll, because a crash turn never polls
(spec Scope 2b); and a poll implies a `turn` line, because `FirstLegal::poll`
**never returns `None`** — it `find`s over a mask the caller guarantees non-empty — `advance` routes an
empty mask to `resolve_crash` and returns `Advance::Crashed` **before** reaching
`roster.poll` (`crates/game/src/race/round.rs:171`, the `if mask.is_empty()`
branch), so every poll yields `Some` and therefore
an `Advance::Moved` that is recorded (`crates/game/tests/replay.rs:75-81`, read
2026-07-29). Without the second direction an `Advance::Pending` poll would log an
entry with no `turn` line and silently desynchronise the log against the record.
Note a **scrub-tick** turn takes the poll path too — `CrashOutcome::action_mask`
is the non-empty `{Coast}` singleton, so it polls *and* emits a line, keeping the
alignment intact. Then:
1. scan the log **from the end backwards** for the first entry whose
   `legal != Actions::all()`. **Keep the scan backward and unconditional** — do
   **not** "optimise" it into a forward scan or an early exit that assumes a
   proper-subset mask appears late. The round-4 rationale for backwardness ("later
   turns carry non-zero velocity, so a proper subset is likely") is **empirically
   refuted**: on the measured record the only proper-subset masks are poll indices
   **0 and 2** — seat 0 at rest against a corridor edge, `legal =
   {Coast, East, North}` — while indices 1, 3, 4, 5, 6, 7 are all `Actions::all()`
   `[measured: 2026-07-29, design-review round 5 probe]`. The prescribed backward
   scan still succeeds: it walks past five full entries and lands on **index 2**,
   the `turn 1 0 East` line. The mechanism survives; only its stated reason was
   wrong;
2. `assert!` such an entry exists, so the case fails loudly instead of degrading
   to (c) if the track ever makes every polled mask full;
3. rewrite that turn line's action token to an action **absent** from that
   recorded mask — on the measured record, **`turn 1 0 East` → `turn 1 0 West`**
   (`West` is not in `{Coast, East, North}`).

**Why (a2) and (b) cannot shadow each other.** The (b) tamper changes only the
action, so `advance` returns `Advance::Illegal` — and the (a2) cursor compares
**only** on `Advance::Moved`, so it never even runs; conversely the (a2) tamper
leaves the action untouched, so `advance` returns `Moved`, layer (b)'s
`mask.contains` passes, and the cursor is the only thing left to fire. That
disjointness is what makes the two per-layer needles non-redundant.

**(c) — end-state. REQUIRED, not optional (§ *Design Amendment 2*).** Alter the
`<x>` field of one **existing** `final` line — `final 0 <x> …` → `final 0 <x+1> …`.
Edit a *field*; never add or remove a line, or `parse_record`'s
`finals.len() != seats` check fires first with a `Malformed` error and the case
degrades into an (a1)-class parse rejection.
- What makes it fail: `finals_agree` sorts both sides and compares each seat's
  `CarState` + `lap_raw`; a changed `x` makes the file's `FinalCarState` differ
  from the recomputed one, so it returns `false`.
- Needle: the `!finals_agree` branch's own message — *"the recomputed end state
  disagrees with the recorded `final` lines"*.
- Discriminating: (a1) passes, because the structural check inspects only `turn`
  lines and the one `final` validation is the count, which a field edit leaves
  intact; (b) cannot fire, because no action changed; (a2) cannot fire, because
  every `turn` line still matches its `Advance::Moved` coordinate. Only (c) is
  left.
- It fires **only after the full replay** — the end state is not known until then
  — which is why it costs one `generate` in the § Risks table.

**Why (c) cannot be waived at all.** It is the only guard for a class layer (a2)
**structurally cannot reach**: a crash turn emits no `turn` line, so a crash- or
collision-induced desync never advances `RecordCursor` and is invisible to the
positional check. Final-state disagreement is the sole detector for that class.
This is also the layer whose rejecting branch is provably untested today (§ Risks
→ the retracted escape hatch), so it is the *last* case to drop, not the first.

**(a1)'s parse-time proof is structural, not wall-clock.** The round-4 text
asserted the child "ran fast (well under one `generate`, ≈3.9 s)" — a timing
assertion, flaky on a loaded CI runner, and ironically on the one case whose point
is being cheap. Replace it: a **parse** failure is reported by
`report_replay_error`, which prints the bare `ReplayError` `Display`
(`playback.rs`'s `report_replay_error`), while **every** post-generation failure
goes through `report_error(&format!("replay diverged: {err}"))` (its two call sites
in `run_headless_replay_from_file`). So
assert stderr **contains** `TurnSequence`'s own text **and does NOT contain**
`"replay diverged:"`. That is a structural proof that no generation ran, with no
clock in it.

- **`max_turns` is a required parameter, not a convenience.** A `FirstLegal`
  pilot is not guaranteed to complete `laps` laps on a generated track, and a
  replay whose stream ends must terminate rather than spin.
  `run_headless_race` ends the race at the round boundary once `max_turns` is
  reached; standings then fall through to the never-finished ordering the spec's
  Key decisions already define. The replay path passes the recorded turn count;
  the AC21 test passes a fixed budget. This gives the non-finisher ordering real
  coverage as a side effect.
- **`crates/game/src/test_fixtures.rs` stays `#[cfg(test)]` and does NOT become
  public.** The integration test needs none of it — it builds its own seat and
  lets `run_headless_race` generate the track. This matches the in-tree posture
  for shared test fixtures: `crates/gen/src/testfix.rs` is declared `mod
  testfix;` and opens with `#![cfg(test)]`
  [measured: 2026-07-28, `crates/gen/src/lib.rs:31` + `crates/gen/src/testfix.rs:10`].
- **Cost: five `generate` calls, ≈19.5 s — conditional on the `LazyLock`
  single-production requirement.** One shared in-process production for the whole
  test binary (**not** one per `#[test]` — see the REQUIRED box in the § Risks
  "Replay wall-clock" row; per-test production would make it 11 ≈ 42.9 s), plus
  one per cross-process step that parses cleanly (the clean replay and the
  (a2)/(b)/(c) tamper cases). The (a1) structural tamper is rejected before any
  generation, and (b)'s mask comes from the shared race's observation log, not a
  second generation. See the § Risks row for the per-step derivation.

**Shared fixtures / helpers to build**

- `crates/game/src/test_fixtures.rs` (`#[cfg(test)] pub(crate)`) — the ring
  track (real `StartFinish` + `TimingGate.behind`, `SField::from_gate_bfs`,
  4-position `StartGrid`, `gp_gen::racing_line` centerline), a 3-position
  short-grid variant, and a `scripted_roster(actions)` helper.
- `RecordingObserver` / `CancelAfter(n)` in `crates/gen/src/generate.rs`'s test
  module.
- `PanicOnPoll` and counting-stub `Controller`s in `race/round.rs`'s test module
  (the `AlwaysCoastStub` idiom, `controller/mod.rs:262-268`).

---

## Open questions

None blocking. The spec's three non-blocking open questions are resolved above
as **KD1** (`gp-core` serialization shape), **KD2** (playback interval) and
**KD3** (short-grid notice placement).

Two items are flagged for the product owner's eye at review, neither blocking:

- **The exact Results label wording** (KD-adjacent, D3): `["Fastest lap, turns",
  "Tempo, cells/turn", "Crashes"]` with `.unit("s")` dropped. AC16 constrains
  only "turn-based labels, no `s` suffix on a turn count"; the wording is a
  presentation choice and is cheap to adjust **before** D4 mints the golden.
  Note the new labels are materially longer than today's inside a fixed
  `CONTENT_MAX_W = 560` column with `spacing::SPACE_6` gaps
  (`crates/render/src/screens/results.rs:22,379-390` [measured: 2026-07-28]), so
  D4's `image-check` pass must explicitly rule out clipping / wrap rather than
  merely accepting the diff — a shortened wording is the fallback if it does clip.
- **`PLAYBACK_TURN_INTERVAL = 250 ms`** (KD2) — § Playback pacing already names
  retuning it as the natural first follow-up.

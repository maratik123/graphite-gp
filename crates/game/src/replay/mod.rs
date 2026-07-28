//! In-memory replay: record, recorder, `ReplayController`, in-process
//! driver (issue #43, A8, AC20).
//!
//! Design § *The central structural decision — one loop, two controller
//! kinds*: a replay drives the **same** [`crate::race::round::RaceRound`]
//! loop through [`ReplayController`] seats that answer from a recorded
//! action stream, rather than a parallel `replay::simulate()` re-runner —
//! this is what makes AC20 ("replaying reproduces identical final car
//! states") true *by construction*.

pub mod format;

use crate::controller::{Controller, FrameInput, PollContext, Roster};
use crate::race::RaceState;
use crate::race::round::{Advance, RaceRound};
use gp_core::sim::{Action, CarState};
use gp_core::track::TrackArtifact;
use gp_render::{BakedTrackGeometry, RaceConfig};
use std::collections::VecDeque;

/// One recorded turn: which round/seat it was, and the action applied.
///
/// Spec § Replay format — crash turns emit no line, since they poll no
/// controller and are recomputed deterministically; only `Advance::Moved`
/// outcomes are recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordedTurn {
    /// The round this turn was taken in.
    pub round: u32,
    /// The seat that took this turn.
    pub seat: usize,
    /// The action applied.
    pub action: Action,
}

/// One seat's final kinematic state at the point recording stopped.
///
/// The persisted format's `final` line (design § *Replay format*), and
/// layer (c) of its three-layer divergence check: "a `final` line
/// disagreeing with the recomputed end state".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalCarState {
    /// The seat this final state belongs to.
    pub seat: usize,
    /// The car's final kinematic state.
    pub state: CarState,
    /// The car's final `LapCounter::raw()` (signed — may be `-1` if the
    /// car never crossed the gate).
    pub lap_raw: i32,
}

/// A per-race record (spec Scope 8).
///
/// The resolved seeds actually used, the resolved race configuration,
/// every seat's per-turn action, and every seat's final state — enough to
/// reproduce the race exactly, self-contained (design § *Replay format*).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayRecord {
    /// The resolved generation seed (provenance; A8's in-memory driver
    /// does not regenerate the track from this — C4's headless runner
    /// does).
    pub generation_seed: u64,
    /// The resolved collision-resolution seed this race actually used.
    pub collision_seed: u64,
    /// The resolved race configuration (cars/laps/`v_target`/difficulty).
    pub race: RaceConfig,
    /// Every seat's per-turn action, in the order they were applied.
    pub turns: Vec<RecordedTurn>,
    /// Every seat's final state, in seat order (C2 — the persisted
    /// format's `final` lines; A8's own in-memory `replay_in_process`
    /// re-derives finals by simulation, so it does not read this field).
    pub finals: Vec<FinalCarState>,
}

/// Feeds a [`ReplayRecord`] from a live race's `RaceRound::advance` calls
/// (design § *Module decomposition* — "fed from A4's apply step").
#[derive(Clone, Debug, Default)]
pub struct Recorder {
    turns: Vec<RecordedTurn>,
}

impl Recorder {
    /// A fresh, empty recorder.
    #[must_use]
    pub const fn new() -> Self {
        Self { turns: Vec::new() }
    }

    /// Records one seat's applied action. Call this on every
    /// `Advance::Moved` outcome — never on `Crashed`/`Pending`/`RaceOver`
    /// (crash turns are recomputed deterministically, never recorded).
    pub fn record(&mut self, round: u32, seat: usize, action: Action) {
        self.turns.push(RecordedTurn {
            round,
            seat,
            action,
        });
    }

    /// Consumes this recorder into a full [`ReplayRecord`], pairing its
    /// turn history with the race's resolved seeds/config and every seat's
    /// final state (C2 — the persisted format's `final` lines).
    #[must_use]
    pub fn into_record(
        self,
        generation_seed: u64,
        collision_seed: u64,
        race: RaceConfig,
        finals: Vec<FinalCarState>,
    ) -> ReplayRecord {
        ReplayRecord {
            generation_seed,
            collision_seed,
            race,
            turns: self.turns,
            finals,
        }
    }
}

/// A [`Controller`] impl over one seat's slice of a recorded action stream.
///
/// Design § *The central structural decision*: pops its next recorded
/// action on every poll. Sets [`Self::diverged`] rather than panicking if
/// ever polled with no recorded action left (the stream exhausted before
/// the race ended) — a genuine divergence signal, never a crash.
pub struct ReplayController {
    actions: VecDeque<Action>,
    diverged: bool,
}

impl ReplayController {
    /// Builds a seat's replay controller from `record`'s full turn
    /// history, filtered down to `seat`'s own actions, in recorded order.
    #[must_use]
    pub fn for_seat(record: &ReplayRecord, seat: usize) -> Self {
        let actions = record
            .turns
            .iter()
            .filter(|turn| turn.seat == seat)
            .map(|turn| turn.action)
            .collect();
        Self {
            actions,
            diverged: false,
        }
    }

    /// Whether this seat's replay has diverged from its recorded stream
    /// (the stream ran out before the seat was polled again).
    #[must_use]
    pub const fn diverged(&self) -> bool {
        self.diverged
    }
}

impl Controller for ReplayController {
    fn poll(&mut self, _ctx: PollContext<'_>) -> Option<Action> {
        let action = self.actions.pop_front();
        if action.is_none() {
            self.diverged = true;
        }
        action
    }
}

/// Replays `record` in-process against a fresh [`RaceState`].
///
/// Runs over `track`/`geometry` (**not** regenerated from
/// `record.generation_seed` — C4's headless runner owns that; A8's
/// in-memory driver takes the track directly, matching AC20's "drops the
/// source race, replays from the record alone" over an already-available
/// track), driving the **same** [`RaceRound`] loop through
/// [`ReplayController`] seats until the race ends or `max_turns`
/// processed turns have elapsed.
///
/// `max_turns` is a required bound, not a convenience (mirrors C4's own
/// headless runner, design § *How AC21's record is produced*): a replayed
/// seat is not guaranteed to reach the recorded race's own natural end
/// within this call (e.g. a caller comparing an in-progress snapshot,
/// never `RaceOver`), and a replay whose stream is shorter than expected
/// must terminate rather than spin — [`ReplayController::diverged`]
/// signals that case without ever panicking.
///
/// Returns the replayed [`RaceState`]/[`RaceRound`] for the caller to
/// compare against the original.
#[must_use]
pub fn replay_in_process(
    record: &ReplayRecord,
    track: TrackArtifact,
    geometry: BakedTrackGeometry,
    max_turns: u32,
) -> (RaceState, RaceRound) {
    let mut race = RaceState::new(track, geometry, record.race.cars, record.collision_seed);
    let mut round = RaceRound::new(i32::try_from(record.race.laps).unwrap_or(i32::MAX));
    let mut roster = Roster::new();
    for seat in 0..race.cars.len() {
        roster.push(Box::new(ReplayController::for_seat(record, seat)));
    }

    let mut processed = 0u32;
    while processed < max_turns {
        let outcome = round.advance(&mut race, &mut roster, FrameInput::default());
        match outcome {
            Advance::RaceOver => break,
            Advance::Crashed { .. } | Advance::Moved { .. } => {
                processed = processed.saturating_add(1);
            }
            Advance::Pending => {}
        }
    }

    (race, round)
}

#[cfg(test)]
mod tests {
    use super::{Recorder, ReplayRecord, replay_in_process};
    use crate::controller::{Controller, FrameInput, PollContext, Roster};
    use crate::race::RaceState;
    use crate::race::round::{Advance, RaceRound};
    use crate::race::standings::RaceOutcome;
    use crate::test_fixtures::ring_track;
    use gp_core::sim::Action;
    use gp_render::{BakedTrackGeometry, Difficulty, RaceConfig};

    /// A controller that always answers a fixed `action` (asserting it is
    /// legal for the mask it was given each time it is polled).
    struct RepeatAction(Action);

    impl Controller for RepeatAction {
        fn poll(&mut self, ctx: PollContext<'_>) -> Option<Action> {
            assert!(
                ctx.legal.contains(self.0),
                "scripted action {:?} not legal, mask={:?}",
                self.0,
                ctx.legal
            );
            Some(self.0)
        }
    }

    /// `laps: 9` — deliberately never reached within this test's short
    /// scripted budget, so the source race is stopped by a turn budget
    /// (matching the replay driver's own `max_turns`), not `RaceOver`;
    /// AC20 only needs "the replayed state matches the source's", which
    /// holds at any snapshot, not only a finished one.
    const TEST_RACE_CONFIG: RaceConfig = RaceConfig {
        cars: 2,
        laps: 9,
        v_target: 7,
        difficulty: Difficulty::Pro,
    };

    /// This test's scripted turn budget: 3 rounds x 2 seats.
    const TEST_MAX_TURNS: u32 = 6;

    /// AC20 — builds the record from a scripted race, drops the source
    /// race, replays from the record alone, and asserts identical final
    /// `CarState`s, `LapCounter::raw()`s, standings, and `RaceOutcome`.
    ///
    /// Both seats race from their REAL seeded grid positions (no test-only
    /// state overrides — a replay must reproduce the ORIGINAL seating, so
    /// the recorded-actions-alone contract only holds if the source race
    /// never diverges from what `RaceState::new` would seat on its own).
    /// Seat 0 (seeded at `(2,1)`) is scripted three `East`s in a row
    /// (`(2,1)->(3,1)->(5,1)->(8,1)`, verified legal at every step for
    /// this fixture — a 4th `East` from `(8,1)` at `v=(3,0)` would leave
    /// the corridor, which is exactly why this test stops at 3, not
    /// "until it finishes"). Seat 1 (seeded at `(2,0)`) just Coasts.
    #[test]
    fn replay_reproduces_identical_final_state_after_dropping_the_source_race() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let collision_seed = 7;
        let mut race = RaceState::new(track, geometry, TEST_RACE_CONFIG.cars, collision_seed);
        assert_eq!(
            (race.cars[0].state.x, race.cars[0].state.y),
            (2, 1),
            "seating fixture assumption for this test's scripted East-East-East path"
        );

        let mut round = RaceRound::new(i32::try_from(TEST_RACE_CONFIG.laps).unwrap_or(i32::MAX));
        let mut roster = Roster::new();
        roster.push(Box::new(RepeatAction(Action::East)));
        roster.push(Box::new(RepeatAction(Action::Coast)));
        let mut recorder = Recorder::new();

        let mut processed = 0u32;
        while processed < TEST_MAX_TURNS {
            let outcome = round.advance(&mut race, &mut roster, FrameInput::default());
            match outcome {
                Advance::Moved {
                    seat,
                    action,
                    round_complete: _,
                } => {
                    recorder.record(round.round(), seat, action);
                    processed = processed.saturating_add(1);
                }
                Advance::Crashed { .. } => processed = processed.saturating_add(1),
                Advance::Pending | Advance::RaceOver => break,
            }
        }
        assert_eq!(
            processed, TEST_MAX_TURNS,
            "scripted turns must never crash/stall"
        );
        assert!(
            race.cars[0].finish_turn.is_none(),
            "laps=9 must not finish within this short scripted budget"
        );

        let original_states: Vec<_> = race.cars.iter().map(|c| c.state).collect();
        let original_raw_laps: Vec<_> = race.cars.iter().map(|c| c.laps.raw()).collect();
        let original_outcome = RaceOutcome::from_race(&race, round.crashes());
        let finals: Vec<super::FinalCarState> = race
            .cars
            .iter()
            .enumerate()
            .map(|(seat, car)| super::FinalCarState {
                seat,
                state: car.state,
                lap_raw: car.laps.raw(),
            })
            .collect();

        let record: ReplayRecord =
            recorder.into_record(0, collision_seed, TEST_RACE_CONFIG, finals);

        // Drop the source race entirely -- replay from the record alone.
        drop(race);
        let _ = round;

        let track_for_replay = ring_track();
        let geometry_for_replay = BakedTrackGeometry::new(&track_for_replay);
        let (replayed_race, replayed_round) = replay_in_process(
            &record,
            track_for_replay,
            geometry_for_replay,
            TEST_MAX_TURNS,
        );

        let replayed_states: Vec<_> = replayed_race.cars.iter().map(|c| c.state).collect();
        let replayed_raw_laps: Vec<_> = replayed_race.cars.iter().map(|c| c.laps.raw()).collect();
        let replayed_outcome = RaceOutcome::from_race(&replayed_race, replayed_round.crashes());

        assert_eq!(replayed_states, original_states);
        assert_eq!(replayed_raw_laps, original_raw_laps);
        assert_eq!(replayed_outcome, original_outcome);
    }

    /// A `ReplayController` polled past the end of its recorded stream
    /// sets `diverged()` rather than panicking.
    #[test]
    fn replay_controller_marks_diverged_when_stream_is_exhausted() {
        use super::ReplayController;

        let record = ReplayRecord {
            generation_seed: 0,
            collision_seed: 0,
            race: TEST_RACE_CONFIG,
            turns: vec![],
            finals: vec![],
        };
        let mut controller = ReplayController::for_seat(&record, 0);
        assert!(!controller.diverged());

        let track = ring_track();
        let pos = track.start_grid.positions[0];
        let state = gp_core::sim::CarState {
            x: pos.x,
            y: pos.y,
            vx: 0,
            vy: 0,
        };
        let legal = gp_core::sim::legal_mask(&track.corridor, state);
        assert!(
            !legal.is_empty(),
            "test fixture precondition: a seated car has a legal mask"
        );
        let ctx = PollContext {
            track: &track,
            state,
            legal,
            input: FrameInput::default(),
        };
        assert_eq!(controller.poll(ctx), None);
        assert!(controller.diverged());
    }
}

//! The per-frame turn/round state machine (issue #43, A4, spec §
//! *Round loop*, design § *The loop is a per-frame state machine*).
//!
//! [`RaceRound::advance`] advances **at most one seat per call** —
//! `Controller::poll` returns `Option<Action>` where `None` means "ask
//! again next frame", so a round cannot be a `for` loop over seats inside
//! one frame (seat 0 may answer on frame 12, seat 1 on frame 40).

use crate::controller::{FrameInput, PollContext, Roster};
use crate::race::RaceState;
use gp_core::sim::{Action, CarState, legal_mask, resolve_collisions, resolve_crash, step};

/// The outcome of one [`RaceRound::advance`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// The polled seat had no answer yet ("ask again next frame" —
    /// [`gp_core`]'s controller seam) — the cursor is unchanged.
    Pending,
    /// `seat`'s mask was empty (a genuine crash): routed straight to
    /// `resolve_crash`, **no controller was polled** (spec Scope 2b).
    /// `round_complete` is `true` iff this was the round's last seat, so
    /// this call also resolved collisions and advanced the round.
    Crashed {
        /// The seat that crashed.
        seat: usize,
        /// Whether this call also completed the round (collision
        /// resolution + round increment).
        round_complete: bool,
    },
    /// `seat` answered `action`, applied via `step` (or `consume_scrub` on
    /// a scrub turn) and registered against the S/F gate (spec Scope 2d).
    /// `round_complete` is `true` iff this was the round's last seat.
    Moved {
        /// The seat that moved.
        seat: usize,
        /// The action applied — always a member of the mask the seat was
        /// given (AC2).
        action: Action,
        /// Whether this call also completed the round (collision
        /// resolution + round increment).
        round_complete: bool,
    },
    /// The race has ended (spec § Key decisions — "play out the round,
    /// then stop"): no further `advance` calls should be made.
    RaceOver,
}

/// The turn/round cursor driving one [`RaceState`] (spec Scope 2).
///
/// Owns only cursor/counter state — `cursor` (the next seat to poll within
/// the current round), `round`/`turn` (the counters the Results screen
/// needs, spec Scope 1), and `finished_at_round` (the round the *first*
/// finisher crossed in, spec § Key decisions — "play out the round, then
/// stop"). The raced-over data itself lives in [`RaceState`], passed to
/// every [`Self::advance`] call.
#[derive(Debug, Clone, Copy)]
pub struct RaceRound {
    /// The next seat to poll within the current round (`< seated`, reset
    /// to `0` on every wrap).
    cursor: usize,
    /// Completed rounds so far (0-based, incremented at every cursor
    /// wrap).
    round: u32,
    /// The global turn counter — increments once per seat successfully
    /// processed (`Crashed` or `Moved`, never `Pending`), across every
    /// round; a `Moved` finisher's [`crate::race::CarRecord::finish_turn`]
    /// is this value at the moment it finished (AC6/AC19).
    turn: u32,
    /// `Some(round)` once the first car finishes, at the round it
    /// finished in — [`Self::is_race_over`] fires once `round` advances
    /// past this value, so the race plays out its current round (every
    /// seated car takes an equal number of turns) and stops there.
    finished_at_round: Option<u32>,
    /// The configured lap count a car must reach (via a valid, legal S/F
    /// crossing) to finish (spec Scope 5).
    total_laps: i32,
    /// Total `resolve_crash` calls this race (AC19's `crashes` metric,
    /// design § Standings and summary semantics).
    crashes: u32,
}

impl RaceRound {
    /// A fresh round cursor at turn 0, round 0, not yet finished.
    #[must_use]
    pub const fn new(total_laps: i32) -> Self {
        Self {
            cursor: 0,
            round: 0,
            turn: 0,
            finished_at_round: None,
            total_laps,
            crashes: 0,
        }
    }

    /// Completed rounds so far.
    #[must_use]
    pub const fn round(&self) -> u32 {
        self.round
    }

    /// The global turn counter (AC6/AC19).
    #[must_use]
    pub const fn turn(&self) -> u32 {
        self.turn
    }

    /// Total `resolve_crash` calls so far this race (AC19).
    #[must_use]
    pub const fn crashes(&self) -> u32 {
        self.crashes
    }

    /// Whether the race has ended: the first finisher's round has been
    /// played out and wrapped (spec § Key decisions).
    #[must_use]
    pub const fn is_race_over(&self) -> bool {
        match self.finished_at_round {
            Some(finished_round) => self.round > finished_round,
            None => false,
        }
    }

    /// Advances at most one seat (spec Scope 2/§ Approach's four-step
    /// per-car chord): mask (crash-or-poll) → apply → `register_move` →
    /// cursor. When the cursor wraps past the last seated car, resolves
    /// this round's collisions once (over `race`'s one collision-RNG
    /// stream) and increments the round counter, before evaluating
    /// finishes for the next call.
    ///
    /// Returns [`Advance::RaceOver`] immediately (no seat touched) once
    /// [`Self::is_race_over`] holds, or if `race` seated no cars at all
    /// (a total guard — never a panicking index).
    pub fn advance(
        &mut self,
        race: &mut RaceState,
        roster: &mut Roster,
        input: FrameInput,
    ) -> Advance {
        if self.is_race_over() || race.cars.is_empty() {
            return Advance::RaceOver;
        }

        let seat = self.cursor;
        let (state, mask) = {
            let car = &race.cars[seat];
            let mask = car.pending_crash.map_or_else(
                || legal_mask(&race.track.corridor, car.state),
                |outcome| outcome.action_mask(&race.track.corridor),
            );
            (car.state, mask)
        };

        if mask.is_empty() {
            // Scope 2b — a genuine crash: routed straight to
            // `resolve_crash`, no `Roster::poll`. No `register_move`
            // either (a crash teleport never touches the lap counter,
            // spec § Crossing-before-collision ordering).
            let outcome = resolve_crash(&race.track.corridor, state);
            let car = &mut race.cars[seat];
            car.state = outcome.state;
            car.pending_crash = Some(outcome);
            car.trail.push(outcome.state.pos());
            self.turn = self.turn.saturating_add(1);
            self.crashes = self.crashes.saturating_add(1);
            let round_complete = self.advance_cursor(race);
            return Advance::Crashed {
                seat,
                round_complete,
            };
        }

        let ctx = PollContext {
            track: &race.track,
            state,
            legal: mask,
            input,
        };
        let Some(action) = roster.poll(seat, ctx) else {
            // Scope 2c — "not decided yet, ask again next frame": the
            // round does not advance.
            return Advance::Pending;
        };

        let from = state.pos();
        let car = &mut race.cars[seat];
        // Scope 2d + "Scrub turns use consume_scrub, not a bare step".
        let new_state = car.pending_crash.take().map_or_else(
            || step(state, action),
            |outcome| outcome.consume_scrub().state,
        );
        car.state = new_state;
        let to = new_state.pos();
        car.laps.register_move(&race.track.sf, from, to);
        car.trail.push(to);

        // AC19's `fastest_lap`: record the turn index of every genuinely
        // NEW lap boundary this car reaches (never a reverse-then-forward
        // re-crossing of a lap it already recorded) — standings.rs (A5)
        // computes per-lap turn deltas from this history.
        let laps_now = usize::try_from(car.laps.laps()).unwrap_or(0);
        if laps_now > car.lap_turns.len() {
            car.lap_turns.push(self.turn);
        }

        // Win detection (spec Scope 5): evaluated on this step's own
        // crossing, never a post-collision position.
        if car.finish_turn.is_none() && car.laps.laps() >= self.total_laps {
            car.finish_turn = Some(self.turn);
            if self.finished_at_round.is_none() {
                self.finished_at_round = Some(self.round);
            }
        }

        self.turn = self.turn.saturating_add(1);
        let round_complete = self.advance_cursor(race);
        Advance::Moved {
            seat,
            action,
            round_complete,
        }
    }

    /// Advances the cursor past `seat`; when it wraps past the last seated
    /// car, resolves this round's collisions once and increments the round
    /// counter (spec Scope 4). Returns whether a wrap happened.
    fn advance_cursor(&mut self, race: &mut RaceState) -> bool {
        self.cursor = self.cursor.saturating_add(1);
        if self.cursor < race.cars.len() {
            return false;
        }
        self.cursor = 0;
        resolve_round(race);
        self.round = self.round.saturating_add(1);
        true
    }
}

/// One `resolve_collisions` pass over every seated car's post-step
/// position, on `race`'s one per-race collision-RNG stream (spec Scope 4 —
/// never re-derived per round).
fn resolve_round(race: &mut RaceState) {
    let mut states: Vec<CarState> = race.cars.iter().map(|record| record.state).collect();
    resolve_collisions(&race.track.corridor, &mut states, &mut race.collision_rng);
    for (car, new_state) in race.cars.iter_mut().zip(states) {
        car.state = new_state;
        car.trail.push(new_state.pos());
    }
}

#[cfg(test)]
mod tests {
    use super::{Advance, RaceRound};
    use crate::controller::{Controller, FrameInput, PollContext, Roster};
    use crate::race::RaceState;
    use crate::test_fixtures::ring_track;
    use gp_core::sim::{Action, Actions, CarState, legal_mask, legal_move};
    use gp_render::BakedTrackGeometry;

    /// A stub seat that records every seat index it was polled at, and
    /// answers the first legal action in a fixed preference order —
    /// deterministic, never `None`, so a driven test race never stalls on
    /// `Advance::Pending`.
    struct RecordingStub {
        polled: std::rc::Rc<std::cell::RefCell<Vec<usize>>>,
        seat: usize,
    }

    impl Controller for RecordingStub {
        fn poll(&mut self, ctx: PollContext<'_>) -> Option<Action> {
            self.polled.borrow_mut().push(self.seat);
            [
                Action::Coast,
                Action::East,
                Action::West,
                Action::North,
                Action::South,
            ]
            .into_iter()
            .find(|&a| ctx.legal.contains(a))
        }
    }

    /// A stub that panics if ever polled — proves a crash turn never
    /// reaches `Roster::poll` (AC4).
    struct PanicOnPoll;

    impl Controller for PanicOnPoll {
        fn poll(&mut self, _ctx: PollContext<'_>) -> Option<Action> {
            panic!("PanicOnPoll::poll called — a crash turn must never poll a controller");
        }
    }

    /// A 3-seat `RecordingStub` roster sharing one `polled` log, plus the
    /// log itself.
    fn recording_roster(n: usize) -> (Roster, std::rc::Rc<std::cell::RefCell<Vec<usize>>>) {
        let polled = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut roster = Roster::new();
        for seat in 0..n {
            roster.push(Box::new(RecordingStub {
                polled: std::rc::Rc::clone(&polled),
                seat,
            }));
        }
        (roster, polled)
    }

    /// AC1 — polled seat indices across >=2 rounds are exactly the fixed
    /// roster-index order, repeated every round.
    #[test]
    fn polled_seat_indices_are_fixed_roster_order_across_rounds() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let mut race = RaceState::new(track, geometry, 3, 0);
        let mut round = RaceRound::new(9);
        let (mut roster, polled) = recording_roster(3);

        for _ in 0..6 {
            let outcome = round.advance(&mut race, &mut roster, FrameInput::default());
            assert!(
                !matches!(outcome, Advance::Pending | Advance::RaceOver),
                "RecordingStub always answers a legal action; got {outcome:?}"
            );
        }

        assert_eq!(*polled.borrow(), vec![0, 1, 2, 0, 1, 2]);
    }

    /// AC2 — every applied move is a member of the mask the car actually
    /// had (`legal_move` re-derived independently of the mask the seat was
    /// given).
    #[test]
    fn every_applied_move_is_legal_for_the_pre_move_state() {
        let track = ring_track();
        let corridor = track.corridor.clone();
        let geometry = BakedTrackGeometry::new(&track);
        let mut race = RaceState::new(track, geometry, 2, 0);
        let mut round = RaceRound::new(9);
        let (mut roster, _polled) = recording_roster(2);

        for _ in 0..20 {
            let before: Vec<CarState> = race.cars.iter().map(|c| c.state).collect();
            let outcome = round.advance(&mut race, &mut roster, FrameInput::default());
            if let Advance::Moved { seat, action, .. } = outcome {
                assert!(
                    legal_move(&corridor, before[seat], action),
                    "seat {seat} applied {action:?} from {:?}, not legal_move-legal",
                    before[seat]
                );
            }
        }
    }

    /// AC4 — an empty-mask car is never polled: it routes to
    /// `resolve_crash`, its next mask is the singleton `{Coast}`, and the
    /// scrub tick is consumed exactly once.
    #[test]
    fn crash_turn_never_polls_and_scrub_mask_is_coast_singleton() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        // A single seat, forced into a crash-prone velocity directly
        // (bypassing normal seating) so its very first turn is a genuine
        // crash: `legal_mask` is empty for a huge velocity inside a small
        // ring.
        let mut race = RaceState::new(track, geometry, 1, 0);
        race.cars[0].state = CarState {
            x: race.cars[0].state.x,
            y: race.cars[0].state.y,
            vx: 100,
            vy: 100,
        };
        assert!(legal_mask(&race.track.corridor, race.cars[0].state).is_empty());

        let mut round = RaceRound::new(9);
        let mut roster = Roster::new();
        roster.push(Box::new(PanicOnPoll));

        let outcome = round.advance(&mut race, &mut roster, FrameInput::default());
        assert!(
            matches!(outcome, Advance::Crashed { seat: 0, .. }),
            "expected a crash outcome, got {outcome:?}"
        );
        assert!(race.cars[0].pending_crash.is_some());
        let scrub_mask = race.cars[0]
            .pending_crash
            .expect("just asserted Some")
            .action_mask(&race.track.corridor);
        assert_eq!(scrub_mask, Actions::from(Action::Coast));

        // Next turn: PanicOnPoll IS polled now (the scrub mask is
        // non-empty), so swap in a stub that answers Coast.
        let mut roster = Roster::new();
        roster.push(Box::new(RecordingStub {
            polled: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
            seat: 0,
        }));
        let outcome = round.advance(&mut race, &mut roster, FrameInput::default());
        assert!(
            matches!(
                outcome,
                Advance::Moved {
                    seat: 0,
                    action: Action::Coast,
                    ..
                }
            ),
            "scrub turn must apply exactly Coast, got {outcome:?}"
        );
        assert!(
            race.cars[0].pending_crash.is_none(),
            "scrub tick must be consumed exactly once"
        );
    }

    /// AC5 — no finish at `laps() == N-1`; a finish fires exactly on the
    /// crossing move that reaches `N`, not a post-collision position.
    ///
    /// Fast-forwards the lap counter to `laps() == total_laps - 1` via
    /// direct `LapCounter::register_move` calls — a pure function of
    /// `(sf, from, to)`, independent of the car's actual state (the same
    /// technique `gp-core`'s own `LapCounter` tests use to reach a given
    /// count without a full navigated race) — then drives the car's real
    /// state through `RaceRound::advance` for exactly the final crossing
    /// move, so the assertion is against the production finish-detection
    /// path, not a synthetic shortcut.
    #[test]
    fn finish_fires_exactly_on_the_crossing_move_reaching_the_lap_count() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let total_laps = 2;
        let mut race = RaceState::new(track, geometry, 1, 0);

        // Car 0 sits on the gate's own `behind` row (`x = 5`), at rest —
        // one `East` step crosses to `x = 6` (`ahead`), the real final
        // crossing (see `collision_relocation_never_registers_a_crossing`'s
        // doc comment for the same gate-coordinate reasoning).
        race.cars[0].state = CarState {
            x: 5,
            y: 1,
            vx: 0,
            vy: 0,
        };
        // `total_laps` synthetic forward crossings: raw() goes from its
        // pre-race -1 to `total_laps - 1`, i.e. laps() == total_laps - 1 —
        // one crossing short of finishing.
        for _ in 0..total_laps {
            race.cars[0].laps.register_move(
                &race.track.sf,
                gp_core::geom::Point::new(4, 1),
                gp_core::geom::Point::new(6, 1),
            );
        }
        assert_eq!(race.cars[0].laps.laps(), total_laps - 1);
        assert!(
            race.cars[0].finish_turn.is_none(),
            "must not be finished at N-1"
        );

        let mut round = RaceRound::new(total_laps);
        let mut roster = Roster::new();
        roster.push(Box::new(ScriptedOnce(Action::East)));

        let outcome = round.advance(&mut race, &mut roster, FrameInput::default());
        assert!(
            matches!(
                outcome,
                Advance::Moved {
                    seat: 0,
                    action: Action::East,
                    ..
                }
            ),
            "unexpected outcome: {outcome:?}"
        );

        assert_eq!(
            race.cars[0].laps.laps(),
            total_laps,
            "the crossing move must reach N"
        );
        assert_eq!(
            race.cars[0].finish_turn,
            Some(0),
            "finish must fire on this exact (first, turn 0) crossing move"
        );
    }

    /// A one-shot scripted controller: asserts its fixed `action` is legal
    /// for the mask it was given, then answers it.
    struct ScriptedOnce(Action);

    impl Controller for ScriptedOnce {
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

    /// AC3 — crossings are registered from each car's own move chord
    /// **before** collision resolution: a `resolve_collisions` relocation
    /// never calls `register_move`, so even when it teleports a car across
    /// the S/F gate, that car's `LapCounter::raw()` is unaffected. Two
    /// cars are forced onto the same cell (`(5,1)`, the gate's own
    /// `behind` row) via gate-neutral moves of their own (`from`/`to` both
    /// `< GATE_LINE` for each — see `ring_sf`'s `gate.behind == chord`
    /// convention in `test_fixtures.rs`), so `LapCounter::raw()` starts
    /// unchanged from each car's own step; the assertion is that
    /// `resolve_collisions`'s subsequent relocation leaves it exactly
    /// there too.
    #[test]
    fn collision_relocation_never_registers_a_crossing() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let mut race = RaceState::new(track, geometry, 2, 7);
        race.cars[0].state = CarState {
            x: 4,
            y: 1,
            vx: 0,
            vy: 0,
        };
        race.cars[1].state = CarState {
            x: 5,
            y: 0,
            vx: 0,
            vy: 0,
        };
        let raw_before = [race.cars[0].laps.raw(), race.cars[1].laps.raw()];

        let mut round = RaceRound::new(9);
        let mut roster = Roster::new();
        roster.push(Box::new(ScriptedOnce(Action::East)));
        roster.push(Box::new(ScriptedOnce(Action::North)));

        let outcome0 = round.advance(&mut race, &mut roster, FrameInput::default());
        assert!(
            matches!(
                outcome0,
                Advance::Moved {
                    seat: 0,
                    action: Action::East,
                    round_complete: false
                }
            ),
            "unexpected outcome0: {outcome0:?}"
        );
        let outcome1 = round.advance(&mut race, &mut roster, FrameInput::default());
        assert!(
            matches!(
                outcome1,
                Advance::Moved {
                    seat: 1,
                    action: Action::North,
                    round_complete: true
                }
            ),
            "unexpected outcome1: {outcome1:?}"
        );

        // Both cars stepped onto (5,1); the round-wrap collision pass must
        // have separated them onto distinct cells.
        assert_ne!(
            race.cars[0].state.pos(),
            race.cars[1].state.pos(),
            "expected resolve_collisions to have separated the two cars"
        );

        assert_eq!(
            race.cars[0].laps.raw(),
            raw_before[0],
            "car 0's LapCounter must be unchanged by collision resolution"
        );
        assert_eq!(
            race.cars[1].laps.raw(),
            raw_before[1],
            "car 1's LapCounter must be unchanged by collision resolution"
        );
    }

    /// AC6 — race end plays out the round: total turns processed is a
    /// multiple of the seated-car count, same-round finishers'
    /// `finish_turn`s order by turn order, and the race stops exactly one
    /// round after the first finish (no extra round).
    ///
    /// Fast-forwards cars 0 and 1 (same technique as the AC5 test) to one
    /// crossing short of finishing, on distinct gate-row cells; car 2
    /// never finishes (Coasts). Both finishers cross in the same round
    /// (seat 0 before seat 1), so their `finish_turn`s must land in that
    /// order.
    #[test]
    fn same_round_finishers_rank_by_turn_order_and_the_race_ends_after_one_wrap() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let total_laps = 1;
        let seated: usize = 3;
        let mut race = RaceState::new(track, geometry, u32::try_from(seated).unwrap_or(0), 0);

        for seat in [0usize, 1] {
            let y = i32::try_from(seat).unwrap_or(0);
            race.cars[seat].state = CarState {
                x: 5,
                y,
                vx: 0,
                vy: 0,
            };
            for _ in 0..total_laps {
                race.cars[seat].laps.register_move(
                    &race.track.sf,
                    gp_core::geom::Point::new(4, y),
                    gp_core::geom::Point::new(6, y),
                );
            }
            assert_eq!(race.cars[seat].laps.laps(), total_laps - 1);
        }

        let mut round = RaceRound::new(total_laps);
        let mut roster = Roster::new();
        roster.push(Box::new(ScriptedOnce(Action::East)));
        roster.push(Box::new(ScriptedOnce(Action::East)));
        roster.push(Box::new(RecordingStub {
            polled: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
            seat: 2,
        }));

        let mut processed_turns: u32 = 0;
        loop {
            let outcome = round.advance(&mut race, &mut roster, FrameInput::default());
            if matches!(outcome, Advance::RaceOver) {
                break;
            }
            processed_turns = processed_turns.saturating_add(1);
            assert!(processed_turns < 20, "race never ended within the budget");
        }

        assert_eq!(
            processed_turns % u32::try_from(seated).unwrap_or(1),
            0,
            "total processed turns ({processed_turns}) must be a multiple of the \
             seated car count ({seated})"
        );
        assert_eq!(
            round.round(),
            1,
            "race must stop right after playing out the finishing round — no extra round"
        );
        assert!(race.cars[0].finish_turn.is_some());
        assert!(race.cars[1].finish_turn.is_some());
        assert!(race.cars[2].finish_turn.is_none(), "car 2 never finished");
        assert!(
            race.cars[0].finish_turn < race.cars[1].finish_turn,
            "same-round finishers must rank by turn order: {:?} vs {:?}",
            race.cars[0].finish_turn,
            race.cars[1].finish_turn
        );
    }

    /// AC6 — the "last-turn finish adds no extra round" clause,
    /// specifically: the finisher is the round's **last** seat. The round
    /// still completes (collision resolution + round increment) in that
    /// same `advance` call, and the very next call is `RaceOver` with the
    /// round counter unchanged since — no round 2 is ever entered.
    #[test]
    fn last_seat_finish_adds_no_extra_round() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let total_laps = 1;
        let mut race = RaceState::new(track, geometry, 2, 0);

        race.cars[1].state = CarState {
            x: 5,
            y: 1,
            vx: 0,
            vy: 0,
        };
        for _ in 0..total_laps {
            race.cars[1].laps.register_move(
                &race.track.sf,
                gp_core::geom::Point::new(4, 1),
                gp_core::geom::Point::new(6, 1),
            );
        }

        let mut round = RaceRound::new(total_laps);
        let mut roster = Roster::new();
        roster.push(Box::new(RecordingStub {
            polled: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
            seat: 0,
        }));
        roster.push(Box::new(ScriptedOnce(Action::East)));

        let seat0_outcome = round.advance(&mut race, &mut roster, FrameInput::default());
        assert!(
            matches!(
                seat0_outcome,
                Advance::Moved {
                    seat: 0,
                    round_complete: false,
                    ..
                }
            ),
            "unexpected seat0_outcome: {seat0_outcome:?}"
        );

        let seat1_outcome = round.advance(&mut race, &mut roster, FrameInput::default());
        assert!(
            matches!(
                seat1_outcome,
                Advance::Moved {
                    seat: 1,
                    action: Action::East,
                    round_complete: true,
                }
            ),
            "unexpected seat1_outcome: {seat1_outcome:?}"
        );
        assert!(race.cars[1].finish_turn.is_some());
        assert_eq!(round.round(), 1);

        let next = round.advance(&mut race, &mut roster, FrameInput::default());
        assert!(matches!(next, Advance::RaceOver), "unexpected: {next:?}");
        assert_eq!(
            round.round(),
            1,
            "no extra round after the finishing round wraps"
        );
    }
}

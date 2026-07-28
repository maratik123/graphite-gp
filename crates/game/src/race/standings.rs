//! Finishing order, ranks, and race-wide summary metrics (issue #43, A5,
//! AC19).
//!
//! Computed natively in `u32` turn counts (design § *Standings and summary
//! semantics*). The `gp-render` `StandingEntry`/`RaceSummary` boundary
//! (today `f32`) converts at the edge — one `as f32` per field, deleted once
//! D3 moves `results.rs` itself to turn-count labels.

use crate::race::RaceState;
use std::cmp::Reverse;

/// One seated car's finishing-order outcome (native `u32` turn counts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarOutcome {
    /// This car's index into `RaceState::cars`.
    pub car_index: usize,
    /// This car's 1-based finishing rank (design § *Non-finisher
    /// ordering*).
    pub rank: u32,
    /// The global turn index this car finished on, or `None` if it never
    /// finished.
    pub finish_turn: Option<u32>,
}

/// The full computed result of a race, finished or ended (spec AC19,
/// design § *Standings and summary semantics*): every seated car's
/// finishing-order outcome, ranked, plus the race-wide summary metrics.
#[derive(Clone, Debug, PartialEq)]
pub struct RaceOutcome {
    /// Every seated car's outcome, in rank order (rank 1 first).
    pub standings: Vec<CarOutcome>,
    /// The fewest turns any car spent on one lap (design KD13); `0` when
    /// no car completed a lap.
    pub fastest_lap: u32,
    /// `centerline.length / fastest_lap` — cells per turn on the fastest
    /// lap (design KD13); `0.0` when `fastest_lap == 0`.
    pub tempo: f32,
    /// Total `resolve_crash` calls this race.
    pub crashes: u32,
}

/// A car's rank-ordering key (design § *Non-finisher ordering*): every
/// `Finisher` sorts before every `NonFinisher` (declaration order gives
/// this for free); finishers order by `finish_turn` ascending (turn
/// indices are pairwise distinct — exactly one seat is processed per
/// global turn); non-finishers order by `LapCounter::laps()` descending,
/// then `SField::scalar_at` descending (`None` as `0`), then car index
/// ascending — `Reverse` on the first two fields turns "descending" into
/// the derived `Ord`'s natural ascending sort.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RankKey {
    /// A finisher, ordered by its finishing turn (ascending — earliest
    /// first).
    Finisher(u32),
    /// A non-finisher, ordered by `(laps desc, scalar desc, car_index
    /// asc)`.
    NonFinisher(Reverse<u32>, Reverse<u32>, usize),
}

/// `entry`'s [`RankKey`] against `race`'s live car state.
fn rank_key(race: &RaceState, entry: &CarOutcome) -> RankKey {
    entry.finish_turn.map_or_else(
        || {
            let car = &race.cars[entry.car_index];
            let laps = u32::try_from(car.laps.laps()).unwrap_or(0);
            let scalar = race.track.s_field.scalar_at(car.state.pos()).unwrap_or(0);
            RankKey::NonFinisher(Reverse(laps), Reverse(scalar), entry.car_index)
        },
        RankKey::Finisher,
    )
}

/// The consecutive turn deltas between `car`'s lap boundaries, prefixed by
/// the race-start-to-first-lap delta (`lap_turns[0] - 0`) — one delta per
/// completed lap, in lap order.
fn lap_deltas(lap_turns: &[u32]) -> Vec<u32> {
    let mut deltas = Vec::with_capacity(lap_turns.len());
    let mut prev = 0u32;
    for &turn in lap_turns {
        deltas.push(turn.saturating_sub(prev));
        prev = turn;
    }
    deltas
}

impl RaceOutcome {
    /// Computes the full race outcome from `race`'s current/final state and
    /// `crashes` (the race's total `resolve_crash` count —
    /// `RaceRound::crashes()`).
    #[must_use]
    pub fn from_race(race: &RaceState, crashes: u32) -> Self {
        let mut standings: Vec<CarOutcome> = race
            .cars
            .iter()
            .enumerate()
            .map(|(car_index, car)| CarOutcome {
                car_index,
                rank: 0,
                finish_turn: car.finish_turn,
            })
            .collect();

        standings.sort_by_key(|entry| rank_key(race, entry));
        for (index, entry) in standings.iter_mut().enumerate() {
            entry.rank = u32::try_from(index).unwrap_or(u32::MAX).saturating_add(1);
        }

        let fastest_lap = race
            .cars
            .iter()
            .flat_map(|car| lap_deltas(&car.lap_turns))
            .min()
            .unwrap_or(0);
        let tempo = if fastest_lap == 0 {
            0.0
        } else {
            #[allow(
                clippy::cast_precision_loss,
                reason = "fastest_lap is a turn count, realistically tiny \
                          relative to f32's 24-bit exact-integer range"
            )]
            let fastest_lap_f32 = fastest_lap as f32;
            race.track.centerline.length / fastest_lap_f32
        };

        Self {
            standings,
            fastest_lap,
            tempo,
            crashes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RaceOutcome;
    use crate::race::RaceState;
    use crate::test_fixtures::ring_track;
    use gp_core::geom::Point;
    use gp_render::BakedTrackGeometry;

    /// AC19 — a fully scripted (directly-constructed) race with
    /// hand-computed expectations for every `RaceOutcome` field.
    #[test]
    fn race_outcome_matches_hand_computed_values_on_a_scripted_race() {
        let track = ring_track();
        let centerline_length = track.centerline.length;
        let geometry = BakedTrackGeometry::new(&track);
        let mut race = RaceState::new(track, geometry, 3, 0);

        // Car 0: finishes at turn 10; laps at turns 4 and 10 (deltas 4, 6).
        race.cars[0].finish_turn = Some(10);
        race.cars[0].lap_turns = vec![4, 10];

        // Car 1: finishes EARLIER, at turn 7; laps at turns 3 and 7 (deltas
        // 3, 4) -- turn 3's delta is the race's overall fastest lap.
        race.cars[1].finish_turn = Some(7);
        race.cars[1].lap_turns = vec![3, 7];

        // Car 2: never finishes, completed no lap (empty lap_turns); one
        // synthetic forward crossing gives it laps() == 0 (a non-finisher
        // with SOME progress, still ranked last since car 0/1 finished).
        race.cars[2]
            .laps
            .register_move(&race.track.sf, Point::new(4, 1), Point::new(6, 1));
        assert_eq!(race.cars[2].laps.laps(), 0);

        let outcome = RaceOutcome::from_race(&race, 2);

        assert_eq!(outcome.crashes, 2);
        assert_eq!(outcome.fastest_lap, 3, "min(4,6,3,4) == 3");
        let expected_tempo = centerline_length / 3.0;
        assert!(
            (outcome.tempo - expected_tempo).abs() < 1e-6,
            "tempo {} != expected {expected_tempo}",
            outcome.tempo
        );

        assert_eq!(outcome.standings.len(), 3);
        let by_car = |index: usize| {
            outcome
                .standings
                .iter()
                .find(|entry| entry.car_index == index)
                .expect("every car has an outcome entry")
        };
        assert_eq!(by_car(1).rank, 1, "car 1 finished earliest (turn 7)");
        assert_eq!(by_car(0).rank, 2, "car 0 finished second (turn 10)");
        assert_eq!(by_car(2).rank, 3, "car 2 never finished — ranked last");
    }

    /// Non-finisher ordering: after every finisher, by `laps()` descending,
    /// then `SField::scalar_at` descending, then car index ascending.
    #[test]
    fn non_finishers_order_by_laps_then_scalar_then_car_index() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let mut race = RaceState::new(track, geometry, 3, 0);

        // Car 0: 1 lap completed (raw 0 -> 1 via two synthetic crossings).
        for _ in 0..2 {
            race.cars[0]
                .laps
                .register_move(&race.track.sf, Point::new(4, 1), Point::new(6, 1));
        }
        assert_eq!(race.cars[0].laps.laps(), 1);

        // Cars 1 and 2 both stay at 0 laps -- tie-broken by car index
        // ascending (car 1 before car 2), since both start at the same
        // `scalar_at` (their seated grid cells differ, but ties are
        // resolved by index regardless).
        let outcome = RaceOutcome::from_race(&race, 0);
        assert_eq!(outcome.standings[0].car_index, 0, "1 lap ranks first");
        assert_eq!(outcome.standings[1].car_index, 1);
        assert_eq!(outcome.standings[2].car_index, 2);
        assert_eq!(outcome.standings[0].rank, 1);
        assert_eq!(outcome.standings[1].rank, 2);
        assert_eq!(outcome.standings[2].rank, 3);
    }

    /// `fastest_lap`/`tempo` are `0`/`0.0` when no car completed a lap
    /// (KD13 — never a division by zero).
    #[test]
    fn fastest_lap_and_tempo_are_zero_when_no_lap_completed() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let race = RaceState::new(track, geometry, 2, 0);

        let outcome = RaceOutcome::from_race(&race, 0);
        assert_eq!(outcome.fastest_lap, 0);
        assert!((outcome.tempo - 0.0).abs() < f32::EPSILON);
    }
}

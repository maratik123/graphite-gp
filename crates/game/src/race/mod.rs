//! Per-race state (issue #43, A3): [`RaceState`] — track, cars, RNG.
//!
//! Owns the generated [`TrackArtifact`] + its [`BakedTrackGeometry`], every
//! seated [`CarRecord`], and the one collision-resolution RNG stream this
//! race threads through every round (spec § Key decisions — re-deriving the
//! stream per round would replay one shuffle forever).

use gp_core::geom::Point;
use gp_core::sim::{CarState, CrashOutcome, LapCounter};
use gp_core::track::TrackArtifact;
use gp_render::BakedTrackGeometry;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

/// One seated car's full race-time record (spec Scope 1).
#[derive(Debug)]
pub struct CarRecord {
    /// The car's current kinematic state.
    pub state: CarState,
    /// The car's lap counter (`LapCounter::new()` at seating — pre-race,
    /// `raw() == -1`, `laps() == 0`).
    pub laps: LapCounter,
    /// `Some` while this car's scrub tick is pending, carrying the
    /// `resolve_crash` outcome its next mask/move come from
    /// (`CrashOutcome::action_mask`/`consume_scrub`, never `Roster::poll`
    /// — round.rs, A4). `None` on every ordinary turn.
    pub pending_crash: Option<CrashOutcome>,
    /// This car's visited cells, in `CarRender` trail order — seeded with
    /// its starting grid cell.
    pub trail: Vec<Point>,
    /// The global turn index this car finished on (a valid S/F-crossing
    /// move reaching the configured lap count), or `None` while still
    /// racing. Set once, by round.rs (A4); a finished car keeps taking its
    /// turns to the round's end (spec § Key decisions).
    pub finish_turn: Option<u32>,
}

impl CarRecord {
    /// A freshly-seated car at `state` (its grid cell, `v = (0,0)`): a
    /// pre-race `LapCounter`, no pending crash, a trail seeded at its
    /// starting cell, and no finish.
    fn seated(state: CarState) -> Self {
        Self {
            state,
            laps: LapCounter::new(),
            pending_crash: None,
            trail: vec![state.pos()],
            finish_turn: None,
        }
    }
}

/// The owned per-race state (spec Scope 1): the generated track + its baked
/// geometry, every seated car's [`CarRecord`], and the one
/// collision-resolution RNG stream this race uses for its whole duration.
pub struct RaceState {
    /// The generated track fixture this race runs on.
    pub track: TrackArtifact,
    /// `track`'s baked geometry (built once, reused every frame).
    pub geometry: BakedTrackGeometry,
    /// Every seated car's record, in roster-index / turn order.
    pub cars: Vec<CarRecord>,
    /// The one `Xoshiro256PlusPlus` collision-resolution stream this race
    /// threads through every round's `resolve_collisions` call (spec § Key
    /// decisions — never re-derived per round).
    collision_rng: Xoshiro256PlusPlus,
}

impl RaceState {
    /// Builds a fresh race over `track`/`geometry`, seating
    /// `min(cars, track.start_grid.positions.len())` cars at the grid's
    /// positions in order (spec § Key decisions — "seat fewer and race",
    /// AC14), each at rest (`v = (0,0)`) — never an error, never a retry.
    /// `collision_seed` seeds the one collision-resolution RNG stream this
    /// race threads through every round (spec § Seed policy).
    pub fn new(
        track: TrackArtifact,
        geometry: BakedTrackGeometry,
        cars: u32,
        collision_seed: u64,
    ) -> Self {
        let requested = usize::try_from(cars).unwrap_or(usize::MAX);
        let seated = requested.min(track.start_grid.positions.len());
        let cars = track.start_grid.positions[..seated]
            .iter()
            .map(|&pos| {
                CarRecord::seated(CarState {
                    x: pos.x,
                    y: pos.y,
                    vx: 0,
                    vy: 0,
                })
            })
            .collect();

        Self {
            track,
            geometry,
            cars,
            collision_rng: Xoshiro256PlusPlus::seed_from_u64(collision_seed),
        }
    }

    /// The number of seated cars (`<=` the requested `cars` — AC14's
    /// short-grid floor).
    #[must_use]
    pub const fn seated(&self) -> usize {
        self.cars.len()
    }

    /// This race's collision-resolution RNG stream, mutably — round.rs (A4)
    /// threads this exact stream through `resolve_collisions` every round.
    pub const fn collision_rng(&mut self) -> &mut Xoshiro256PlusPlus {
        &mut self.collision_rng
    }
}

#[cfg(test)]
mod tests {
    use super::RaceState;
    use crate::test_fixtures::{ring_track, short_grid_track};
    use gp_render::BakedTrackGeometry;

    /// AC14 — a full-size grid seats exactly the requested `cars` when the
    /// grid has room for them.
    #[test]
    fn full_grid_seats_the_requested_car_count() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let race = RaceState::new(track, geometry, 4, 0);
        assert_eq!(race.seated(), 4);
    }

    /// AC14 — a short grid (3 positions) seats `min(cars, positions.len())`
    /// when `cars` exceeds the grid's capacity, never errors, never
    /// retries.
    #[test]
    fn short_grid_seats_the_grid_capacity_not_the_requested_count() {
        let track = short_grid_track();
        let geometry = BakedTrackGeometry::new(&track);
        let race = RaceState::new(track, geometry, 6, 0);
        assert_eq!(race.seated(), 3, "min(6 requested, 3 available) == 3");
    }

    /// A request below the grid's capacity seats exactly the request (the
    /// `min` is not always the grid side).
    #[test]
    fn request_below_capacity_seats_exactly_the_request() {
        let track = ring_track();
        let geometry = BakedTrackGeometry::new(&track);
        let race = RaceState::new(track, geometry, 2, 0);
        assert_eq!(race.seated(), 2);
    }

    /// Every seated car starts at rest, on its grid cell, with a pre-race
    /// lap counter and no pending crash or finish.
    #[test]
    fn seated_cars_start_at_rest_on_their_grid_cell() {
        let track = ring_track();
        let positions = track.start_grid.positions.clone();
        let geometry = BakedTrackGeometry::new(&track);
        let race = RaceState::new(track, geometry, 4, 0);
        for (car, &pos) in race.cars.iter().zip(&positions) {
            assert_eq!(car.state.x, pos.x);
            assert_eq!(car.state.y, pos.y);
            assert_eq!(car.state.vx, 0);
            assert_eq!(car.state.vy, 0);
            assert_eq!(car.laps.raw(), -1);
            assert!(car.pending_crash.is_none());
            assert!(car.finish_turn.is_none());
            assert_eq!(car.trail, vec![pos]);
        }
    }

    /// The same `collision_seed` yields a deterministic collision-RNG
    /// stream (byte-identical first draw) across two independent races —
    /// replay determinism depends on this.
    #[test]
    fn same_collision_seed_yields_the_same_stream() {
        use rand::Rng;

        let track_a = ring_track();
        let geometry_a = BakedTrackGeometry::new(&track_a);
        let mut race_a = RaceState::new(track_a, geometry_a, 4, 42);

        let track_b = ring_track();
        let geometry_b = BakedTrackGeometry::new(&track_b);
        let mut race_b = RaceState::new(track_b, geometry_b, 4, 42);

        assert_eq!(
            race_a.collision_rng().next_u64(),
            race_b.collision_rng().next_u64()
        );
    }
}

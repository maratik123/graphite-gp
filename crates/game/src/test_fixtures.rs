//! Shared `#[cfg(test)]` race fixtures (issue #43, A3) — a small hand-built
//! ring track with a real `StartFinish`/`TimingGate.behind`, a real
//! `SField::from_gate_bfs`, a `StartGrid`, and a real `gp_gen::racing_line`
//! centerline. **Not** built via `gp_gen::generate` (design KD14): `generate`
//! costs ≈3.8 s/call in debug and is Miri-cost-prohibitive, and a tiny
//! hand-built ring makes AC3/AC5/AC6's lap-boundary cases precisely
//! controllable.
//!
//! Mirrors `crates/gen/src/testfix.rs`'s posture — declared `mod
//! test_fixtures;` in `lib.rs`, opening with `#![cfg(test)]`, and never
//! made `pub`.

#![cfg(test)]

use gp_core::geom::{Corridor, Orient, Point, Side, walls_from_boundary};
use gp_core::track::{
    RaceDir, SField, StartFinish, StartGrid, TimingGate, TrackArtifact, TrackMetrics,
};

/// The ring's outer bounding box side (an `11×11` filled square minus a
/// centered `5×5` hole, `x,y ∈ 3..8` — mirrors `gp-gen`'s own
/// `testfix::annulus_corridor`, rebuilt here since that fixture is
/// `pub(crate)` to `gp-gen`). Gives a 3-cell-wide annulus band on every
/// side — wide enough to seat several cars and engineer a same-cell
/// collision (AC3).
const RING_SIDE: usize = 11;
/// The centered hole's inclusive-exclusive bound (`3..8`) on both axes.
const HOLE: std::ops::Range<i32> = 3..8;

/// A width-3 annulus ring: an [`RING_SIDE`]×`RING_SIDE` filled square minus
/// a centered `5×5` hole.
pub(crate) fn ring_corridor() -> Corridor {
    let mut d = Corridor::filled(Point::new(0, 0), RING_SIDE, RING_SIDE);
    for y in HOLE {
        for x in HOLE {
            d.set(Point::new(x, y), false);
        }
    }
    d
}

/// The ring's start/finish gate: a `Vertical` chord at column `x = 5`
/// spanning the whole top band (`y ∈ {0,1,2}`), `forward: East` — a car
/// crosses forward when it steps from `x = 5` to `x = 6` (mirrors
/// `gp-gen`'s `testfix::ring_sf`'s "chord `==` gate.behind" pattern,
/// widened to a real multi-row span so [`LapCounter::register_move`]'s
/// along-chord extent covers the whole band).
pub(crate) fn ring_sf() -> StartFinish {
    let behind = vec![Point::new(5, 0), Point::new(5, 1), Point::new(5, 2)];
    StartFinish {
        chord: behind.clone(),
        orient: Orient::Vertical,
        gate: TimingGate {
            behind,
            forward: Side::East,
        },
    }
}

/// Up to `n` distinct start-grid positions, behind the gate (`x = 2`, within
/// the top band), in a fixed order — `n` beyond the fixture's own supply
/// saturates at its length (a caller asking for more than exists is a test
/// bug, not a panic path).
pub(crate) fn ring_grid_positions(n: usize) -> Vec<Point> {
    const POSITIONS: [Point; 4] = [
        Point::new(2, 1),
        Point::new(2, 0),
        Point::new(2, 2),
        Point::new(1, 1),
    ];
    POSITIONS.iter().copied().take(n).collect()
}

/// A [`ring_corridor`] track with the given `positions` as its `StartGrid`
/// — the shared assembly every `ring_track*` fixture calls, so
/// `SField`/`Centerline`/`walls`/`width_min` are computed identically
/// everywhere.
pub(crate) fn ring_track_with_grid(positions: Vec<Point>) -> TrackArtifact {
    let corridor = ring_corridor();
    let walls = walls_from_boundary(&corridor);
    let sf = ring_sf();
    let s_field = SField::from_gate_bfs(&corridor, &sf.gate);
    let start_grid = StartGrid { positions };
    let centerline = gp_gen::racing_line(&corridor, &sf.gate, RaceDir::Cw);

    TrackArtifact {
        walls,
        sf,
        corridor,
        race_dir: RaceDir::Cw,
        s_field,
        start_grid,
        centerline,
        metrics: TrackMetrics::default(),
        width_min: 3,
    }
}

/// The default ring fixture: a 4-position `StartGrid` (mirrors design §
/// Decomposition A3's "a 4-cell `StartGrid`").
pub(crate) fn ring_track() -> TrackArtifact {
    ring_track_with_grid(ring_grid_positions(4))
}

/// The AC14 short-grid fixture: the same ring, seated with only 3
/// positions, so a `cars > 3` request exercises `min(cars,
/// positions.len())` seating.
pub(crate) fn short_grid_track() -> TrackArtifact {
    ring_track_with_grid(ring_grid_positions(3))
}

#[cfg(test)]
mod tests {
    use super::{ring_grid_positions, ring_track, short_grid_track};

    /// Sanity: the fixture assembles without panicking and its grids are
    /// the documented sizes.
    #[test]
    fn ring_track_has_a_four_position_grid() {
        assert_eq!(ring_track().start_grid.positions.len(), 4);
    }

    #[test]
    fn short_grid_track_has_a_three_position_grid() {
        assert_eq!(short_grid_track().start_grid.positions.len(), 3);
    }

    #[test]
    fn ring_grid_positions_are_distinct() {
        let positions = ring_grid_positions(4);
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                assert_ne!(positions[i], positions[j]);
            }
        }
    }
}

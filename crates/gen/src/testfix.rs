//! Shared `#[cfg(test)]` fixtures for the Ф5a (`phase5.rs`) and Ф5b
//! (`phase5b.rs`) test suites (design doc § Risks — test-fixture
//! duplication).
//!
//! Lifted verbatim from `phase5.rs`'s original `#[cfg(test)] mod tests` so
//! both modules share one definition rather than duplicating it (the
//! `>=2`-call-site, more-coming shared-helper rule) — Ф6/Ф7 are expected to
//! need the same ring fixture too.

#![cfg(test)]

use gp_core::geom::{Corridor, Orient, Point};
use gp_core::sim::CarState;
use gp_core::track::{StartFinish, StartGrid, TimingGate};

/// Shorthand `CarState` constructor for test fixtures.
pub(crate) const fn car(x: i32, y: i32, vx: i32, vy: i32) -> CarState {
    CarState { x, y, vx, vy }
}

/// A closed width-1, 4-connected ring: the border of a 5×5 box (drivable iff
/// `x ∈ {0, 4}` or `y ∈ {0, 4}`).
pub(crate) fn ring_corridor() -> Corridor {
    let mut d = Corridor::new(Point::new(0, 0), 5, 5);
    for y in 0..5 {
        for x in 0..5 {
            if x == 0 || x == 4 || y == 0 || y == 4 {
                d.set(Point::new(x, y), true);
            }
        }
    }
    d
}

/// The ring's start/finish gate: behind row `(2, 0)`, forward `East`, so the
/// ahead cell is `(3, 0)`.
pub(crate) fn ring_sf() -> StartFinish {
    StartFinish {
        chord: vec![Point::new(2, 0)],
        orient: Orient::Vertical,
        gate: TimingGate {
            behind: vec![Point::new(2, 0)],
            forward: gp_core::geom::Side::East,
        },
    }
}

/// The ring's start grid: one car, behind the gate, at rest.
pub(crate) fn ring_grid() -> StartGrid {
    StartGrid {
        positions: vec![Point::new(2, 0)],
    }
}

/// A minimal fixture that **permits** the initial race-start (`-1 → 0`)
/// forward S/F crossing but dead-ends immediately after — no cells beyond
/// the ahead cell and no return path, so a lap-close (`0 → 1`) can never
/// occur. Two drivable cells: `(2, 0)` (behind, the grid seed) and `(3, 0)`
/// (ahead).
pub(crate) fn dead_end_corridor() -> (Corridor, StartFinish, StartGrid) {
    let d = Corridor::filled(Point::new(2, 0), 2, 1);
    let sf = StartFinish {
        chord: vec![Point::new(2, 0)],
        orient: Orient::Vertical,
        gate: TimingGate {
            behind: vec![Point::new(2, 0)],
            forward: gp_core::geom::Side::East,
        },
    };
    let grid = StartGrid {
        positions: vec![Point::new(2, 0)],
    };
    (d, sf, grid)
}

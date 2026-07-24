//! Shared `#[cfg(test)]` fixtures for the Ф5a (`phase5.rs`) and Ф5b
//! (`phase5b.rs`) test suites (design doc § Risks — test-fixture
//! duplication).
//!
//! Lifted verbatim from `phase5.rs`'s original `#[cfg(test)] mod tests` so
//! both modules share one definition rather than duplicating it (the
//! `>=2`-call-site, more-coming shared-helper rule) — Ф6/Ф7 are expected to
//! need the same ring fixture too.

#![cfg(test)]

use gp_core::geom::{Corridor, Orient, Point, Side};
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
            forward: Side::East,
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
            forward: Side::East,
        },
    };
    let grid = StartGrid {
        positions: vec![Point::new(2, 0)],
    };
    (d, sf, grid)
}

/// A straight, 1-wide, 8-cell track (`(0,0)..(7,0)`) with an S/F gate at its
/// very start: `crosses_sf_forward` fires exactly once, on the track's first
/// move (`(0,0) -> (1,0)`), and never again (no return path on a straight
/// track). Building speed by accelerating every turn from rest reaches a
/// state with no legal successor at all (every action's minimum resulting
/// `x` exceeds the track's last index) -- a genuine provable crash:
/// reachable (`R`), but from which no forward crossing (hence no `G`) is
/// ever reachable, so absent from `B`.
pub(crate) fn crash_pocket_fixture() -> (Corridor, StartFinish, StartGrid) {
    let d = Corridor::filled(Point::new(0, 0), 8, 1);
    let sf = StartFinish {
        chord: vec![Point::new(0, 0)],
        orient: Orient::Vertical,
        gate: TimingGate {
            behind: vec![Point::new(0, 0)],
            forward: Side::East,
        },
    };
    let grid = StartGrid {
        positions: vec![Point::new(0, 0)],
    };
    (d, sf, grid)
}

/// A closed, width-1, 4-connected ring shaped as an **elongated** rectangle
/// (14×5, vs. `ring_corridor`'s square 5×5): drivable iff `x ∈ {0, 13}` or
/// `y ∈ {0, 4}` — border cells only, interior walled off, same construction
/// as `ring_corridor` (design § Test Design AC7 long-straight fixture). The
/// top (`y = 0`) and bottom (`y = 4`) edges are each a 14-cell **long
/// straight** along the x-axis — long enough (design's ~10–14-cell target)
/// for a car accelerating from rest to build up a materially higher L∞
/// speed than the short 5×5 ring's corner-limited corridor ever permits,
/// while the four 90-degree corners still force braking before the turn
/// (AC7's "tempo integrates the required braking" clause).
pub(crate) fn long_straight_corridor() -> Corridor {
    let mut d = Corridor::new(Point::new(0, 0), 14, 5);
    for y in 0..5 {
        for x in 0..14 {
            if x == 0 || x == 13 || y == 0 || y == 4 {
                d.set(Point::new(x, y), true);
            }
        }
    }
    d
}

/// The long-straight ring's start/finish gate: behind cell `(7, 0)` (the
/// midpoint of the top straight), forward `East` — so the ahead cell is
/// `(8, 0)`, mirroring `ring_sf`'s gate placement pattern.
pub(crate) fn long_straight_sf() -> StartFinish {
    StartFinish {
        chord: vec![Point::new(7, 0)],
        orient: Orient::Vertical,
        gate: TimingGate {
            behind: vec![Point::new(7, 0)],
            forward: Side::East,
        },
    }
}

/// The long-straight ring's start grid: one car, behind the gate on the long
/// straight, at rest — forward along the straight, mirroring `ring_grid`.
pub(crate) fn long_straight_grid() -> StartGrid {
    StartGrid {
        positions: vec![Point::new(7, 0)],
    }
}

/// A closed 12×8 ring (V=1 lappable, same border construction as
/// `ring_corridor`) plus a 5-cell dead-end **braking-trap spur** hanging
/// north off the bottom straight at `x = 6`, separated from the top straight
/// by the single wall row `y = 6`. A car that enters the spur and builds
/// `|v| = 2` has **no legal move at all** at `(6, 5)`: every action leaves
/// `vy ∈ {1, 2, 3}`, so `y' ≥ 6`; `(6, 6)`, `(5, 6)`, `(7, 6)` are all walls,
/// and `y' = 7` requires a chord whose `supercover` passes through the wall
/// row `y = 6`, which `legal_move` rejects. Reached from rest by
/// `(2,0)→…→(6,0)` at `v=(1,0)`, then `NorthWest` → `(6,1)`, `North` →
/// `(6,3)`, `Coast` → `(6,5)`. This is the AC7 purpose-built candidate
/// counterexample: a corridor that **is** V=1 lappable and additionally
/// contains a hazard un-brakeable at higher speed (a non-empty `R \ B` at
/// `V_ceil = 2`, witness `CarState { x: 6, y: 5, vx: 0, vy: 2 }`).
pub(crate) fn trap_ring() -> (Corridor, StartFinish, StartGrid) {
    let mut d = Corridor::new(Point::new(0, 0), 12, 8);
    for y in 0..8 {
        for x in 0..12 {
            if x == 0 || x == 11 || y == 0 || y == 7 {
                d.set(Point::new(x, y), true);
            }
        }
    }
    for y in 1..=5 {
        d.set(Point::new(6, y), true); // the spur
    }
    let sf = StartFinish {
        chord: vec![Point::new(2, 0)],
        orient: Orient::Vertical,
        gate: TimingGate {
            behind: vec![Point::new(2, 0)],
            forward: Side::East,
        },
    };
    let grid = StartGrid {
        positions: vec![Point::new(2, 0)],
    };
    (d, sf, grid)
}

/// The AC2 tier-2 fallback witness: a single-cell corridor
/// (`Corridor::filled(Point::new(2, 0), 1, 1)`) with the `ring_sf`-shaped
/// gate at `(2, 0)`. No cell ahead of the gate exists, so no forward
/// crossing and no lap-close goal exists, `live = ∅`, and hence `P0 = ∅` —
/// the P0-boundary-wall tier-1 diagnostic is empty and the driver must fall
/// back to `walls_from_boundary`.
pub(crate) fn no_crossing_corridor() -> (Corridor, StartFinish, StartGrid) {
    let d = Corridor::filled(Point::new(2, 0), 1, 1);
    let sf = StartFinish {
        chord: vec![Point::new(2, 0)],
        orient: Orient::Vertical,
        gate: TimingGate {
            behind: vec![Point::new(2, 0)],
            forward: Side::East,
        },
    };
    let grid = StartGrid {
        positions: vec![Point::new(2, 0)],
    };
    (d, sf, grid)
}

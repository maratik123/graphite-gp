//! Shared `#[cfg(test)]` fixtures for the Ф5a (`phase5.rs`) and Ф5b
//! (`phase5b.rs`) test suites (design doc § Risks — test-fixture
//! duplication).
//!
//! Lifted verbatim from `phase5.rs`'s original `#[cfg(test)] mod tests` so
//! both modules share one definition rather than duplicating it (the
//! `>=2`-call-site, more-coming shared-helper rule) — Ф6/Ф7 are expected to
//! need the same ring fixture too.

#![cfg(test)]

use std::collections::BTreeSet;

use gp_core::geom::{Coord, Corridor, Orient, Point, Side};
use gp_core::sim::CarState;
use gp_core::track::{StartFinish, StartGrid, TimingGate};

/// Build a corridor over `[origin, origin + (w, h))` with the given `(x, y)`
/// cells marked drivable — the shared 4-arg builder consumed by phase7 (GO-note
/// 1, `2026-07-24-gp-gen-phase7-centerline.design.md` § Decomposition): the
/// same shape already duplicated at `phase4.rs:279` and
/// `phase4_defects.rs:196`, so phase7 reuses this copy rather than adding a
/// 3rd.
pub(crate) fn corridor(
    origin: (Coord, Coord),
    w: usize,
    h: usize,
    drivable: &[(Coord, Coord)],
) -> Corridor {
    let mut d = Corridor::new(Point::new(origin.0, origin.1), w, h);
    for &(x, y) in drivable {
        d.set(Point::new(x, y), true);
    }
    d
}

/// An 11×11 filled square minus a centred 5×5 hole (`x, y ∈ 3..8`) — the
/// odd-thickness annulus shape whose medial axis is 4 disjoint corner-gapped
/// strips (`gp_core::geom::distance` tests this shape's exact medial set).
/// Ф7's `bridge_gaps`/AC7 fixtures reuse this shape to exercise real
/// diagonal-corner bridging.
pub(crate) fn annulus_corridor() -> Corridor {
    let mut d = Corridor::filled(Point::new(0, 0), 11, 11);
    for y in 3..8 {
        for x in 3..8 {
            d.set(Point::new(x, y), false);
        }
    }
    d
}

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

/// AC3's discriminating fixture (Ф6 local-repair,
/// `ai-docs/plans/2026-07-24-gp-gen-phase6-local-repair.design.md` § Test
/// Design): a straight feeding a 90-degree corner, sized and speed-tuned so
/// a fixed-radius recheck wrongly reports "fixed" while the sink-to-sink
/// recheck correctly reports "still deficient".
///
/// Box `origin (0,0)`, `14 × 6`. Drivable: the straight `y = 0, x ∈ 0..=11`,
/// plus the corner leg `x = 11, y ∈ 1..=4`. `(12,0)` and `(13,0)` are
/// **in-box and `¬D`** — so the add-edit tested against this fixture is a
/// real flip, not a `Corridor::set` no-op (the corridor returned here is the
/// **pre-edit** state; the caller applies the edit with `d.set(Point::new(12,
/// 0), true)`).
///
/// Returns `(d, path, sinks)`: the frozen `fastest_lap` path
/// `[(0,0), …, (11,0), (11,1), (11,2), (11,3), (11,4)]` and the sink index
/// set `{0}` — both hand-supplied rather than derived from a real oracle
/// run, so the fixture does not depend on which lap the oracle happens to
/// pick.
pub(crate) fn brake_deficit_corridor() -> (Corridor, Vec<Point>, BTreeSet<usize>) {
    let mut d = Corridor::new(Point::new(0, 0), 14, 6);
    for x in 0..=11 {
        d.set(Point::new(x, 0), true);
    }
    for y in 1..=4 {
        d.set(Point::new(11, y), true);
    }

    let mut path: Vec<Point> = (0..=11).map(|x| Point::new(x, 0)).collect();
    path.extend((1..=4).map(|y| Point::new(11, y)));

    let sinks = BTreeSet::from([0]);

    (d, path, sinks)
}

/// AC1 helper (Ф6 local-repair): asserts that `after` differs from `before`
/// in **exactly one** cell's drivability, over `before`'s own bounding box
/// (every arm's `apply_edit` clones `before` verbatim, so the two share a
/// box by construction), and that the differing cell is `expected_cell` at
/// `expected_drivable`.
pub(crate) fn assert_single_cell_flip(
    before: &Corridor,
    after: &Corridor,
    expected_cell: Point,
    expected_drivable: bool,
) {
    let diffs: Vec<Point> = crate::phase4::box_points(before)
        .filter(|&p| before.contains(p) != after.contains(p))
        .collect();
    assert_eq!(
        diffs,
        vec![expected_cell],
        "expected exactly one cell to flip drivability"
    );
    assert_eq!(after.contains(expected_cell), expected_drivable);
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

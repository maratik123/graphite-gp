//! Ф6's local-repair task — edit plumbing (`in_box`, `add_edit_wall`,
//! `remove_edit_wall`, `apply_edit`) shared by all five repair arms
//! (design.md § Decomposition subtask 9; § The five arms).
//!
//! Every arm derives its candidate wall through `add_edit_wall`/
//! `remove_edit_wall` and applies it through `apply_edit`, so "one wall, one
//! cell flip" (AC1) is enforced in exactly one place rather than once per
//! arm.
#![allow(
    dead_code,
    reason = "no production caller until subtask 10 wires the first arms in — every item here \
              is already exercised by this module's own tests"
)]

use gp_core::geom::{Corridor, Point, Side, Wall};
use strum::IntoEnumIterator;

use crate::phase5b::{wall_neighbor, wall_sort_key};

/// Whether `p` lies inside `d`'s own bounding box (in-box, independent of
/// drivability) — `Corridor` exposes no public box-membership test distinct
/// from [`Corridor::contains`] (drivable), so this is derived from the
/// public `origin`/`width`/`height` accessors, mirroring
/// `phase4_defects::in_box`'s own re-derivation (a second, small,
/// independently-sited copy — the shared-module `≥3`-site rule does not
/// fire at two).
pub(crate) fn in_box(d: &Corridor, p: Point) -> bool {
    let origin = d.origin();
    let w = i32::try_from(d.width()).unwrap_or(i32::MAX);
    let h = i32::try_from(d.height()).unwrap_or(i32::MAX);
    p.x >= origin.x
        && p.x < origin.x.saturating_add(w)
        && p.y >= origin.y
        && p.y < origin.y.saturating_add(h)
}

/// The canonical wall naming the add-edit that makes `q` drivable, or `None`
/// when there is no such wall (design.md § The five arms, "add on cell
/// `q`"): `q` must be in-box, currently `¬D`, and have at least one
/// 4-neighbor already in `D`. Among the walls `w` with `w.cell ∈ D` and
/// `wall_neighbor(w) == Some(q)`, picks the min [`wall_sort_key`] — the
/// canonical wall when several identify the same flip.
pub(crate) fn add_edit_wall(d: &Corridor, q: Point) -> Option<Wall> {
    if !in_box(d, q) || d.contains(q) {
        return None;
    }
    Side::iter()
        .filter_map(|side| {
            let (dx, dy) = side.delta();
            let cell = Point::new(q.x.checked_sub(dx)?, q.y.checked_sub(dy)?);
            d.contains(cell).then_some(Wall { cell, side })
        })
        .min_by_key(|&w| wall_sort_key(w))
}

/// The canonical wall naming the remove-edit that makes `c` non-drivable, or
/// `None` when there is no such wall (design.md § The five arms, "remove on
/// cell `c`"): `c` must be currently `D`, with at least one side whose
/// neighbor is `¬D`/out-of-box. `c` with **no** such side is `D`-interior —
/// removing it would punch a new hole, so `None` (the caller returns
/// `NoEdit(NoCandidate)`) rather than proposing a non-boundary flip. Among
/// the admissible sides, picks the min [`wall_sort_key`].
pub(crate) fn remove_edit_wall(d: &Corridor, c: Point) -> Option<Wall> {
    if !d.contains(c) {
        return None;
    }
    Side::iter()
        .filter(|&side| {
            wall_neighbor(Wall { cell: c, side }).is_none_or(|neighbor| !d.contains(neighbor))
        })
        .map(|side| Wall { cell: c, side })
        .min_by_key(|&w| wall_sort_key(w))
}

/// Applies one dual-edge edit to a scratch copy of `d` (design.md § The five
/// arms): `drivable == true` makes the cell across `wall` drivable (an
/// add-edit); `drivable == false` makes `wall.cell` itself non-drivable (a
/// remove-edit). Returns the scratch corridor and the single cell whose
/// drivability flipped (AC1), or `None` when the add side's neighbor cannot
/// be resolved (`wall_neighbor` overflow) — never panics, total over any
/// caller-supplied `Wall`.
pub(crate) fn apply_edit(d: &Corridor, wall: Wall, drivable: bool) -> Option<(Corridor, Point)> {
    let mut scratch = d.clone();
    if drivable {
        let q = wall_neighbor(wall)?;
        scratch.set(q, true);
        Some((scratch, q))
    } else {
        scratch.set(wall.cell, false);
        Some((scratch, wall.cell))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testfix::assert_single_cell_flip;

    fn strip() -> Corridor {
        // A 5x1 strip, drivable at x in 1..=3, so (0,0) and (4,0) are
        // in-box and ¬D -- real add-edit targets, not Corridor::set no-ops.
        let mut d = Corridor::new(Point::new(0, 0), 5, 1);
        for x in 1..=3 {
            d.set(Point::new(x, 0), true);
        }
        d
    }

    #[test]
    fn in_box_is_true_only_within_the_bounding_box() {
        let d = strip();
        assert!(in_box(&d, Point::new(0, 0)));
        assert!(in_box(&d, Point::new(4, 0)));
        assert!(!in_box(&d, Point::new(5, 0)));
        assert!(!in_box(&d, Point::new(-1, 0)));
    }

    #[test]
    fn add_edit_wall_picks_the_canonical_min_wall_sort_key() {
        let d = strip();
        // (0,0)'s only D-neighbor is (1,0) via West -- one candidate.
        let w = add_edit_wall(&d, Point::new(0, 0)).expect("must find a candidate");
        assert_eq!(
            w,
            Wall {
                cell: Point::new(1, 0),
                side: Side::West,
            }
        );
        assert_eq!(wall_neighbor(w), Some(Point::new(0, 0)));
    }

    #[test]
    fn add_edit_wall_declines_an_out_of_box_target() {
        let d = strip();
        assert_eq!(add_edit_wall(&d, Point::new(99, 0)), None);
    }

    #[test]
    fn add_edit_wall_declines_an_already_drivable_target() {
        let d = strip();
        assert_eq!(add_edit_wall(&d, Point::new(2, 0)), None);
    }

    #[test]
    fn remove_edit_wall_picks_the_canonical_min_wall_sort_key() {
        // (1,0) is D-boundary: West neighbor (0,0) is ¬D.
        let d = strip();
        let w = remove_edit_wall(&d, Point::new(1, 0)).expect("must find a candidate");
        assert_eq!(w.cell, Point::new(1, 0));
    }

    #[test]
    fn remove_edit_wall_declines_a_d_interior_cell() {
        // A 3x3 filled square: the center cell has all four neighbors in D.
        let d = Corridor::filled(Point::new(0, 0), 3, 3);
        assert_eq!(remove_edit_wall(&d, Point::new(1, 1)), None);
    }

    #[test]
    fn remove_edit_wall_declines_a_non_drivable_cell() {
        let d = strip();
        assert_eq!(remove_edit_wall(&d, Point::new(0, 0)), None);
    }

    #[test]
    fn apply_edit_add_flips_exactly_the_named_neighbor_cell() {
        let d = strip();
        let w = add_edit_wall(&d, Point::new(0, 0)).unwrap();
        let (scratch, cell) = apply_edit(&d, w, true).expect("add must resolve a neighbor");
        assert_eq!(cell, Point::new(0, 0));
        assert_single_cell_flip(&d, &scratch, cell, true);
    }

    #[test]
    fn apply_edit_remove_flips_exactly_the_wall_cell() {
        let d = strip();
        let w = remove_edit_wall(&d, Point::new(1, 0)).unwrap();
        let (scratch, cell) = apply_edit(&d, w, false).expect("remove always resolves");
        assert_eq!(cell, Point::new(1, 0));
        assert_single_cell_flip(&d, &scratch, cell, false);
    }
}

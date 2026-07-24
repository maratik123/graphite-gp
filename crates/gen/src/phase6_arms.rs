//! Ф6's local-repair task — edit plumbing (`in_box`, `add_edit_wall`,
//! `remove_edit_wall`, `apply_edit`) shared by all five repair arms
//! (design.md § Decomposition subtask 9; § The five arms).
//!
//! Every arm derives its candidate wall through `add_edit_wall`/
//! `remove_edit_wall` and applies it through `apply_edit`, so "one wall, one
//! cell flip" (AC1) is enforced in exactly one place rather than once per
//! arm.

use gp_core::geom::{Corridor, Orient, Point, Side, Wall};
use gp_core::track::TrackMetrics;
use strum::IntoEnumIterator;

use crate::phase4_defects::{axis_width, is_concave_chord_cut};
use crate::phase5_runout::{end_of_ray, sink_to_sink_deficit, travel_dir};
use crate::phase5b::{wall_neighbor, wall_sort_key};
use crate::phase6_repair::{ArmOutcome, CommittedEdit, DeclineReason, RepairArm, recheck_scope};

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

/// The two [`Side`]s an [`Orient`]'s cap walls sit on: `(negative, positive)`
/// — `South`/`North` for `Vertical`, `West`/`East` for `Horizontal` (the
/// same convention [`Issue::Narrow`](crate::Issue::Narrow)'s `axis` field
/// documents).
const fn axis_cap_sides(axis: Orient) -> (Side, Side) {
    match axis {
        Orient::Vertical => (Side::South, Side::North),
        Orient::Horizontal => (Side::West, Side::East),
    }
}

/// The unit step along `axis`'s positive direction — `(0, 1)` for
/// `Vertical`, `(1, 0)` for `Horizontal`.
const fn axis_positive_dir(axis: Orient) -> (i32, i32) {
    match axis {
        Orient::Vertical => (0, 1),
        Orient::Horizontal => (1, 0),
    }
}

/// The `Narrow`/`NarrowSf` repair arm (design.md § The five arms,
/// `push_outer_wall_out`): tries both of the narrow chord's cap walls
/// (`{center, -axis}` and `{far_end, +axis}`, `far_end` re-derived by
/// walking `end_of_ray` from `center`), picks the admissible candidate with
/// max [`axis_width`] gain (tie broken by min [`wall_sort_key`]).
///
/// Re-validates `center ∈ D` against the working corridor first (the
/// "never trust the diagnostic" discipline) — a staled payload declines
/// rather than acting on a cell an earlier edit already touched.
pub(crate) fn push_outer_wall_out(d: &Corridor, center: Point, axis: Orient) -> ArmOutcome {
    if !d.contains(center) {
        return ArmOutcome::NoEdit(DeclineReason::StalePayload);
    }
    let far_end = end_of_ray(d, center, axis_positive_dir(axis));
    let (neg_side, pos_side) = axis_cap_sides(axis);
    let candidates = [
        Wall {
            cell: center,
            side: neg_side,
        },
        Wall {
            cell: far_end,
            side: pos_side,
        },
    ];
    let working_width = axis_width(d, center, axis);

    let mut admissible = false;
    let mut best: Option<(u32, Wall, Point)> = None;
    for &w in &candidates {
        let Some(q) = wall_neighbor(w) else { continue };
        if !in_box(d, q) || d.contains(q) {
            continue;
        }
        admissible = true;
        let Some((scratch, cell)) = apply_edit(d, w, true) else {
            continue;
        };
        let new_width = axis_width(&scratch, center, axis);
        if new_width <= working_width {
            continue;
        }
        let gain = new_width.saturating_sub(working_width);
        best = Some(match best {
            None => (gain, w, cell),
            Some((best_gain, best_w, best_cell)) => {
                if gain > best_gain
                    || (gain == best_gain && wall_sort_key(w) < wall_sort_key(best_w))
                {
                    (gain, w, cell)
                } else {
                    (best_gain, best_w, best_cell)
                }
            }
        });
    }

    match best {
        Some((_, w, cell)) => ArmOutcome::Edit(CommittedEdit {
            arm: RepairArm::PushOuterWallOut,
            wall: w,
            cell,
            drivable: true,
            recheck: recheck_scope(RepairArm::PushOuterWallOut),
        }),
        None if admissible => ArmOutcome::NoEdit(DeclineReason::MetricNotImproved),
        None => ArmOutcome::NoEdit(DeclineReason::NoCandidate),
    }
}

/// The `ConcaveChordCut` repair arm (design.md § The five arms,
/// `fill_inner_tooth`): re-validates `tooth`'s local hole-preservation guard
/// against the working corridor (`is_concave_chord_cut` — exactly one
/// 4-neighbor is `¬D` and it is in-box), then fills it. The guard alone
/// proves the local metric (`tooth` becomes drivable, one bounded
/// complement component survives — design.md § The five arms), so no
/// further scratch measurement is needed; `bounded_complement_components`
/// is asserted only test-side, as the proof the guard is sound (AC2: the
/// production path never calls it for an add-edit).
pub(crate) fn fill_inner_tooth(d: &Corridor, tooth: Point) -> ArmOutcome {
    if d.contains(tooth) || !is_concave_chord_cut(d, tooth) {
        return ArmOutcome::NoEdit(DeclineReason::StalePayload);
    }
    let Some(w) = add_edit_wall(d, tooth) else {
        return ArmOutcome::NoEdit(DeclineReason::NoCandidate);
    };
    let Some((_, cell)) = apply_edit(d, w, true) else {
        return ArmOutcome::NoEdit(DeclineReason::NoCandidate);
    };
    ArmOutcome::Edit(CommittedEdit {
        arm: RepairArm::FillInnerTooth,
        wall: w,
        cell,
        drivable: true,
        recheck: recheck_scope(RepairArm::FillInnerTooth),
    })
}

/// The [`Side`] `dir` points toward, or `None` for a non-cardinal (`(0, 0)`
/// or diagonal) direction — [`travel_dir`] never returns a diagonal, but
/// this stays total over any `(i32, i32)` input.
const fn side_of(dir: (i32, i32)) -> Option<Side> {
    match dir {
        (1, 0) => Some(Side::East),
        (-1, 0) => Some(Side::West),
        (0, 1) => Some(Side::North),
        (0, -1) => Some(Side::South),
        _ => None,
    }
}

/// The two [`Side`]s perpendicular to `dir` — `North`/`South` when `dir` is
/// horizontal, `East`/`West` when `dir` is vertical (design.md § Decision 3,
/// `widen_corner`'s candidate derivation).
const fn perpendicular_sides(dir: (i32, i32)) -> [Side; 2] {
    if dir.1 == 0 {
        [Side::North, Side::South]
    } else {
        [Side::East, Side::West]
    }
}

/// The joint `NoBraking` repair arm (design.md § Decision 3): both
/// `lengthen_straight` and `widen_corner` candidates are generated,
/// evaluated under the **same** metric ([`sink_to_sink_deficit`] on a
/// scratch copy), and the winner is chosen by max deficit reduction → arm
/// rank (`LengthenStraight` before `WidenCorner`) → min [`wall_sort_key`].
///
/// Re-validates `c ∈ metrics.fastest_lap` and `c ∈ d` against the working
/// corridor and metrics first — a stale payload (an earlier edit already
/// touched `c`, or `c` is no longer deficient) declines rather than acting.
pub(crate) fn run_out_repair(
    d: &Corridor,
    metrics: &TrackMetrics,
    v_target: i32,
    c: Point,
) -> ArmOutcome {
    if !d.contains(c) {
        return ArmOutcome::NoEdit(DeclineReason::StalePayload);
    }
    let Some(working_deficit) = sink_to_sink_deficit(d, metrics, c, v_target) else {
        return ArmOutcome::NoEdit(DeclineReason::StalePayload);
    };
    if working_deficit <= 0 {
        return ArmOutcome::NoEdit(DeclineReason::StalePayload);
    }
    let dir = travel_dir(&metrics.fastest_lap, {
        let Some(idx) = metrics.fastest_lap.iter().position(|&p| p == c) else {
            return ArmOutcome::NoEdit(DeclineReason::StalePayload);
        };
        idx
    });
    let Some(prim_side) = side_of(dir) else {
        return ArmOutcome::NoEdit(DeclineReason::StalePayload);
    };
    let end = end_of_ray(d, c, dir);

    let mut candidates = vec![(
        RepairArm::LengthenStraight,
        0u8,
        Wall {
            cell: end,
            side: prim_side,
        },
    )];
    for side in perpendicular_sides(dir) {
        candidates.push((RepairArm::WidenCorner, 1u8, Wall { cell: end, side }));
    }

    let mut admissible = false;
    let mut best: Option<(i32, u8, Wall, RepairArm, Point)> = None;
    for (arm, arm_rank, w) in candidates {
        let Some(q) = wall_neighbor(w) else { continue };
        if !in_box(d, q) || d.contains(q) {
            continue;
        }
        admissible = true;
        let Some((scratch, cell)) = apply_edit(d, w, true) else {
            continue;
        };
        let Some(new_deficit) = sink_to_sink_deficit(&scratch, metrics, c, v_target) else {
            continue;
        };
        if new_deficit >= working_deficit {
            continue;
        }
        let reduction = working_deficit.saturating_sub(new_deficit);
        best = Some(match best {
            None => (reduction, arm_rank, w, arm, cell),
            Some((br, bar, bw, ba, bc)) => {
                if reduction > br
                    || (reduction == br && arm_rank < bar)
                    || (reduction == br && arm_rank == bar && wall_sort_key(w) < wall_sort_key(bw))
                {
                    (reduction, arm_rank, w, arm, cell)
                } else {
                    (br, bar, bw, ba, bc)
                }
            }
        });
    }

    match best {
        Some((_, _, w, arm, cell)) => ArmOutcome::Edit(CommittedEdit {
            arm,
            wall: w,
            cell,
            drivable: true,
            recheck: recheck_scope(arm),
        }),
        None if admissible => ArmOutcome::NoEdit(DeclineReason::MetricNotImproved),
        None => ArmOutcome::NoEdit(DeclineReason::NoCandidate),
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

    // ---- push_outer_wall_out ---------------------------------------------

    /// A 3-row-tall corridor pinched to a single-row neck at `x=3` (mirrors
    /// `phase4_defects`'s `narrow_sharp_single_cross_section_neck_fires_once`
    /// fixture): the neck cell `(3,2)` has `axis_width == 1` along
    /// `Vertical`, with both cap neighbors `(3,1)`/`(3,3)` in-box and `¬D`.
    fn neck_d() -> Corridor {
        let mut drivable = Vec::new();
        for x in 0..7 {
            if x == 3 {
                drivable.push((x, 2));
            } else {
                for y in 1..4 {
                    drivable.push((x, y));
                }
            }
        }
        let mut d = Corridor::new(Point::new(0, 0), 7, 5);
        for (x, y) in drivable {
            d.set(Point::new(x, y), true);
        }
        d
    }

    #[test]
    fn push_outer_wall_out_strictly_grows_the_width_on_the_neck_fixture() {
        let d = neck_d();
        let center = Point::new(3, 2);
        let before = axis_width(&d, center, Orient::Vertical);

        let ArmOutcome::Edit(edit) = push_outer_wall_out(&d, center, Orient::Vertical) else {
            panic!("expected an Edit on the neck fixture");
        };
        assert_eq!(edit.arm, RepairArm::PushOuterWallOut);
        assert!(edit.drivable);
        assert_eq!(edit.recheck, recheck_scope(RepairArm::PushOuterWallOut));

        let (scratch, cell) = apply_edit(&d, edit.wall, true).expect("edit must apply");
        assert_eq!(cell, edit.cell);
        assert_single_cell_flip(&d, &scratch, edit.cell, true); // AC1
        let after = axis_width(&scratch, center, Orient::Vertical);
        assert!(
            after > before,
            "width must strictly grow: {before} -> {after}"
        );

        // Both cap candidates tie at gain 1; North (rank 2) < South (rank 3)
        // breaks the tie deterministically.
        assert_eq!(
            edit.wall,
            Wall {
                cell: center,
                side: Side::North,
            }
        );
        assert_eq!(edit.cell, Point::new(3, 3));
    }

    #[test]
    fn push_outer_wall_out_declines_when_both_cap_walls_lack_bounding_box_headroom() {
        // A single-row (height=1) corridor: both the north and south cap
        // neighbors of any cell fall outside the box.
        let d = Corridor::filled(Point::new(0, 0), 5, 1);
        let center = Point::new(2, 0);
        assert!(matches!(
            push_outer_wall_out(&d, center, Orient::Vertical),
            ArmOutcome::NoEdit(DeclineReason::NoCandidate)
        ));
    }

    // ---- fill_inner_tooth --------------------------------------------------

    /// A fully-drivable 9x9 box with a solid 3x3 non-drivable hole at
    /// `(4..=6, 4..=6)` — mirrors `phase4_defects`'s `hole_frame_d`.
    fn hole_frame_d() -> Corridor {
        let mut d = Corridor::filled(Point::new(0, 0), 9, 9);
        for y in 4..=6 {
            for x in 4..=6 {
                d.set(Point::new(x, y), false);
            }
        }
        d
    }

    #[test]
    fn fill_inner_tooth_commits_and_preserves_the_one_hole() {
        use gp_core::geom::bounded_complement_components;

        let mut d = hole_frame_d();
        d.set(Point::new(3, 5), false); // degree-1 tooth, sole ¬D neighbor (4,5)

        let ArmOutcome::Edit(edit) = fill_inner_tooth(&d, Point::new(3, 5)) else {
            panic!("expected an Edit on the tooth fixture");
        };
        assert_eq!(edit.arm, RepairArm::FillInnerTooth);
        assert_eq!(edit.cell, Point::new(3, 5));
        assert!(edit.drivable);

        let (scratch, cell) = apply_edit(&d, edit.wall, true).expect("edit must apply");
        assert!(scratch.contains(Point::new(3, 5)));
        assert_single_cell_flip(&d, &scratch, cell, true); // AC1
        // Test-side proof of the local guard's soundness (AC2: never called
        // in the production path).
        assert_eq!(bounded_complement_components(&scratch), 1);
    }

    #[test]
    fn fill_inner_tooth_declines_a_border_tooth() {
        // A border tooth's sole ¬D neighbor is out-of-box -- not a
        // ConcaveChordCut candidate at all (design.md's local guard scope).
        let mut d = Corridor::filled(Point::new(0, 0), 5, 5);
        d.set(Point::new(0, 2), false);
        assert!(matches!(
            fill_inner_tooth(&d, Point::new(0, 2)),
            ArmOutcome::NoEdit(DeclineReason::StalePayload)
        ));
    }

    // ---- run_out_repair -----------------------------------------------

    use crate::testfix::brake_deficit_corridor;
    use gp_core::geom::Point as P;

    #[test]
    fn lengthen_straight_wins_on_brake_deficit_corridor() {
        let (d, path, _sinks) = brake_deficit_corridor();
        let metrics = TrackMetrics {
            fastest_lap: path.clone(),
            ..Default::default()
        };
        let c = path[10];
        assert_eq!(c, P::new(10, 0));

        let ArmOutcome::Edit(edit) = run_out_repair(&d, &metrics, 3, c) else {
            panic!("expected an Edit on brake_deficit_corridor");
        };
        assert_eq!(edit.arm, RepairArm::LengthenStraight);
        assert_eq!(
            edit.wall,
            Wall {
                cell: P::new(11, 0),
                side: Side::East,
            }
        );
        assert_eq!(edit.cell, P::new(12, 0));
        assert!(edit.drivable);
        assert_eq!(edit.recheck, crate::phase6_repair::RecheckScope::SinkToSink);

        let (scratch, cell) = apply_edit(&d, edit.wall, true).expect("edit must apply");
        assert_single_cell_flip(&d, &scratch, cell, true); // AC1
    }

    /// Direct proof that `widen_corner`'s lever (`corner_speed`) can
    /// strictly increase (AC4/binding rule 3: every arm's metric must
    /// actually be able to improve under its own edit). Uses
    /// `window_speed`/`corner_speed` directly rather than the full
    /// `run_out_repair` dispatch: seeded straight at the exact state that
    /// exercises the mechanism (agnostic to how the flood was seeded, the
    /// same property the AC3 counter-scope relies on).
    ///
    /// A single drivable cell `(1, 0)`, isolated (no other cell in the box
    /// is drivable pre-edit). Seeding a state there with `vy = 2` gives it
    /// **zero** legal successors pre-edit (every action overshoots the
    /// empty box); widening `(1, 1)` (`North` of `(1, 0)`) makes `South`
    /// (decelerate `vy: 2 -> 1`, landing exactly on the new cell) legal,
    /// so `corner_speed` jumps from `0` (the seed doesn't qualify at all)
    /// to `2` (`vnorm` of the qualifying state).
    #[test]
    fn widen_corner_lever_strictly_increases_corner_speed() {
        use crate::phase5_runout::{corner_speed, window_speed};
        use std::collections::HashSet as HSet;

        let end = P::new(1, 0);
        let seed = crate::testfix::car(1, 0, 0, 2);

        let mut d_pre = Corridor::new(P::new(0, 0), 3, 3);
        d_pre.set(end, true);
        let flood_pre = window_speed(&d_pre, &[seed], &HSet::new(), 3);
        assert_eq!(corner_speed(&d_pre, &flood_pre, end), 0);

        let mut d_post = d_pre.clone();
        d_post.set(P::new(1, 1), true);
        let flood_post = window_speed(&d_post, &[seed], &HSet::new(), 3);
        let after = corner_speed(&d_post, &flood_post, end);
        assert_eq!(
            after, 2,
            "widening must let the vy=2 seed decelerate onto the new cell"
        );
    }

    #[test]
    fn run_out_repair_picks_widen_corner_when_lengthen_is_inadmissible() {
        // A single-column shaft x=3, y in -2..=1, minus the widen target
        // (3,1). The path is vertical up to c=(3,0) then turns east
        // (`travel_dir`'s forward-step preference makes dir(c) == East),
        // so the sink seed at path[0]=(3,-2) has |vy| <= 1 already -- one
        // North action reaches (3,0) with vx=0, vy=2 in a single hop,
        // giving a vy-dominant arrival that can only be rescued by a
        // *perpendicular* (to dir=East) edit: South-decelerating onto the
        // widened (3,1) cell. East of c is out-of-box, so
        // lengthen_straight is inadmissible and cannot compete.
        let mut d = Corridor::new(P::new(3, -2), 1, 4);
        d.set(P::new(3, -2), true);
        d.set(P::new(3, -1), true);
        d.set(P::new(3, 0), true);

        let path = vec![P::new(3, -2), P::new(3, -1), P::new(3, 0), P::new(4, 0)];
        let metrics = TrackMetrics {
            fastest_lap: path.clone(),
            ..Default::default()
        };
        let c = path[2];
        let v_target = 2;

        let working = sink_to_sink_deficit(&d, &metrics, c, v_target).expect("c is a path point");
        assert!(working > 0, "fixture must fire: deficit was {working}");

        let ArmOutcome::Edit(edit) = run_out_repair(&d, &metrics, v_target, c) else {
            panic!("expected an Edit; working deficit was {working}");
        };
        assert_eq!(edit.arm, RepairArm::WidenCorner);
        assert_eq!(edit.wall.cell, P::new(3, 0));
        assert_eq!(edit.wall.side, Side::North);
        assert_eq!(edit.cell, P::new(3, 1));

        let (scratch, cell) = apply_edit(&d, edit.wall, true).expect("edit must apply");
        assert_single_cell_flip(&d, &scratch, cell, true); // AC1
        let after =
            sink_to_sink_deficit(&scratch, &metrics, c, v_target).expect("still a path point");
        assert!(
            after < working,
            "deficit must strictly decrease: {working} -> {after}"
        );
    }

    #[test]
    fn run_out_repair_declines_metric_not_improved_when_no_candidate_helps() {
        // A single-column shaft x=3, y in 0..=3, box 7 wide so both East
        // and West of every shaft cell are in-box and admissible. Path
        // walks straight up the shaft then turns East at the very top row
        // (y=3) -- `dir(c) == East` for the probe point c=(3,2) (its
        // forward step in the frozen path is (3,3), same axis as the
        // shaft, so `travel_dir` reads East only at the true turn index;
        // c sits one row below the turn, so dir(c) is North -- lengthen
        // extends the shaft upward, WidenCorner tries East/West of the
        // shaft top). Found by exhaustive search over shaft lengths and
        // v_target (see the design's own `[C3]` adverse-effect note,
        // Decision 2: lengthening a straight can raise the arrival speed
        // by more than the room it adds); this exact combination is a
        // case where EVERY admissible candidate's deficit is unchanged --
        // `admissible == true`, `best == None`.
        let mut d = Corridor::new(P::new(0, 0), 7, 4);
        for y in 0..4 {
            d.set(P::new(3, y), true);
        }
        let path = vec![
            P::new(3, 0),
            P::new(3, 1),
            P::new(3, 2),
            P::new(3, 3),
            P::new(4, 3),
        ];
        let metrics = TrackMetrics {
            fastest_lap: path.clone(),
            ..Default::default()
        };
        let c = path[2];
        let v_target = 2;

        let working = sink_to_sink_deficit(&d, &metrics, c, v_target).expect("c is a path point");
        assert_eq!(working, 1, "fixture premise: must fire with deficit 1");

        assert!(matches!(
            run_out_repair(&d, &metrics, v_target, c),
            ArmOutcome::NoEdit(DeclineReason::MetricNotImproved)
        ));
    }

    #[test]
    fn run_out_repair_breaks_an_arm_rank_tie_in_favor_of_lengthen_straight() {
        // Found by exhaustive search (design § Test Design subtask 11's
        // "arm-rank tie-break asserted on a constructed tie", never
        // implemented -- self-review finding 3): a vertical shaft x=3,
        // y=-5..=0, with a short EAST dogleg starting at (5,0) (a single
        // drivable cell, (4,0) left as an in-box gap) before the real
        // 90-degree turn. At c=(3,0), v_target=3:
        // - LengthenStraight (East of c -> (4,0) admissible, since (4,0)
        //   is an in-box gap) reduces the deficit from 5 to 3 (bridges the
        //   gap to the pre-existing hook cell (5,0), a 2-cell runout_room
        //   gain in one edit).
        // - WidenCorner (North of c -> (3,1) admissible) ALSO reduces the
        //   deficit from 5 to 3 -- an exact tie in `reduction`.
        // LengthenStraight has arm_rank 0 < WidenCorner's 1, so it must
        // win despite being evaluated second in `run_out_repair`'s own
        // candidate list (Lengthen is pushed first, but the code's `<`
        // comparison must still resolve the tie correctly, not just by
        // insertion order).
        let mut d = Corridor::new(P::new(3, -5), 5, 7);
        for y in -5..=0 {
            d.set(P::new(3, y), true);
        }
        d.set(P::new(5, 0), true); // the hook cell; (4,0) stays an in-box gap
        let mut path: Vec<P> = (-5..=0).map(|y| P::new(3, y)).collect();
        path.push(P::new(4, 0));
        path.push(P::new(5, 0));
        let metrics = TrackMetrics {
            fastest_lap: path,
            ..Default::default()
        };
        let c = P::new(3, 0);
        let v_target = 3;

        let working = sink_to_sink_deficit(&d, &metrics, c, v_target).expect("c is a path point");
        assert_eq!(working, 5, "fixture premise: working deficit");

        let lengthen_wall = Wall {
            cell: c,
            side: Side::East,
        };
        let (lengthen_scratch, _) = apply_edit(&d, lengthen_wall, true).unwrap();
        let nd1 = sink_to_sink_deficit(&lengthen_scratch, &metrics, c, v_target)
            .expect("still a path point");

        let widen_wall = Wall {
            cell: c,
            side: Side::North,
        };
        let (widen_scratch, _) = apply_edit(&d, widen_wall, true).unwrap();
        let nd2 = sink_to_sink_deficit(&widen_scratch, &metrics, c, v_target)
            .expect("still a path point");

        assert_eq!(nd1, 3, "fixture premise: lengthen's post-edit deficit");
        assert_eq!(nd2, 3, "fixture premise: widen's post-edit deficit");
        assert_eq!(
            working - nd1,
            working - nd2,
            "fixture premise: the two candidates' reductions must tie"
        );

        let ArmOutcome::Edit(edit) = run_out_repair(&d, &metrics, v_target, c) else {
            panic!("expected an Edit on the tied fixture");
        };
        assert_eq!(
            edit.arm,
            RepairArm::LengthenStraight,
            "arm-rank tie-break: LengthenStraight (rank 0) must beat WidenCorner (rank 1)"
        );
        assert_eq!(edit.wall, lengthen_wall);
        assert_eq!(edit.cell, P::new(4, 0));

        let (scratch, cell) = apply_edit(&d, edit.wall, true).expect("edit must apply");
        assert_single_cell_flip(&d, &scratch, cell, true); // AC1
    }
}

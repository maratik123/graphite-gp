//! Ф6's local-repair task — the two remove arms (`trim_arm_wall`,
//! `nudge_finger`) and their shared first-admissible search
//! (design.md § Decomposition subtask 12).
//!
//! Mechanically split out of `phase6_arms.rs` at subtask 12 — that module
//! reached **859** lines incl. tests once these two arms landed, crossing
//! the 800-line soft cap. § Approach's pre-decided split rule names this
//! exact move (`trim_arm_wall` + `nudge_finger` + their tests), so it is
//! applied directly rather than re-derived: no logic change, only the file
//! boundary.

use std::collections::{BTreeSet, HashSet};

use gp_core::geom::{Corridor, Point, bounded_complement_components, component_count};

use crate::CoarseSkeleton;
use crate::coarse::block_points;
use crate::phase4::{absorbed, infield_fingers};
use crate::phase4_defects::expanded_hole_mask;
use crate::phase6_arms::{apply_edit, remove_edit_wall};
use crate::phase6_repair::{ArmOutcome, CommittedEdit, DeclineReason, RepairArm, recheck_scope};

/// The 4-connected component of `remaining` containing `anchor` (a BFS
/// confined to `remaining`, mirroring `phase4_defects::arms_merging_issues`'
/// component discovery).
fn component_of(remaining: &BTreeSet<Point>, anchor: Point) -> HashSet<Point> {
    let mut stack = vec![anchor];
    let mut component = HashSet::from([anchor]);
    while let Some(p) = stack.pop() {
        for n in p.neighbors4() {
            if remaining.contains(&n) && component.insert(n) {
                stack.push(n);
            }
        }
    }
    component
}

/// First-admissible remove search shared by `trim_arm_wall`/`nudge_finger`
/// (design.md § The five arms): tries `candidates` (already sorted
/// ascending `Point`) in order, committing the first whose remove-edit
/// passes the `[C3]` global-flood-fill recheck
/// (`component_count == 1 && bounded_complement_components == 1`) — this
/// combined condition **is** both the arm's own metric (`|H ∩ D|` strictly
/// decreases by construction: removing one cell drops the set's size by
/// exactly one) and the recheck (design.md's remove-arm metric row states
/// them jointly).
fn first_admissible_remove(d: &Corridor, candidates: &[Point], arm: RepairArm) -> ArmOutcome {
    let mut admissible = false;
    for &c in candidates {
        let Some(w) = remove_edit_wall(d, c) else {
            continue;
        };
        admissible = true;
        let Some((scratch, cell)) = apply_edit(d, w, false) else {
            continue;
        };
        if component_count(&scratch) == 1 && bounded_complement_components(&scratch) == 1 {
            return ArmOutcome::Edit(CommittedEdit {
                arm,
                wall: w,
                cell,
                drivable: false,
                recheck: recheck_scope(arm),
            });
        }
    }
    if admissible {
        ArmOutcome::NoEdit(DeclineReason::RecheckFailed)
    } else {
        ArmOutcome::NoEdit(DeclineReason::NoCandidate)
    }
}

/// The `ArmsMerging` repair arm (design.md § The five arms, `trim_arm_wall`):
/// re-validates `bridge ∈ H ∩ D` against the working corridor, re-derives
/// its 4-connected component within `H ∩ D`, and tries each `D`-boundary
/// cell of that component (ascending `Point`) with [`first_admissible_remove`].
pub(crate) fn trim_arm_wall(
    d: &Corridor,
    skel: &CoarseSkeleton,
    k: i32,
    bridge: Point,
) -> ArmOutcome {
    let h = expanded_hole_mask(skel, k);
    if !d.contains(bridge) || !h.contains(&bridge) {
        return ArmOutcome::NoEdit(DeclineReason::StalePayload);
    }
    let remaining: BTreeSet<Point> = h.into_iter().filter(|&p| d.contains(p)).collect();
    let mut candidates: Vec<Point> = component_of(&remaining, bridge).into_iter().collect();
    candidates.sort();
    first_admissible_remove(d, &candidates, RepairArm::TrimArmWall)
}

/// The `LostHairpin` repair arm (design.md § The five arms, `nudge_finger`):
/// re-validates `tip` is still an absorbed finger against the working
/// corridor, then tries each `D`-boundary cell of the finger's fine
/// footprint (ascending `Point`) with [`first_admissible_remove`].
pub(crate) fn nudge_finger(d: &Corridor, skel: &CoarseSkeleton, k: i32, tip: Point) -> ArmOutcome {
    let fingers = infield_fingers(skel);
    let Some(finger) = fingers.get(&tip) else {
        return ArmOutcome::NoEdit(DeclineReason::StalePayload);
    };
    if !absorbed(finger, d, k) {
        return ArmOutcome::NoEdit(DeclineReason::StalePayload);
    }
    let mut candidates: Vec<Point> = finger
        .iter()
        .flat_map(|&c| block_points(c, k))
        .filter(|&p| d.contains(p))
        .collect();
    candidates.sort();
    candidates.dedup();
    first_admissible_remove(d, &candidates, RepairArm::NudgeFinger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gp_core::geom::Point as P;

    fn arms_skel() -> CoarseSkeleton {
        CoarseSkeleton {
            ring: BTreeSet::new(),
            hole: BTreeSet::from([P::new(1, 1)]),
            dir: gp_core::track::RaceDir::Cw,
        }
    }

    #[test]
    fn trim_arm_wall_clears_a_one_cell_infield_intrusion() {
        // A 9x9 filled box with the coarse hole {(1,1)} at k=3 (H = fine
        // 3..=5, 3..=5) entirely ¬D except a single drivable intrusion cell
        // at its center, isolated inside the hole (mirrors
        // phase4_defects's arms_merging fixtures).
        let mut d = Corridor::filled(P::new(0, 0), 9, 9);
        for x in 3..=5 {
            for y in 3..=5 {
                d.set(P::new(x, y), false);
            }
        }
        d.set(P::new(4, 4), true);

        let ArmOutcome::Edit(edit) = trim_arm_wall(&d, &arms_skel(), 3, P::new(4, 4)) else {
            panic!("expected an Edit clearing the intrusion");
        };
        assert_eq!(edit.arm, RepairArm::TrimArmWall);
        assert_eq!(edit.cell, P::new(4, 4));
        assert!(!edit.drivable);
        assert_eq!(
            edit.recheck,
            crate::phase6_repair::RecheckScope::GlobalFloodFill
        );

        let (scratch, cell) = apply_edit(&d, edit.wall, false).expect("edit must apply");
        crate::testfix::assert_single_cell_flip(&d, &scratch, cell, false); // AC1
        assert_eq!(component_count(&scratch), 1);
        assert_eq!(bounded_complement_components(&scratch), 1);
    }

    #[test]
    fn trim_arm_wall_rejects_every_candidate_when_all_would_disconnect_d() {
        // Two rooms (x:0..3 and x:6..9) joined ONLY by a 3-cell drivable
        // row through H (x:3..=5, y=4) -- every cell of that row is the
        // sole connector, so removing any one of them splits D into two
        // components; the arm must exhaust all three candidates and
        // decline rather than committing a disconnecting edit.
        let mut d = Corridor::new(P::new(0, 0), 9, 9);
        for y in 0..9 {
            for x in 0..3 {
                d.set(P::new(x, y), true);
            }
            for x in 6..9 {
                d.set(P::new(x, y), true);
            }
        }
        for x in 3..=5 {
            d.set(P::new(x, 4), true);
        }

        assert!(matches!(
            trim_arm_wall(&d, &arms_skel(), 3, P::new(3, 4)),
            ArmOutcome::NoEdit(DeclineReason::RecheckFailed)
        ));
    }

    #[test]
    fn nudge_finger_reopens_an_absorbed_finger_at_its_boundary_cell() {
        // Mirrors phase4.rs's base_ring_d + hole_with_finger_skel: a 21x21
        // frame with a 9x9 hole (x:6..15, y:6..15) and a coarse peninsula
        // (5,3) whose k=3 fine footprint (x:15..18, y:9..12) is fully
        // filled in -- an absorbed LostHairpin finger.
        let mut d = Corridor::filled(P::new(0, 0), 21, 21);
        for y in 6..15 {
            for x in 6..15 {
                d.set(P::new(x, y), false);
            }
        }
        for x in 15..18 {
            for y in 9..12 {
                d.set(P::new(x, y), true);
            }
        }
        let mut skel = CoarseSkeleton {
            ring: BTreeSet::new(),
            hole: (2..5)
                .flat_map(|x| (2..5).map(move |y| P::new(x, y)))
                .collect(),
            dir: gp_core::track::RaceDir::Cw,
        };
        skel.hole.insert(P::new(5, 3));

        let ArmOutcome::Edit(edit) = nudge_finger(&d, &skel, 3, P::new(5, 3)) else {
            panic!("expected an Edit re-opening the finger");
        };
        assert_eq!(edit.arm, RepairArm::NudgeFinger);
        assert!(!edit.drivable);
        // The only D-boundary column of the footprint is x=15 (its west
        // neighbor x=14 lies in the main hole); ascending Point picks
        // (15, 9) first.
        assert_eq!(edit.cell, P::new(15, 9));

        let (scratch, cell) = apply_edit(&d, edit.wall, false).expect("edit must apply");
        crate::testfix::assert_single_cell_flip(&d, &scratch, cell, false); // AC1
        assert_eq!(component_count(&scratch), 1);
        assert_eq!(bounded_complement_components(&scratch), 1);
    }
}

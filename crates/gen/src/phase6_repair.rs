//! Ф6 — the single-pass local-repair driver: public types, the crate-local
//! dispatch vocabulary, `recheck_scope`, the severity ordering
//! (`issue_rank`/`issue_sort_key`), the per-label `dispatch` function, and
//! the top-level [`phase6_local_repair`] entry point (design doc §2 Ф6,
//! `[C3]`; `ai-docs/plans/2026-07-24-gp-gen-phase6-local-repair.design.md`
//! § Approach, § Decision 4).

use gp_core::geom::{Corridor, Point, Wall};
use gp_core::track::{RaceDir, StartFinish, StartGrid, TrackMetrics};

use crate::CoarseSkeleton;
use crate::Issue;
use crate::phase6::RepairCandidate;

/// Everything Ф6's single pass needs beyond the working corridor and issue list.
///
/// design.md § Approach — `Ф6's own signature`. A context struct (rather
/// than 11 parameters) keeps `clippy::too_many_arguments` (pedantic, deny)
/// satisfied.
pub struct RepairContext<'a> {
    /// The corridor to repair.
    pub d: &'a Corridor,
    /// The coarse skeleton (drives the infield mask arms read).
    pub skel: &'a CoarseSkeleton,
    /// The coarse-block size (Ф2's `k`).
    pub k: i32,
    /// The global width floor (`GenParams::min_width`).
    pub n: u32,
    /// The S/F width floor (`GenParams::start_finish_width`).
    pub m: u32,
    /// The start grid.
    pub grid: &'a StartGrid,
    /// The start/finish chord.
    pub sf: &'a StartFinish,
    /// Race direction.
    pub race_dir: RaceDir,
    /// The run-out speed target (`v_target`).
    pub v_target: i32,
    /// The Ф5b oracle's `Lappable` payload, when the last oracle run was
    /// lappable — `None` declines every run-out arm (`MissingOracleInput`).
    pub metrics: Option<&'a TrackMetrics>,
    /// The Ф5b oracle's `NotLappable` stall diagnostic, when the last oracle
    /// run was not lappable — `None` declines the dynamic arm.
    pub stall_walls: Option<&'a [Wall]>,
}

/// The outcome of one `phase6_local_repair` pass (design.md § Approach).
#[derive(Clone, Debug)]
pub enum RepairOutcome {
    /// `≥ 1` edit committed. `edits` is non-empty by construction, in pass
    /// order.
    Repaired {
        /// The repaired corridor.
        d: Corridor,
        /// Every committed edit, in the order it was applied.
        edits: Vec<CommittedEdit>,
    },
    /// Zero edits committed — the `[N4]` reseed signal.
    Failed,
}

/// One committed repair edit — exactly one dual edge, exactly one cell
/// drivability flip (AC1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CommittedEdit {
    /// Which arm produced the edit.
    pub arm: RepairArm,
    /// The single dual edge naming the flip (AC1).
    pub wall: Wall,
    /// The single cell whose drivability flipped (AC1).
    pub cell: Point,
    /// `true` = add-edit, `false` = remove-edit.
    pub drivable: bool,
    /// The recheck scope actually taken (AC2) — read off this field, never
    /// inferred from timing.
    pub recheck: RecheckScope,
}

/// The repair arm that produced a [`CommittedEdit`] (design.md § Approach).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RepairArm {
    /// Pushes an outer wall out to widen a narrow cross-section.
    PushOuterWallOut,
    /// Fills a degree-1 non-drivable protrusion.
    FillInnerTooth,
    /// Extends a braking straight upstream of a deficient corner entry.
    LengthenStraight,
    /// Widens a corner so a faster arrival keeps a legal successor.
    WidenCorner,
    /// Trims a drivable intrusion into the infield.
    TrimArmWall,
    /// Re-opens a lost-hairpin finger's separating strip.
    NudgeFinger,
    /// Maps a Ф5b stall diagnostic to a frontier-gap edge (#30).
    MapFrontierGap,
}

/// The `[C3]` recheck scope an edit type takes (design.md § Approach).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RecheckScope {
    /// The arm's own local metric only — no whole-corridor flood-fill.
    Local,
    /// `component_count == 1` and `bounded_complement_components == 1` on
    /// the scratch-edited `D`.
    GlobalFloodFill,
    /// `deficit_at` re-measured under the sink-to-sink flood.
    SinkToSink,
}

/// The `[C3]` recheck scope for `arm` (design.md § Approach, `[C3]` recheck
/// routing table): the single source of truth both the driver's `verify`
/// call and `CommittedEdit.recheck` read, so the scope actually taken is
/// observable from the output, never inferred from timing (AC2).
#[must_use]
pub(crate) const fn recheck_scope(arm: RepairArm) -> RecheckScope {
    match arm {
        RepairArm::PushOuterWallOut | RepairArm::FillInnerTooth | RepairArm::MapFrontierGap => {
            RecheckScope::Local
        }
        RepairArm::LengthenStraight | RepairArm::WidenCorner => RecheckScope::SinkToSink,
        RepairArm::TrimArmWall | RepairArm::NudgeFinger => RecheckScope::GlobalFloodFill,
    }
}

/// The result of dispatching one label against the working corridor
/// (design.md § Approach).
pub(crate) enum ArmOutcome {
    /// The arm produced and committed an edit.
    Edit(CommittedEdit),
    /// No edit was committed, for the stated reason.
    #[allow(
        dead_code,
        reason = "the DeclineReason payload is diagnostic surface for tests asserting which \
                  decline fired per label — phase6_local_repair itself only branches on \
                  Edit-vs-NoEdit, it never reads why a decline happened"
    )]
    NoEdit(DeclineReason),
}

/// Why `dispatch` produced no edit (design.md § Approach).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DeclineReason {
    /// The label has no single-dual-edge repair (`Disconnected`/`BadTopology`).
    NotRepairable,
    /// The arm's family needs oracle input `RepairContext` does not carry.
    MissingOracleInput,
    /// The payload no longer holds against the working `D` (an earlier edit
    /// staled it).
    StalePayload,
    /// No admissible candidate wall/cell was found.
    NoCandidate,
    /// A candidate was found, but its scratch metric did not strictly
    /// improve.
    MetricNotImproved,
    /// A candidate strictly improved its own metric but failed the `[C3]`
    /// recheck.
    RecheckFailed,
}

/// One entry of the single pass's dispatch list: either a static/dynamic
/// [`Issue`] or the dynamic (Ф5b stall) arm, which carries no `Issue`
/// payload of its own (design.md § Approach; the `Issue` enum has no
/// `DynamicallyDisconnected` variant — #30 settled that the dynamic verdict
/// rides `OracleResult::NotLappable` instead).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DispatchLabel {
    /// A statically or dynamically detected [`Issue`].
    Issue(Issue),
    /// The dynamic frontier-gap arm, driven by `ctx.stall_walls`.
    DynamicStall,
}

/// The axis-order rank of an [`gp_core::geom::Orient`] for [`issue_sort_key`]
/// — `Horizontal` before `Vertical`. Neither `Orient` nor `Issue` derives
/// `Ord` (a `gp-core`/`phase4.rs` change, out of scope), so this crate-local
/// key gives the sort a total order, mirroring `phase5b::wall_sort_key`.
const fn axis_rank(axis: gp_core::geom::Orient) -> u8 {
    match axis {
        gp_core::geom::Orient::Horizontal => 0,
        gp_core::geom::Orient::Vertical => 1,
    }
}

/// The fixed severity rank of `Issue` `i`, `0` (highest) to `7` (design.md
/// § Decision 4's rank table) — **not** `phase4_static_checks`' emission
/// order, so a future Ф4 refactor cannot silently change Ф6's outcome.
#[must_use]
pub(crate) const fn issue_rank(i: Issue) -> u8 {
    match i {
        Issue::Disconnected => 0,
        Issue::BadTopology => 1,
        Issue::LostHairpin { .. } => 2,
        Issue::ArmsMerging { .. } => 3,
        Issue::ConcaveChordCut { .. } => 4,
        Issue::Narrow { .. } => 5,
        Issue::NarrowSf { .. } => 6,
        Issue::NoBraking { .. } => 7,
    }
}

/// The total sort key for `label`: `(rank, payload Point, axis rank, width)`
/// (design.md § Decision 4), rank `8` for [`DispatchLabel::DynamicStall`]
/// (runs last — the most global add, benefiting from every earlier repair).
/// A `DispatchLabel::Issue` payload with no natural `Point`/axis/width
/// (`Disconnected`/`BadTopology`) keys on the zero point — both decline
/// before any arm reads a payload, so their relative order among themselves
/// is immaterial.
pub(crate) const fn issue_sort_key(label: DispatchLabel) -> (u8, Point, u8, u32) {
    match label {
        DispatchLabel::DynamicStall => (8, Point::new(0, 0), 0, 0),
        DispatchLabel::Issue(i) => {
            let rank = issue_rank(i);
            match i {
                Issue::Disconnected | Issue::BadTopology => (rank, Point::new(0, 0), 0, 0),
                Issue::LostHairpin { tip } => (rank, tip, 0, 0),
                Issue::ArmsMerging { bridge } => (rank, bridge, 0, 0),
                Issue::ConcaveChordCut { tooth } => (rank, tooth, 0, 0),
                Issue::NoBraking { at } => (rank, at, 0, 0),
                Issue::Narrow {
                    center,
                    axis,
                    width,
                }
                | Issue::NarrowSf {
                    center,
                    axis,
                    width,
                } => (rank, center, axis_rank(axis), width),
            }
        }
    }
}

/// Dispatches one `label` against the working corridor `working`
/// (design.md § Approach): total over every [`DispatchLabel`] — the two
/// decline labels reach a defined `NotRepairable`, every other label routes
/// to its arm, re-validating its own precondition on `working` first (the
/// "never trust the diagnostic" discipline — a payload an earlier edit
/// already staled declines rather than acting). `ctx` supplies the oracle
/// input (`metrics`/`stall_walls`) the run-out and dynamic arms need;
/// either being `None` declines with `MissingOracleInput`.
pub(crate) fn dispatch(
    ctx: &RepairContext<'_>,
    working: &Corridor,
    label: DispatchLabel,
) -> ArmOutcome {
    match label {
        DispatchLabel::Issue(Issue::Disconnected | Issue::BadTopology) => {
            ArmOutcome::NoEdit(DeclineReason::NotRepairable)
        }
        DispatchLabel::Issue(Issue::LostHairpin { tip }) => {
            crate::phase6_remove::nudge_finger(working, ctx.skel, ctx.k, tip)
        }
        DispatchLabel::Issue(Issue::ArmsMerging { bridge }) => {
            crate::phase6_remove::trim_arm_wall(working, ctx.skel, ctx.k, bridge)
        }
        DispatchLabel::Issue(Issue::ConcaveChordCut { tooth }) => {
            crate::phase6_arms::fill_inner_tooth(working, tooth)
        }
        DispatchLabel::Issue(
            Issue::Narrow { center, axis, .. } | Issue::NarrowSf { center, axis, .. },
        ) => crate::phase6_arms::push_outer_wall_out(working, center, axis),
        DispatchLabel::Issue(Issue::NoBraking { at }) => ctx.metrics.map_or(
            ArmOutcome::NoEdit(DeclineReason::MissingOracleInput),
            |metrics| crate::phase6_arms::run_out_repair(working, metrics, ctx.v_target, at),
        ),
        DispatchLabel::DynamicStall => ctx.stall_walls.map_or(
            ArmOutcome::NoEdit(DeclineReason::MissingOracleInput),
            |walls| dispatch_dynamic_stall(ctx, working, walls),
        ),
    }
}

/// The dynamic (Ф5b stall) arm: maps `walls` to a verified frontier-gap edge
/// via [`map_frontier_gap_to_edge`](crate::phase6::map_frontier_gap_to_edge)
/// (#30, unchanged semantics) and applies it — the mapper already verifies
/// strict `|P0|` growth internally, so this is not re-verified (design.md §
/// The five arms, `map_frontier_gap_to_edge` row).
fn dispatch_dynamic_stall(
    ctx: &RepairContext<'_>,
    working: &Corridor,
    walls: &[Wall],
) -> ArmOutcome {
    match crate::phase6::map_frontier_gap_to_edge(working, ctx.grid, ctx.sf, ctx.race_dir, walls) {
        RepairCandidate::Edge(w) => match crate::phase6_arms::apply_edit(working, w, true) {
            Some((_, cell)) => ArmOutcome::Edit(CommittedEdit {
                arm: RepairArm::MapFrontierGap,
                wall: w,
                cell,
                drivable: true,
                recheck: recheck_scope(RepairArm::MapFrontierGap),
            }),
            None => ArmOutcome::NoEdit(DeclineReason::NoCandidate),
        },
        RepairCandidate::NoCandidate => ArmOutcome::NoEdit(DeclineReason::NoCandidate),
    }
}

/// The single-pass Ф6 local-repair entry point (design doc §2 Ф6; KD2
/// "single pass").
///
/// Builds the dispatch list — every `issues` entry plus one
/// `DispatchLabel::DynamicStall` — sorts it by `issue_sort_key` (fixed
/// severity order, design.md § Decision 4), then walks it once: each
/// `dispatch` call sees the corridor as edited by every prior commit in
/// the pass, and a committed edit is applied to the working corridor before
/// the next label is dispatched. Returns [`RepairOutcome::Failed`] iff
/// **zero** edits were committed across the whole pass (AC4) — never an
/// unchanged `D` reported as success.
#[must_use]
pub fn phase6_local_repair(ctx: &RepairContext<'_>, issues: &[Issue]) -> RepairOutcome {
    let mut labels: Vec<DispatchLabel> = issues.iter().copied().map(DispatchLabel::Issue).collect();
    labels.push(DispatchLabel::DynamicStall);
    labels.sort_by_key(|&l| issue_sort_key(l));

    let mut working = ctx.d.clone();
    let mut edits = Vec::new();
    for label in labels {
        if let ArmOutcome::Edit(edit) = dispatch(ctx, &working, label) {
            working.set(edit.cell, edit.drivable);
            edits.push(edit);
        }
    }

    if edits.is_empty() {
        RepairOutcome::Failed
    } else {
        RepairOutcome::Repaired { d: working, edits }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gp_core::geom::{Orient, Side};

    #[test]
    fn recheck_scope_maps_add_arms_to_local() {
        assert_eq!(
            recheck_scope(RepairArm::PushOuterWallOut),
            RecheckScope::Local
        );
        assert_eq!(
            recheck_scope(RepairArm::FillInnerTooth),
            RecheckScope::Local
        );
        assert_eq!(
            recheck_scope(RepairArm::MapFrontierGap),
            RecheckScope::Local
        );
    }

    #[test]
    fn recheck_scope_maps_run_out_arms_to_sink_to_sink() {
        assert_eq!(
            recheck_scope(RepairArm::LengthenStraight),
            RecheckScope::SinkToSink
        );
        assert_eq!(
            recheck_scope(RepairArm::WidenCorner),
            RecheckScope::SinkToSink
        );
    }

    #[test]
    fn recheck_scope_maps_remove_arms_to_global_flood_fill() {
        assert_eq!(
            recheck_scope(RepairArm::TrimArmWall),
            RecheckScope::GlobalFloodFill
        );
        assert_eq!(
            recheck_scope(RepairArm::NudgeFinger),
            RecheckScope::GlobalFloodFill
        );
    }

    #[test]
    fn committed_edit_carries_the_scope_actually_taken() {
        // AC2: the scope is a field on the output, not inferred from timing.
        let edit = CommittedEdit {
            arm: RepairArm::PushOuterWallOut,
            wall: Wall {
                cell: Point::new(0, 0),
                side: Side::East,
            },
            cell: Point::new(1, 0),
            drivable: true,
            recheck: recheck_scope(RepairArm::PushOuterWallOut),
        };
        assert_eq!(edit.recheck, RecheckScope::Local);
    }

    #[test]
    fn issue_rank_matches_the_pinned_severity_table() {
        assert_eq!(issue_rank(Issue::Disconnected), 0);
        assert_eq!(issue_rank(Issue::BadTopology), 1);
        assert_eq!(
            issue_rank(Issue::LostHairpin {
                tip: Point::new(0, 0)
            }),
            2
        );
        assert_eq!(
            issue_rank(Issue::ArmsMerging {
                bridge: Point::new(0, 0)
            }),
            3
        );
        assert_eq!(
            issue_rank(Issue::ConcaveChordCut {
                tooth: Point::new(0, 0)
            }),
            4
        );
        assert_eq!(
            issue_rank(Issue::Narrow {
                center: Point::new(0, 0),
                axis: Orient::Horizontal,
                width: 1,
            }),
            5
        );
        assert_eq!(
            issue_rank(Issue::NarrowSf {
                center: Point::new(0, 0),
                axis: Orient::Horizontal,
                width: 1,
            }),
            6
        );
        assert_eq!(
            issue_rank(Issue::NoBraking {
                at: Point::new(0, 0)
            }),
            7
        );
    }

    #[test]
    fn issue_sort_key_orders_removes_before_adds_regardless_of_input_order() {
        let mut labels = [
            DispatchLabel::Issue(Issue::NoBraking {
                at: Point::new(0, 0),
            }),
            DispatchLabel::DynamicStall,
            DispatchLabel::Issue(Issue::ArmsMerging {
                bridge: Point::new(5, 5),
            }),
            DispatchLabel::Issue(Issue::Disconnected),
        ];
        labels.sort_by_key(|&l| issue_sort_key(l));
        assert_eq!(labels[0], DispatchLabel::Issue(Issue::Disconnected));
        assert_eq!(
            labels[1],
            DispatchLabel::Issue(Issue::ArmsMerging {
                bridge: Point::new(5, 5)
            })
        );
        assert_eq!(
            labels[2],
            DispatchLabel::Issue(Issue::NoBraking {
                at: Point::new(0, 0)
            })
        );
        assert_eq!(labels[3], DispatchLabel::DynamicStall);
    }

    #[test]
    fn issue_sort_key_breaks_same_rank_ties_by_ascending_payload_point() {
        let a = Issue::ArmsMerging {
            bridge: Point::new(2, 2),
        };
        let b = Issue::ArmsMerging {
            bridge: Point::new(1, 1),
        };
        let mut keys = [
            issue_sort_key(DispatchLabel::Issue(a)),
            issue_sort_key(DispatchLabel::Issue(b)),
        ];
        keys.sort();
        assert_eq!(keys[0].1, Point::new(1, 1));
        assert_eq!(keys[1].1, Point::new(2, 2));
    }

    // ---- dispatch / phase6_local_repair ------------------------------------

    use crate::testfix::{crash_pocket_fixture, ring_corridor, ring_grid, ring_sf};
    use crate::{OracleResult, phase5_full_oracle};
    use gp_core::track::TimingGate;

    fn empty_skel() -> CoarseSkeleton {
        CoarseSkeleton {
            ring: std::collections::BTreeSet::new(),
            hole: std::collections::BTreeSet::new(),
            dir: RaceDir::Cw,
        }
    }

    fn minimal_ctx<'a>(
        d: &'a Corridor,
        skel: &'a CoarseSkeleton,
        grid: &'a StartGrid,
        sf: &'a StartFinish,
    ) -> RepairContext<'a> {
        RepairContext {
            d,
            skel,
            k: 3,
            n: 1,
            m: 1,
            grid,
            sf,
            race_dir: RaceDir::Cw,
            v_target: 1,
            metrics: None,
            stall_walls: None,
        }
    }

    #[test]
    fn dispatch_is_total_over_all_nine_labels_no_panic() {
        let d = ring_corridor();
        let skel = empty_skel();
        let grid = ring_grid();
        let sf = ring_sf();
        let ctx = minimal_ctx(&d, &skel, &grid, &sf);

        let labels = [
            DispatchLabel::Issue(Issue::Disconnected),
            DispatchLabel::Issue(Issue::BadTopology),
            DispatchLabel::Issue(Issue::LostHairpin {
                tip: Point::new(0, 0),
            }),
            DispatchLabel::Issue(Issue::ArmsMerging {
                bridge: Point::new(0, 0),
            }),
            DispatchLabel::Issue(Issue::ConcaveChordCut {
                tooth: Point::new(0, 0),
            }),
            DispatchLabel::Issue(Issue::Narrow {
                center: Point::new(0, 0),
                axis: Orient::Horizontal,
                width: 1,
            }),
            DispatchLabel::Issue(Issue::NarrowSf {
                center: Point::new(0, 0),
                axis: Orient::Horizontal,
                width: 1,
            }),
            DispatchLabel::Issue(Issue::NoBraking {
                at: Point::new(0, 0),
            }),
            DispatchLabel::DynamicStall,
        ];
        for label in labels {
            let _ = dispatch(&ctx, &d, label);
        }
    }

    #[test]
    fn dispatch_declines_disconnected_and_bad_topology_as_not_repairable() {
        let d = ring_corridor();
        let skel = empty_skel();
        let grid = ring_grid();
        let sf = ring_sf();
        let ctx = minimal_ctx(&d, &skel, &grid, &sf);

        assert!(matches!(
            dispatch(&ctx, &d, DispatchLabel::Issue(Issue::Disconnected)),
            ArmOutcome::NoEdit(DeclineReason::NotRepairable)
        ));
        assert!(matches!(
            dispatch(&ctx, &d, DispatchLabel::Issue(Issue::BadTopology)),
            ArmOutcome::NoEdit(DeclineReason::NotRepairable)
        ));
    }

    #[test]
    fn ac4_failed_iff_zero_edits_committed() {
        let d = ring_corridor();
        let skel = empty_skel();
        let grid = ring_grid();
        let sf = ring_sf();
        let ctx = minimal_ctx(&d, &skel, &grid, &sf);

        let issues = [Issue::Disconnected, Issue::BadTopology];
        assert!(matches!(
            phase6_local_repair(&ctx, &issues),
            RepairOutcome::Failed
        ));
    }

    #[test]
    fn ac4_repaired_edits_are_non_empty_and_the_returned_d_reflects_them() {
        // A single ConcaveChordCut tooth: fill_inner_tooth must commit.
        let mut d = Corridor::filled(Point::new(0, 0), 9, 9);
        for x in 4..=6 {
            for y in 4..=6 {
                d.set(Point::new(x, y), false);
            }
        }
        d.set(Point::new(3, 5), false); // degree-1 tooth
        let skel = empty_skel();
        let grid = ring_grid();
        let sf = ring_sf();
        let ctx = minimal_ctx(&d, &skel, &grid, &sf);

        let issues = [Issue::ConcaveChordCut {
            tooth: Point::new(3, 5),
        }];
        let RepairOutcome::Repaired { d: repaired, edits } = phase6_local_repair(&ctx, &issues)
        else {
            panic!("expected Repaired");
        };
        assert!(!edits.is_empty());
        for edit in &edits {
            assert_eq!(repaired.contains(edit.cell), edit.drivable);
            assert_ne!(d.contains(edit.cell), edit.drivable);
        }
    }

    #[test]
    fn ac7_dynamic_arm_wires_map_frontier_gap_to_edge_with_unchanged_semantics() {
        let mut d = ring_corridor();
        d.set(Point::new(4, 2), false);
        let sf = ring_sf();
        let grid = ring_grid();
        let OracleResult::NotLappable { stall_walls } =
            phase5_full_oracle(&d, &grid, &sf, RaceDir::Ccw)
        else {
            panic!("expected NotLappable");
        };
        let skel = empty_skel();
        let ctx = RepairContext {
            d: &d,
            skel: &skel,
            k: 1,
            n: 1,
            m: 1,
            grid: &grid,
            sf: &sf,
            race_dir: RaceDir::Ccw,
            v_target: 1,
            metrics: None,
            stall_walls: Some(&stall_walls),
        };

        let RepairOutcome::Repaired { edits, .. } = phase6_local_repair(&ctx, &[]) else {
            panic!("expected Repaired via the dynamic arm alone");
        };
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].arm, RepairArm::MapFrontierGap);
        assert_eq!(
            edits[0].wall,
            Wall {
                cell: Point::new(4, 1),
                side: Side::North,
            }
        );
    }

    #[test]
    fn ac7_no_candidate_routes_to_no_edit_no_candidate() {
        let (d, sf, grid) = crash_pocket_fixture();
        let OracleResult::NotLappable { stall_walls } =
            phase5_full_oracle(&d, &grid, &sf, RaceDir::Ccw)
        else {
            panic!("expected NotLappable");
        };
        assert!(!stall_walls.is_empty());
        let skel = empty_skel();
        let ctx = RepairContext {
            d: &d,
            skel: &skel,
            k: 1,
            n: 1,
            m: 1,
            grid: &grid,
            sf: &sf,
            race_dir: RaceDir::Ccw,
            v_target: 1,
            metrics: None,
            stall_walls: Some(&stall_walls),
        };
        assert!(matches!(
            dispatch(&ctx, &d, DispatchLabel::DynamicStall),
            ArmOutcome::NoEdit(DeclineReason::NoCandidate)
        ));
    }

    #[test]
    fn severity_order_processes_removes_before_adds_regardless_of_input_order() {
        // One ArmsMerging (remove, rank 3) and one ConcaveChordCut (add,
        // rank 4) sharing ONE bounded complement component (a dumbbell hole
        // y:3..=12) so the global flood-fill recheck (bounded_complement_
        // components == 1) holds throughout -- H (the coarse hole mask,
        // block((1,1),3) = x:3..6,y:3..6) covers only the upper lobe, so
        // ArmsMerging's own scope stays local to that lobe.
        let mut d = Corridor::filled(Point::new(0, 0), 9, 17);
        for x in 3..=5 {
            for y in 3..=12 {
                d.set(Point::new(x, y), false);
            }
        }
        d.set(Point::new(4, 4), true); // ArmsMerging bridge (in H's upper lobe)
        d.set(Point::new(2, 11), false); // ConcaveChordCut tooth, off H, in the lower lobe
        let skel = CoarseSkeleton {
            ring: std::collections::BTreeSet::new(),
            hole: std::collections::BTreeSet::from([Point::new(1, 1)]),
            dir: RaceDir::Cw,
        };
        let grid = StartGrid { positions: vec![] };
        let sf = StartFinish {
            chord: vec![Point::new(0, 0)],
            orient: Orient::Horizontal,
            gate: TimingGate {
                behind: vec![Point::new(0, 0)],
                forward: Side::East,
            },
        };
        let ctx = RepairContext {
            d: &d,
            skel: &skel,
            k: 3,
            n: 1,
            m: 1,
            grid: &grid,
            sf: &sf,
            race_dir: RaceDir::Cw,
            v_target: 1,
            metrics: None,
            stall_walls: None,
        };

        let arms_merging = Issue::ArmsMerging {
            bridge: Point::new(4, 4),
        };
        let concave = Issue::ConcaveChordCut {
            tooth: Point::new(2, 11),
        };

        for issues in [[arms_merging, concave], [concave, arms_merging]] {
            let RepairOutcome::Repaired { edits, .. } = phase6_local_repair(&ctx, &issues) else {
                panic!("expected Repaired");
            };
            assert_eq!(edits.len(), 2, "both issues must commit: {edits:?}");
            assert_eq!(
                edits[0].arm,
                RepairArm::TrimArmWall,
                "remove must be processed before add regardless of input order"
            );
            assert_eq!(edits[1].arm, RepairArm::FillInnerTooth);
        }
    }

    // ---- AC2 consequence discriminators ------------------------------------

    use gp_core::geom::component_count;

    #[test]
    fn ac2_add_edit_commits_despite_disconnection_elsewhere_proving_local_only() {
        // A neck fixture (7x5, pinched single-row neck at x=3) plus a
        // totally separate, disconnected 1-cell blob far away --
        // component_count(d) == 2 *before* any edit. push_outer_wall_out
        // has no global check, so it must still commit on the strictly
        // local width improvement: a global flood-fill recheck would have
        // rejected (component_count != 1), so committing proves none ran.
        let mut d = Corridor::new(Point::new(0, 0), 10, 5);
        for x in 0..7 {
            if x == 3 {
                d.set(Point::new(x, 2), true);
            } else {
                for y in 1..4 {
                    d.set(Point::new(x, y), true);
                }
            }
        }
        d.set(Point::new(9, 0), true); // isolated, disconnected blob
        assert_eq!(component_count(&d), 2, "fixture must start disconnected");

        let ArmOutcome::Edit(edit) =
            crate::phase6_arms::push_outer_wall_out(&d, Point::new(3, 2), Orient::Vertical)
        else {
            panic!("expected an Edit despite the disconnection elsewhere");
        };

        // The edit is genuine (AC1) and the corridor remains disconnected
        // after applying it -- direct evidence the arm never touched
        // global connectivity.
        let (scratch, _) = crate::phase6_arms::apply_edit(&d, edit.wall, true).unwrap();
        assert_eq!(component_count(&scratch), 2);
    }

    #[test]
    fn ac2_remove_edit_that_disconnects_d_is_rejected_proving_global_flood_fill_ran() {
        // Two rooms joined only by a 3-cell drivable row through H: every
        // candidate cell's local metric (|H ∩ D| strictly decreases) holds
        // trivially -- removing any single cell always drops the set's
        // size by exactly one -- yet every candidate disconnects D. The
        // arm must reject all of them; a purely-local check would have
        // committed on the first candidate's trivial |H ∩ D| decrease, so
        // rejection proves the global flood-fill recheck ran.
        let mut d = Corridor::new(Point::new(0, 0), 9, 9);
        for y in 0..9 {
            for x in 0..3 {
                d.set(Point::new(x, y), true);
            }
            for x in 6..9 {
                d.set(Point::new(x, y), true);
            }
        }
        for x in 3..=5 {
            d.set(Point::new(x, 4), true);
        }
        let skel = CoarseSkeleton {
            ring: std::collections::BTreeSet::new(),
            hole: std::collections::BTreeSet::from([Point::new(1, 1)]),
            dir: gp_core::track::RaceDir::Cw,
        };

        assert!(matches!(
            crate::phase6_remove::trim_arm_wall(&d, &skel, 3, Point::new(3, 4)),
            ArmOutcome::NoEdit(DeclineReason::RecheckFailed)
        ));
    }

    // ---- AC9 totality / determinism ----------------------------------------

    fn minimal_sf() -> StartFinish {
        StartFinish {
            chord: vec![Point::new(0, 0)],
            orient: Orient::Horizontal,
            gate: TimingGate {
                behind: vec![Point::new(0, 0)],
                forward: Side::East,
            },
        }
    }

    #[test]
    fn ac9_is_total_on_adversarial_inputs_no_panic() {
        let sf = minimal_sf();
        let grid = StartGrid { positions: vec![] };
        let skel = empty_skel();

        // Empty issue list, no oracle input.
        let d = Corridor::new(Point::new(0, 0), 3, 3);
        let ctx = minimal_ctx(&d, &skel, &grid, &sf);
        assert!(matches!(
            phase6_local_repair(&ctx, &[]),
            RepairOutcome::Failed
        ));

        // Out-of-box / overflow-adjacent stall walls.
        let adversarial = vec![
            Wall {
                cell: Point::new(9999, 9999),
                side: Side::East,
            },
            Wall {
                cell: Point::new(i32::MAX, i32::MAX),
                side: Side::East,
            },
        ];
        let ctx_walls = RepairContext {
            stall_walls: Some(&adversarial),
            ..minimal_ctx(&d, &skel, &grid, &sf)
        };
        assert!(matches!(
            phase6_local_repair(&ctx_walls, &[]),
            RepairOutcome::Failed
        ));

        // A degenerate zero-area corridor, plus the two decline-only
        // labels.
        let empty_d = Corridor::new(Point::new(0, 0), 0, 0);
        let ctx_empty = minimal_ctx(&empty_d, &skel, &grid, &sf);
        let issues = [Issue::Disconnected, Issue::BadTopology];
        assert!(matches!(
            phase6_local_repair(&ctx_empty, &issues),
            RepairOutcome::Failed
        ));
    }

    #[test]
    fn ac9_repeated_and_shuffled_calls_yield_identical_outcome() {
        let mut d = Corridor::filled(Point::new(0, 0), 9, 17);
        for x in 3..=5 {
            for y in 3..=12 {
                d.set(Point::new(x, y), false);
            }
        }
        d.set(Point::new(4, 4), true);
        d.set(Point::new(2, 11), false);
        let skel = CoarseSkeleton {
            ring: std::collections::BTreeSet::new(),
            hole: std::collections::BTreeSet::from([Point::new(1, 1)]),
            dir: gp_core::track::RaceDir::Cw,
        };
        let grid = StartGrid { positions: vec![] };
        let sf = minimal_sf();
        let ctx = minimal_ctx(&d, &skel, &grid, &sf);

        let a = Issue::ArmsMerging {
            bridge: Point::new(4, 4),
        };
        let b = Issue::ConcaveChordCut {
            tooth: Point::new(2, 11),
        };

        let r1 = phase6_local_repair(&ctx, &[a, b]);
        let r2 = phase6_local_repair(&ctx, &[a, b]);
        let r3 = phase6_local_repair(&ctx, &[b, a]);

        // `Corridor`/`RepairOutcome` carry no `PartialEq` (design.md §
        // Ф6's signature — `Corridor` doesn't implement it), so equality
        // is compared via `Debug`, which both derive.
        let fmt = |r: &RepairOutcome| format!("{r:?}");
        assert_eq!(fmt(&r1), fmt(&r2), "repeated calls must agree");
        assert_eq!(
            fmt(&r1),
            fmt(&r3),
            "a shuffled issue list must yield the same outcome"
        );
    }
}

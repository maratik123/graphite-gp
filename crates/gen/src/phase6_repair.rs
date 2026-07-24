//! Ф6 — the single-pass local-repair driver: public types, the crate-local
//! dispatch vocabulary, `recheck_scope`, and the severity ordering
//! (`issue_rank`/`issue_sort_key`) (design doc §2 Ф6, `[C3]`;
//! `ai-docs/plans/2026-07-24-gp-gen-phase6-local-repair.design.md` § Approach,
//! § Decision 4).
//!
//! The dispatch driver itself (`dispatch`, `phase6_local_repair`) is built in
//! `Group B` subtasks 9-13; this module carries the shapes every arm and the
//! driver agree on, plus the two pure crate-local helpers (`recheck_scope`,
//! `issue_rank`/`issue_sort_key`) that do not depend on any arm body.
#![allow(
    dead_code,
    reason = "the pub(crate) dispatch vocabulary (ArmOutcome, DeclineReason, DispatchLabel) \
              and helpers (recheck_scope, issue_rank, issue_sort_key) have no production caller \
              until subtask 13's dispatch driver wires them in — every item here is already \
              exercised by this module's own tests"
)]

use gp_core::geom::{Corridor, Point, Wall};
use gp_core::track::{RaceDir, StartFinish, StartGrid, TrackMetrics};

use crate::CoarseSkeleton;
use crate::Issue;

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
}

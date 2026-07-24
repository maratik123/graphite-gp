// `phase7.rs`'s `#[cfg(test)] mod tests` body, split into a sibling file
// (GO-note 3, file-size soft cap) and pulled back in via `include!` so the
// tests keep full access to `phase7`'s private items (`use super::*`) — see
// `phase7.rs`'s `mod tests { include!("phase7_tests.rs"); }`.

use super::*;
use gp_core::geom::Side;
use gp_core::track::TimingGate;

/// Subtask 1: an empty corridor (no drivable cells) has an empty medial
/// axis, so `racing_line` falls back to `Centerline::default()` — empty
/// samples, zero length, and `at` returning `None`.
#[test]
fn empty_corridor_yields_default_centerline() {
    let d = Corridor::new(Point::new(0, 0), 4, 4);
    let gate = TimingGate {
        behind: vec![],
        forward: Side::East,
    };
    let cl = racing_line(&d, &gate, RaceDir::Ccw);
    assert!(cl.samples.is_empty());
    assert!(cl.length.abs() < f32::EPSILON);
    assert!(cl.at(0.0).is_none());
}

/// Subtask 2 (happy), rebuilt for Ф7 follow-through (design doc § Ф7
/// follow-through, subtask 7): the real `annulus_corridor` medial axis is now
/// one connected loop straight out of `medial_axis` (DT-ordered anchored
/// thinning), so this test instead hand-builds the OLD 4 corner-gapped
/// strips (the shape the previous strict-local-max `medial_axis` used to
/// return on this corridor) to keep exercising `bridge_gaps`'s
/// cross-component-corner-gap bridging directly.
#[test]
fn bridge_gaps_joins_annulus_corner_gaps_into_one_component() {
    let d = crate::testfix::annulus_corridor();
    let mut medial = BTreeSet::new();
    for x in 3..8 {
        medial.insert(Point::new(x, 1));
        medial.insert(Point::new(x, 9));
    }
    for y in 3..8 {
        medial.insert(Point::new(1, y));
        medial.insert(Point::new(9, y));
    }
    assert!(
        components(&medial).len() > 1,
        "the hand-built 4-strip set starts as >1 disjoint strip"
    );

    let bridged = bridge_gaps(&d, medial).expect("annulus corner gaps are bridgeable");
    assert_eq!(
        components(&bridged).len(),
        1,
        "bridging must join all 4 strips into one component"
    );
}

/// Subtask 2 (edge): two components a `MAX_BRIDGE_GAP`-exceeding Manhattan
/// distance apart abandon bridging (fallback signal).
#[test]
fn bridge_gaps_abandons_over_max_gap() {
    let d = crate::testfix::corridor((0, 0), 1, 40, &[(0, 0), (0, 39)]);
    let medial = BTreeSet::from([Point::new(0, 0), Point::new(0, 39)]);
    assert!(bridge_gaps(&d, medial).is_none());
}

/// Subtask 2 (edge): two components within `MAX_BRIDGE_GAP` bridge into
/// one component, using only cells that lie in `d`.
#[test]
fn bridge_gaps_joins_a_close_gap() {
    let d = crate::testfix::corridor((0, 0), 1, 5, &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)]);
    let medial = BTreeSet::from([
        Point::new(0, 0),
        Point::new(0, 1),
        Point::new(0, 3),
        Point::new(0, 4),
    ]);
    let bridged = bridge_gaps(&d, medial).expect("a 2-cell gap is within MAX_BRIDGE_GAP");
    assert_eq!(components(&bridged).len(), 1);
    assert!(bridged.contains(&Point::new(0, 2)));
}

/// Subtask 2: an empty medial set is not bridgeable (fallback signal).
#[test]
fn bridge_gaps_rejects_empty_medial() {
    let d = Corridor::new(Point::new(0, 0), 4, 4);
    assert!(bridge_gaps(&d, BTreeSet::new()).is_none());
}

/// A hand-built 4×4 square ring (border of a 4×4 box), 4-connected and
/// already all-degree-2 — the "clean loop" prune fixture.
fn small_ring() -> BTreeSet<Point> {
    BTreeSet::from([
        Point::new(0, 0),
        Point::new(1, 0),
        Point::new(2, 0),
        Point::new(3, 0),
        Point::new(3, 1),
        Point::new(3, 2),
        Point::new(3, 3),
        Point::new(2, 3),
        Point::new(1, 3),
        Point::new(0, 3),
        Point::new(0, 2),
        Point::new(0, 1),
    ])
}

/// Subtask 3 (happy): an all-degree-2 loop is unaffected by pruning.
#[test]
fn prune_spurs_is_a_no_op_on_a_clean_loop() {
    let ring = small_ring();
    assert_eq!(prune_spurs(&ring), Some(ring));
}

/// Subtask 3 (happy): a 2-cell dead-end finger hanging off ring cell
/// `(1, 0)` (poking outward, below the ring's own box) is fully peeled
/// away, leaving exactly the ring, all-degree-2.
#[test]
fn prune_spurs_removes_a_dangling_finger() {
    let mut with_finger = small_ring();
    with_finger.insert(Point::new(1, -1)); // attaches to ring's (1, 0)
    with_finger.insert(Point::new(1, -2)); // the dead-end tip

    let core = prune_spurs(&with_finger).expect("the ring survives pruning");
    assert_eq!(core, small_ring());
    for &p in &core {
        assert!(degree(&core, p) >= 2, "{p:?} must have degree >= 2");
    }
}

/// Subtask 3 (edge): a pure tree (no cycle) prunes to nothing (fallback
/// signal).
#[test]
fn prune_spurs_rejects_a_pure_tree() {
    let tree = BTreeSet::from([Point::new(0, 0), Point::new(1, 0), Point::new(2, 0)]);
    assert!(prune_spurs(&tree).is_none());
}

/// A gate whose `behind`/`forward_face` anchor sits just outside
/// `small_ring`'s bottom edge, near `(1, 0)`/`(2, 0)`.
fn small_ring_gate() -> TimingGate {
    TimingGate {
        behind: vec![Point::new(1, -1), Point::new(2, -1)],
        forward: Side::North,
    }
}

/// Subtask 4 (happy): the ring core walks into an ordered cycle that
/// covers every ring cell and closes back to its own start.
#[test]
fn walk_cycle_covers_and_closes_a_clean_ring() {
    let ring = small_ring();
    let order = walk_cycle(&ring, &small_ring_gate()).expect("a clean ring must close");
    assert_eq!(order.len(), ring.len());
    assert_eq!(order.iter().copied().collect::<BTreeSet<_>>(), ring);
    let start = order[0];
    assert!(start.neighbors4().contains(order.last().unwrap()));
}

/// Subtask 4 (thinning): a 2-cell-wide band traces a single strand — the
/// walk never visits both rails of the band.
#[test]
fn walk_cycle_thins_a_two_cell_band_to_one_strand() {
    // A closed loop with an even-width (2-cell) top/bottom band: rows
    // y=0 and y=3 are single-wide; the "band" is the two parallel side
    // columns x=0..=1 (west) and x=4..=5 (east), each 2 cells wide across
    // rows y=1..=2 — mimicking medial_axis_even_width_band_is_two_cell's
    // documented 2-cell ridge.
    let mut band = BTreeSet::new();
    for x in 0..6 {
        band.insert(Point::new(x, 0));
        band.insert(Point::new(x, 3));
    }
    for y in 1..3 {
        for x in [0, 1, 4, 5] {
            band.insert(Point::new(x, y));
        }
    }
    let gate = TimingGate {
        behind: vec![Point::new(2, -1)],
        forward: Side::North,
    };
    let order = walk_cycle(&band, &gate).expect("a thinnable band must close");
    // Every visited cell distinct (a simple cycle, not a doubled-back walk).
    assert_eq!(
        order.len(),
        order.iter().collect::<BTreeSet<_>>().len(),
        "walk must not revisit a cell"
    );
    // The walk never uses both rails of a side band in the same row: for
    // each side, at most one of the two columns appears per row.
    for y in 1..3 {
        let west = [Point::new(0, y), Point::new(1, y)]
            .iter()
            .filter(|p| order.contains(p))
            .count();
        let east = [Point::new(4, y), Point::new(5, y)]
            .iter()
            .filter(|p| order.contains(p))
            .count();
        assert!(west <= 1, "row {y} west rail must be single-strand");
        assert!(east <= 1, "row {y} east rail must be single-strand");
    }
}

/// Subtask 4 (edge): a broken (open) core — a straight line, not a ring —
/// dead-ends and never returns to `start` (fallback signal).
#[test]
fn walk_cycle_rejects_an_open_core() {
    let open: BTreeSet<Point> = (0..5).map(|x| Point::new(x, 0)).collect();
    let gate = TimingGate {
        behind: vec![Point::new(2, -1)],
        forward: Side::North,
    };
    assert!(walk_cycle(&open, &gate).is_none());
}

/// Subtask 5 (GO-note 2): the CCW unit square's integer shoelace sums to
/// `+2` — pinning the sign convention this grid's x-east/y-north
/// handedness implies.
#[test]
fn shoelace_ccw_unit_square_is_positive_two() {
    let square = vec![
        Point::new(0, 0),
        Point::new(1, 0),
        Point::new(1, 1),
        Point::new(0, 1),
    ];
    assert_eq!(shoelace(&square), 2);
}

/// Subtask 5 (GO-note 2): the reversed (CW) square sums to `-2` — pinning
/// the mapping in both directions, not merely "the two are reversed".
#[test]
fn shoelace_cw_unit_square_is_negative_two() {
    let mut square = vec![
        Point::new(0, 0),
        Point::new(1, 0),
        Point::new(1, 1),
        Point::new(0, 1),
    ];
    square.reverse();
    assert_eq!(shoelace(&square), -2);
}

/// Subtask 5: `orient` reverses (or not) to make the shoelace sign match
/// `race_dir` (`Ccw` ⇔ `> 0`, `Cw` ⇔ `< 0`).
#[test]
fn orient_matches_race_dir_sign() {
    let ccw_square = vec![
        Point::new(0, 0),
        Point::new(1, 0),
        Point::new(1, 1),
        Point::new(0, 1),
    ];
    assert!(shoelace(&orient(ccw_square.clone(), RaceDir::Ccw)) > 0);
    assert!(shoelace(&orient(ccw_square, RaceDir::Cw)) < 0);
}

/// The `small_ring` fixture (subtask 3) as a real `Corridor` — a clean,
/// already-width-1 4×4 border loop, so its true `medial_axis` is exactly
/// `small_ring` itself (no bridging/pruning needed).
fn small_ring_corridor() -> Corridor {
    let cells: Vec<(i32, i32)> = small_ring().into_iter().map(|p| (p.x, p.y)).collect();
    crate::testfix::corridor((0, 0), 4, 4, &cells)
}

/// The float shoelace signed area of `cl`'s resampled sample polygon —
/// mirrors [`shoelace`] but over the `f32` sample positions, to check
/// that orientation survives resampling.
fn sample_signed_area(cl: &Centerline) -> f64 {
    cl.samples
        .iter()
        .zip(cl.samples.iter().skip(1).chain(cl.samples.iter().take(1)))
        .map(|(a, b)| {
            f64::from(b.pos.0).mul_add(-f64::from(a.pos.1), f64::from(a.pos.0) * f64::from(b.pos.1))
        })
        .sum()
}

/// Subtask 5 (end-to-end): `racing_line` on a clean ring produces
/// `samples[0].s == 0`, strictly increasing `s` at ~`RESAMPLE_STEP`
/// spacing, unit tangents, and an overall sample-polygon orientation
/// matching `race_dir`.
#[test]
fn racing_line_orients_resamples_and_tangents_a_clean_ring() {
    let d = small_ring_corridor();
    let gate = small_ring_gate();

    for race_dir in [RaceDir::Ccw, RaceDir::Cw] {
        let cl = racing_line(&d, &gate, race_dir);
        assert!(!cl.samples.is_empty(), "{race_dir:?} must produce samples");
        assert!(cl.samples[0].s.abs() < f32::EPSILON);
        for w in cl.samples.windows(2) {
            assert!(w[1].s > w[0].s, "s must be strictly increasing");
            assert!(
                (w[1].s - w[0].s - RESAMPLE_STEP).abs() < 0.5,
                "spacing must be close to RESAMPLE_STEP"
            );
        }
        for sample in &cl.samples {
            let mag = sample.tangent.0.hypot(sample.tangent.1);
            assert!((mag - 1.0).abs() < 1e-4, "tangent must be unit-length");
        }

        let area = sample_signed_area(&cl);
        match race_dir {
            RaceDir::Ccw => assert!(area > 0.0, "Ccw sample polygon must wind positive"),
            RaceDir::Cw => assert!(area < 0.0, "Cw sample polygon must wind negative"),
        }
    }
}

/// Recursively collects every `.rs` file under `dir`.
fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// AC5: no `gp-ai` source file references the (render-only) `Centerline`
/// type or the `.centerline` field — a case-sensitive source scan, so the
/// doc-comment prose's lowercase "centerline" mentions don't false-positive
/// (design doc § "AC5 enforcement mechanism"). Lives in `gp-gen`
/// (Miri-`--exclude`d, `#[134]`), so this file-reading test needs no
/// `#[cfg_attr(miri, ignore)]`.
#[test]
fn ac5_gp_ai_never_references_centerline() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../ai/src"));
    let mut files = Vec::new();
    collect_rs_files(root, &mut files);
    assert!(!files.is_empty(), "expected to find gp-ai source files");

    let offenders: Vec<String> = files
        .into_iter()
        .filter_map(|path| {
            let contents = std::fs::read_to_string(&path).ok()?;
            (contents.contains("Centerline") || contents.contains(".centerline"))
                .then(|| path.display().to_string())
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "gp-ai must not reference Centerline/.centerline: {offenders:?}"
    );
}

/// Subtask 7 (AC7, primary fixture): the odd-thickness annulus ring's
/// centerline closes, `s` is monotone and ~evenly spaced, and tangents are
/// unit-length and `race_dir`-aligned (overall winding sign positive for
/// `Ccw`).
#[test]
fn ac7_annulus_closes_monotone_and_race_dir_aligned() {
    let d = crate::testfix::annulus_corridor();
    let gate = crate::testfix::annulus_gate();
    let cl = racing_line(&d, &gate, RaceDir::Ccw);

    assert!(
        !cl.samples.is_empty(),
        "the annulus must produce a closed loop"
    );
    assert!(cl.samples[0].s.abs() < f32::EPSILON);
    for w in cl.samples.windows(2) {
        assert!(w[1].s > w[0].s, "s must be strictly increasing");
        assert!(
            (w[1].s - w[0].s - RESAMPLE_STEP).abs() < 0.5,
            "spacing must be close to RESAMPLE_STEP"
        );
    }
    for sample in &cl.samples {
        let mag = sample.tangent.0.hypot(sample.tangent.1);
        assert!((mag - 1.0).abs() < 1e-4, "tangent must be unit-length");
    }
    assert!(
        sample_signed_area(&cl) > 0.0,
        "Ccw sample polygon must wind positive"
    );
}

/// Subtask 7 (AC1): the `trap_ring` fixture (a clean border ring plus a
/// dangling interior spur) trims to a single non-branching loop — no sample
/// sits on the pruned spur's cells (`x == 6`, `y ∈ 1..=5`).
#[test]
fn ac1_prunes_spur_to_a_single_non_branching_loop() {
    let (d, sf, _grid) = crate::testfix::trap_ring();
    let gate = sf.gate;
    let cl = racing_line(&d, &gate, RaceDir::Ccw);

    assert!(
        !cl.samples.is_empty(),
        "the ring must still close despite the spur"
    );
    for sample in &cl.samples {
        let (x, y) = sample.pos;
        let on_spur = (x - 6.0).abs() < 0.5 && (1.0..=5.0).contains(&y);
        assert!(!on_spur, "sample at ({x}, {y}) sits on the pruned spur");
    }
}

/// Subtask 7 (AC2): the closed loop wraps — `at(length) ≡ at(0)` and
/// `at(length + x) ≡ at(x)`.
#[test]
fn ac2_wraps_around_the_closed_loop() {
    let d = small_ring_corridor();
    let gate = small_ring_gate();
    let cl = racing_line(&d, &gate, RaceDir::Ccw);

    let at0 = cl.at(0.0).expect("must have samples");
    let at_len = cl.at(cl.length).expect("must wrap at length");
    assert!((at0.pos.0 - at_len.pos.0).abs() < 1e-3);
    assert!((at0.pos.1 - at_len.pos.1).abs() < 1e-3);

    let x = 1.5;
    let a = cl.at(x).expect("must sample at x");
    let b = cl.at(cl.length + x).expect("must sample at length + x");
    assert!((a.pos.0 - b.pos.0).abs() < 1e-3);
    assert!((a.pos.1 - b.pos.1).abs() < 1e-3);
}

/// Whether `a` and `b` are field-by-field bit-identical (AC6).
#[allow(
    clippy::float_cmp,
    reason = "AC6 explicitly requires byte-identical repeats of a fully \
              deterministic pipeline (medial_axis/bridge/prune/walk are all \
              integer-only; resample/tangent math runs the same fixed \
              arithmetic on the same inputs both times) - an epsilon would \
              mask a real determinism regression, which is exactly what this \
              test exists to catch"
)]
fn centerlines_match_exactly(a: &Centerline, b: &Centerline) -> bool {
    a.length == b.length
        && a.samples.len() == b.samples.len()
        && a.samples
            .iter()
            .zip(b.samples.iter())
            .all(|(sa, sb)| sa.s == sb.s && sa.pos == sb.pos && sa.tangent == sb.tangent)
}

/// Subtask 7 (AC6): repeated `racing_line` runs on the same `(d, gate,
/// race_dir)` produce a field-by-field identical `Centerline`.
#[test]
fn ac6_racing_line_is_deterministic() {
    let d = crate::testfix::annulus_corridor();
    let gate = crate::testfix::annulus_gate();
    let cl1 = racing_line(&d, &gate, RaceDir::Ccw);
    let cl2 = racing_line(&d, &gate, RaceDir::Ccw);
    assert!(!cl1.samples.is_empty(), "must produce a real loop");
    assert!(centerlines_match_exactly(&cl1, &cl2));
}

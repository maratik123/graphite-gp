//! The exported track artifact (design doc §2, Ф7) — the contract produced by
//! block 1 (generation) and consumed by blocks 2 (render), 3a (physics) and
//! 4 (AI).

use crate::geom::{Corridor, Orient, Point, Rect, Side, Wall, barrier_distance_field};

/// Global traversal orientation of the ring, fixed during generation (design
/// doc §2, Ф1). Everything downstream — the lap counter, AI progress/reward,
/// the ideal line — is oriented by this.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RaceDir {
    /// Clockwise.
    Cw,
    /// Counter-clockwise.
    Ccw,
}

/// The exact timing-gate segment used for [`StartFinish`]'s signed-crossing
/// test (design doc §3, \[C2\]).
///
/// `behind` holds the *drivable* cross-section cells immediately behind the
/// gate; each implied dual edge is `{cell: behind[i], side: forward}` — one
/// edge ahead of the front row, spanning the cross-section. This is **not** a
/// [`Wall`] set: a gate edge sits between two *drivable* cells (`D`'s
/// interior), not on `D`'s `¬D` boundary — a future reader of
/// `LapCounter::register_move` must not conflate the two.
#[derive(Clone, Debug)]
pub struct TimingGate {
    /// The drivable cross-section cells immediately behind the gate.
    pub behind: Vec<Point>,
    /// The forward (`+race_dir`) side each `behind` cell's implied dual edge
    /// faces.
    pub forward: Side,
}

impl TimingGate {
    /// The unit `(f32, f32)` direction of `forward`.
    #[inline]
    pub const fn forward_unit(&self) -> (f32, f32) {
        side_unit_f32(self.forward)
    }

    /// Whether the dual edge between `a` and `b` is one of this gate's implied
    /// cut edges.
    ///
    /// Order-independent / symmetric: `separates(a, b) == separates(b, a)` for
    /// all `a`, `b` — a reversed-pair query (as a 4-neighbor gradient walk may
    /// issue) must report the same cut, or the barrier silently leaks the
    /// cross-cut jump into the gradient for one traversal direction.
    pub fn separates(&self, a: Point, b: Point) -> bool {
        let (dx, dy) = self.forward.delta();
        let ahead_of = |p: Point| -> Option<Point> {
            Some(Point::new(p.x.checked_add(dx)?, p.y.checked_add(dy)?))
        };
        self.behind.iter().any(|&behind| {
            ahead_of(behind)
                .is_some_and(|ahead| (behind == a && ahead == b) || (behind == b && ahead == a))
        })
    }

    /// The forward face — `{ behind[i] + forward.delta() }` — the drivable
    /// cells one step ahead of the gate's `behind` cross-section, on the
    /// `+race_dir` side.
    ///
    /// These are the BFS seed cells for [`SField::from_gate_bfs`]: distance
    /// `0` here, growing the long way around the loop to reach its maximum at
    /// `behind`. Overflow-filtered exactly as [`separates`](Self::separates)'s
    /// `ahead_of` — a `behind` cell whose `+forward` step would overflow `i32`
    /// contributes no seed rather than panicking.
    pub fn forward_face(&self) -> impl Iterator<Item = Point> + '_ {
        let (dx, dy) = self.forward.delta();
        self.behind
            .iter()
            .filter_map(move |p| Some(Point::new(p.x.checked_add(dx)?, p.y.checked_add(dy)?)))
    }
}

/// The unit `(f32, f32)` direction of `side`, via a literal `match` — no
/// numeric cast.
const fn side_unit_f32(side: Side) -> (f32, f32) {
    match side {
        Side::East => (1.0, 0.0),
        Side::West => (-1.0, 0.0),
        Side::North => (0.0, 1.0),
        Side::South => (0.0, -1.0),
    }
}

/// The start/finish line — a full chord cutting the annulus into a simply
/// connected strip (design doc §3, lap counter).
///
/// Being a *full* chord is what makes the signed-crossing lap counter provably
/// sufficient. `gate` carries the exact timing-gate segment(s) and forward
/// direction that `LapCounter::register_move`'s half-open signed-crossing test
/// needs.
#[derive(Clone, Debug)]
pub struct StartFinish {
    /// The drivable points forming the chord across the corridor.
    pub chord: Vec<Point>,
    /// Chord orientation across the corridor (H or V).
    pub orient: Orient,
    /// The exact timing-gate segment(s) used for the signed-crossing test.
    pub gate: TimingGate,
}

impl StartFinish {
    /// Start/finish width in lattice points — the chord length across the
    /// corridor.
    #[inline]
    pub const fn width(&self) -> usize {
        self.chord.len()
    }
}

/// An ordered list of distinct start positions in the corridor `D`, each
/// implicitly at rest (`v = (0, 0)`).
///
/// Invariant (upheld by the generator, not enforced here): every `positions`
/// element is distinct, lies in `D`, and the list is ordered front-to-back
/// along `−race_dir` (design doc §2, Ф3, \[C2\]).
#[derive(Clone, Debug, Default)]
pub struct StartGrid {
    /// Distinct start positions, ordered front-to-back along `−race_dir`.
    pub positions: Vec<Point>,
}

/// Fallback unit tangent for a degenerate (flat/saddle) gradient — shaping-only
/// (design doc §2, P1); every band cell still gets a defined tangent.
const FLAT_FALLBACK: (f32, f32) = (1.0, 0.0);

/// The monotone integer distance field over the corridor `D \ gate`, seeded
/// from the gate's forward face (design doc §2, N1), plus gradient/tangent
/// accessors (design doc §2, P2/M1).
///
/// Dense grid mirroring [`Corridor`]'s bounding-box storage: row-major over
/// `rect`, `None` = not in the band (`¬D`).
#[derive(Clone, Debug, Default)]
pub struct SField {
    /// The bounding box `dist` is indexed over.
    pub rect: Rect,
    /// Row-major (`y`-outer, `x`-inner) distance per cell; `None` = not in the
    /// band.
    pub dist: Vec<Option<u32>>,
}

impl SField {
    /// Builds a field over `rect`, filling each cell's distance via
    /// `dist_at` — a caller-supplied per-cell lookup (the generator's BFS
    /// result later; a hand-filled closure in tests today). No BFS is run
    /// here.
    pub fn new(rect: Rect, dist_at: impl Fn(Point) -> Option<u32>) -> Self {
        let dist = rect.points().map(dist_at).collect();
        Self { rect, dist }
    }

    /// The real BFS producer (design doc §2, N1 / P1 / D2): the 4-connected
    /// distance field over `d \ gate`, seeded (distance `0`) from
    /// `gate.forward_face()`, with `gate.separates` as the impassable barrier
    /// in both directions.
    ///
    /// `rect` mirrors `d`'s own bounding box (via [`Corridor::rect`]), so
    /// `dist.len() == rect.area()` holds by construction — the invariant
    /// [`scalar_at`](Self::scalar_at) / [`gradient_at`](Self::gradient_at)
    /// rely on. Every in-`D` cell is `Some(distance)`; every `¬D` cell, and
    /// any in-`D` cell the cut leaves unreachable from the forward face, is
    /// `None`. Deterministic: identical `(d, gate)` inputs yield an identical
    /// field.
    pub fn from_gate_bfs(d: &Corridor, gate: &TimingGate) -> Self {
        let dist = barrier_distance_field(d, gate.forward_face(), |a, b| gate.separates(a, b));
        Self {
            rect: d.rect(),
            dist,
        }
    }

    /// The scalar distance `s` at `p`, or `None` if `p` is outside the band.
    ///
    /// Total: never panics, even if `dist` is shorter than `rect.area()` (a
    /// hand-built `SField` violating the `dist.len() == rect.area()`
    /// invariant `new` upholds) — an out-of-range index yields `None` rather
    /// than indexing off the end.
    pub fn scalar_at(&self, p: Point) -> Option<u32> {
        self.rect
            .index(p)
            .and_then(|i| self.dist.get(i).copied().flatten())
    }

    /// Whether `p` sits immediately against `gate`'s cut (a gate cell).
    fn is_gate_cell(gate: &TimingGate, p: Point) -> bool {
        p.neighbors4().into_iter().any(|q| gate.separates(p, q))
    }

    /// The raw integer gradient `∇s` at `p`: the sum, over in-band 4-neighbors
    /// not separated from `p` by `gate`'s cut, of `(s(q) − s(p))·(q − p)`.
    ///
    /// Unifies central difference (both axis neighbors in-band) and one-sided
    /// difference (one neighbor out of band) without special-casing; `gate`'s
    /// cut is skipped so the field is never differenced across it. `None` if
    /// `p` is outside the band.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "each neighbor contributes a bounded (s(q) - s(p)) * (q - p) \
                  term: the u32 distances are converted through \
                  i32::try_from (saturating to i32::MAX), the axis delta is \
                  always -1/0/1, and at most 4 neighbors are summed — the \
                  accumulation stays well within i32 range for any \
                  grid-realistic s-field"
    )]
    pub fn gradient_at(&self, gate: &TimingGate, p: Point) -> Option<(i32, i32)> {
        let sp = i32::try_from(self.scalar_at(p)?).unwrap_or(i32::MAX);
        let mut grad = (0, 0);
        for q in p.neighbors4() {
            if gate.separates(p, q) {
                continue;
            }
            let Some(sq) = self.scalar_at(q) else {
                continue;
            };
            let sq = i32::try_from(sq).unwrap_or(i32::MAX);
            let d = sq - sp;
            grad.0 += d * (q.x - p.x);
            grad.1 += d * (q.y - p.y);
        }
        Some(grad)
    }

    /// The unit tangent at `p`.
    ///
    /// On a gate cell, exactly `gate.forward_unit()` — the cut is not
    /// differenced across (design doc §2, P2, AC3). Otherwise the normalized
    /// gradient, or the flat fallback `(1.0, 0.0)` when the gradient is
    /// degenerate (`(0, 0)`). `None` if `p` is outside the band.
    pub fn tangent_at(&self, gate: &TimingGate, p: Point) -> Option<(f32, f32)> {
        self.scalar_at(p)?;
        if Self::is_gate_cell(gate, p) {
            return Some(gate.forward_unit());
        }
        let grad = self.gradient_at(gate, p)?;
        Some(if grad == (0, 0) {
            FLAT_FALLBACK
        } else {
            normalize(grad)
        })
    }
}

/// Normalizes a nonzero integer gradient to a unit `(f32, f32)`.
///
/// Precondition: `g != (0, 0)` — both callers (a gate cell, and the degenerate
/// `(0, 0)` gradient case) return before reaching this, so the division is
/// never by zero.
#[allow(
    clippy::cast_precision_loss,
    reason = "gradient components are small (bounded BFS-distance \
              differences) integers, exactly representable in f32"
)]
fn normalize(g: (i32, i32)) -> (f32, f32) {
    let (gx, gy) = (g.0 as f32, g.1 as f32);
    let len = gx.hypot(gy);
    (gx / len, gy / len)
}

/// One sample of the parameterized centerline.
#[derive(Clone, Copy, Debug)]
pub struct CenterlineSample {
    /// Arc length from the start of the loop.
    pub s: f32,
    /// Sub-cell position of the centerline at `s`.
    pub pos: (f32, f32),
    /// Unit tangent, pointing along `race_dir`.
    pub tangent: (f32, f32),
}

/// The parameterized centerline (design doc §2) — a *first-class product* of
/// generation, not an internal Ф1 detail, because AI progress/reward and the
/// renderer's ideal line both depend on it.
///
/// `s` = distance along the track, grows along `race_dir`, and closes on itself.
#[derive(Clone, Debug, Default)]
pub struct Centerline {
    /// Ordered samples along the closed loop.
    pub samples: Vec<CenterlineSample>,
    /// Total loop length (used to normalize progress → track-invariance).
    pub length: f32,
}

impl Centerline {
    /// Samples the centerline at arc length `s`, wrapping around the closed
    /// loop (`at(length) ≡ at(0)`, `at(length + x) ≡ at(x)`, and a negative `s`
    /// wraps positively); linearly interpolates `pos` and `tangent` between
    /// the bracketing samples. Samples already carry `race_dir`-oriented
    /// tangents, so the blend is oriented along `race_dir` by construction.
    ///
    /// `None` only when `samples` is empty. Returns `samples[0]` when there is
    /// a single sample, or when `length` is not a positive finite value
    /// (guards div-by-zero / NaN).
    ///
    /// **Precondition:** `samples[0].s == 0` (the closed-loop centerline
    /// invariant: the arc-length resample seeds the first sample at `s = 0`).
    /// If violated, `at` still returns a *defined* value rather than
    /// panicking, by falling back to the closing bracket `[samples[last],
    /// samples[first]]`.
    pub fn at(&self, query_s: f32) -> Option<CenterlineSample> {
        let count = self.samples.len();
        if count == 0 {
            return None;
        }
        if count == 1 || self.length.is_nan() || self.length <= 0.0 {
            return Some(self.samples[0]);
        }
        let wrapped_s = query_s.rem_euclid(self.length);
        let lo = self
            .samples
            .iter()
            .rposition(|sample| sample.s <= wrapped_s)
            .unwrap_or_else(|| count.saturating_sub(1));
        let hi = lo.checked_add(1).filter(|&h| h < count).unwrap_or(0);
        let lo_sample = self.samples[lo];
        let mut hi_sample = self.samples[hi];
        if hi == 0 {
            hi_sample.s += self.length;
        }
        let span = hi_sample.s - lo_sample.s;
        let frac = if span > 0.0 {
            (wrapped_s - lo_sample.s) / span
        } else {
            0.0
        };
        Some(CenterlineSample {
            s: wrapped_s,
            pos: lerp(lo_sample.pos, hi_sample.pos, frac),
            tangent: lerp(lo_sample.tangent, hi_sample.tangent, frac),
        })
    }
}

/// Component-wise linear interpolation `a + (b - a) * t` for a `(f32, f32)`
/// pair.
fn lerp(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    ((b.0 - a.0).mul_add(t, a.0), (b.1 - a.1).mul_add(t, a.1))
}

/// Speed metrics derived by the passability oracle (design doc §3). Not inputs
/// to generation — *outputs* of it, produced almost for free on top of the
/// forward∩backward reachable set.
#[derive(Clone, Debug, Default)]
pub struct TrackMetrics {
    /// Peak attainable speed `Vmax_attain` (a poor scalar for "fastness").
    pub vmax_attain: Option<i32>,
    /// Lap tempo = lap length / move-count of the fastest lap (the honest one).
    pub tempo: Option<f32>,
    /// The path of the fastest lap.
    pub fastest_lap: Vec<Point>,
    /// Per-point max speed across live states — the where's-fast/slow heatmap.
    pub speed_heatmap: Vec<(Point, i32)>,
}

/// The full exported track artifact (design doc §2, Ф7).
///
/// Carries the corridor `D`, its boundary walls, the start/finish line
/// (carrying the timing gate), the global traversal orientation, the
/// s-field, the start grid, the parameterized centerline, and the
/// oracle-derived metrics.
#[derive(Clone, Debug)]
pub struct TrackArtifact {
    /// The corridor `D` — the set of drivable points.
    pub corridor: Corridor,
    /// Walls = dual edges on the boundary of `D`.
    pub walls: Vec<Wall>,
    /// Start/finish line.
    pub sf: StartFinish,
    /// Global traversal orientation.
    pub race_dir: RaceDir,
    /// The monotone distance field over `D \ gate`, plus its gradient/tangent
    /// accessors.
    pub s_field: SField,
    /// The ordered, distinct start positions.
    pub start_grid: StartGrid,
    /// Parameterized centerline.
    pub centerline: Centerline,
    /// Oracle-derived speed metrics.
    pub metrics: TrackMetrics,
    /// Minimum cross-section width in lattice points (Ф4 static validation).
    pub width_min: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Size;

    // ---- TimingGate (subtask 1) --------------------------------------

    #[test]
    fn forward_unit_matches_each_side() {
        let gate = |forward| TimingGate {
            behind: vec![],
            forward,
        };
        assert_eq!(gate(Side::East).forward_unit(), (1.0, 0.0));
        assert_eq!(gate(Side::West).forward_unit(), (-1.0, 0.0));
        assert_eq!(gate(Side::North).forward_unit(), (0.0, 1.0));
        assert_eq!(gate(Side::South).forward_unit(), (0.0, -1.0));
    }

    #[test]
    fn separates_true_only_for_the_implied_cut_edge() {
        let gate = TimingGate {
            behind: vec![Point::new(1, 1)],
            forward: Side::East,
        };
        // The implied cut edge.
        assert!(gate.separates(Point::new(1, 1), Point::new(2, 1)));
        // Non-adjacent pair.
        assert!(!gate.separates(Point::new(1, 1), Point::new(3, 1)));
        // Lateral pair (not along `forward`).
        assert!(!gate.separates(Point::new(1, 1), Point::new(1, 2)));
        // A different, non-gate adjacent pair.
        assert!(!gate.separates(Point::new(0, 1), Point::new(1, 1)));
    }

    #[test]
    fn separates_is_symmetric() {
        let gate = TimingGate {
            behind: vec![Point::new(1, 1)],
            forward: Side::East,
        };
        let (a, b) = (Point::new(1, 1), Point::new(2, 1));
        assert!(gate.separates(a, b));
        assert!(gate.separates(b, a));
    }

    #[test]
    fn forward_face_shifts_behind_by_forward_delta() {
        let gate = TimingGate {
            behind: vec![Point::new(1, 1)],
            forward: Side::East,
        };
        assert_eq!(
            gate.forward_face().collect::<Vec<_>>(),
            vec![Point::new(2, 1)]
        );
    }

    #[test]
    fn forward_face_shifts_by_each_side_delta() {
        let behind = vec![Point::new(1, 1)];
        let face = |forward| {
            TimingGate {
                behind: behind.clone(),
                forward,
            }
            .forward_face()
            .collect::<Vec<_>>()
        };
        assert_eq!(face(Side::East), vec![Point::new(2, 1)]);
        assert_eq!(face(Side::West), vec![Point::new(0, 1)]);
        assert_eq!(face(Side::North), vec![Point::new(1, 2)]);
        assert_eq!(face(Side::South), vec![Point::new(1, 0)]);
    }

    #[test]
    fn forward_face_empty_when_behind_is_empty() {
        let gate = TimingGate {
            behind: vec![],
            forward: Side::East,
        };
        assert!(gate.forward_face().next().is_none());
    }

    #[test]
    fn forward_face_filters_overflowing_seed() {
        // A `behind` cell at i32::MAX with forward East: the +1 step
        // overflows i32, so the seed is filtered out rather than panicking.
        let gate = TimingGate {
            behind: vec![Point::new(i32::MAX, 1)],
            forward: Side::East,
        };
        assert!(gate.forward_face().next().is_none());
    }

    // ---- StartGrid (subtask 2) ----------------------------------------

    #[test]
    fn start_grid_preserves_front_to_back_order_and_distinctness() {
        // race_dir such that "front" is +x, so front-to-back is decreasing x.
        let grid = StartGrid {
            positions: vec![Point::new(5, 0), Point::new(4, 0), Point::new(3, 0)],
        };
        assert_eq!(
            grid.positions,
            vec![Point::new(5, 0), Point::new(4, 0), Point::new(3, 0)]
        );
        let distinct: std::collections::HashSet<_> = grid.positions.iter().collect();
        assert_eq!(distinct.len(), grid.positions.len());
    }

    #[test]
    fn start_grid_default_is_empty() {
        assert!(StartGrid::default().positions.is_empty());
    }

    // ---- SField (subtask 3) --------------------------------------------

    /// A 4-wide, 1-tall band at `y == 1`, with `s(x, 1) = x` and no gate.
    fn linear_field_no_gate() -> (SField, TimingGate) {
        let rect = Rect {
            origin: Point::new(0, 0),
            size: Size::new(4, 2),
        };
        let field = SField::new(rect, |p| {
            (p.y == 1 && (0..4).contains(&p.x)).then_some(u32::try_from(p.x).unwrap())
        });
        let gate = TimingGate {
            behind: vec![],
            forward: Side::East,
        };
        (field, gate)
    }

    #[test]
    fn scalar_at_is_monotone_and_none_off_band() {
        let (field, _gate) = linear_field_no_gate();
        assert_eq!(field.scalar_at(Point::new(0, 1)), Some(0));
        assert_eq!(field.scalar_at(Point::new(1, 1)), Some(1));
        assert_eq!(field.scalar_at(Point::new(2, 1)), Some(2));
        assert_eq!(field.scalar_at(Point::new(3, 1)), Some(3));
        assert_eq!(field.scalar_at(Point::new(0, 0)), None);
    }

    #[test]
    fn scalar_at_is_total_even_when_dist_is_short() {
        // A hand-built `SField` violating the `dist.len() == rect.area()`
        // invariant `SField::new` upholds: a 2x2 rect (area 4) with only one
        // `dist` entry.
        let rect = Rect {
            origin: Point::new(0, 0),
            size: Size::new(2, 2),
        };
        let field = SField {
            rect,
            dist: vec![Some(0)],
        };
        // In-bounds cell backed by `dist` still reads back correctly.
        assert_eq!(field.scalar_at(Point::new(0, 0)), Some(0));
        // In-`rect` cell past the end of the short `dist` — must return
        // `None`, not panic.
        assert_eq!(field.scalar_at(Point::new(1, 1)), None);
    }

    #[test]
    fn gradient_at_is_central_or_one_sided() {
        let (field, gate) = linear_field_no_gate();
        // Central difference: both neighbors in band.
        assert_eq!(field.gradient_at(&gate, Point::new(1, 1)), Some((2, 0)));
        // One-sided difference: west neighbor is out of band.
        assert_eq!(field.gradient_at(&gate, Point::new(0, 1)), Some((1, 0)));
    }

    #[test]
    fn tangent_at_points_along_increasing_s() {
        let (field, gate) = linear_field_no_gate();
        assert_eq!(field.tangent_at(&gate, Point::new(1, 1)), Some((1.0, 0.0)));
    }

    #[test]
    fn tangent_at_gate_cells_is_forward_and_not_cross_cut() {
        // Cells (0..=4, 1); gate cut between (1,1) and (2,1); a wrap in `s`
        // straddles the cut (L = 4).
        let rect = Rect {
            origin: Point::new(0, 0),
            size: Size::new(5, 2),
        };
        let s = [3u32, 4, 0, 1, 2];
        let field = SField::new(rect, |p| {
            (p.y == 1 && (0..5).contains(&p.x)).then(|| s[usize::try_from(p.x).unwrap()])
        });
        let gate = TimingGate {
            behind: vec![Point::new(1, 1)],
            forward: Side::East,
        };
        assert_eq!(field.tangent_at(&gate, Point::new(1, 1)), Some((1.0, 0.0)));
        assert_eq!(field.tangent_at(&gate, Point::new(2, 1)), Some((1.0, 0.0)));
        // Backward-only diff (+forward), not the spurious cross-cut jump.
        assert_eq!(field.gradient_at(&gate, Point::new(1, 1)), Some((1, 0)));
    }

    #[test]
    fn tangent_at_falls_back_when_gradient_is_flat() {
        let rect = Rect {
            origin: Point::new(0, 0),
            size: Size::new(3, 3),
        };
        // All cells in the box share the same s: every gradient is (0, 0).
        let field = SField::new(rect, |_| Some(0));
        let gate = TimingGate {
            behind: vec![],
            forward: Side::East,
        };
        assert_eq!(
            field.tangent_at(&gate, Point::new(1, 1)),
            Some(FLAT_FALLBACK)
        );
    }

    // ---- SField::from_gate_bfs (Ф7 s-field, subtask 4) ------------------

    /// Build a corridor over the box `[origin, origin + (w, h))` with the
    /// given `(x, y)` cells marked drivable (mirrors the `geom` test helper of
    /// the same name/shape).
    fn corridor(
        origin: (crate::geom::Coord, crate::geom::Coord),
        w: usize,
        h: usize,
        drivable: &[(crate::geom::Coord, crate::geom::Coord)],
    ) -> Corridor {
        let mut d = Corridor::new(Point::new(origin.0, origin.1), w, h);
        for &(x, y) in drivable {
            d.set(Point::new(x, y), true);
        }
        d
    }

    /// The 8-cell 3×3 ring `{(1,1),(2,1),(3,1),(1,2),(3,2),(1,3),(2,3),(3,3)}`
    /// (center `(2,2)` excluded) — the AC6 hand-computed fixture, with a gate
    /// behind `(1,1)` forward `East` (cut edge `(1,1)–(2,1)`, forward face
    /// `{(2,1)}`).
    fn ring_gate_fixture() -> (Corridor, TimingGate) {
        let ring = [
            (1, 1),
            (2, 1),
            (3, 1),
            (1, 2),
            (3, 2),
            (1, 3),
            (2, 3),
            (3, 3),
        ];
        let d = corridor((0, 0), 5, 5, &ring);
        let gate = TimingGate {
            behind: vec![Point::new(1, 1)],
            forward: Side::East,
        };
        (d, gate)
    }

    #[test]
    fn from_gate_bfs_matches_hand_computed_ring_distances() {
        // AC6: exact distances the long way around the 8-cycle, cell-by-cell.
        let (d, gate) = ring_gate_fixture();
        let field = SField::from_gate_bfs(&d, &gate);
        let expected = [
            ((2, 1), 0),
            ((3, 1), 1),
            ((3, 2), 2),
            ((3, 3), 3),
            ((2, 3), 4),
            ((1, 3), 5),
            ((1, 2), 6),
            ((1, 1), 7),
        ];
        for ((x, y), s) in expected {
            assert_eq!(
                field.scalar_at(Point::new(x, y)),
                Some(s),
                "cell ({x}, {y})"
            );
        }
        // The cell across the cut reads the long-way max (7), not 1 — proves
        // the barrier (AC1).
        assert_eq!(field.scalar_at(Point::new(1, 1)), Some(7));
    }

    #[test]
    fn from_gate_bfs_ac1_ac2_seed_max_and_total_coverage() {
        // AC1/AC2: forward-face seed is 0, the behind cross-section is the
        // max, every in-D cell is Some, (2,2) (¬D) is None, and dist.len() ==
        // rect.area() (the SField invariant).
        let (d, gate) = ring_gate_fixture();
        let field = SField::from_gate_bfs(&d, &gate);
        assert_eq!(field.scalar_at(Point::new(2, 1)), Some(0));
        assert_eq!(field.scalar_at(Point::new(1, 1)), Some(7));
        for (x, y) in [
            (1, 1),
            (2, 1),
            (3, 1),
            (1, 2),
            (3, 2),
            (1, 3),
            (2, 3),
            (3, 3),
        ] {
            assert!(field.scalar_at(Point::new(x, y)).is_some());
        }
        assert_eq!(field.scalar_at(Point::new(2, 2)), None);
        assert_eq!(field.dist.len(), field.rect.size.area());
    }

    #[test]
    fn from_gate_bfs_ac3_no_forward_fold_except_the_gate_step() {
        // AC3: walking the ring in race_dir order from the seed, every
        // forward unit step has Δs >= 0, except the single L -> 0 reset at
        // the gate.
        let (d, gate) = ring_gate_fixture();
        let field = SField::from_gate_bfs(&d, &gate);
        let order = [
            (2, 1),
            (3, 1),
            (3, 2),
            (3, 3),
            (2, 3),
            (1, 3),
            (1, 2),
            (1, 1),
            (2, 1), // closing gate step: 7 -> 0
        ];
        for w in order.windows(2) {
            let s0 = field.scalar_at(Point::new(w[0].0, w[0].1)).unwrap();
            let s1 = field.scalar_at(Point::new(w[1].0, w[1].1)).unwrap();
            let is_gate_step = w[0] == (1, 1) && w[1] == (2, 1);
            if is_gate_step {
                assert_eq!((s0, s1), (7, 0), "the gate step must be the 7 -> 0 reset");
            } else {
                assert!(
                    i64::from(s1) - i64::from(s0) >= 0,
                    "forward step {w:?} decreased s: {s0} -> {s1}"
                );
            }
        }
    }

    #[test]
    fn from_gate_bfs_ac4_only_discontinuity_is_the_gate_reset() {
        // AC4: every non-gate adjacent forward pair differs by exactly 1; the
        // sole exception is the 7 -> 0 gate step.
        let (d, gate) = ring_gate_fixture();
        let field = SField::from_gate_bfs(&d, &gate);
        let order = [
            (2, 1),
            (3, 1),
            (3, 2),
            (3, 3),
            (2, 3),
            (1, 3),
            (1, 2),
            (1, 1),
            (2, 1),
        ];
        for w in order.windows(2) {
            let s0 = field.scalar_at(Point::new(w[0].0, w[0].1)).unwrap();
            let s1 = field.scalar_at(Point::new(w[1].0, w[1].1)).unwrap();
            let is_gate_step = w[0] == (1, 1) && w[1] == (2, 1);
            if is_gate_step {
                assert_eq!((s0, s1), (7, 0));
            } else {
                assert_eq!(s1, s0 + 1, "non-gate step {w:?} is not a unit increase");
            }
        }
    }

    #[test]
    fn from_gate_bfs_ac5_hairpin_is_single_valued_no_projection_fold() {
        // AC5: a U-shaped (hairpin) corridor where a nearest-point-on-
        // centerline projection would fold the two parallel arms onto the
        // same s; the BFS instead assigns each arm cell a distinct, strictly
        // increasing distance along the true corridor path through the
        // pocket. Shape (x horizontal 0..=4, y vertical 0..=3):
        //   arm A: (0,1),(0,2),(0,3)
        //   pocket bottom: (0,3),(1,3),(2,3),(3,3),(4,3)
        //   arm B: (4,0),(4,1),(4,2),(4,3)
        // Gate `behind` = (0,0), a cell deliberately *outside* D (¬D, so the
        // implied cut edge never matches a real D-D BFS step); its
        // `forward_face` (North) is (0,1) — the mouth of arm A — which seeds
        // the BFS with no path disconnected.
        let hairpin: Vec<(crate::geom::Coord, crate::geom::Coord)> = vec![
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 3),
            (2, 3),
            (3, 3),
            (4, 3),
            (4, 2),
            (4, 1),
            (4, 0),
        ];
        let d = corridor((0, 0), 5, 4, &hairpin);
        let gate = TimingGate {
            behind: vec![Point::new(0, 0)],
            forward: Side::North,
        };
        let field = SField::from_gate_bfs(&d, &gate);
        // Each cell is single-valued (structural: Option<u32>) and Some.
        let path = [
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 3),
            (2, 3),
            (3, 3),
            (4, 3),
            (4, 2),
            (4, 1),
            (4, 0),
        ];
        let mut prev = None;
        for (x, y) in path {
            let s = field.scalar_at(Point::new(x, y));
            assert!(s.is_some(), "cell ({x}, {y}) must be reachable");
            if let Some(prev_s) = prev {
                assert_eq!(
                    s.unwrap(),
                    prev_s + 1,
                    "s must strictly increase along the true path, no fold"
                );
            }
            prev = s;
        }
        // A projection-based centerline definition would fold arm A's (0,3)
        // and arm B's (4,3) onto a similar "distance from center" value
        // despite being 7 apart along the true path — the BFS keeps them
        // distinct.
        assert_ne!(
            field.scalar_at(Point::new(0, 3)),
            field.scalar_at(Point::new(4, 3))
        );
    }

    #[test]
    fn from_gate_bfs_is_deterministic() {
        // AC6: rerunning the producer on identical inputs yields identical
        // output.
        let (d, gate) = ring_gate_fixture();
        assert_eq!(
            SField::from_gate_bfs(&d, &gate).dist,
            SField::from_gate_bfs(&d, &gate).dist
        );
    }

    // ---- Centerline::at (subtask 4) ------------------------------------

    fn sample(s: f32, pos: (f32, f32), tangent: (f32, f32)) -> CenterlineSample {
        CenterlineSample { s, pos, tangent }
    }

    fn assert_sample_eq(actual: CenterlineSample, expected: CenterlineSample) {
        let eps = 1e-5;
        assert!(
            (actual.s - expected.s).abs() < eps,
            "{actual:?} vs {expected:?}"
        );
        assert!(
            (actual.pos.0 - expected.pos.0).abs() < eps
                && (actual.pos.1 - expected.pos.1).abs() < eps,
            "{actual:?} vs {expected:?}"
        );
        assert!(
            (actual.tangent.0 - expected.tangent.0).abs() < eps
                && (actual.tangent.1 - expected.tangent.1).abs() < eps,
            "{actual:?} vs {expected:?}"
        );
    }

    fn fixture_centerline() -> Centerline {
        Centerline {
            samples: vec![
                sample(0.0, (0.0, 0.0), (1.0, 0.0)),
                sample(1.0, (1.0, 0.0), (1.0, 0.0)),
                sample(2.0, (1.0, 1.0), (0.0, 1.0)),
            ],
            length: 4.0,
        }
    }

    #[test]
    fn at_returns_none_for_empty_centerline() {
        let cl = Centerline::default();
        assert!(cl.at(0.0).is_none());
    }

    #[test]
    fn at_returns_only_sample_when_single_or_zero_length() {
        let cl = Centerline {
            samples: vec![sample(0.0, (0.0, 0.0), (1.0, 0.0))],
            length: 0.0,
        };
        assert_sample_eq(cl.at(3.0).unwrap(), cl.samples[0]);

        let cl_multi_zero_len = Centerline {
            samples: vec![
                sample(0.0, (0.0, 0.0), (1.0, 0.0)),
                sample(1.0, (1.0, 0.0), (1.0, 0.0)),
            ],
            length: 0.0,
        };
        assert_sample_eq(
            cl_multi_zero_len.at(5.0).unwrap(),
            cl_multi_zero_len.samples[0],
        );
    }

    #[test]
    fn at_wraps_by_length() {
        let cl = fixture_centerline();
        assert_sample_eq(cl.at(0.0).unwrap(), cl.samples[0]);
        assert_sample_eq(cl.at(4.0).unwrap(), cl.at(0.0).unwrap());
        assert_sample_eq(cl.at(4.5).unwrap(), cl.at(0.5).unwrap());
    }

    #[test]
    fn at_interpolates_interior_and_closing_segment() {
        let cl = fixture_centerline();
        assert_sample_eq(cl.at(1.5).unwrap(), sample(1.5, (1.0, 0.5), (0.5, 0.5)));
        assert_sample_eq(cl.at(3.0).unwrap(), sample(3.0, (0.5, 0.5), (0.5, 0.5)));
    }

    #[test]
    fn at_is_total_even_when_first_sample_precondition_is_violated() {
        // `samples[0].s == 0.5 != 0`, violating the documented precondition.
        let cl = Centerline {
            samples: vec![
                sample(0.5, (0.0, 0.0), (1.0, 0.0)),
                sample(1.5, (1.0, 0.0), (1.0, 0.0)),
                sample(2.5, (1.0, 1.0), (0.0, 1.0)),
            ],
            length: 4.0,
        };
        // `wrapped_s = 0.2 < samples[0].s`: must return a defined value, not panic.
        assert!(cl.at(0.2).is_some());
    }

    // ---- TrackArtifact (subtask 5) --------------------------------------

    #[test]
    fn track_artifact_carries_all_nine_members() {
        let rect = Rect {
            origin: Point::new(0, 0),
            size: Size::new(1, 1),
        };
        let artifact = TrackArtifact {
            corridor: Corridor::new(Point::new(0, 0), 1, 1),
            walls: vec![],
            sf: StartFinish {
                chord: vec![],
                orient: Orient::Horizontal,
                gate: TimingGate {
                    behind: vec![],
                    forward: Side::East,
                },
            },
            race_dir: RaceDir::Cw,
            s_field: SField::new(rect, |_| None),
            start_grid: StartGrid::default(),
            centerline: Centerline::default(),
            metrics: TrackMetrics::default(),
            width_min: 1,
        };
        assert!(artifact.start_grid.positions.is_empty());
        assert!(artifact.s_field.dist.iter().all(Option::is_none));
    }

    // ---- StartFinish::width (AC7) ----------------------------------------

    #[test]
    fn start_finish_width_returns_chord_len() {
        let sf = StartFinish {
            chord: vec![Point::new(0, 0), Point::new(0, 1), Point::new(0, 2)],
            orient: Orient::Horizontal,
            gate: TimingGate {
                behind: vec![],
                forward: Side::East,
            },
        };
        assert_eq!(sf.width(), 3);
        assert_eq!(sf.width(), sf.chord.len());
    }

    #[test]
    fn start_finish_width_empty_chord_is_zero() {
        let sf = StartFinish {
            chord: vec![],
            orient: Orient::Horizontal,
            gate: TimingGate {
                behind: vec![],
                forward: Side::East,
            },
        };
        assert_eq!(sf.width(), 0);
    }
}

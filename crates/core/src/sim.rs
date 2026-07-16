//! The deterministic simulation core (design doc §3a).
//!
//! Pure and deterministic under a fixed seed, with no I/O. This is the shared
//! engine that the renderer and AI training both drive — the technical guarantee
//! that "bots play the same game" as the player.

use crate::geom::{Corridor, Point, Side, supercover};
use crate::track::StartFinish;
use enumflags2::bitflags;

/// Re-exported so consumers of [`legal_mask`]'s `BitFlags<Action>` return type
/// (e.g. `gp-ai`) do not need a direct `enumflags2` dependency (Rust API
/// guideline C-REEXPORT).
pub use enumflags2::BitFlags;

/// One car's state `(x, y, vx, vy)` (design doc §3). Start state has `v = (0,0)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct CarState {
    /// Cell x-coordinate (grid column).
    pub x: i32,
    /// Cell y-coordinate (grid row).
    pub y: i32,
    /// Velocity x-component, in cells per turn.
    pub vx: i32,
    /// Velocity y-component, in cells per turn.
    pub vy: i32,
}

impl CarState {
    /// The car's current cell.
    #[inline]
    pub const fn pos(self) -> Point {
        Point::new(self.x, self.y)
    }
}

/// The 5 von-Neumann acceleration actions (design doc §3): `(0,0)`, `(±1,0)`,
/// `(0,±1)`.
///
/// Diagonal acceleration in a single turn is forbidden — this is the
/// foundation of every braking-distance argument in the design.
#[bitflags]
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
// `#[bitflags]`-generated code triggers `clippy::use_self` (nursery) against
// the enum's own declaration span; no `Self`-eligible code exists in this
// hand-written block.
#[allow(clippy::use_self)]
pub enum Action {
    /// `(0, 0)` — hold velocity. Always available; the basis of V=1 liveness.
    Coast,
    /// `(+1, 0)`.
    East,
    /// `(-1, 0)`.
    West,
    /// `(0, +1)`.
    North,
    /// `(0, -1)`.
    South,
}

impl Action {
    /// All five actions, in a fixed order (matches the policy's logit order).
    pub const ALL: [Self; 5] = [
        Self::Coast,
        Self::East,
        Self::West,
        Self::North,
        Self::South,
    ];

    /// The acceleration `(a, b)` this action applies to velocity.
    #[inline]
    pub const fn accel(self) -> (i32, i32) {
        match self {
            Self::Coast => (0, 0),
            Self::East => (1, 0),
            Self::West => (-1, 0),
            Self::North => (0, 1),
            Self::South => (0, -1),
        }
    }
}

/// Legality of applying `a` from state `s` in corridor `d` (design doc §3):
///
/// ```text
/// legal_move(D, x,y,vx,vy, (a,b)):
///     (vx2,vy2) = (vx+a, vy+b)
///     p1 = (x+vx2, y+vy2)
///     return p1 ∈ D  AND  supercover((x,y), p1) ⊆ D
/// ```
///
/// The strict supercover check is what stops a fast chord from jumping a wall or
/// threading a pinched dual vertex. This is *both* the runtime rule and the
/// oracle's graph edge — one code path.
pub fn legal_move(d: &Corridor, s: CarState, a: Action) -> bool {
    let (ax, ay) = a.accel();
    let Some(vx2) = s.vx.checked_add(ax) else {
        return false;
    };
    let Some(vy2) = s.vy.checked_add(ay) else {
        return false;
    };
    let Some(px) = s.x.checked_add(vx2) else {
        return false;
    };
    let Some(py) = s.y.checked_add(vy2) else {
        return false;
    };
    let p1 = Point::new(px, py);
    if !d.contains(p1) {
        return false;
    }
    supercover(s.pos(), p1).all(|c| d.contains(c))
}

/// The legal-action mask for `s`, in [`Action::ALL`] order. Consumed by the
/// player UI, the AI policy (as the pre-softmax `−inf` mask), and the oracle.
pub fn legal_mask(d: &Corridor, s: CarState) -> BitFlags<Action> {
    Action::ALL
        .into_iter()
        .filter(|&a| legal_move(d, s, a))
        .collect()
}

/// Advances one car by one (assumed-legal) action, returning the new state.
///
/// Accelerate-then-advance (design doc §3): `(vx', vy') = (vx + ax, vy + ay)`,
/// then `(x', y') = (x + vx', y + vy')`, where `(ax, ay)` is `a`'s
/// [`accel()`](Action::accel).
///
/// **Assumed-legal precondition:** `step` performs no legality check — that is
/// [`legal_move`]'s sole job. Callers must pass an `a` that is legal for `s`
/// (i.e. `legal_move(d, s, a)` holds for the relevant corridor `d`); behavior
/// for an illegal action is unsupported.
///
/// **Overflow precondition:** the four adds this function performs — `vx + ax`,
/// `vy + ay`, `x + vx'`, `y + vy'` — are exactly the four sums [`legal_move`]
/// computes via its `checked_add` chain and proves in-range (never overflowing
/// `i32`) before returning `true`. On the assumed-legal domain above, these
/// plain adds therefore never overflow.
#[inline]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "assumed-legal precondition above: the four adds mirror legal_move's \
              checked_add chain, which proves them in-range for any action legal \
              under legal_move/legal_mask; out-of-domain (illegal-action) input is \
              unsupported, per this fn's documented precondition"
)]
pub const fn step(s: CarState, a: Action) -> CarState {
    let (ax, ay) = a.accel();
    let vx = s.vx + ax;
    let vy = s.vy + ay;
    let x = s.x + vx;
    let y = s.y + vy;
    CarState { x, y, vx, vy }
}

/// Signed start/finish crossing counter (design doc §3).
///
/// The S/F is a full chord, so the annulus cut along it is a simply connected
/// strip: you can only return to the line by backing off (a decrement cancels an
/// increment) or by a full lap. Initialized to `-1` (the first forward cross is
/// the race start, not a lap); `laps = max(0, counter)`.
#[derive(Clone, Copy, Debug)]
pub struct LapCounter {
    counter: i32,
}

impl Default for LapCounter {
    fn default() -> Self {
        Self { counter: -1 }
    }
}

impl LapCounter {
    /// A new lap counter in the pre-race state (`raw()` is `-1`, `laps()` is `0`).
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Completed laps.
    #[inline]
    pub fn laps(&self) -> i32 {
        self.counter.max(0)
    }

    /// Raw signed crossing count (may be negative before the race starts).
    #[inline]
    pub const fn raw(&self) -> i32 {
        self.counter
    }

    /// Registers the move segment `from → to` against `sf`'s timing gate and
    /// mutates the counter: `+1` for a forward crossing, `−1` for a reverse one,
    /// no change otherwise — **at most one event per call**, even for a long
    /// chord (a straight segment's perpendicular coordinate is monotone, so it
    /// meets the gate line at most once).
    ///
    /// The gate's supporting line is the **half-grid dual edge** one edge ahead
    /// of the `behind` row (design doc §2 Ф3, §3 \[C2\]) — not any integer cell
    /// line — so no real `Point` ever lands on it. The classification is
    /// half-open: `from` strictly behind (`−`) and `to` ahead-**or**-on-line
    /// (`+`) is forward; `from` ahead-or-on-line and `to` strictly behind is
    /// reverse. Because the on-line branch is geometrically unreachable for
    /// integer `Point`s, every real chord is scored purely by the behind/ahead
    /// partition. The sign derives from `sf.gate.forward` alone (the local
    /// `+race_dir` projection) — there is no separate race-direction parameter.
    ///
    /// Scores the swept `from → to` of the *committed legal move* only: no
    /// teleport handling, no collision/crash resolution. Callers **must** gate
    /// this call on [`legal_move`] first — `register_move` itself is
    /// legality-agnostic (the single legality path stays in `legal_move`), so
    /// an illegal chord's crossing must never reach here.
    ///
    /// No-op (does not panic) when `sf.gate.behind` is empty — a degenerate gate
    /// has no supporting line to cross.
    pub fn register_move(&mut self, sf: &StartFinish, from: Point, to: Point) {
        let Some(&r) = sf.gate.behind.first() else {
            return;
        };
        let from_c = gate_coord(from, r, sf.gate.forward);
        let to_c = gate_coord(to, r, sf.gate.forward);
        self.counter = self.counter.saturating_add(crossing_event(from_c, to_c));
    }
}

/// The half-grid gate line's coordinate value — the odd midpoint between the
/// `behind` row (`0`) and the `behind + forward` row (`+2`). Real integer
/// `Point`s always yield an even [`gate_coord`], so this value is unreachable
/// in play (design doc §2 Ф3).
const GATE_LINE: i32 = 1;

/// The doubled signed perpendicular coordinate of `p` relative to the gate's
/// reference cell `r` (`sf.gate.behind[0]`) along `forward`'s unit axis
/// (`forward.delta()`): `2 * ((p.x − r.x)·dx + (p.y − r.y)·dy)`.
///
/// The `behind` row (`r`'s row) maps to `0`, `behind + forward` to `+2`, and
/// each further row to a further `±2` — always even. Doubling is what turns
/// the half-grid gate line into an addressable integer ([`GATE_LINE`], the odd
/// midpoint `1`) without a fractional coordinate.
///
/// **Precondition:** `p`, `r`, and every other point this is evaluated against
/// in the same call lie in the grid-realistic, allocatable-corridor domain
/// that `supercover`/`Size::area`/`step` also document — pairwise coordinate
/// differences and the `×2` doubling stay within `i32`. Not proven for
/// adversarial out-of-domain `Point`s.
#[inline]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "in-D precondition documented above: p/r are grid-realistic, \
              allocatable-corridor coordinates (matching supercover/step/Size::area's \
              domain), so the subtractions and doubling stay within i32"
)]
const fn gate_coord(p: Point, r: Point, forward: Side) -> i32 {
    let (dx, dy) = forward.delta();
    2 * ((p.x - r.x) * dx + (p.y - r.y) * dy)
}

/// The signed crossing event for a chord whose endpoints have doubled
/// perpendicular coordinates `from_c` / `to_c` (see [`gate_coord`]): `+1`
/// forward (`from_c` strictly `< GATE_LINE`, `to_c >= GATE_LINE`), `−1` reverse
/// (`from_c >= GATE_LINE`, `to_c < GATE_LINE`), `0` otherwise.
///
/// Pure comparison — no arithmetic side effects, hence no overflow precondition
/// and no `#[allow]`.
const fn crossing_event(from_c: i32, to_c: i32) -> i32 {
    if from_c < GATE_LINE && to_c >= GATE_LINE {
        1
    } else if from_c >= GATE_LINE && to_c < GATE_LINE {
        -1
    } else {
        0
    }
}

/// The result of resolving a crash (design doc §3, `[D4]`/`[N5]`).
///
/// Carries the post-crash [`CarState`] plus the scrub-tick marker that forces
/// the immediately-following move to be [`Action::Coast`] for exactly one tick.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CrashOutcome {
    /// The post-crash kinematic state (respawn cell + quenched velocity).
    pub state: CarState,
    /// `true` while the scrub tick is still pending — the next move must be
    /// [`Action::Coast`]. Cleared by [`CrashOutcome::consume_scrub`].
    pub scrub: bool,
}

impl CrashOutcome {
    /// The action mask available from this outcome: the singleton `{Coast}`
    /// while [`CrashOutcome::scrub`] holds (`[N5]`'s "один ход без права
    /// реакселерации"), otherwise the ordinary [`legal_mask`].
    pub fn action_mask(self, d: &Corridor) -> BitFlags<Action> {
        if self.scrub {
            BitFlags::from(Action::Coast)
        } else {
            legal_mask(d, self.state)
        }
    }

    /// Advances past the scrub tick. Total: while `scrub` holds, applies the
    /// forced `Coast` (guaranteed legal, see [`resolve_crash`]) and clears the
    /// marker; otherwise a no-op returning `self` unchanged — an
    /// already-consumed outcome never double-advances.
    #[must_use]
    pub const fn consume_scrub(self) -> Self {
        if self.scrub {
            Self {
                state: step(self.state, Action::Coast),
                scrub: false,
            }
        } else {
            self
        }
    }
}

/// Crash resolution — a wall collision, arising as a search dead-end where all 5
/// moves leave `D` (design doc §3, `[D4]`/`[N5]`, finalized).
///
/// Leaning rule: zero the into-wall velocity component, keep the along-wall
/// component with strong damping (e.g. `/2`), respawn at the last valid cell —
/// so a crash never yields a controlled `v = 0` (which would make "brake by
/// crashing" the dominant strategy). Fail-safe: if every move from the respawn
/// state is illegal, damp again, down to `v = 0` in the limit.
///
/// TODO(3a): finalize once the crash rule is settled (open question).
pub fn resolve_crash(_d: &Corridor, _s: CarState) -> CrashOutcome {
    todo!("crash rule (design doc §3 [OPEN])")
}

/// Resolves several cars occupying the same point (design doc §3) — a layer
/// *outside* movement physics.
///
/// Each displaced car is moved to the nearest free point by in-`D` geodesic BFS
/// (4-conn, seeded-RNG shuffle for replay determinism). The displaced car keeps
/// its velocity (zeroing it would revive the "ram the pack to brake" abuse); the
/// displacement is a teleport — no supercover check, no lap-counter change.
///
/// TODO(3a): BFS-outward nearest-free placement.
pub fn resolve_collisions(_d: &Corridor, _cars: &mut [CarState]) {
    todo!("car-collision resolution (design doc §3)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_move_false_for_every_action_on_i32_max_overflow_without_panic() {
        // AC4: a state at x/y = i32::MAX with vx/vy already at i32::MAX overflows
        // the checked_add chain (either at vx+ax or at x+vx2) for every one of the
        // 5 actions; legal_move/legal_mask must return false, never panic.
        let d = Corridor::new(Point::new(0, 0), 5, 5);
        let s = CarState {
            x: i32::MAX,
            y: 0,
            vx: i32::MAX,
            vy: 0,
        };
        for a in Action::ALL {
            assert!(!legal_move(&d, s, a));
        }
        assert_eq!(legal_mask(&d, s), BitFlags::empty());
    }

    #[test]
    fn legal_move_false_for_every_action_on_i32_min_underflow_without_panic() {
        // AC4: the i32::MIN-symmetric underflow case (vx/vy already at i32::MIN).
        let d = Corridor::new(Point::new(0, 0), 5, 5);
        let s = CarState {
            x: i32::MIN,
            y: 0,
            vx: i32::MIN,
            vy: 0,
        };
        for a in Action::ALL {
            assert!(!legal_move(&d, s, a));
        }
        assert_eq!(legal_mask(&d, s), BitFlags::empty());
    }

    #[test]
    fn step_from_rest_shifts_by_action_delta() {
        // AC5: from rest, each action's velocity equals its accel delta, and
        // position shifts by that same delta.
        let s = CarState {
            x: 5,
            y: 5,
            vx: 0,
            vy: 0,
        };
        assert_eq!(
            step(s, Action::Coast),
            CarState {
                x: 5,
                y: 5,
                vx: 0,
                vy: 0
            }
        );
        assert_eq!(
            step(s, Action::East),
            CarState {
                x: 6,
                y: 5,
                vx: 1,
                vy: 0
            }
        );
        assert_eq!(
            step(s, Action::West),
            CarState {
                x: 4,
                y: 5,
                vx: -1,
                vy: 0
            }
        );
        assert_eq!(
            step(s, Action::North),
            CarState {
                x: 5,
                y: 6,
                vx: 0,
                vy: 1
            }
        );
        assert_eq!(
            step(s, Action::South),
            CarState {
                x: 5,
                y: 4,
                vx: 0,
                vy: -1
            }
        );
    }

    #[test]
    fn step_advances_by_new_velocity() {
        // AC1: position advances by the *new* velocity, not the old one. If the
        // body advanced by the old velocity, this would yield y = 1 instead of 2.
        let s = CarState {
            x: 0,
            y: 0,
            vx: 3,
            vy: 1,
        };
        assert_eq!(
            step(s, Action::North),
            CarState {
                x: 3,
                y: 2,
                vx: 3,
                vy: 2
            }
        );

        // Symmetric x-axis case.
        let s2 = CarState {
            x: 2,
            y: 7,
            vx: -1,
            vy: 4,
        };
        assert_eq!(
            step(s2, Action::East),
            CarState {
                x: 2,
                y: 11,
                vx: 0,
                vy: 4
            }
        );
    }

    #[test]
    fn step_is_deterministic() {
        // AC2: step is a pure function — repeated calls on the same input yield
        // the same output.
        let s = CarState {
            x: 2,
            y: 7,
            vx: -1,
            vy: 4,
        };
        assert_eq!(step(s, Action::East), step(s, Action::East));
    }

    #[test]
    fn legal_move_rejects_wall_clipping_chord() {
        // AC3: a clear in-corridor chord is legal; a chord whose supercover
        // clips a wall cell (not itself the endpoint) is illegal.
        let mut d = Corridor::new(Point::new(0, 0), 4, 4);
        d.set(Point::new(0, 0), true);
        d.set(Point::new(1, 0), true);
        d.set(Point::new(2, 0), true);
        d.set(Point::new(1, 1), true);
        // (0,1) is deliberately left off-D.

        // Clear chord: (0,0) + Coast, v=(2,0) -> p1=(2,0). supercover =
        // {(0,0),(1,0),(2,0)} ⊆ D.
        let s_clear = CarState {
            x: 0,
            y: 0,
            vx: 2,
            vy: 0,
        };
        assert!(legal_move(&d, s_clear, Action::Coast));

        // Wall-clipping chord: (0,0) + East, v=(0,1)+accel(1,0)=(1,1) ->
        // p1=(1,1). supercover((0,0),(1,1)) is the dual-vertex tie, all four of
        // {(0,0),(1,0),(0,1),(1,1)}; (0,1) is off-D, so the move is illegal.
        let s_clip = CarState {
            x: 0,
            y: 0,
            vx: 0,
            vy: 1,
        };
        // Non-vacuous: p1 itself is drivable, so rejection comes from the
        // supercover rule, not the endpoint check.
        assert!(d.contains(Point::new(1, 1)));
        assert!(!legal_move(&d, s_clip, Action::East));
    }

    #[test]
    fn legal_mask_contains_exactly_the_legal_actions() {
        // AC2: legal_mask contains exactly the actions for which legal_move is
        // true. Carve a corridor so the car at the center has only Coast/East/West
        // drivable — North/South lead off the carved shape — a proper subset of
        // Action::ALL, so the check is non-vacuous in both directions.
        let mut d = Corridor::new(Point::new(0, 0), 3, 3);
        d.set(Point::new(1, 1), true);
        d.set(Point::new(2, 1), true);
        d.set(Point::new(0, 1), true);
        let s = CarState {
            x: 1,
            y: 1,
            vx: 0,
            vy: 0,
        };
        let mask = legal_mask(&d, s);
        for a in Action::ALL {
            assert_eq!(mask.contains(a), legal_move(&d, s, a));
        }
        assert!(!mask.is_empty());
        assert_ne!(mask, BitFlags::all());
    }

    use crate::geom::Orient;
    use crate::track::TimingGate;

    /// Fixture gate (design's Test Design): `behind = [(1,1)]`, `forward =
    /// East` ⇒ `gate_coord(p) = 2·(p.x−1)`, `GATE_LINE = 1` (no integer `x`
    /// reaches it). `chord`/`orient` are unused by `register_move`.
    fn sf_east_gate() -> StartFinish {
        StartFinish {
            chord: vec![Point::new(1, 1), Point::new(2, 1)],
            orient: Orient::Horizontal,
            gate: TimingGate {
                behind: vec![Point::new(1, 1)],
                forward: Side::East,
            },
        }
    }

    /// A car at rest at `(x, y)` with velocity `(vx, vy)`, for AC5.
    fn car(x: i32, y: i32, vx: i32, vy: i32) -> CarState {
        CarState { x, y, vx, vy }
    }

    #[test]
    fn register_move_ac1_forward_reverse_no_cross() {
        // AC1: table-driven over the spec rows.
        let cases = [
            (Point::new(1, 1), Point::new(2, 1), 1),
            (Point::new(3, 1), Point::new(1, 1), -1),
            (Point::new(2, 1), Point::new(1, 1), -1),
            (Point::new(0, 1), Point::new(1, 1), 0),
        ];
        let sf = sf_east_gate();
        for (from, to, expected_delta) in cases {
            let mut lap = LapCounter::new();
            let before = lap.raw();
            lap.register_move(&sf, from, to);
            assert_eq!(
                lap.raw() - before,
                expected_delta,
                "from {from:?} to {to:?}"
            );
        }
    }

    #[test]
    fn register_move_ac2_long_chord_scores_at_most_one_event() {
        // AC2: a long chord spanning many cells across the gate still yields
        // exactly one event, not more.
        let sf = sf_east_gate();

        let mut lap = LapCounter::new();
        lap.register_move(&sf, Point::new(0, 1), Point::new(4, 1));
        assert_eq!(lap.raw(), 0); // -1 init + 1 forward event

        let mut lap = LapCounter::new();
        lap.register_move(&sf, Point::new(4, 1), Point::new(0, 1));
        assert_eq!(lap.raw(), -2); // -1 init + 1 reverse event
    }

    #[test]
    fn register_move_ac3_no_rescore_when_already_on_one_side() {
        // AC3: a chord that stays wholly ahead (or wholly behind) the gate does
        // not re-score, even though both endpoints are on the "far" side.
        let sf = sf_east_gate();

        let mut lap = LapCounter::new();
        lap.register_move(&sf, Point::new(2, 1), Point::new(3, 1));
        assert_eq!(lap.raw(), -1); // ahead -> ahead: no cross

        let mut lap = LapCounter::new();
        lap.register_move(&sf, Point::new(1, 1), Point::new(0, 1));
        assert_eq!(lap.raw(), -1); // behind -> behind: no cross
    }

    #[test]
    fn crossing_event_locks_the_odd_line_into_the_forward_side() {
        // AC3: direct unit test of the private half-open comparison at the
        // odd (half-grid) line value. Real Points never produce gate_coord ==
        // 1 (design §2 Ф3) — this only locks the defensive on-line convention.
        assert_eq!(crossing_event(0, 1), 1);
        assert_eq!(crossing_event(1, 0), -1);
        assert_eq!(crossing_event(2, 4), 0);
    }

    #[test]
    fn register_move_ac4_init_and_laps() {
        // AC4: -1 at construction; laps() == 0 until the raw count is
        // positive; first forward cross -> raw 0 / laps 0 (race start); second
        // -> raw 1 / laps 1.
        let lap = LapCounter::new();
        assert_eq!(lap.raw(), -1);
        assert_eq!(lap.laps(), 0);

        let lap = LapCounter::default();
        assert_eq!(lap.raw(), -1);
        assert_eq!(lap.laps(), 0);

        let sf = sf_east_gate();
        let mut lap = LapCounter::new();
        lap.register_move(&sf, Point::new(1, 1), Point::new(2, 1));
        assert_eq!(lap.raw(), 0);
        assert_eq!(lap.laps(), 0);

        lap.register_move(&sf, Point::new(1, 1), Point::new(2, 1));
        assert_eq!(lap.raw(), 1);
        assert_eq!(lap.laps(), 1);
    }

    #[test]
    fn register_move_ac5_valid_finish_gates_on_legal_move_first() {
        // AC5: the valid-finish conjunction is legal_move first, then the
        // gate-cross. An illegal would-be forward-crosser must not score.
        let mut d = Corridor::new(Point::new(0, 0), 4, 4);
        d.set(Point::new(1, 0), true);
        d.set(Point::new(1, 1), true);
        d.set(Point::new(2, 1), true);
        // (2,0) is deliberately left off-D.
        let sf = sf_east_gate();

        // Illegal: (1,0), v=(0,1), East -> v2=(1,1) -> p1=(2,1). supercover
        // hits the dual-vertex tie including off-D (2,0) -> legal_move false.
        let s_illegal = car(1, 0, 0, 1);
        let p1 = Point::new(2, 1);
        assert!(d.contains(p1)); // non-vacuous: rejection is the supercover rule
        assert!(!legal_move(&d, s_illegal, Action::East));

        let mut lap = LapCounter::new();
        if legal_move(&d, s_illegal, Action::East) {
            lap.register_move(&sf, s_illegal.pos(), step(s_illegal, Action::East).pos());
        }
        assert_eq!(lap.raw(), -1); // unchanged: skipped

        // Legal: (1,1), v=(0,0), East -> v2=(1,0) -> p1=(2,1). supercover
        // {(1,1),(2,1)} subset D -> legal_move true -> register_move runs.
        let s_legal = car(1, 1, 0, 0);
        assert!(legal_move(&d, s_legal, Action::East));

        let mut lap = LapCounter::new();
        if legal_move(&d, s_legal, Action::East) {
            lap.register_move(&sf, s_legal.pos(), step(s_legal, Action::East).pos());
        }
        assert_eq!(lap.raw(), 0); // -1 init + 1 forward event
    }

    #[test]
    fn register_move_ac6_scripted_telescoping_and_parallel_move() {
        // AC6: a scripted sequence asserts exact counter/laps values,
        // including a back-and-forth pair telescoping to net 0 and a parallel
        // (tangent) move leaving the counter unchanged.
        let sf = sf_east_gate();
        let mut lap = LapCounter::new();
        assert_eq!(lap.raw(), -1);
        assert_eq!(lap.laps(), 0);

        lap.register_move(&sf, Point::new(1, 1), Point::new(2, 1)); // forward
        assert_eq!(lap.raw(), 0);
        assert_eq!(lap.laps(), 0);

        lap.register_move(&sf, Point::new(2, 1), Point::new(1, 1)); // reverse
        assert_eq!(lap.raw(), -1);
        assert_eq!(lap.laps(), 0);

        lap.register_move(&sf, Point::new(1, 1), Point::new(2, 1)); // forward
        lap.register_move(&sf, Point::new(2, 1), Point::new(1, 1)); // reverse
        assert_eq!(lap.raw(), -1); // telescopes to net 0 over the pair
        assert_eq!(lap.laps(), 0);

        lap.register_move(&sf, Point::new(1, 1), Point::new(2, 1)); // forward
        lap.register_move(&sf, Point::new(1, 1), Point::new(2, 1)); // forward
        assert_eq!(lap.raw(), 1);
        assert_eq!(lap.laps(), 1);

        // Parallel move along the gate (pure-y, constant gate_coord): no
        // perpendicular crossing.
        let before = lap.raw();
        lap.register_move(&sf, Point::new(2, 0), Point::new(2, 3));
        assert_eq!(lap.raw(), before);
    }

    #[test]
    fn register_move_ac7_empty_gate_is_a_no_op_without_panic() {
        // AC7: a degenerate empty gate leaves counter unchanged and does not
        // panic.
        let sf = StartFinish {
            chord: vec![],
            orient: Orient::Horizontal,
            gate: TimingGate {
                behind: vec![],
                forward: Side::East,
            },
        };
        let mut lap = LapCounter::new();
        lap.register_move(&sf, Point::new(0, 0), Point::new(5, 0));
        assert_eq!(lap.raw(), -1);
    }
}

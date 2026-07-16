//! The deterministic simulation core (design doc §3a).
//!
//! Pure and deterministic under a fixed seed, with no I/O. This is the shared
//! engine that the renderer and AI training both drive — the technical guarantee
//! that "bots play the same game" as the player.

use crate::geom::{Corridor, Point, supercover};
use crate::track::{RaceDir, StartFinish};
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

    /// Registers the move segment `from → to` against the S/F chord: `+1` for a
    /// forward crossing (along `race_dir`), `−1` for a reverse one, at most one
    /// event per move. Scored *before* collision resolution — teleports never
    /// touch the counter.
    ///
    /// TODO(3a): the signed segment/chord crossing test.
    pub fn register_move(
        &mut self,
        _sf: &StartFinish,
        _race_dir: RaceDir,
        _from: Point,
        _to: Point,
    ) {
        todo!("signed S/F crossing (design doc §3)")
    }
}

/// Crash resolution — a wall collision, arising as a search dead-end where all 5
/// moves leave `D` (design doc §3, marked **\[OPEN\]**).
///
/// Leaning rule: zero the into-wall velocity component, keep the along-wall
/// component with strong damping (e.g. `/2`), respawn at the last valid cell —
/// so a crash never yields a controlled `v = 0` (which would make "brake by
/// crashing" the dominant strategy). Fail-safe: if every move from the respawn
/// state is illegal, damp again, down to `v = 0` in the limit.
///
/// TODO(3a): finalize once the crash rule is settled (open question).
pub fn resolve_crash(_d: &Corridor, _s: CarState) -> CarState {
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
}

//! The deterministic simulation core (design doc §3a).
//!
//! Pure and deterministic under a fixed seed, with no I/O. This is the shared
//! engine that the renderer and AI training both drive — the technical guarantee
//! that "bots play the same game" as the player.

use crate::geom::{Corridor, Point, supercover};
use crate::track::{RaceDir, StartFinish};

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
    pub const fn pos(self) -> Point {
        Point::new(self.x, self.y)
    }
}

/// The 5 von-Neumann acceleration actions (design doc §3): `(0,0)`, `(±1,0)`,
/// `(0,±1)`.
///
/// Diagonal acceleration in a single turn is forbidden — this is the
/// foundation of every braking-distance argument in the design.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
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
    let (vx2, vy2) = (s.vx + ax, s.vy + ay);
    let p1 = Point::new(s.x + vx2, s.y + vy2);
    if !d.contains(p1) {
        return false;
    }
    supercover(s.pos(), p1).iter().all(|&c| d.contains(c))
}

/// The legal-action mask for `s`, in [`Action::ALL`] order. Consumed by the
/// player UI, the AI policy (as the pre-softmax `−inf` mask), and the oracle.
pub fn legal_mask(d: &Corridor, s: CarState) -> [bool; 5] {
    Action::ALL.map(|a| legal_move(d, s, a))
}

/// Advance one car by one (assumed-legal) action, returning the new state.
///
/// TODO(3a): apply `(vx',vy') = v + accel`, then `pos += (vx',vy')`.
pub fn step(_d: &Corridor, _s: CarState, _a: Action) -> CarState {
    todo!("step (design doc §3)")
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
    pub fn new() -> Self {
        Self::default()
    }

    /// Completed laps.
    pub fn laps(&self) -> i32 {
        self.counter.max(0)
    }

    /// Raw signed crossing count (may be negative before the race starts).
    pub const fn raw(&self) -> i32 {
        self.counter
    }

    /// Register the move segment `from → to` against the S/F chord: `+1` for a
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

/// Resolve several cars occupying the same point (design doc §3) — a layer
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

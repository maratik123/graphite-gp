//! # gp-core — Block 3a: pure physics core
//!
//! The deterministic, I/O-free heart of the game (design doc §3a). It is the
//! **shared dependency** of the renderer (block 2) and AI training (block 4):
//! bots and the player run *the exact same* physics, by construction.
//!
//! ## The duality invariant (design doc §0)
//!
//! A **point** is the center of a unit cell (integer coordinates); a **wall** is
//! a dual edge on the half-grid (the boundary between a drivable and a
//! non-drivable cell). Every module reads geometry through this duality, from
//! which "a wall never passes through a point", "a car never touches a wall", and
//! the correctness of legality masks all follow *by construction*.
//!
//! ## Modules
//! - [`geom`]  — dual-grid primitives: points, walls, the corridor `D`, supercover.
//! - [`rng`]   — grouped seeded-RNG configuration (`Seeds`), shared by every
//!   consumer (collision, generation, AI learning/inference).
//! - [`track`] — the exported track artifact (contract with block 1).
//! - [`sim`]   — the deterministic simulation: `step`, `legal_move`, lap counter,
//!   crash, car-collision resolution.

pub mod geom;
pub mod rng;
pub mod sim;
pub mod track;

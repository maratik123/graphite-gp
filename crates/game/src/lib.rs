//! `gp-game` library surface (design `2026-07-25-game-controller-player` § Q4).
//!
//! `crates/game`'s runnable binary (`src/main.rs`) keeps its own,
//! independent `mod config;` subtree and does **not** `use gp_game::…` — the
//! controller abstraction below is a #43-facing seam, proven by test today
//! (issue #42), not yet consumed in-binary (owner ruling R2-Q2: `main.rs`
//! behaviour is out of this task's scope). The lib target exists so the
//! seam's public items are reachable from the crate root — a bin-only crate
//! would hard-error every `pub` item here under `dead_code` with
//! `-D warnings`, since nothing reachable from `fn main` names them yet.
pub mod controller;

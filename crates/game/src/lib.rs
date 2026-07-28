//! `gp-game` library surface (design `2026-07-25-game-controller-player` §
//! Q4; widened by design `2026-07-28-game-loop-orchestration` A2).
//!
//! `crates/game`'s runnable binary (`src/main.rs`) is now a thin CLI-parse →
//! dispatch → `run_native` shim over this lib target: [`config`] (CLI
//! parsing + validated [`config::GameConfig`]) and [`app`] (the
//! `eframe::App` glue) both live here, reached from the bin as
//! `gp_game::config::…` / `gp_game::app::…`. The lib target also carries the
//! #42 controller seam ([`controller`]) and the game-loop-orchestration
//! modules ([`race`], [`replay`], [`gen_worker`]) — [`race`]/[`replay`]/
//! [`gen_worker`]/[`app::session`] are empty skeletons as of A2, populated
//! by later subtasks (A3, A6, A7, A8).
pub mod app;
pub mod config;
pub mod controller;
pub mod gen_worker;
pub mod race;
pub mod replay;
#[cfg(test)]
mod test_fixtures;

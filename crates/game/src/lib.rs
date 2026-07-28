//! `gp-game` library surface (design `2026-07-25-game-controller-player` §
//! Q4; widened by design `2026-07-28-game-loop-orchestration`).
//!
//! `crates/game`'s runnable binary (`src/main.rs`) is a thin CLI-parse →
//! dispatch → `run_native` shim over this lib target: [`config`] (CLI
//! parsing + validated [`config::GameConfig`]) and [`app`] (the real
//! `eframe::App` glue, [`app::GraphiteGpApp`], backed by
//! [`app::session::GameSession`]) both live here, reached from the bin as
//! `gp_game::config::…` / `gp_game::app::…`. The lib target also carries the
//! #42 controller seam ([`controller`]) and the game-loop-orchestration
//! modules: [`race`] (per-race state, the turn/round loop, standings),
//! [`replay`] (in-memory record/recorder/driver), and [`gen_worker`] (the
//! background generation worker).
pub mod app;
pub mod config;
pub mod controller;
pub mod gen_worker;
pub mod race;
pub mod replay;
#[cfg(test)]
mod test_fixtures;

#[cfg(test)]
mod tests {
    /// AC23 — a player-only roster: the manifest declares no `gp-ai`
    /// dependency.
    #[test]
    fn cargo_toml_declares_no_gp_ai_dependency() {
        let manifest = include_str!("../Cargo.toml");
        assert!(
            !manifest.contains("gp-ai"),
            "Cargo.toml must not declare a gp-ai dependency (AC23, player-only roster)"
        );
    }

    /// AC23 — a roster of `m` `PlayerController` seats runs the AC18
    /// sequence end-to-end (the shared drive already builds its roster
    /// entirely from real `PlayerController` seats, never a stub).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline on a worker thread — a \
                  multi-second integer sweep whose interpreted wall-clock is \
                  prohibitive"
    )]
    fn player_only_roster_runs_the_end_to_end_sequence() {
        let config = crate::app::session::tests::ac18_config();
        let cars = config.race.cars;
        let (session, _shell, roster) = crate::app::session::tests::drive_ac18_sequence(config);
        assert_eq!(roster.len(), cars as usize);
        assert!(session.race_outcome().is_some());
    }
}

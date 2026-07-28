//! The real `eframe::App` glue (issue #43, A9).
//!
//! Replaces the hand-built fixture wiring (A2) with
//! [`session::GameSession`]-backed data. Every fixture constructor A2
//! relocated here is deleted (AC24, verified by this module's own
//! structural scan test): rendered data comes from generation and the live
//! race, never a hand-built stand-in.

pub mod session;

use eframe::egui;
use gp_core::sim::CarState;
use gp_render::screens::{RaceSummary, StandingEntry};
use gp_render::widgets::CarKind;
use gp_render::{AppShell, CarRender, Nav, ShellSession, TrackView};
use session::GameSession;

use crate::controller::player::PlayerController;
use crate::controller::{FrameInput, Roster, keys};

/// The real app shell, driven by a live [`GameSession`] (AC18, AC23, AC24)
/// — a player-only roster (`m` [`PlayerController`] seats, hot-seat on one
/// screen, AC23) end-to-end from generation to Results.
pub struct GraphiteGpApp {
    /// The router owning `Screen`/`RaceConfig`/`Overlays`/`has_generated`.
    shell: AppShell,
    /// The session owning generation/race/replay state.
    session: GameSession,
    /// The current race's roster — `m` `PlayerController` seats, rebuilt
    /// whenever a fresh race starts (`Nav::TestLap`/`Nav::Again`).
    roster: Roster,
    /// Whether `Screen::Results` has already been reached for the current
    /// race, so the automatic race-end → `Nav::Finish` transition
    /// (design § *The loop is a per-frame state machine*) fires exactly
    /// once, not every frame after the race ends.
    finished_transitioned: bool,
}

impl GraphiteGpApp {
    /// Builds the app shell over a fresh, empty [`GameSession`], seeded by
    /// the CLI-derived `config` (issue #41).
    #[must_use]
    pub fn new(config: crate::config::GameConfig) -> Self {
        Self {
            shell: AppShell::new(config.race),
            session: GameSession::new(config),
            roster: Roster::new(),
            finished_transitioned: false,
        }
    }

    /// Rebuilds `self.roster` to exactly the current race's seated car
    /// count, all `PlayerController` seats (AC23 — no `gp-ai` edge).
    fn rebuild_roster(&mut self) {
        let seated = self.session.race().map_or(0, |race| race.cars.len());
        self.roster = Roster::new();
        for _ in 0..seated {
            self.roster.push(Box::new(PlayerController));
        }
    }

    /// This frame's [`FrameInput`] — `shell_action` forwarded from
    /// `ShellResponse::action` (only non-`None` on `Screen::Race`), and
    /// `key_action` read via [`keys::keyboard_action`]. Passes
    /// `Actions::all()` (no masking here): [`PlayerController::decide`]
    /// masks against the REAL legal mask `RaceRound::advance` supplies —
    /// masking twice would be redundant, not incorrect.
    fn frame_input(ui: &egui::Ui, shell_action: Option<gp_core::sim::Action>) -> FrameInput {
        let key_action = ui.input(|input| {
            keys::keyboard_action(gp_core::sim::Actions::all(), |key| input.key_pressed(key))
        });
        FrameInput {
            shell_action,
            key_action,
        }
    }

    /// Assembles this frame's `ShellSession` render inputs from the
    /// current race (if any) — cars, the active seat, laps done, and the
    /// rank-ordered standings/summary (converted from [`RaceOutcome`]'s
    /// native `u32` turn counts to today's `f32` shape; D3 deletes these
    /// casts once `results.rs` itself moves to turn-count labels).
    #[allow(
        clippy::cast_precision_loss,
        reason = "turn counts are realistically tiny relative to f32's 24-bit \
                  exact-integer range; a temporary A9 boundary cast, deleted by D3"
    )]
    fn results_view(&self) -> (Vec<StandingEntry>, RaceSummary) {
        let Some(outcome) = self.session.race_outcome() else {
            return (
                Vec::new(),
                RaceSummary {
                    fastest_lap: 0.0,
                    tempo: 0.0,
                    crashes: 0,
                },
            );
        };
        let standings = outcome
            .standings
            .iter()
            .map(|entry| StandingEntry {
                car_index: entry.car_index,
                kind: CarKind::You,
                rank: entry.rank,
                finish_time: entry.finish_turn.map_or(0.0, |turn| turn as f32),
            })
            .collect();
        let summary = RaceSummary {
            fastest_lap: outcome.fastest_lap as f32,
            tempo: outcome.tempo,
            crashes: outcome.crashes,
        };
        (standings, summary)
    }
}

impl eframe::App for GraphiteGpApp {
    // Not `update` — `eframe` 0.35's `App` trait has no such method; `ui` is
    // the required call, `logic` is the optional pre-paint hook (unused here).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.session.poll_generation();
        if self.session.landed().is_none() {
            // Scope 6 — the UI keeps painting and requests repaints while a
            // generation job is in flight.
            ui.ctx().request_repaint();
        }

        let track_view = self
            .session
            .landed()
            .map_or(TrackView::Pending, |(track, geometry)| TrackView::Ready {
                track,
                geometry,
            });

        let cars: Vec<CarState> = self
            .session
            .race()
            .map(|race| race.cars.iter().map(|car| car.state).collect())
            .unwrap_or_default();
        let trails: Vec<&[gp_core::geom::Point]> = self
            .session
            .race()
            .map(|race| race.cars.iter().map(|car| car.trail.as_slice()).collect())
            .unwrap_or_default();
        let car_renders: Vec<CarRender<'_>> = cars
            .iter()
            .zip(&trails)
            .enumerate()
            .map(|(index, (&state, trail))| CarRender::new(state, index, trail, true, 0.0))
            .collect();

        let active = self
            .session
            .round()
            .map_or(0, crate::race::round::RaceRound::cursor);
        let laps_done = self
            .session
            .race()
            .and_then(|race| race.cars.get(active))
            .map_or(0, |car| car.laps.laps());
        let total_laps = i32::try_from(self.session.config().race.laps).unwrap_or(i32::MAX);

        let (standings, summary) = self.results_view();

        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            reason = "display-only Lab header seed, pre-D1 (still i32); D1 \
                      widens ShellSession::seed to u64 and removes this cast"
        )]
        let seed = self
            .session
            .installed_generation_seed()
            .map_or(0, |seed| seed as i32);

        let session_view = ShellSession {
            track: track_view,
            setup_error: self.session.setup_error(),
            cars: &car_renders,
            reduced_motion: false,
            active,
            laps_done,
            total_laps,
            phases: self.session.phases(),
            valid: true,
            seed,
            standings: &standings,
            summary,
        };

        let response = self.shell.show(ui, session_view);
        let frame_input = Self::frame_input(ui, response.action);

        if let Some(nav) = response.nav {
            self.session.on_nav(nav);
            if matches!(nav, Nav::TestLap | Nav::Again) {
                self.rebuild_roster();
                self.finished_transitioned = false;
            }
        }

        let _ = self.session.advance_race(&mut self.roster, frame_input);

        let race_over = self
            .session
            .round()
            .is_some_and(crate::race::round::RaceRound::is_race_over);
        if race_over && !self.finished_transitioned {
            self.shell.apply(Nav::Finish);
            self.finished_transitioned = true;
        }
    }
}

#[cfg(test)]
mod tests {
    /// AC24 — a structural scan: none of the deleted fixture identifiers
    /// appear in `main.rs` or `app/mod.rs` production source (mirrors
    /// `controller::tests::controller_module_calls_no_physics`'s
    /// `include_str!` idiom).
    #[test]
    fn no_fixture_identifiers_remain_in_main_or_app_mod() {
        // Strips the `#[cfg(test)]` region and every doc/comment/`use`
        // line, mirroring `controller::tests::controller_module_calls_no_physics`'s
        // `code_lines` idiom — this test's OWN source names every forbidden
        // identifier (in this array literal and its doc comment), which
        // would otherwise trip on itself.
        fn code_lines(src: &str) -> String {
            let mut out = String::new();
            for line in src.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("#[cfg(test)]") {
                    break;
                }
                if trimmed.starts_with("///")
                    || trimmed.starts_with("//!")
                    || trimmed.starts_with("//")
                {
                    continue;
                }
                if trimmed.starts_with("use ") {
                    continue;
                }
                out.push_str(line);
                out.push('\n');
            }
            out
        }

        let haystack = code_lines(include_str!("../main.rs")) + &code_lines(include_str!("mod.rs"));

        for forbidden in [
            "fixture_track",
            "fixture_cars",
            "fixture_standings",
            "FIXTURE_SEED",
            "FIXTURE_CAR_COUNT",
        ] {
            assert!(
                !haystack.contains(forbidden),
                "main.rs/app/mod.rs production code still contains {forbidden} — AC24 requires it deleted"
            );
        }
    }

    /// AC24 — `--cars` is honoured up to grid capacity (closing #41's known
    /// inconsistency): a short 3-position grid seats `min(cars,
    /// positions.len())`; a full 6-position-worth request against a
    /// 4-position grid ([`crate::test_fixtures::ring_track`]'s own default)
    /// seats exactly the grid's capacity, never the raw `--cars` value.
    #[test]
    fn cars_is_honoured_up_to_grid_capacity() {
        use crate::race::RaceState;
        use crate::test_fixtures::{ring_track, short_grid_track};
        use gp_render::BakedTrackGeometry;

        let short = short_grid_track();
        let geometry = BakedTrackGeometry::new(&short);
        let race = RaceState::new(short, geometry, 6, 0);
        assert_eq!(
            race.seated(),
            3,
            "3-position grid must seat exactly 3, not 6"
        );

        let full = ring_track();
        let geometry = BakedTrackGeometry::new(&full);
        let race = RaceState::new(full, geometry, 6, 0);
        assert_eq!(
            race.seated(),
            4,
            "4-position grid must seat exactly 4, not 6"
        );
    }
}

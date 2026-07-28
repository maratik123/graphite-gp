//! The real `eframe::App` glue (issue #43, A9; `AppMode`/playback, C5).
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
use gp_render::{AppShell, CarRender, Nav, SeatedGrid, ShellSession, TrackView};
use session::GameSession;

use crate::controller::player::PlayerController;
use crate::controller::{FrameInput, Roster, keys};
use crate::race::standings::RaceOutcome;
use crate::replay::playback::{PLAYBACK_TURN_INTERVAL, PlaybackDriver};

/// Which of the two loops [`GraphiteGpApp`] is currently driving.
///
/// C5, spec § Playback pacing: `Interactive` is today's
/// `GameSession`-backed race (unchanged); `Playback` drives a
/// [`PlaybackDriver`] one turn per [`PLAYBACK_TURN_INTERVAL`] instead,
/// with no controller input accepted (transport controls —
/// pause/step/scrub/speed — are explicitly out of scope).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    /// The ordinary `GameSession`-driven interactive race.
    Interactive,
    /// A `--replay-mode gui` playback (spec § Replay CLI).
    Playback,
}

/// The real app shell, driven by a live [`GameSession`].
///
/// AC18, AC23, AC24 — a player-only roster (`m` [`PlayerController`]
/// seats, hot-seat on one screen, AC23) end-to-end from generation to
/// Results. `--replay-mode gui` (C5) instead drives a [`PlaybackDriver`],
/// with [`AppMode`] selecting which loop [`eframe::App::ui`] advances this
/// frame.
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
    /// Which loop this frame drives (C5).
    mode: AppMode,
    /// The playback driver, `Some` only in [`AppMode::Playback`].
    playback: Option<PlaybackDriver>,
}

impl GraphiteGpApp {
    /// Builds the app shell over a fresh, empty [`GameSession`], seeded by
    /// the CLI-derived `config` (issue #41). When `config.replay` is `Some`
    /// and `config.replay_mode` is `Gui` (C5), attempts to load that
    /// persisted record into a [`PlaybackDriver`] and starts in
    /// [`AppMode::Playback`] instead — a load failure (unreadable file,
    /// malformed record, track regeneration failure) is reported to stderr
    /// and falls back to the ordinary [`AppMode::Interactive`] session
    /// (there is no Setup-screen error slot to route a *pre-window* load
    /// failure into, unlike a live `GenerationFailure`, which `poll_generation`
    /// surfaces mid-session).
    #[must_use]
    pub fn new(config: crate::config::GameConfig) -> Self {
        if let (Some(path), crate::config::ReplayMode::Gui) = (&config.replay, config.replay_mode) {
            match Self::load_playback(path) {
                Ok(playback) => {
                    let mut shell = AppShell::new(config.race);
                    shell.apply(Nav::Generate);
                    shell.apply(Nav::TestLap);
                    return Self {
                        shell,
                        session: GameSession::new(config),
                        roster: Roster::new(),
                        finished_transitioned: false,
                        mode: AppMode::Playback,
                        playback: Some(playback),
                    };
                }
                Err(message) => {
                    use std::io::Write as _;
                    let _ = writeln!(std::io::stderr(), "graphite-gp: {message}");
                }
            }
        }

        Self {
            shell: AppShell::new(config.race),
            session: GameSession::new(config),
            roster: Roster::new(),
            finished_transitioned: false,
            mode: AppMode::Interactive,
            playback: None,
        }
    }

    /// Reads and parses `path`, then builds a [`PlaybackDriver`] over it
    /// (regenerating the track once, synchronously — the same one-time cost
    /// `PlaybackDriver::new` itself documents).
    fn load_playback(path: &std::path::Path) -> Result<PlaybackDriver, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read --replay file {}: {err}", path.display()))?;
        let (file_config, record) =
            crate::replay::format::parse_record(&text).map_err(|err| err.to_string())?;
        PlaybackDriver::new(&file_config, &record).map_err(|err| err.to_string())
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
    /// current race (if any) — the interactive path's own `outcome`
    /// source, `session.race_outcome()`. Delegates to
    /// [`standings_and_summary`], shared with [`Self::ui_playback`] (C5).
    fn results_view(&self) -> (Vec<StandingEntry>, RaceSummary) {
        standings_and_summary(self.session.race_outcome())
    }

    /// [`AppMode::Playback`]'s per-frame body (C5): ticks the
    /// [`PlaybackDriver`] at most once per [`PLAYBACK_TURN_INTERVAL`],
    /// renders the SAME `Race`/`Results` screens the interactive path uses
    /// (sourced from the driver instead of [`GameSession`]), and accepts
    /// no controller input — transport controls (pause/step/scrub/speed)
    /// are explicitly out of scope (spec § Playback pacing).
    fn ui_playback(&mut self, ui: &mut egui::Ui) {
        // `request_repaint_after`, not a bare `request_repaint` (unlike the
        // interactive pending-generation path): playback has a known next
        // event time, so it need not busy-repaint every frame.
        ui.ctx().request_repaint_after(PLAYBACK_TURN_INTERVAL);

        let Some(playback) = self.playback.as_mut() else {
            return;
        };
        let _ = playback.tick(std::time::Instant::now());

        let race = playback.race();
        let round = playback.round();

        let cars: Vec<CarState> = race.cars.iter().map(|car| car.state).collect();
        let trails: Vec<&[gp_core::geom::Point]> =
            race.cars.iter().map(|car| car.trail.as_slice()).collect();
        let car_renders: Vec<CarRender<'_>> = cars
            .iter()
            .zip(&trails)
            .enumerate()
            .map(|(index, (&state, trail))| CarRender::new(state, index, trail, true, 0.0))
            .collect();

        let active = round.cursor();
        let laps_done = race.cars.get(active).map_or(0, |car| car.laps.laps());
        let total_laps = round.total_laps();

        let outcome = RaceOutcome::from_race(race, round.crashes());
        let (standings, summary) = standings_and_summary(Some(outcome));

        let session_view = ShellSession {
            track: TrackView::Ready {
                track: &race.track,
                geometry: &race.geometry,
            },
            setup_error: None,
            cars: &car_renders,
            reduced_motion: false,
            active,
            laps_done,
            total_laps,
            // Playback regenerates its track synchronously, once, before
            // this loop ever starts (`Self::load_playback`) — there is no
            // in-flight generation to observe, so every phase reads as the
            // A9-era interim placeholder the interactive path used before
            // B3 wired the real worker aggregate.
            phases: [gp_render::PhaseStatus::Ok; 7],
            valid: true,
            // Not wired: `PlaybackDriver` does not carry the file's
            // `master`/generation seed for display. A known, cosmetic gap
            // (the Lab header's `seed <N>` tag would read `0`), not a
            // functional one — playback never re-generates or re-derives
            // anything from this value. Flagged as a follow-up alongside
            // the transport controls § Playback pacing already defers.
            seed: 0,
            // Not wired, same gap as `seed` above: `PlaybackDriver` does not
            // carry the file's originally-requested `--cars` count, only
            // the already-seated roster — so there is no `requested` to
            // compare against. Cosmetic only (playback never re-seats).
            seated: None,
            standings: &standings,
            summary,
        };

        let _ = self.shell.show(ui, session_view);

        let race_over = round.is_race_over();
        if race_over && !self.finished_transitioned {
            self.shell.apply(Nav::Finish);
            self.finished_transitioned = true;
        }
    }
}

/// Converts a [`RaceOutcome`] (native `u32` turn counts) into the
/// `gp-render` boundary types, which (issue #43 D3) speak the same native
/// `u32`/`Option<u32>` turn-count shape — shared by
/// [`GraphiteGpApp::results_view`] (interactive) and
/// [`GraphiteGpApp::ui_playback`] (C5). `None` (no race started/finished
/// yet) renders as an empty, all-zero result.
fn standings_and_summary(outcome: Option<RaceOutcome>) -> (Vec<StandingEntry>, RaceSummary) {
    let Some(outcome) = outcome else {
        return (
            Vec::new(),
            RaceSummary {
                fastest_lap: 0,
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
            finish_turn: entry.finish_turn,
        })
        .collect();
    let summary = RaceSummary {
        fastest_lap: outcome.fastest_lap,
        tempo: outcome.tempo,
        crashes: outcome.crashes,
    };
    (standings, summary)
}

impl eframe::App for GraphiteGpApp {
    // Not `update` — `eframe` 0.35's `App` trait has no such method; `ui` is
    // the required call, `logic` is the optional pre-paint hook (unused here).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.mode == AppMode::Playback {
            self.ui_playback(ui);
            return;
        }

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

        let seed = self.session.installed_generation_seed().unwrap_or(0);

        let requested = self.session.config().race.cars;
        let seated = self.session.race().map(|race| SeatedGrid {
            seated: u32::try_from(race.seated()).unwrap_or(u32::MAX),
            requested,
        });

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
            seated,
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
            self.session.write_record_if_requested();
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

    /// AC10's `gp-game`-side half — a structural scan mirroring
    /// [`no_fixture_identifiers_remain_in_main_or_app_mod`]'s `code_lines`
    /// idiom: no production (non-`#[cfg(test)]`) `gp-game` source constructs
    /// a `TrackArtifact` struct literal. `gp-game` only ever *receives* a
    /// `TrackArtifact` — from `crate::gen_worker::Worker::poll` (real generation) or
    /// `GameSession::landed`/`spawn_race` (pass-through) — never builds a
    /// placeholder one itself; the only in-crate construction sites are the
    /// `#[cfg(test)]`-gated `controller::tests::fixture_track` and the
    /// `#![cfg(test)]` `test_fixtures` module, neither of which reaches this
    /// scan (this test's own doc comment names the forbidden pattern, hence
    /// reusing the comment-stripping `code_lines` idiom rather than a naive
    /// `str::contains` over `include_str!`).
    #[test]
    fn no_placeholder_track_artifact_constructed_in_production_gp_game() {
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

        let haystack = code_lines(include_str!("../main.rs"))
            + &code_lines(include_str!("mod.rs"))
            + &code_lines(include_str!("session.rs"))
            + &code_lines(include_str!("../race/mod.rs"))
            + &code_lines(include_str!("../gen_worker.rs"))
            + &code_lines(include_str!("../replay/mod.rs"))
            + &code_lines(include_str!("../controller/mod.rs"));

        assert!(
            !haystack.contains("TrackArtifact {"),
            "production gp-game source constructs a `TrackArtifact` struct \
             literal — AC10 requires every artifact to come from generation, \
             never a placeholder built in gp-game"
        );
    }

    /// AC22's GUI-mode half — `format::unrecognised_version_is_rejected`
    /// (`crates/game/src/replay/format.rs`) and `tests/replay.rs`'s
    /// `unrecognised_version_exits_nonzero` both only exercise the headless
    /// path (`--replay-mode headless`); this closes the GUI path,
    /// `GraphiteGpApp::load_playback`, which reads the file and calls
    /// `parse_record` before `PlaybackDriver::new` ever runs — so this test
    /// pays no `gp_gen::generate` cost, unlike a genuine playback-load test
    /// would.
    #[test]
    fn load_playback_rejects_an_unrecognised_version() {
        struct ScratchFile(std::path::PathBuf);
        impl ScratchFile {
            fn new(name: &str) -> Self {
                Self(std::env::temp_dir().join(format!(
                    "gp-game-app-mod-test-{}-{name}.replay",
                    std::process::id()
                )))
            }
        }
        impl Drop for ScratchFile {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }

        let scratch = ScratchFile::new("bad-version-gui");
        std::fs::write(&scratch.0, "graphite-gp-replay 2\nmaster 1\n")
            .expect("failed to write the scratch replay file");

        // `Result::expect_err` needs `Debug` on the `Ok` type
        // (`PlaybackDriver`), which no other test-only assertion requires —
        // matching the `Err` arm directly avoids that (AGENTS.md § Rust
        // Test Conventions — never add a production `#[derive(Debug)]` for
        // a test-only assertion).
        match super::GraphiteGpApp::load_playback(&scratch.0) {
            Err(message) => assert!(
                message.contains("version"),
                "error message must name the version problem: {message}"
            ),
            Ok(_) => panic!("an unrecognised version must be rejected"),
        }
    }
}

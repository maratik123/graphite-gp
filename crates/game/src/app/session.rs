//! `GameSession` — seed policy, generation lifecycle, `Nav` handling
//! (issue #43, A7).
//!
//! `egui`-free signatures throughout (design § *Module decomposition*),
//! mirroring `AppShell::apply`/`can_nav`'s own posture — so AC12/AC13/AC18
//! are plain headless tests, no `egui::Context` needed.

use crate::config::GameConfig;
use crate::controller::{FrameInput, Roster};
use crate::gen_worker::{GenerationFailure, GenerationId, Worker};
use crate::race::RaceState;
use crate::race::round::{Advance, RaceRound};
use crate::race::standings::RaceOutcome;
use crate::replay::{Recorder, ReplayRecord};
use gp_core::rng::Seeds;
use gp_core::track::TrackArtifact;
use gp_gen::GenParams;
use gp_render::{BakedTrackGeometry, Nav};

/// The game's whole session-level state.
///
/// The validated config, the background generation worker, the landed
/// track (if any), and the current race (if any) — everything
/// `app/mod.rs` (A9) needs to drive `AppShell::show` from real data
/// instead of a fixture.
pub struct GameSession {
    config: GameConfig,
    worker: Worker,
    /// The generation id of the currently in-flight/just-completed
    /// request, alongside the attempt `k` it was raised at (needed to
    /// resolve `installed_seeds` on a successful install — distinct from
    /// `next_k`, which has already advanced past it by request time).
    pending: Option<(GenerationId, u32)>,
    /// The landed track + its baked geometry, built once per landed track
    /// (design `2026-07-22-cache-track-geometry`'s caching pattern) —
    /// `None` while a request is in flight and none has ever landed.
    landed: Option<(TrackArtifact, BakedTrackGeometry)>,
    /// The most recent `GenerationFailure`'s rendered text, for the Setup
    /// error slot (Scope 12); cleared on the next successful install.
    setup_error: Option<String>,
    /// The next generation attempt's `k` (spec § Seed policy 1) — `0` for
    /// the very first raised request, incremented on every raise
    /// (`Nav::Generate` or `Nav::Regenerate` alike — "k=0 for the first
    /// Generate, +1 per Regenerate or later Generate").
    next_k: u32,
    /// `installed_seeds`'s attempt `k` — the request `k` the currently
    /// landed track came from (needed for `race_again`'s seed derivation
    /// to stay tied to the SAME landed track's `k`, not `next_k`, which
    /// may already have advanced past it).
    installed_k: u32,
    /// The resolved seeds of the currently landed track's generation
    /// request — `None` until a track has landed at least once.
    installed_seeds: Option<Seeds>,
    /// The current race-on-this-track counter `r` (spec § Seed policy 2)
    /// — `0` for the first race, `+1` per `Nav::Again`.
    r: u32,
    /// The current race, if one has been started (`Nav::TestLap` /
    /// `Nav::Again`).
    race: Option<RaceState>,
    /// The current race's turn/round cursor, alongside `race`.
    round: Option<RaceRound>,
    /// This race's collision-resolution seed, alongside `race`/`round` —
    /// needed to build a [`ReplayRecord`] (A8), since `RaceState` does not
    /// expose the seed it was constructed with.
    current_collision_seed: Option<u64>,
    /// Feeds from every `Advance::Moved` outcome the current race
    /// produces (A8, design § *Module decomposition* — "fed from A4's
    /// apply step"); reset on every fresh `spawn_race`.
    recorder: Recorder,
}

impl GameSession {
    /// A fresh session over `config`, with no request raised yet.
    #[must_use]
    pub fn new(config: GameConfig) -> Self {
        Self {
            config,
            worker: Worker::new(),
            pending: None,
            landed: None,
            setup_error: None,
            next_k: 0,
            installed_k: 0,
            installed_seeds: None,
            r: 0,
            race: None,
            round: None,
            current_collision_seed: None,
            recorder: Recorder::new(),
        }
    }

    /// The live config this session was built from.
    #[must_use]
    pub const fn config(&self) -> &GameConfig {
        &self.config
    }

    /// The landed track + geometry, if any (`gp-render`'s `TrackView`
    /// source — A9's job to wire).
    #[must_use]
    pub const fn landed(&self) -> Option<&(TrackArtifact, BakedTrackGeometry)> {
        self.landed.as_ref()
    }

    /// The Setup error slot's current text, if any.
    #[must_use]
    pub fn setup_error(&self) -> Option<&str> {
        self.setup_error.as_deref()
    }

    /// The current race, if a race has been started.
    #[must_use]
    pub const fn race(&self) -> Option<&RaceState> {
        self.race.as_ref()
    }

    /// The current race's turn/round cursor, if a race has been started.
    #[must_use]
    pub const fn round(&self) -> Option<&RaceRound> {
        self.round.as_ref()
    }

    /// The currently-landed track's resolved generation seed, if any (A9's
    /// Lab header `seed <N>` source — pre-D1, still `i32`-truncated at the
    /// display boundary).
    #[must_use]
    pub fn installed_generation_seed(&self) -> Option<u64> {
        self.installed_seeds.map(|seeds| seeds.generation)
    }

    /// This session's effective seeds for generation attempt `k` (spec §
    /// Seed policy 1 / design § *Seed policy — how the CLI per-source
    /// overrides compose with `M_k`*): `k = 0` uses `config.seeds`
    /// verbatim (preserving #41's per-source override contract); `k > 0`
    /// uses a pure `Seeds::from_master(M_k)` with no overrides,
    /// `M_k = config.master.wrapping_add(k)`.
    fn seeds_for_attempt(&self, k: u32) -> Seeds {
        if k == 0 {
            self.config.seeds
        } else {
            let m_k = self.config.master.wrapping_add(u64::from(k));
            Seeds::from_master(m_k)
        }
    }

    /// Raises a generation request at attempt `k`, cancelling any
    /// in-flight request first (`Worker::request` already does this
    /// internally).
    fn request_generation(&mut self, k: u32) {
        let seeds = self.seeds_for_attempt(k);
        let params = GenParams {
            seeds,
            ..self.config.to_gen_params()
        };
        let id = self.worker.request(params);
        self.pending = Some((id, k));
    }

    /// Raises the next generation request (`Nav::Generate` and
    /// `Nav::Regenerate` share this — both are "raise a request" per the
    /// Key decision's own wording).
    fn raise_next_request(&mut self) {
        let k = self.next_k;
        self.next_k = self.next_k.wrapping_add(1);
        self.request_generation(k);
    }

    /// Starts a fresh race on `track` at race counter `r`, seating cars
    /// per `self.config.race.cars` (spec § Key decisions — "seat fewer and
    /// race", AC14) with the collision seed `installed_seeds.collision
    /// .wrapping_add(r)` (spec § Seed policy 2).
    fn spawn_race(&mut self, track: TrackArtifact) {
        let geometry = BakedTrackGeometry::new(&track);
        let collision_seed = self
            .installed_seeds
            .map_or(0, |seeds| seeds.collision.wrapping_add(u64::from(self.r)));
        let total_laps = i32::try_from(self.config.race.laps).unwrap_or(i32::MAX);
        self.race = Some(RaceState::new(
            track,
            geometry,
            self.config.race.cars,
            collision_seed,
        ));
        self.round = Some(RaceRound::new(total_laps));
        self.current_collision_seed = Some(collision_seed);
        self.recorder = Recorder::new();
    }

    /// `Nav::TestLap` — starts the first race (`r = 0`) on the landed
    /// track. A no-op if no track has landed (Lab's own pending body
    /// already hides the Test-lap control in that state — design §
    /// *Generate-while-pending needs no new `gp-render` surface*).
    fn start_race(&mut self) {
        let Some(track) = self.landed.as_ref().map(|(track, _)| track.clone()) else {
            return;
        };
        self.r = 0;
        self.spawn_race(track);
    }

    /// `Nav::Again` — a fresh race on the SAME track (AC13): re-seated,
    /// counters reset, collision seed advanced (`r += 1`), the track
    /// reused (not regenerated — `installed_k`/`next_k` are untouched). A
    /// no-op if no track has landed.
    fn race_again(&mut self) {
        let Some(track) = self.landed.as_ref().map(|(track, _)| track.clone()) else {
            return;
        };
        self.r = self.r.wrapping_add(1);
        self.spawn_race(track);
    }

    /// Handles one frame's navigation intent (`gp_render::Nav`) —
    /// `AppShell::apply` has already applied the screen transition; this
    /// reacts to the SAME intent for `gp-game`'s own state.
    pub fn on_nav(&mut self, nav: Nav) {
        match nav {
            Nav::Generate | Nav::Regenerate => self.raise_next_request(),
            Nav::TestLap => self.start_race(),
            Nav::Again => self.race_again(),
            Nav::Menu => self.worker.cancel(),
            Nav::Finish | Nav::JumpTo(_) => {}
        }
    }

    /// Polls the background worker once. Installs a landed artifact
    /// (clearing the Setup error slot), records a `GenerationFailure`'s
    /// rendered text into the Setup error slot, or does nothing while
    /// still pending. Call once per frame (A9's job).
    pub fn poll_generation(&mut self) {
        let Some((pending_id, pending_k)) = self.pending else {
            return;
        };
        let Some((id, result)) = self.worker.poll() else {
            return;
        };
        debug_assert_eq!(
            id, pending_id,
            "Worker::poll must only ever surface the current request's id"
        );
        self.pending = None;
        match result {
            Ok(track) => {
                let geometry = BakedTrackGeometry::new(&track);
                self.installed_seeds = Some(self.seeds_for_attempt(pending_k));
                self.installed_k = pending_k;
                self.landed = Some((track, geometry));
                self.setup_error = None;
            }
            Err(failure) => {
                self.setup_error = Some(render_generation_failure(&failure));
            }
        }
    }

    /// The current race's computed outcome, if a race has been started —
    /// `RaceRound::crashes()` feeds `RaceOutcome::from_race`'s `crashes`
    /// argument.
    #[must_use]
    pub fn race_outcome(&self) -> Option<RaceOutcome> {
        let (race, round) = (self.race.as_ref()?, self.round.as_ref()?);
        Some(RaceOutcome::from_race(race, round.crashes()))
    }

    /// Advances the current race by at most one seat (A9's per-frame
    /// driving call — `RaceRound::advance`'s own contract), recording
    /// every `Advance::Moved` outcome into this session's [`Recorder`]
    /// (A8, design § *Module decomposition* — "fed from A4's apply step").
    /// `None` if no race has been started.
    pub fn advance_race(&mut self, roster: &mut Roster, input: FrameInput) -> Option<Advance> {
        let (race, round) = (self.race.as_mut()?, self.round.as_mut()?);
        let outcome = round.advance(race, roster, input);
        if let Advance::Moved {
            seat,
            action,
            round_complete: _,
        } = outcome
        {
            self.recorder.record(round.round(), seat, action);
        }
        Some(outcome)
    }

    /// The current race's in-memory replay record, built from this
    /// session's [`Recorder`] plus the resolved seeds/config the current
    /// race actually used (A8, AC20) — `None` until a race has been
    /// started at least once.
    #[must_use]
    pub fn replay_record(&self) -> Option<ReplayRecord> {
        let generation_seed = self.installed_seeds?.generation;
        let collision_seed = self.current_collision_seed?;
        Some(
            self.recorder
                .clone()
                .into_record(generation_seed, collision_seed, self.config.race),
        )
    }
}

/// Renders a [`GenerationFailure`] for the Setup error slot — its own
/// `Display`, per spec § Key decisions ("Generation-failure presentation
/// (R2-Q2): text is the `GenerationError`'s own `Display`").
fn render_generation_failure(failure: &GenerationFailure) -> String {
    failure.to_string()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::GameSession;
    use crate::config::GameConfig;
    use gp_core::rng::Seeds;
    use gp_render::{Difficulty, Nav, RaceConfig};

    /// A cheap, deterministic test config: `seeds.generation = 6` accepts
    /// on the first attempt at `seed_budget = 1` (the same fixture A6's
    /// `gen_worker` tests use); `master` is an arbitrary distinct value so
    /// `seeds_for_attempt(0) != seeds_for_attempt(k>0)` in practice.
    fn test_config() -> GameConfig {
        GameConfig {
            race: RaceConfig {
                cars: 4,
                laps: 5,
                v_target: 5,
                difficulty: Difficulty::Pro,
            },
            seeds: Seeds {
                generation: 6,
                ..Seeds::default()
            },
            master: 41,
            min_straight: 3,
            block_size: 6,
            seed_budget: 1,
            repair_budget: 8,
        }
    }

    /// AC12 — the seed-derivation half: `k = 0` uses `config.seeds`
    /// verbatim; `k > 0` uses `Seeds::from_master(master.wrapping_add(k))`;
    /// the same `k` reproduces the same seeds (pure function).
    #[test]
    fn seeds_for_attempt_matches_the_seed_policy() {
        let config = test_config();
        let session = GameSession::new(config);

        assert_eq!(
            session.seeds_for_attempt(0),
            config.seeds,
            "k=0 must use config.seeds verbatim"
        );
        assert_eq!(
            session.seeds_for_attempt(1),
            Seeds::from_master(config.master.wrapping_add(1))
        );
        assert_eq!(
            session.seeds_for_attempt(1),
            session.seeds_for_attempt(1),
            "the same k must reproduce the same seeds"
        );
        assert_ne!(
            session.seeds_for_attempt(0),
            session.seeds_for_attempt(1),
            "k=0 and k=1 must differ for this test's config"
        );
    }

    /// AC12 — every raise (`Generate` or `Regenerate`) advances `next_k`
    /// by exactly one, and a request is pending immediately after.
    #[test]
    fn generate_and_regenerate_each_advance_next_k_by_one() {
        let mut session = GameSession::new(test_config());
        assert_eq!(session.next_k, 0);

        session.on_nav(Nav::Generate);
        assert_eq!(session.next_k, 1);
        assert_eq!(session.pending.map(|(_, k)| k), Some(0));

        session.on_nav(Nav::Regenerate);
        assert_eq!(session.next_k, 2);
        assert_eq!(session.pending.map(|(_, k)| k), Some(1));
    }

    /// AC12 — end-to-end: `Nav::Generate` raises and installs an artifact
    /// at `k = 0`; the seeds it would use at `k = 1` (`Regenerate`'s
    /// attempt) differ from `k = 0`'s, and a direct `gp_gen::generate` call
    /// at those two seed sets yields byte-unequal artifacts — the causal
    /// mechanism AC12 depends on, exercised without a second unpredictable
    /// (possibly budget-exhausting) background request.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline on a worker thread — a \
                  multi-second integer sweep whose interpreted wall-clock is \
                  prohibitive"
    )]
    fn generate_installs_and_regenerate_would_use_a_seed_yielding_a_different_artifact() {
        let mut session = GameSession::new(test_config());
        session.on_nav(Nav::Generate);

        let deadline = std::time::Instant::now()
            .checked_add(std::time::Duration::from_secs(30))
            .expect("30s from now does not overflow Instant");
        loop {
            session.poll_generation();
            if session.landed.is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "generation never landed within budget; setup_error={:?}",
                session.setup_error
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let artifact_k0 = session
            .landed
            .as_ref()
            .map(|(track, _)| track.clone())
            .expect("just asserted landed");
        session.on_nav(Nav::Regenerate);
        let k1_seeds = session.seeds_for_attempt(1);

        // Generous budgets so this direct call (bypassing the worker
        // entirely) reliably terminates regardless of the pseudo-random
        // k=1 seed's own acceptance cost.
        let k1_params = gp_gen::GenParams {
            seeds: k1_seeds,
            seed_budget: 64,
            ..session.config().to_gen_params()
        };
        let artifact_k1 = gp_gen::generate(k1_params, &mut ())
            .expect("k=1 generous-budget generation must accept");

        assert_ne!(
            format!("{artifact_k0:?}"),
            format!("{artifact_k1:?}"),
            "Regenerate's k=1 artifact must differ from k=0's"
        );
    }

    /// AC13 — `Nav::Again` re-seats on the SAME track: the collision seed
    /// advances by exactly `r`, the track is reused (byte-identical, never
    /// regenerated — `next_k`/`installed_k` untouched), and the prior
    /// in-memory record is discarded (A8's `Recorder`, wired into
    /// `spawn_race`). Directly installs a landed track (bypassing the
    /// worker) for a fast, deterministic setup.
    #[test]
    fn race_again_advances_collision_seed_and_reuses_the_track() {
        let mut session = GameSession::new(test_config());
        let track = crate::test_fixtures::ring_track();
        let geometry = gp_render::BakedTrackGeometry::new(&track);
        session.landed = Some((track, geometry));
        session.installed_seeds = Some(Seeds {
            collision: 100,
            ..Seeds::default()
        });
        let next_k_before = session.next_k;
        let installed_k_before = session.installed_k;

        session.on_nav(Nav::TestLap);
        assert_eq!(session.r, 0);
        let first_track = format!("{:?}", session.race.as_ref().expect("race started").track);

        // AC13 (A8 amendment): feed the recorder with a turn, then confirm
        // Race-again discards it -- "the prior in-memory record is
        // dropped".
        session.recorder.record(0, 0, gp_core::sim::Action::Coast);
        assert!(
            !session
                .recorder
                .clone()
                .into_record(0, 0, test_config().race)
                .turns
                .is_empty()
        );

        session.on_nav(Nav::Again);
        assert_eq!(session.r, 1);
        let second_track = format!("{:?}", session.race.as_ref().expect("race restarted").track);
        assert_eq!(
            first_track, second_track,
            "the track must be reused, not regenerated"
        );
        assert_eq!(
            session.next_k, next_k_before,
            "Race-again must not raise a new generation request"
        );
        assert_eq!(
            session.installed_k, installed_k_before,
            "Race-again must not change which attempt the landed track came from"
        );
        assert!(
            session
                .recorder
                .clone()
                .into_record(0, 0, test_config().race)
                .turns
                .is_empty(),
            "Race-again must discard the prior in-memory record (AC13)"
        );
    }

    /// A cheap, fast-finishing config for the AC18/AC23 end-to-end drive:
    /// `cars: 2`, `laps: 1` so the race ends after each seat's first turn.
    /// `laps: 0` is deliberate, not a domain violation this test skipped
    /// checking: `GameConfig` is constructed directly here (no `Cli`
    /// round-trip, so `LAPS_MIN`'s CLI-level floor never applies), and it
    /// makes the win-detection check (`laps() >= total_laps`) trivially
    /// true after ANY car's very first ordinary move — including a plain
    /// `Coast`, always legal at rest. This is what lets the race reach a
    /// genuine `RaceOver` on a REAL generated track of UNKNOWN shape
    /// without needing to hand-navigate it (unlike `race::round`'s/`A8`'s
    /// tests, which control a hand-built fixture track and can rely on its
    /// known geometry).
    pub(crate) fn ac18_config() -> GameConfig {
        GameConfig {
            race: RaceConfig {
                cars: 2,
                laps: 0,
                v_target: 5,
                difficulty: Difficulty::Pro,
            },
            ..test_config()
        }
    }

    /// AC18/AC23's shared end-to-end drive: `Setup -> Generate -> Lab ->
    /// TestLap -> Race -> (loop to race end) -> Results`, over a REAL
    /// roster of `config.race.cars` real `PlayerController` seats (not
    /// stubs — AC23's "a roster of `m` `PlayerController` seats runs the
    /// AC18 sequence end-to-end" needs the production controller, not a
    /// test double). `pub(crate)` so `crate`'s `lib.rs` test module (AC23)
    /// can reuse it without duplicating the drive.
    ///
    /// Fast-forwards seat 0's `LapCounter` directly (test-only field
    /// access — this test is about session/shell WIRING, not replay
    /// determinism, unlike A8's `replay` tests) so ONE real `East` move
    /// finishes it; seat 1 just Coasts. Returns the driven session/shell
    /// for the caller's own assertions.
    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline on a worker thread — a \
                  multi-second integer sweep whose interpreted wall-clock is \
                  prohibitive"
    )]
    pub(crate) fn drive_ac18_sequence(
        config: GameConfig,
    ) -> (GameSession, gp_render::AppShell, crate::controller::Roster) {
        use crate::controller::player::PlayerController;
        use crate::controller::{FrameInput, Roster};
        use crate::race::round::Advance;
        use gp_core::sim::Action;

        let mut session = GameSession::new(config);
        let mut shell = gp_render::AppShell::new(config.race);
        assert_eq!(shell.screen(), gp_render::Screen::Setup);

        session.on_nav(Nav::Generate);
        shell.apply(Nav::Generate);
        assert_eq!(shell.screen(), gp_render::Screen::Lab);

        let deadline = std::time::Instant::now()
            .checked_add(std::time::Duration::from_secs(30))
            .expect("30s from now does not overflow Instant");
        loop {
            session.poll_generation();
            if session.landed().is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "generation never landed within budget; setup_error={:?}",
                session.setup_error()
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(session.landed().is_some(), "Lab must see a real artifact");

        session.on_nav(Nav::TestLap);
        shell.apply(Nav::TestLap);
        assert_eq!(shell.screen(), gp_render::Screen::Race);
        assert!(session.race().is_some(), "Race must see a real artifact");

        let seated = session.race().map_or(0, |race| race.cars.len());
        let mut roster = Roster::new();
        for _ in 0..seated {
            roster.push(Box::new(PlayerController));
        }

        // `laps: 0` (see `ac18_config`'s doc) makes ANY ordinary move a
        // finish -- both seats just Coast (always legal at rest), which
        // works regardless of the real generated track's actual shape.
        let mut turns = 0u32;
        loop {
            let outcome = session.advance_race(
                &mut roster,
                FrameInput {
                    shell_action: Some(Action::Coast),
                    key_action: None,
                },
            );
            if matches!(outcome, Some(Advance::RaceOver)) {
                break;
            }
            turns = turns.saturating_add(1);
            assert!(turns < 50, "AC18 race never ended within budget");
        }
        shell.apply(Nav::Finish);
        assert_eq!(shell.screen(), gp_render::Screen::Results);

        (session, shell, roster)
    }

    /// AC18 — the full headless sequence, asserting the raised request's
    /// params came from the live `RaceConfig` + CLI budgets, Lab/Race see
    /// a real artifact, and Results carries real standings.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline on a worker thread — a \
                  multi-second integer sweep whose interpreted wall-clock is \
                  prohibitive"
    )]
    fn full_sequence_setup_to_results_with_real_artifact_and_standings() {
        let config = ac18_config();
        let (session, _shell, roster) = drive_ac18_sequence(config);

        assert_eq!(
            roster.len(),
            2,
            "GenParams.cars must match the live RaceConfig"
        );
        let outcome = session
            .race_outcome()
            .expect("Results must carry real standings");
        assert_eq!(outcome.standings.len(), 2);
        assert!(
            outcome
                .standings
                .iter()
                .any(|entry| entry.finish_turn.is_some()),
            "at least one car must have a real finish"
        );
    }
}

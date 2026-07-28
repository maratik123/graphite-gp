//! The headless replay runner + GUI playback driver (issue #43, C4/C5,
//! spec § Replay CLI / § Playback pacing, design § *How AC21's record is
//! produced* / *Playback interval — 250 ms*).
//!
//! [`run_headless_race`] is the **one** entry point that drives a race
//! without a window — the same function both a producing caller (a
//! `FirstLegal` seat building a fresh record, `AC21`'s own integration
//! test) and `--replay-mode headless` (`ReplayController` seats fed from a
//! parsed persisted file) drive, differing only in which
//! [`crate::controller::Controller`]s populate the [`Roster`]. It is
//! *defined* here and *re-exported* from `replay::mod` as
//! `pub use playback::run_headless_race;`, so its single public path is
//! `gp_game::replay::run_headless_race`.
//!
//! [`PlaybackClock`]/[`PlaybackDriver`] (C5) are the `--replay-mode gui`
//! counterpart: the same [`RaceRound`] loop over the same
//! [`ReplayController`] seats, but gated one turn per real-time interval
//! instead of run-to-completion — the advance predicate
//! ([`PlaybackClock::tick`]) is pure and takes `now` as a parameter, so
//! `AC21c` tests it headlessly with a synthetic clock: no sleeping, no
//! `egui::Context`, no Miri gate.

use crate::config::GameConfig;
use crate::controller::{FrameInput, Roster};
use crate::race::RaceState;
use crate::race::round::{Advance, RaceRound};
use crate::race::standings::RaceOutcome;
use crate::replay::format::{self, ReplayError};
use crate::replay::{FinalCarState, Recorder, ReplayController, ReplayRecord};
use gp_render::BakedTrackGeometry;
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Errors [`run_headless_race`] can fail with — kept separate from
/// [`ReplayError`] (a parse failure over file *text*, not a race-driving
/// failure).
#[derive(Debug, Error)]
pub enum HeadlessError {
    /// The regenerated track never accepted (spec § Replay format — the
    /// track is regenerated from `seed-generation` plus the persisted
    /// `GenParams`-completing fields, never itself persisted).
    #[error(transparent)]
    Generation(#[from] gp_gen::GenerationError),
    /// A seat gave no answer. Headless mode never waits for input (unlike
    /// the interactive GUI race), so a [`crate::controller::Controller`]
    /// returning `None` can only mean a [`ReplayController`] exhausted its
    /// recorded stream or was handed an action outside the current legal
    /// mask — design § *Replay format*'s divergence layers (a)/(b).
    #[error("replay diverged: a seat had no legal recorded action left")]
    Diverged,
}

/// Drives a fresh race headlessly: regenerates the track from `config`,
/// seats `roster`, and advances the round loop until the race ends or
/// `max_turns` processed turns have elapsed.
///
/// `max_turns` is a REQUIRED bound, not a convenience (design § *How
/// AC21's record is produced*): a roster is not guaranteed to reach
/// `RaceOver` within any given call (a `FirstLegal` pilot on an unknown
/// generated track, or a diverging `ReplayController`), and a headless run
/// must always terminate rather than spin.
///
/// # Errors
/// [`HeadlessError::Generation`] if the regenerated track never accepts;
/// [`HeadlessError::Diverged`] if any seat gives no answer (headless mode
/// never retries — an immediate, terminal signal, not a stall-then-give-up
/// loop).
pub fn run_headless_race(
    config: &GameConfig,
    mut roster: Roster,
    max_turns: u32,
) -> Result<(RaceOutcome, ReplayRecord), HeadlessError> {
    let track = gp_gen::generate(config.to_gen_params(), &mut ())?;
    let geometry = BakedTrackGeometry::new(&track);
    let mut race = RaceState::new(track, geometry, config.race.cars, config.seeds.collision);
    let mut round = RaceRound::new(config.total_laps());
    let mut recorder = Recorder::new();

    let mut processed = 0u32;
    while processed < max_turns {
        // Captured BEFORE `advance` — see `GameSession::advance_race`'s own
        // comment: a round-wrapping `Moved` has already incremented
        // `round.round()` by the time `advance` returns.
        let round_before = round.round();
        match round.advance(&mut race, &mut roster, FrameInput::default()) {
            Advance::RaceOver => break,
            Advance::Moved { seat, action, .. } => {
                recorder.record(round_before, seat, action);
                processed = processed.saturating_add(1);
            }
            Advance::Crashed { .. } => {
                recorder.record_crash();
                processed = processed.saturating_add(1);
            }
            // Headless mode never waits for a "not yet" answer — the loop
            // is not interactive, so a controller with nothing to say has
            // genuinely diverged.
            Advance::Pending => return Err(HeadlessError::Diverged),
        }
    }

    let finals: Vec<FinalCarState> = race
        .cars
        .iter()
        .enumerate()
        .map(|(seat, car)| FinalCarState {
            seat,
            state: car.state,
            lap_raw: car.laps.raw(),
        })
        .collect();
    let outcome = RaceOutcome::from_race(&race, round.crashes());
    let record = recorder.into_record(
        config.seeds.generation,
        config.seeds.collision,
        config.race,
        finals,
    );

    Ok((outcome, record))
}

/// Runs the whole `--replay <PATH> --replay-mode headless` flow (C4).
///
/// `main.rs`'s dispatch: reads and parses `path`, drives
/// [`run_headless_race`] through [`ReplayController`] seats built from the
/// parsed record, checks the recomputed final states against the file's
/// own `final` lines (design § *Replay format*'s divergence layer (c)),
/// prints the final standings, and returns a process exit code — `0` on a
/// clean round-trip, non-zero on any read/parse/divergence failure.
#[must_use]
pub fn run_headless_replay_from_file(path: &Path) -> i32 {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            report_error(&format!(
                "failed to read --replay file {}: {err}",
                path.display()
            ));
            return 1;
        }
    };

    let (config, file_record) = match format::parse_record(&text) {
        Ok(pair) => pair,
        Err(err) => {
            report_replay_error(&err);
            return 1;
        }
    };

    let mut roster = Roster::new();
    for seat in 0..file_record.finals.len() {
        roster.push(Box::new(ReplayController::for_seat(&file_record, seat)));
    }

    // The EXACT recorded processed-turn count, not a generous guess: a
    // record that stopped at an external turn cap (rather than
    // `RaceOver`) must replay to the SAME point and stop cleanly. A
    // larger bound would run the `ReplayController`s dry past the last
    // recorded turn and register a false divergence; `turns.len()` alone
    // would under-play a record that ever crashed (see
    // `ReplayRecord::total_processed_turns`'s own doc).
    let max_turns = file_record.total_processed_turns;
    let (outcome, driven_record) = match run_headless_race(&config, roster, max_turns) {
        Ok(pair) => pair,
        Err(err) => {
            report_error(&format!("replay diverged: {err}"));
            return 1;
        }
    };

    if !finals_agree(&file_record.finals, &driven_record.finals) {
        report_error(
            "replay diverged: the recomputed end state disagrees with the recorded `final` lines",
        );
        return 1;
    }

    for entry in &outcome.standings {
        let finish = entry
            .finish_turn
            .map_or_else(|| "-".to_string(), |turn| turn.to_string());
        let _ = writeln!(
            std::io::stdout(),
            "graphite-gp: rank {} car {} finish_turn {finish}",
            entry.rank,
            entry.car_index,
        );
    }
    0
}

/// Whether `expected`/`actual` agree on every seat's final state, order
/// notwithstanding (both are seat-indexed, but not guaranteed to be
/// produced in the same order).
fn finals_agree(expected: &[FinalCarState], actual: &[FinalCarState]) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    let mut expected_sorted = expected.to_vec();
    let mut actual_sorted = actual.to_vec();
    expected_sorted.sort_by_key(|f| f.seat);
    actual_sorted.sort_by_key(|f| f.seat);
    expected_sorted == actual_sorted
}

/// Writes one `graphite-gp: <message>` line to stderr — `writeln!`, not
/// `eprintln!`, to avoid a new panic-on-broken-pipe production path
/// (`main.rs`'s own precedent).
fn report_error(message: &str) {
    let _ = writeln!(std::io::stderr(), "graphite-gp: {message}");
}

/// [`report_error`] specialised for a [`ReplayError`] (its own `Display`).
fn report_replay_error(err: &ReplayError) {
    report_error(&err.to_string());
}

/// The GUI playback advance interval: one turn every 250 ms.
///
/// Design KD2, spec § Playback pacing — 4 turns/second. A representative
/// race is ~10 turns/lap × 5 laps × 4 seats ≈ 200 turns ≈ 50 s of
/// playback — long enough to follow individual moves, short enough to
/// watch end-to-end. Flagged (design § Playback pacing) as the natural
/// first follow-up to tune if it proves too fast or too slow; transport
/// controls (pause / step / scrub / speed) stay explicitly out of scope.
pub const PLAYBACK_TURN_INTERVAL: Duration = Duration::from_millis(250);

/// A pure "has [`PLAYBACK_TURN_INTERVAL`] elapsed" predicate (`AC21c`).
///
/// Takes `now` as a parameter rather than reading `Instant::now()`
/// internally, so it is testable headlessly with a synthetic clock: no
/// sleeping, no `egui::Context`, no Miri gate.
#[derive(Debug, Default)]
pub struct PlaybackClock {
    /// The instant of the last tick that returned `true`, or the instant
    /// of construction/first tick if none has yet.
    last: Option<Instant>,
}

impl PlaybackClock {
    /// A fresh clock. Its very first [`Self::tick`] call always seeds the
    /// reference instant and returns `false` — nothing has elapsed yet
    /// relative to a clock that has never ticked.
    #[must_use]
    pub const fn new() -> Self {
        Self { last: None }
    }

    /// `true` iff at least [`PLAYBACK_TURN_INTERVAL`] has elapsed since
    /// the last `true` (or since construction, for the first call) —
    /// advances at most once per call, and never fires twice for the same
    /// `now` (the immediately following call at an unchanged `now` has
    /// zero elapsed time, which is `< PLAYBACK_TURN_INTERVAL`).
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some(last) = self.last else {
            self.last = Some(now);
            return false;
        };
        if now.saturating_duration_since(last) >= PLAYBACK_TURN_INTERVAL {
            self.last = Some(now);
            true
        } else {
            false
        }
    }
}

/// Drives a persisted [`ReplayRecord`] on screen, one turn per
/// [`PlaybackClock`] tick.
///
/// `AC21c` — the GUI counterpart to [`run_headless_race`]'s
/// run-to-completion loop, over the exact same [`RaceRound`] machinery and
/// [`ReplayController`] seats.
///
/// Regenerates the track once, at construction — the same one-time cost
/// [`run_headless_race`] and `GameSession`'s own background worker both
/// pay, here paid synchronously since playback has no interactive
/// Setup/Lab window to cover it. A background-worker playback load is a
/// natural follow-up (same footing as the transport controls § Playback
/// pacing already flags as out of scope for this pass).
pub struct PlaybackDriver {
    race: RaceState,
    round: RaceRound,
    roster: Roster,
    clock: PlaybackClock,
}

impl PlaybackDriver {
    /// Builds a fresh driver over `record`, regenerating its track from
    /// `config`.
    ///
    /// # Errors
    /// [`HeadlessError::Generation`] if the regenerated track never
    /// accepts.
    pub fn new(config: &GameConfig, record: &ReplayRecord) -> Result<Self, HeadlessError> {
        let track = gp_gen::generate(config.to_gen_params(), &mut ())?;
        let geometry = BakedTrackGeometry::new(&track);
        let mut roster = Roster::new();
        for seat in 0..record.finals.len() {
            roster.push(Box::new(ReplayController::for_seat(record, seat)));
        }
        let race = RaceState::new(track, geometry, config.race.cars, config.seeds.collision);
        let round = RaceRound::new(config.total_laps());
        Ok(Self {
            race,
            round,
            roster,
            clock: PlaybackClock::new(),
        })
    }

    /// Advances at most one turn, and only if [`PlaybackClock::tick`]
    /// fires for `now` — [`Advance::Pending`] on every "not yet" frame,
    /// mirroring [`RaceRound::advance`]'s own "ask again next frame"
    /// contract (so a caller's per-frame call site needs no branching
    /// between the two).
    pub fn tick(&mut self, now: Instant) -> Advance {
        if !self.clock.tick(now) {
            return Advance::Pending;
        }
        self.round
            .advance(&mut self.race, &mut self.roster, FrameInput::default())
    }

    /// The current race state, for rendering (cars, trails, track).
    #[must_use]
    pub const fn race(&self) -> &RaceState {
        &self.race
    }

    /// The current turn/round cursor, for rendering (active seat, laps
    /// done) and race-over detection.
    #[must_use]
    pub const fn round(&self) -> &RaceRound {
        &self.round
    }
}

#[cfg(test)]
mod tests {
    use super::{HeadlessError, run_headless_race};
    use crate::config::GameConfig;
    use crate::controller::{Controller, PollContext, Roster};
    use gp_core::rng::Seeds;
    use gp_core::sim::Action;
    use gp_render::{Difficulty, RaceConfig};

    /// A cheap, deterministic config: `seeds.generation = 6` accepts on the
    /// first attempt at `seed_budget = 1` (the same fixture A6/A7/B2 use).
    fn cheap_config() -> GameConfig {
        GameConfig {
            race: RaceConfig {
                cars: 2,
                laps: 1,
                v_target: 5,
                difficulty: Difficulty::Pro,
            },
            seeds: Seeds {
                generation: 6,
                collision: 0,
                ai_learning: 0,
                ai_inference: 0,
            },
            master: 6,
            min_straight: 3,
            block_size: 6,
            seed_budget: 1,
            repair_budget: 8,
            record: None,
            replay: None,
            replay_mode: crate::config::ReplayMode::Gui,
        }
    }

    /// Always answers `Coast` (legal at rest on any track — the cheapest
    /// possible seat that never diverges and never finishes, so the race
    /// runs out its `max_turns` budget deterministically).
    struct AlwaysCoast;

    impl Controller for AlwaysCoast {
        fn poll(&mut self, ctx: PollContext<'_>) -> Option<Action> {
            ctx.legal.contains(Action::Coast).then_some(Action::Coast)
        }
    }

    /// A seat that never answers — headless mode must treat this as an
    /// immediate divergence, never spin waiting.
    struct NeverAnswers;

    impl Controller for NeverAnswers {
        fn poll(&mut self, _ctx: PollContext<'_>) -> Option<Action> {
            None
        }
    }

    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline — a multi-second integer \
                  sweep whose interpreted wall-clock is prohibitive"
    )]
    #[test]
    fn run_headless_race_regenerates_and_drives_to_the_turn_budget() {
        let config = cheap_config();
        let mut roster = Roster::new();
        roster.push(Box::new(AlwaysCoast));
        roster.push(Box::new(AlwaysCoast));

        let (outcome, record) =
            run_headless_race(&config, roster, 4).expect("cheap config must accept");
        assert_eq!(record.turns.len(), 4);
        assert_eq!(record.finals.len(), 2);
        assert_eq!(outcome.standings.len(), 2);
    }

    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline — a multi-second integer \
                  sweep whose interpreted wall-clock is prohibitive"
    )]
    #[test]
    fn recorded_turns_pin_the_round_of_every_seat_including_the_last_of_each_round() {
        // Pins the persisted `round` coordinate for a multi-round race: two
        // `AlwaysCoast` seats, six turns = three full rounds. Each seat's
        // `round.round()` must be captured BEFORE `RaceRound::advance` runs
        // -- the last seat polled in a round also wraps the cursor and
        // increments `round.round()` INSIDE that same `advance` call, so
        // recording the round number AFTER `advance` returns would persist
        // that seat under the NEXT round's number (self-review Round 1
        // finding 6).
        let config = cheap_config();
        let mut roster = Roster::new();
        roster.push(Box::new(AlwaysCoast));
        roster.push(Box::new(AlwaysCoast));

        let (_, record) = run_headless_race(&config, roster, 6).expect("cheap config must accept");

        let rounds: Vec<u32> = record.turns.iter().map(|turn| turn.round).collect();
        assert_eq!(
            rounds,
            vec![0, 0, 1, 1, 2, 2],
            "both seats of each round must share that round's number -- the \
             LAST seat of a round must never be persisted under round+1"
        );
        let seats: Vec<usize> = record.turns.iter().map(|turn| turn.seat).collect();
        assert_eq!(seats, vec![0, 1, 0, 1, 0, 1]);
    }

    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline — a multi-second integer \
                  sweep whose interpreted wall-clock is prohibitive"
    )]
    #[test]
    fn run_headless_race_reports_diverged_when_a_seat_never_answers() {
        let config = cheap_config();
        let mut roster = Roster::new();
        roster.push(Box::new(NeverAnswers));
        roster.push(Box::new(AlwaysCoast));

        let result = run_headless_race(&config, roster, 4);
        assert!(matches!(result, Err(HeadlessError::Diverged)));
    }

    // ---- AC21c: PlaybackClock ----

    #[test]
    fn playback_clock_ticks_true_exactly_once_per_elapsed_interval() {
        use super::{PLAYBACK_TURN_INTERVAL, PlaybackClock};
        use std::time::{Duration, Instant};

        let t0 = Instant::now();
        let mut clock = PlaybackClock::new();

        assert!(
            !clock.tick(t0),
            "the first tick only seeds the reference instant"
        );
        let just_short = (t0 + PLAYBACK_TURN_INTERVAL)
            .checked_sub(Duration::from_millis(1))
            .expect("t0 + interval - 1ms must not underflow Instant");
        assert!(
            !clock.tick(just_short),
            "1 ms short of the interval must not fire"
        );
        assert!(
            clock.tick(t0 + PLAYBACK_TURN_INTERVAL),
            "exactly the interval elapsed must fire"
        );
        assert!(
            !clock.tick(t0 + PLAYBACK_TURN_INTERVAL),
            "the immediately following tick at the SAME instant must not fire again"
        );
    }

    #[test]
    fn playback_clock_fires_again_after_a_second_full_interval() {
        use super::{PLAYBACK_TURN_INTERVAL, PlaybackClock};
        use std::time::Instant;

        let t0 = Instant::now();
        let mut clock = PlaybackClock::new();
        assert!(!clock.tick(t0));
        assert!(clock.tick(t0 + PLAYBACK_TURN_INTERVAL));
        assert!(clock.tick(t0 + PLAYBACK_TURN_INTERVAL * 2));
    }

    // ---- AC21c: PlaybackDriver reaches the same final state as the
    // headless runner ----

    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline (twice: once to produce the \
                  record, once to regenerate for playback) — a multi-second integer \
                  sweep whose interpreted wall-clock is prohibitive"
    )]
    #[test]
    fn playback_driver_reaches_the_same_final_state_as_the_headless_runner() {
        use super::{PLAYBACK_TURN_INTERVAL, PlaybackDriver};
        use std::time::Instant;

        let config = cheap_config();
        let mut roster = Roster::new();
        roster.push(Box::new(AlwaysCoast));
        roster.push(Box::new(AlwaysCoast));
        let (_, record) = run_headless_race(&config, roster, 4).expect("cheap config must accept");

        let mut driver = PlaybackDriver::new(&config, &record).expect("cheap config must accept");
        let mut now = Instant::now();
        // One extra tick beyond `total_processed_turns` to exercise the
        // "nothing left to advance" tail without asserting on it.
        for _ in 0..=record.total_processed_turns {
            now += PLAYBACK_TURN_INTERVAL;
            let _ = driver.tick(now);
        }

        let driven_finals: Vec<_> = driver
            .race()
            .cars
            .iter()
            .enumerate()
            .map(|(seat, car)| crate::replay::FinalCarState {
                seat,
                state: car.state,
                lap_raw: car.laps.raw(),
            })
            .collect();
        assert_eq!(driven_finals, record.finals);
        assert_eq!(driver.round().crashes(), 0);
    }
}

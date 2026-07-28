//! Cross-process replay round-trip tests (issue #43, C4, `AC21`/`AC21b`),
//! plus the Design Amendment 1 divergence-layer discrimination cases.
//!
//! A hand-built record cannot be shown divergence-free: the headless
//! replay **regenerates** the track from `seed-generation`, and the
//! divergence checks require every recorded action to be a member of that
//! *regenerated* track's live legal mask. The record here therefore comes
//! from a REAL race on the regenerated track, driven through the same
//! [`gp_game::replay::run_headless_race`] entry point `--replay-mode
//! headless` itself runs (design § *How `AC21`'s record is produced*).
//!
//! **REQUIRED (design § Risks "Replay wall-clock"):** the record — and the
//! poll-aligned observation log the (b) tamper case needs (design §
//! *`AC21` tamper construction*) — are produced exactly ONCE per test
//! binary, via [`std::sync::LazyLock`], not once per `#[test]`. Every test
//! copies the shared text into its own [`ScratchFile`] instead of calling
//! `run_headless_race` itself.

use gp_core::sim::{Action, Actions};
use gp_game::config::{GameConfig, ReplayMode};
use gp_game::controller::{Controller, PollContext, Roster};
use gp_game::replay::format::write_record;
use gp_game::replay::run_headless_race;
use gp_render::{Difficulty, RaceConfig};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::sync::LazyLock;
use strum::VariantArray;

/// A cheap, deterministic config: `seeds.generation = 6` accepts on the
/// first attempt at `seed_budget = 1` (the same fixture every other Group
/// A/B/C subtask's cheap tests use).
const fn cheap_config() -> GameConfig {
    GameConfig {
        race: RaceConfig {
            cars: 2,
            laps: 1,
            v_target: 5,
            difficulty: Difficulty::Pro,
        },
        seeds: gp_core::rng::Seeds {
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
        replay_mode: ReplayMode::Gui,
    }
}

/// A test-local seat: picks the first action of [`Action::VARIANTS`] that
/// is in `ctx.legal`, starting the scan at `turn_index % 5` rather than
/// index `0` (design § *How `AC21`'s record is produced*, prescribing
/// `Action::VARIANTS` by name). A naive "first legal in declaration order"
/// pilot picks `Action::Coast` whenever it is legal, and `Coast` at
/// `v = (0, 0)` never moves the car — the race would never advance. The
/// rotating start makes the pilot actually drive while staying fully
/// deterministic.
///
/// Also pushes `ctx.legal` onto a shared log on every poll (design §
/// *`AC21` tamper construction*) — the (b) tamper case derives its
/// out-of-mask token from this log rather than guessing.
struct FirstLegal {
    turn_index: usize,
    legal_log: Rc<RefCell<Vec<Actions>>>,
}

impl FirstLegal {
    const fn new(legal_log: Rc<RefCell<Vec<Actions>>>) -> Self {
        Self {
            turn_index: 0,
            legal_log,
        }
    }
}

impl Controller for FirstLegal {
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "Action::VARIANTS.len() is the fixed compile-time constant 5 (never 0), so \
                  both `%` operations are non-panicking by construction; \
                  `offset < Action::VARIANTS.len()` and `start < Action::VARIANTS.len()` bound \
                  their sum comfortably under usize::MAX"
    )]
    fn poll(&mut self, ctx: PollContext<'_>) -> Option<Action> {
        self.legal_log.borrow_mut().push(ctx.legal);
        let start = self.turn_index % Action::VARIANTS.len();
        self.turn_index = self.turn_index.wrapping_add(1);
        (0..Action::VARIANTS.len())
            .map(|offset| Action::VARIANTS[(start + offset) % Action::VARIANTS.len()])
            .find(|&a| ctx.legal.contains(a))
    }
}

/// This test file's fixed turn budget (design § *How `AC21`'s record is
/// produced*: "the `AC21` test passes a fixed budget").
const MAX_TURNS: u32 = 8;

/// A fresh [`Command`] for the built `graphite-gp` binary.
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_graphite-gp"))
}

/// Spawns the built binary against `path` in `--replay-mode headless`.
fn run_replay(path: &std::path::Path) -> std::process::Output {
    bin()
        .args([
            "--replay",
            path.to_str().expect("scratch path must be UTF-8"),
            "--replay-mode",
            "headless",
        ])
        .output()
        .expect("failed to spawn the built graphite-gp binary")
}

/// A process-unique scratch path under the system temp dir, with
/// best-effort cleanup on drop. A `tempfile` dependency would be overkill
/// for this file's one helper, so this hand-rolls the path instead.
struct ScratchFile(std::path::PathBuf);

impl ScratchFile {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gp-game-replay-test-{}-{name}.replay",
            std::process::id()
        ));
        Self(path)
    }
}

impl Drop for ScratchFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The one shared record production for the whole test binary (design §
/// Risks "Replay wall-clock" — REQUIRED box): the persisted record text,
/// plus the [`FirstLegal`] observation log the (b) tamper case needs,
/// produced by exactly ONE `run_headless_race` call regardless of how many
/// `#[test]`s reuse it.
struct Fixture {
    /// The record `write_record` produced — the exact text `--record`
    /// writes at race end.
    text: String,
    /// One entry per `Controller::poll` call, in order — 1:1 aligned with
    /// `text`'s `turn` lines (design § *`AC21` tamper construction*'s
    /// biconditional alignment argument: a `turn` line implies a poll
    /// since a crash turn never polls, and a poll implies a `turn` line
    /// since `FirstLegal::poll` never returns `None`).
    legal_log: Vec<Actions>,
}

static FIXTURE: LazyLock<Fixture> = LazyLock::new(|| {
    let config = cheap_config();
    let legal_log = Rc::new(RefCell::new(Vec::new()));
    let mut roster = Roster::new();
    roster.push(Box::new(FirstLegal::new(Rc::clone(&legal_log))));
    roster.push(Box::new(FirstLegal::new(Rc::clone(&legal_log))));

    let (_, record) =
        run_headless_race(&config, roster, MAX_TURNS).expect("cheap config must accept");
    let text = write_record(&config, &record);
    let legal_log = Rc::try_unwrap(legal_log)
        .expect("no other Rc clone of legal_log survives run_headless_race")
        .into_inner();
    Fixture { text, legal_log }
});

/// Writes the shared [`FIXTURE`]'s record text to `path` — every test's
/// starting point, never a fresh `run_headless_race` call of its own.
fn write_fixture_record(path: &std::path::Path) {
    std::fs::write(path, &FIXTURE.text).expect("failed to write the scratch replay file");
}

/// Parses `text`'s `turn <round> <seat> <action>` lines into
/// `(round, seat, action)` triples, in file order.
fn parse_turn_lines(text: &str) -> Vec<(u32, usize, String)> {
    text.lines()
        .filter(|line| line.starts_with("turn "))
        .map(|line| {
            let mut words = line.split_whitespace();
            words.next(); // "turn"
            let round: u32 = words
                .next()
                .expect("turn line has a round field")
                .parse()
                .expect("turn line's round field must be a valid u32");
            let seat: usize = words
                .next()
                .expect("turn line has a seat field")
                .parse()
                .expect("turn line's seat field must be a valid usize");
            let action = words
                .next()
                .expect("turn line has an action field")
                .to_string();
            (round, seat, action)
        })
        .collect()
}

/// (a1)/(a2) — rewrites the LAST `turn` line's `<round>` field to
/// `new_round`, leaving `<seat>`/`<action>` untouched.
fn bump_last_turn_round(text: &str, new_round: u32) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let last_turn_index = lines
        .iter()
        .rposition(|line| line.starts_with("turn "))
        .expect("fixture record must contain at least one turn line");
    let mut words: Vec<String> = lines[last_turn_index]
        .split_whitespace()
        .map(str::to_string)
        .collect();
    words[1] = new_round.to_string();
    lines[last_turn_index] = words.join(" ");
    lines.join("\n")
}

/// (b) — rewrites the `turn` line at `turn_index` (0-based among `turn`
/// lines, in file order — aligned with [`Fixture::legal_log`]) to carry
/// `new_action` instead, leaving `<round>`/`<seat>` untouched.
fn retoken_turn_action(text: &str, turn_index: usize, new_action: Action) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut seen = 0usize;
    for line in &mut lines {
        if line.starts_with("turn ") {
            if seen == turn_index {
                let mut words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
                *words.last_mut().expect("turn line has an action token") = new_action.to_string();
                *line = words.join(" ");
                break;
            }
            seen = seen.saturating_add(1);
        }
    }
    lines.join("\n")
}

/// (c) — end-state: rewrites the `final` line for `seat`, bumping its `x`
/// field by 1, leaving `<seat>`/`vx`/`vy`/`lap-raw` untouched. Format:
/// `final <seat> <x> <y> <vx> <vy> <lap-raw>` (`format.rs`'s
/// `write_record`/`parse_finals`).
fn tamper_final_x(text: &str, seat: usize) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for line in &mut lines {
        if line.starts_with("final ") {
            let mut words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
            let line_seat: usize = words[1].parse().expect("final line seat must be a usize");
            if line_seat == seat {
                let x: i32 = words[2].parse().expect("final line x must be an i32");
                words[2] = (x.wrapping_add(1)).to_string();
                *line = words.join(" ");
                return lines.join("\n");
            }
        }
    }
    panic!("fixture record must contain a final line for seat {seat}");
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn replay_round_trips_headless_and_prints_standings() {
    let scratch = ScratchFile::new("round-trip");
    write_fixture_record(&scratch.0);

    let output = run_replay(&scratch.0);

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rank"), "{stdout}");
    assert!(stdout.contains("car"), "{stdout}");
}

/// (a1) — structural: bumping the LAST `turn` line's `<round>` DOWN
/// violates "round non-decreasing" and is rejected at PARSE time, before
/// any `generate` call runs. Proven structurally, not by wall-clock
/// (design § *`AC21` tamper construction*, "(a1)'s parse-time proof is
/// structural, not wall-clock"): a parse failure prints the bare
/// `ReplayError` `Display` (`report_replay_error`), while every
/// post-generation failure is wrapped in `"replay diverged: "`
/// (`run_headless_replay_from_file`'s two call sites) — so asserting the
/// (a1) needle **and** the absence of `"replay diverged:"` is a structural
/// proof that no generation ran.
#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn structural_tamper_a1_is_rejected_at_parse_time_before_any_generation() {
    let scratch = ScratchFile::new("a1-structural");
    let turns = parse_turn_lines(&FIXTURE.text);
    let (last_round, _, _) = *turns.last().expect("fixture record must have turn lines");
    assert!(
        last_round > 0,
        "test precondition: the fixture must span more than one round to \
         construct a round DECREASE at its last line"
    );
    let tampered = bump_last_turn_round(&FIXTURE.text, last_round.saturating_sub(1));
    std::fs::write(&scratch.0, tampered).expect("failed to write the tampered scratch file");

    let output = run_replay(&scratch.0);

    assert!(
        !output.status.success(),
        "a structurally invalid record must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("turn sequence violation"),
        "expected the (a1) TurnSequence needle: {stderr}"
    );
    assert!(
        !stderr.contains("replay diverged:"),
        "an (a1) rejection must fire before any generation runs -- \
         \"replay diverged:\" only wraps a POST-generation failure: {stderr}"
    );
}

/// (a2) — positional: bumping the LAST `turn` line's `<round>` UP by one
/// still parses as structurally valid (design § *`AC21` tamper
/// construction*: rounds stay non-decreasing, the new round is a
/// singleton so "seat strictly increasing within a round" holds
/// trivially), but the driver's own `round_before` disagrees with the
/// recorded round at that position — layer (a2) fires. The action token
/// is untouched, so layer (b) cannot fire; the cursor is the only thing
/// left to disagree (design's own "why (a2) and (b) cannot shadow each
/// other").
#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn positional_tamper_a2_diverges_mid_replay_after_a_clean_parse() {
    let scratch = ScratchFile::new("a2-positional");
    let turns = parse_turn_lines(&FIXTURE.text);
    let (last_round, _, _) = *turns.last().expect("fixture record must have turn lines");
    let tampered = bump_last_turn_round(&FIXTURE.text, last_round.saturating_add(1));
    std::fs::write(&scratch.0, tampered).expect("failed to write the tampered scratch file");

    let output = run_replay(&scratch.0);

    assert!(
        !output.status.success(),
        "a positionally tampered record must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("recorded turn mismatch"),
        "expected the (a2) TurnMismatch needle: {stderr}"
    );
}

/// (b) — legality: retokenizes the LAST proper-subset-mask poll (scanned
/// BACKWARD and unconditionally from `FIXTURE.legal_log` — design §
/// *`AC21` tamper construction* explicitly forbids "optimising" this into
/// a forward scan or an early exit) to an action absent from that
/// recorded mask. Parses clean (round/seat untouched), so (a1)/(a2) cannot
/// fire; `RaceRound::advance`'s own mask-membership check is the only
/// thing left to disagree.
#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn legality_tamper_b_diverges_mid_replay_after_a_clean_parse() {
    let scratch = ScratchFile::new("b-legality");
    let (turn_index, legal) = FIXTURE
        .legal_log
        .iter()
        .enumerate()
        .rev()
        .find(|(_, legal)| **legal != Actions::all())
        .expect(
            "the fixture race must poll at least one proper-subset legal mask -- \
             if this ever fails, the track/seating changed and (b) needs re-deriving, \
             per design § *AC21 tamper construction*'s own `assert!`",
        );
    let out_of_mask = *Action::VARIANTS
        .iter()
        .find(|a| !legal.contains(**a))
        .expect("a proper-subset mask always excludes at least one Action variant");

    let tampered = retoken_turn_action(&FIXTURE.text, turn_index, out_of_mask);
    std::fs::write(&scratch.0, tampered).expect("failed to write the tampered scratch file");

    let output = run_replay(&scratch.0);

    assert!(
        !output.status.success(),
        "an out-of-mask recorded action must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is illegal for seat"),
        "expected the (b) IllegalRecordedAction needle: {stderr}"
    );
}

/// (c) — end-state: bumping seat 0's recorded `final` `x` by one leaves
/// every `turn` line untouched, so (a1)/(a2)/(b) all pass cleanly and the
/// full replay runs to completion — only the post-replay `final`-line
/// comparison (`playback.rs::finals_agree`) can disagree. This is the
/// **only** layer able to catch a crash/collision-induced desync: a crash
/// turn emits no `turn` line, so it is invisible to (a1)/(a2), and it
/// applies nothing so (b) never sees an out-of-mask action either (design §
/// *`AC21` tamper construction*, self-review Round 2 finding 1). Costs one
/// `generate` in the child process, on top of `FIXTURE`'s own — expected
/// and already budgeted (design § Risks' cost table).
#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn end_state_tamper_c_diverges_after_a_full_clean_replay() {
    let scratch = ScratchFile::new("c-end-state");
    let tampered = tamper_final_x(&FIXTURE.text, 0);
    std::fs::write(&scratch.0, tampered).expect("failed to write the tampered scratch file");

    let output = run_replay(&scratch.0);

    assert!(
        !output.status.success(),
        "an end-state-tampered record must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("recomputed end state disagrees"),
        "expected the (c) finals_agree needle: {stderr}"
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn unrecognised_version_exits_nonzero() {
    let scratch = ScratchFile::new("bad-version");
    write_fixture_record(&scratch.0);
    let bad_version = FIXTURE
        .text
        .replacen("graphite-gp-replay 1", "graphite-gp-replay 2", 1);
    std::fs::write(&scratch.0, bad_version).expect("failed to write the scratch replay file");

    let output = run_replay(&scratch.0);

    assert!(
        !output.status.success(),
        "an unrecognised version must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("version"), "{stderr}");
}

/// `AC21b` — the written record is valid `UTF-8` (guaranteed by
/// `write_record` returning a `String`) and the format version is
/// greppable from the raw file. Reuses [`FIXTURE`] — no additional
/// `generate` call.
#[test]
#[cfg_attr(
    miri,
    ignore = "does real filesystem I/O (write_fixture_record's std::fs::write, \
              this test's own std::fs::read) -- Miri aborts with \
              'unsupported operation: `open` not available when isolation \
              is enabled' before any run_headless_race generation cost is \
              ever reached"
)]
fn written_record_is_utf8_and_the_version_is_greppable() {
    let scratch = ScratchFile::new("ac21b");
    write_fixture_record(&scratch.0);
    let bytes = std::fs::read(&scratch.0).expect("just-written scratch file");
    let text = String::from_utf8(bytes).expect("record must be valid UTF-8");
    assert!(text.contains("graphite-gp-replay 1"), "{text}");
}

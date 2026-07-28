//! Cross-process replay round-trip tests (issue #43, C4, `AC21`/`AC21b`).
//!
//! A hand-built record cannot be shown divergence-free: the headless
//! replay **regenerates** the track from `seed-generation`, and the
//! three-layer divergence check requires every recorded action to be a
//! member of that *regenerated* track's live legal mask. The record here
//! therefore comes from a REAL race on the regenerated track, driven
//! through the same [`gp_game::replay::run_headless_race`] entry point
//! `--replay-mode headless` itself runs (design § *How `AC21`'s record is
//! produced*).

use gp_core::sim::Action;
use gp_game::config::{GameConfig, ReplayMode};
use gp_game::controller::{Controller, PollContext, Roster};
use gp_game::replay::format::write_record;
use gp_game::replay::run_headless_race;
use gp_render::{Difficulty, RaceConfig};
use std::process::Command;

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

/// Every [`Action`] variant, in declaration order — a local array rather
/// than `strum::VariantArray`/`Action::VARIANTS`: `strum` is not a direct
/// `gp-game` dependency, and this workspace's dependency-addition gate is
/// deliberately conservative (mirrors `race/round.rs`'s own `RecordingStub`
/// fixture, which hand-lists the same five variants for the same reason).
const ALL_ACTIONS: [Action; 5] = [
    Action::Coast,
    Action::East,
    Action::West,
    Action::North,
    Action::South,
];

/// A test-local seat: picks the first action of [`ALL_ACTIONS`] that is in
/// `ctx.legal`, starting the scan at `turn_index % 5` rather than index
/// `0`. A naive "first legal in declaration order" pilot picks
/// `Action::Coast` whenever it is legal, and `Coast` at `v = (0, 0)` never
/// moves the car — the race would never advance. The rotating start makes
/// the pilot actually drive while staying fully deterministic (design §
/// *How `AC21`'s record is produced*).
struct FirstLegal {
    turn_index: usize,
}

impl FirstLegal {
    const fn new() -> Self {
        Self { turn_index: 0 }
    }
}

impl Controller for FirstLegal {
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "ALL_ACTIONS.len() is the fixed compile-time constant 5 (never 0), so both \
                  `%` operations are non-panicking by construction; `offset < ALL_ACTIONS.len()` \
                  and `start < ALL_ACTIONS.len()` bound their sum comfortably under usize::MAX"
    )]
    fn poll(&mut self, ctx: PollContext<'_>) -> Option<Action> {
        let start = self.turn_index % ALL_ACTIONS.len();
        self.turn_index = self.turn_index.wrapping_add(1);
        (0..ALL_ACTIONS.len())
            .map(|offset| ALL_ACTIONS[(start + offset) % ALL_ACTIONS.len()])
            .find(|&a| ctx.legal.contains(a))
    }
}

/// This test's fixed turn budget (design § *How `AC21`'s record is
/// produced*: "the `AC21` test passes a fixed budget").
const MAX_TURNS: u32 = 8;

/// A fresh [`Command`] for the built `graphite-gp` binary.
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_graphite-gp"))
}

/// A process-unique scratch path under the system temp dir — no `tempfile`
/// dependency (this workspace's dependency-addition gate is deliberately
/// conservative); best-effort cleanup on drop.
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

/// Builds a real record (a real race on a real regenerated track, driven
/// through the same entry point `--replay-mode headless` uses) and writes
/// it to `path` — the shared setup for every test in this file.
fn write_real_record(path: &std::path::Path) {
    let config = cheap_config();
    let mut roster = Roster::new();
    roster.push(Box::new(FirstLegal::new()));
    roster.push(Box::new(FirstLegal::new()));

    let (_, record) =
        run_headless_race(&config, roster, MAX_TURNS).expect("cheap config must accept");
    let text = write_record(&config, &record);
    std::fs::write(path, text).expect("failed to write the scratch replay file");
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn replay_round_trips_headless_and_prints_standings() {
    let scratch = ScratchFile::new("round-trip");
    write_real_record(&scratch.0);

    let output = bin()
        .args([
            "--replay",
            scratch.0.to_str().expect("scratch path must be UTF-8"),
            "--replay-mode",
            "headless",
        ])
        .output()
        .expect("failed to spawn the built graphite-gp binary");

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

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns the built binary via std::process::Command; process \
              spawning is unsupported under Miri"
)]
fn tampered_action_token_exits_nonzero() {
    let scratch = ScratchFile::new("tampered");
    write_real_record(&scratch.0);

    let original = std::fs::read_to_string(&scratch.0).expect("just-written scratch file");
    let mut tampered_a_line = false;
    let tampered: String = original
        .lines()
        .map(|line| {
            if tampered_a_line || !line.starts_with("turn ") {
                return line.to_string();
            }
            tampered_a_line = true;
            // Every seated seat's mask is a proper subset of
            // `Actions::all()`, so at least one of the 5 tokens is
            // illegal from any real state -- try each until one
            // actually changes the line (never a no-op tamper).
            for &candidate in &ALL_ACTIONS {
                let mut words: Vec<&str> = line.split_whitespace().collect();
                let owned = candidate.to_string();
                if let Some(last) = words.last_mut() {
                    *last = &owned;
                }
                let candidate_line = words.join(" ");
                if candidate_line != line {
                    return candidate_line;
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(tampered_a_line, "no turn line found to tamper with");
    std::fs::write(&scratch.0, tampered).expect("failed to write the tampered scratch file");

    let output = bin()
        .args([
            "--replay",
            scratch.0.to_str().expect("scratch path must be UTF-8"),
            "--replay-mode",
            "headless",
        ])
        .output()
        .expect("failed to spawn the built graphite-gp binary");

    assert!(
        !output.status.success(),
        "a tampered record must exit non-zero"
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
    write_real_record(&scratch.0);
    let original = std::fs::read_to_string(&scratch.0).expect("just-written scratch file");
    let bad_version = original.replacen("graphite-gp-replay 1", "graphite-gp-replay 2", 1);
    std::fs::write(&scratch.0, bad_version).expect("failed to write the scratch replay file");

    let output = bin()
        .args([
            "--replay",
            scratch.0.to_str().expect("scratch path must be UTF-8"),
            "--replay-mode",
            "headless",
        ])
        .output()
        .expect("failed to spawn the built graphite-gp binary");

    assert!(
        !output.status.success(),
        "an unrecognised version must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("version"), "{stderr}");
}

/// `AC21b` — the written record is valid `UTF-8` (guaranteed by `write_record`
/// returning a `String`) and the format version is greppable from the raw
/// file.
#[test]
#[cfg_attr(
    miri,
    ignore = "runs the gp-gen generation pipeline via run_headless_race — a \
              multi-second integer sweep whose interpreted wall-clock is \
              prohibitive"
)]
fn written_record_is_utf8_and_the_version_is_greppable() {
    let scratch = ScratchFile::new("ac21b");
    write_real_record(&scratch.0);
    let bytes = std::fs::read(&scratch.0).expect("just-written scratch file");
    let text = String::from_utf8(bytes).expect("record must be valid UTF-8");
    assert!(text.contains("graphite-gp-replay 1"), "{text}");
}

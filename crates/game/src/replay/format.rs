//! The persisted (on-disk) replay format: version, writer, total parser,
//! and its error type (issue #43, C2, spec § Replay CLI / § Replay
//! format).
//!
//! Human-readable text (AC21b): `\n`-separated, one directive per line,
//! `#`-prefixed comments allowed and skipped. The track is **regenerated**
//! from `seed-generation` plus the `GenParams`-completing fields this
//! format carries (`min-straight`/`block-size`/`seed-budget`/
//! `repair-budget`) — it is never itself persisted (design § *Replay
//! format*). [`parse_record`] is total: it never `unwrap`s, `expect`s, or
//! indexes into untrusted input — a malformed or truncated file is always
//! reported as an [`ReplayError`], never a panic.
//!
//! An unrecognised version (AC22) is rejected before any other line is
//! interpreted, via [`ReplayError::UnsupportedVersion`].
//!
//! **`processed <u32>` (C4 addition to the design's own grammar sketch):**
//! the total number of `Moved`-OR-`Crashed` outcomes the source race
//! processed — `turns.len()` alone undercounts whenever the source race
//! crashed at least once (a crash turn emits no `turn` line, since it polls
//! no controller). A persisted replay driven from a `--record` file that
//! stopped at an external turn cap (not `RaceOver`) must replay to the
//! SAME point and stop cleanly; using `turns.len()` as that bound would
//! either stop short (if there were crashes) or, if the caller instead
//! guesses a large `max_turns`, run the `ReplayController`s dry past the
//! last recorded turn and register a false divergence. See
//! `ReplayRecord::total_processed_turns`'s own doc for the full rationale.

use crate::config::GameConfig;
use crate::replay::{FinalCarState, RecordedTurn, ReplayRecord};
use gp_core::rng::Seeds;
use gp_core::sim::{Action, CarState};
use gp_render::RaceConfig;
use gp_render::screens::{DIFFICULTY_LABELS, Difficulty};
use std::fmt::Write as _;
use std::str::FromStr;
use std::str::SplitWhitespace;
use thiserror::Error;

/// The persisted format's magic token — the first word of every record.
const MAGIC: &str = "graphite-gp-replay";

/// The current persisted format version (design § *Replay format*).
pub const FORMAT_VERSION: u32 = 1;

/// A persisted-replay parse error — always total, never a panic (this
/// module parses untrusted file content).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// The record's version field does not match [`FORMAT_VERSION`]
    /// (AC22). Detected before any other line is interpreted, in both
    /// replay modes.
    #[error("unrecognised replay format version {found} (expected {expected})")]
    UnsupportedVersion {
        /// The version the file declared.
        found: u32,
        /// The version this build understands.
        expected: u32,
    },
    /// A line does not match the format's grammar (missing field,
    /// unparseable number, wrong keyword, truncated file, `seats` count
    /// disagreeing with the number of `final` lines actually present).
    #[error("malformed replay record at line {line}: {reason}")]
    Malformed {
        /// The 1-based source line this error was found at (`0` for
        /// whole-file errors, e.g. an empty file).
        line: usize,
        /// A human-readable description of what was expected.
        reason: String,
    },
    /// A `turn` line's action token is not one of [`Action`]'s variants.
    #[error("unknown action token {token:?} at line {line}")]
    UnknownAction {
        /// The 1-based source line.
        line: usize,
        /// The unrecognised token.
        token: String,
    },
    /// Divergence layer (a1) — structural: the `turn` block violates its
    /// own well-formedness, checked with no track and no simulation
    /// (design § *Replay format*, Design Amendment 1). `round` must be
    /// non-decreasing; within one `round`, `seat` must strictly increase;
    /// every `seat` must be `< seats`. Deliberately **not** a full
    /// seat-cycle check — a crash turn emits no `turn` line, so the seat
    /// sequence within a round is a *subsequence* of `0..seats`, never
    /// necessarily the full cycle.
    #[error("turn sequence violation at line {line}: {reason}")]
    TurnSequence {
        /// The 1-based source line the violation was found at.
        line: usize,
        /// A human-readable description of the violated invariant.
        reason: String,
    },
}

/// Writes `record` (paired with `config`'s provenance/regeneration fields)
/// as a persisted replay file — the exact text `--record` writes at race
/// end, and what [`parse_record`] reads back.
///
/// `config.race`/`config.seeds` are **not** used here: `record.race` and
/// `record.generation_seed`/`record.collision_seed` are the *resolved*
/// values the race actually ran with (which may differ from `config`'s own
/// `k = 0, r = 0` base after a Regenerate/Race-again, per spec § Seed
/// policy) and are therefore the authoritative source. Only `config`'s
/// `master` (provenance) and the `GenParams`-completing tuning knobs
/// (`min_straight`/`block_size`/`seed_budget`/`repair_budget`, which
/// Regenerate/Race-again never change) come from `config` itself.
#[must_use]
pub fn write_record(config: &GameConfig, record: &ReplayRecord) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{MAGIC} {FORMAT_VERSION}");
    let _ = writeln!(out, "master {}", config.master);
    let _ = writeln!(out, "seed-generation {}", record.generation_seed);
    let _ = writeln!(out, "seed-collision {}", record.collision_seed);
    let _ = writeln!(
        out,
        "cars {} laps {} v-target {} difficulty {}",
        record.race.cars,
        record.race.laps,
        record.race.v_target,
        record.race.difficulty.label(),
    );
    let _ = writeln!(
        out,
        "min-straight {} block-size {} seed-budget {} repair-budget {}",
        config.min_straight, config.block_size, config.seed_budget, config.repair_budget,
    );
    let _ = writeln!(out, "seats {}", record.finals.len());
    let _ = writeln!(out, "processed {}", record.total_processed_turns);
    for turn in &record.turns {
        let _ = writeln!(out, "turn {} {} {}", turn.round, turn.seat, turn.action);
    }
    for fin in &record.finals {
        let _ = writeln!(
            out,
            "final {} {} {} {} {} {}",
            fin.seat, fin.state.x, fin.state.y, fin.state.vx, fin.state.vy, fin.lap_raw,
        );
    }
    out
}

/// Parses a persisted replay file back into a [`GameConfig`] (regeneration
/// inputs only — `seeds.ai_learning`/`seeds.ai_inference` are set to `0`,
/// unused by a replayed race) and its [`ReplayRecord`].
///
/// # Errors
/// [`ReplayError::UnsupportedVersion`] if the version field does not match
/// [`FORMAT_VERSION`] (checked first, before any other line). Otherwise
/// [`ReplayError::Malformed`] for any structural problem (missing/extra
/// field, unparseable number, wrong keyword, truncated file, a `seats`
/// count disagreeing with the number of `final` lines actually present),
/// or [`ReplayError::UnknownAction`] for an unrecognised `turn` action
/// token.
pub fn parse_record(text: &str) -> Result<(GameConfig, ReplayRecord), ReplayError> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index.saturating_add(1), line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'));

    parse_header(&mut lines)?;
    let master: u64 = keyed_line(&mut lines, "master", "master")?;
    let generation_seed: u64 = keyed_line(&mut lines, "seed-generation", "seed-generation")?;
    let collision_seed: u64 = keyed_line(&mut lines, "seed-collision", "seed-collision")?;
    let (cars, laps, v_target, difficulty) = parse_race_line(&mut lines)?;
    let (min_straight, block_size, seed_budget, repair_budget) = parse_tuning_line(&mut lines)?;
    let seats: usize = keyed_line(&mut lines, "seats", "seats")?;
    let total_processed_turns: u32 = keyed_line(&mut lines, "processed", "processed")?;

    let (turns, pending) = parse_turns(&mut lines, seats)?;
    let finals = parse_finals(pending.into_iter().chain(lines))?;
    if finals.len() != seats {
        return Err(ReplayError::Malformed {
            line: 0,
            reason: format!(
                "declared {seats} seats but found {} final lines",
                finals.len()
            ),
        });
    }

    let config = GameConfig {
        race: RaceConfig {
            cars,
            laps,
            v_target,
            difficulty,
        },
        seeds: Seeds {
            collision: collision_seed,
            generation: generation_seed,
            ai_learning: 0,
            ai_inference: 0,
        },
        master,
        min_straight,
        block_size,
        seed_budget,
        repair_budget,
        // The persisted format carries no `--record`/`--replay`/
        // `--replay-mode` directives (those are the CLI's own I/O
        // controls, not part of what a race needs to be reproduced) —
        // neutral values; C4's caller sets its own as needed.
        record: None,
        replay: None,
        replay_mode: crate::config::ReplayMode::Gui,
    };
    let record = ReplayRecord {
        generation_seed,
        collision_seed,
        race: config.race,
        turns,
        finals,
        total_processed_turns,
    };

    Ok((config, record))
}

/// Reads and validates the record's first line: the magic token, then the
/// version (AC22 — rejected before any other line is interpreted).
fn parse_header<'a>(lines: &mut impl Iterator<Item = (usize, &'a str)>) -> Result<(), ReplayError> {
    let (header_line, header) = lines.next().ok_or_else(|| ReplayError::Malformed {
        line: 0,
        reason: "empty file".to_string(),
    })?;
    let mut header_words = header.split_whitespace();
    let magic = header_words.next().unwrap_or_default();
    if magic != MAGIC {
        return Err(ReplayError::Malformed {
            line: header_line,
            reason: format!("expected magic {MAGIC:?}, found {magic:?}"),
        });
    }
    let found: u32 = parse_field(&mut header_words, header_line, "format version")?;
    if found != FORMAT_VERSION {
        return Err(ReplayError::UnsupportedVersion {
            found,
            expected: FORMAT_VERSION,
        });
    }
    Ok(())
}

/// Reads the `cars <u32> laps <u32> v-target <i32> difficulty <label>`
/// line.
fn parse_race_line<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<(u32, u32, i32, Difficulty), ReplayError> {
    let (race_line, race_text) = next_line(lines, "cars/laps/v-target/difficulty line")?;
    let mut race_words = race_text.split_whitespace();
    expect_word(&mut race_words, "cars", race_line)?;
    let cars: u32 = parse_field(&mut race_words, race_line, "cars")?;
    expect_word(&mut race_words, "laps", race_line)?;
    let laps: u32 = parse_field(&mut race_words, race_line, "laps")?;
    expect_word(&mut race_words, "v-target", race_line)?;
    let v_target: i32 = parse_field(&mut race_words, race_line, "v-target")?;
    expect_word(&mut race_words, "difficulty", race_line)?;
    let difficulty_token = field(&mut race_words, race_line, "difficulty")?;
    let difficulty = DIFFICULTY_LABELS
        .iter()
        .position(|&label| label == difficulty_token)
        .and_then(Difficulty::from_index)
        .ok_or_else(|| ReplayError::Malformed {
            line: race_line,
            reason: format!("unrecognised difficulty {difficulty_token:?}"),
        })?;
    Ok((cars, laps, v_target, difficulty))
}

/// Reads the
/// `min-straight <i32> block-size <i32> seed-budget <u32> repair-budget <u32>`
/// line.
fn parse_tuning_line<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<(i32, i32, u32, u32), ReplayError> {
    let (tuning_line, tuning_text) = next_line(
        lines,
        "min-straight/block-size/seed-budget/repair-budget line",
    )?;
    let mut tuning_words = tuning_text.split_whitespace();
    expect_word(&mut tuning_words, "min-straight", tuning_line)?;
    let min_straight: i32 = parse_field(&mut tuning_words, tuning_line, "min-straight")?;
    expect_word(&mut tuning_words, "block-size", tuning_line)?;
    let block_size: i32 = parse_field(&mut tuning_words, tuning_line, "block-size")?;
    expect_word(&mut tuning_words, "seed-budget", tuning_line)?;
    let seed_budget: u32 = parse_field(&mut tuning_words, tuning_line, "seed-budget")?;
    expect_word(&mut tuning_words, "repair-budget", tuning_line)?;
    let repair_budget: u32 = parse_field(&mut tuning_words, tuning_line, "repair-budget")?;
    Ok((min_straight, block_size, seed_budget, repair_budget))
}

/// Reads every `turn <round> <seat> <action>` line until a non-`turn` line
/// is reached (returned as `pending`, to be re-fed into [`parse_finals`])
/// or the input is exhausted.
///
/// Enforces divergence layer (a1) as it goes (design § *Replay format*,
/// Design Amendment 1): `round` non-decreasing across the whole block;
/// within one `round`, `seat` strictly increasing; every `seat < seats`.
/// Deliberately not a full seat-cycle check — see [`ReplayError::TurnSequence`].
#[allow(
    clippy::type_complexity,
    reason = "the pending-line carry-over is a plain (line_no, &str) pair; a named \
              wrapper type would not clarify a two-field tuple used nowhere else"
)]
fn parse_turns<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
    seats: usize,
) -> Result<(Vec<RecordedTurn>, Option<(usize, &'a str)>), ReplayError> {
    let mut turns = Vec::new();
    let mut last_round: Option<u32> = None;
    let mut last_seat_in_round: Option<usize> = None;
    for (line_no, line) in lines {
        let mut words = line.split_whitespace();
        let Some(keyword) = words.next() else {
            continue;
        };
        if keyword != "turn" {
            return Ok((turns, Some((line_no, line))));
        }
        let round: u32 = parse_field(&mut words, line_no, "turn round")?;
        let seat: usize = parse_field(&mut words, line_no, "turn seat")?;
        let action_token = field(&mut words, line_no, "turn action")?;
        let action = Action::from_str(action_token).map_err(|_| ReplayError::UnknownAction {
            line: line_no,
            token: action_token.to_string(),
        })?;

        if seat >= seats {
            return Err(ReplayError::TurnSequence {
                line: line_no,
                reason: format!("seat {seat} is not < declared seats {seats}"),
            });
        }
        if let Some(prev_round) = last_round {
            if round < prev_round {
                return Err(ReplayError::TurnSequence {
                    line: line_no,
                    reason: format!(
                        "round {round} is less than the previous turn's round {prev_round}"
                    ),
                });
            }
            if round > prev_round {
                last_seat_in_round = None;
            }
        }
        if let Some(prev_seat) = last_seat_in_round
            && seat <= prev_seat
        {
            return Err(ReplayError::TurnSequence {
                line: line_no,
                reason: format!(
                    "seat {seat} does not strictly increase after seat {prev_seat} within round {round}"
                ),
            });
        }
        last_round = Some(round);
        last_seat_in_round = Some(seat);

        turns.push(RecordedTurn {
            round,
            seat,
            action,
        });
    }
    Ok((turns, None))
}

/// Reads every `final <seat> <x> <y> <vx> <vy> <lap-raw>` line.
fn parse_finals<'a>(
    lines: impl Iterator<Item = (usize, &'a str)>,
) -> Result<Vec<FinalCarState>, ReplayError> {
    let mut finals = Vec::new();
    for (line_no, line) in lines {
        let mut words = line.split_whitespace();
        expect_word(&mut words, "final", line_no)?;
        let seat: usize = parse_field(&mut words, line_no, "final seat")?;
        let x: i32 = parse_field(&mut words, line_no, "final x")?;
        let y: i32 = parse_field(&mut words, line_no, "final y")?;
        let vx: i32 = parse_field(&mut words, line_no, "final vx")?;
        let vy: i32 = parse_field(&mut words, line_no, "final vy")?;
        let lap_raw: i32 = parse_field(&mut words, line_no, "final lap-raw")?;
        finals.push(FinalCarState {
            seat,
            state: CarState { x, y, vx, vy },
            lap_raw,
        });
    }
    Ok(finals)
}

/// Reads the next significant (non-blank, non-comment) line, erroring with
/// `what` describing what was expected if the input is exhausted.
fn next_line<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
    what: &str,
) -> Result<(usize, &'a str), ReplayError> {
    lines.next().ok_or_else(|| ReplayError::Malformed {
        line: 0,
        reason: format!("unexpected end of file, expected {what}"),
    })
}

/// Reads the next significant line, checks it starts with `keyword`, and
/// parses the single value that follows.
fn keyed_line<'a, T: FromStr>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
    keyword: &str,
    what: &str,
) -> Result<T, ReplayError> {
    let (line_no, line) = next_line(lines, what)?;
    let mut words = line.split_whitespace();
    expect_word(&mut words, keyword, line_no)?;
    parse_field(&mut words, line_no, what)
}

/// Consumes the next word, erroring if it is not exactly `expected`.
fn expect_word(
    words: &mut SplitWhitespace<'_>,
    expected: &str,
    line: usize,
) -> Result<(), ReplayError> {
    match words.next() {
        Some(word) if word == expected => Ok(()),
        Some(word) => Err(ReplayError::Malformed {
            line,
            reason: format!("expected {expected:?}, found {word:?}"),
        }),
        None => Err(ReplayError::Malformed {
            line,
            reason: format!("expected {expected:?}, found end of line"),
        }),
    }
}

/// Consumes the next word verbatim, erroring (naming `what`) if the line is
/// exhausted.
fn field<'a>(
    words: &mut SplitWhitespace<'a>,
    line: usize,
    what: &str,
) -> Result<&'a str, ReplayError> {
    words.next().ok_or_else(|| ReplayError::Malformed {
        line,
        reason: format!("missing {what}"),
    })
}

/// Consumes the next word and parses it as `T`, erroring (naming `what`) on
/// a missing or unparseable token.
fn parse_field<T: FromStr>(
    words: &mut SplitWhitespace<'_>,
    line: usize,
    what: &str,
) -> Result<T, ReplayError> {
    let raw = field(words, line, what)?;
    raw.parse::<T>().map_err(|_| ReplayError::Malformed {
        line,
        reason: format!("invalid {what} {raw:?}"),
    })
}

#[cfg(test)]
mod tests {
    use super::{FORMAT_VERSION, ReplayError, parse_record, write_record};
    use crate::config::GameConfig;
    use crate::replay::{FinalCarState, RecordedTurn, ReplayRecord};
    use gp_core::rng::Seeds;
    use gp_core::sim::{Action, CarState};
    use gp_render::{Difficulty, RaceConfig};

    fn sample_config() -> GameConfig {
        GameConfig {
            race: RaceConfig {
                cars: 2,
                laps: 3,
                v_target: 5,
                difficulty: Difficulty::Pro,
            },
            seeds: Seeds::default(),
            master: 41,
            min_straight: 3,
            block_size: 6,
            seed_budget: 1,
            repair_budget: 8,
            record: None,
            replay: None,
            replay_mode: crate::config::ReplayMode::Gui,
        }
    }

    fn sample_record() -> ReplayRecord {
        ReplayRecord {
            generation_seed: 6,
            collision_seed: 7,
            race: RaceConfig {
                cars: 2,
                laps: 3,
                v_target: 5,
                difficulty: Difficulty::Pro,
            },
            turns: vec![
                RecordedTurn {
                    round: 0,
                    seat: 0,
                    action: Action::East,
                },
                RecordedTurn {
                    round: 0,
                    seat: 1,
                    action: Action::Coast,
                },
            ],
            finals: vec![
                FinalCarState {
                    seat: 0,
                    state: CarState {
                        x: 3,
                        y: 1,
                        vx: 1,
                        vy: 0,
                    },
                    lap_raw: -1,
                },
                FinalCarState {
                    seat: 1,
                    state: CarState {
                        x: 2,
                        y: 0,
                        vx: 0,
                        vy: 0,
                    },
                    lap_raw: -1,
                },
            ],
            total_processed_turns: 2,
        }
    }

    /// `AC21b` — the written bytes are valid `UTF-8` (guaranteed by `String`)
    /// and a hand-inspectable field (the format version) is greppable from
    /// the raw file.
    #[test]
    fn written_record_is_greppable_utf8_and_carries_the_version() {
        let text = write_record(&sample_config(), &sample_record());
        assert!(text.contains("graphite-gp-replay 1"), "{text}");
    }

    /// Round-trip: `parse_record(write_record(..))` recovers the same
    /// resolved seeds/race/turns/finals (the regeneration-relevant
    /// `GameConfig` fields, not `ai_learning`/`ai_inference`, which the
    /// format does not carry).
    #[test]
    fn write_then_parse_round_trips_record_fields() {
        let config = sample_config();
        let record = sample_record();
        let text = write_record(&config, &record);

        let (parsed_config, parsed_record) = parse_record(&text).expect("well-formed record");
        assert_eq!(parsed_config.master, config.master);
        assert_eq!(parsed_config.min_straight, config.min_straight);
        assert_eq!(parsed_config.block_size, config.block_size);
        assert_eq!(parsed_config.seed_budget, config.seed_budget);
        assert_eq!(parsed_config.repair_budget, config.repair_budget);
        assert_eq!(parsed_config.seeds.generation, record.generation_seed);
        assert_eq!(parsed_config.seeds.collision, record.collision_seed);
        assert_eq!(parsed_record, record);
    }

    /// AC22 — an unrecognised version is rejected before any other line is
    /// interpreted.
    #[test]
    fn unrecognised_version_is_rejected() {
        let text = "graphite-gp-replay 2\nmaster 1\n";
        assert_eq!(
            parse_record(text),
            Err(ReplayError::UnsupportedVersion {
                found: 2,
                expected: FORMAT_VERSION,
            })
        );
    }

    #[test]
    fn empty_file_is_malformed_not_a_panic() {
        assert!(matches!(
            parse_record(""),
            Err(ReplayError::Malformed { .. })
        ));
    }

    #[test]
    fn truncated_file_is_malformed_not_a_panic() {
        assert!(matches!(
            parse_record("graphite-gp-replay 1\nmaster 1\n"),
            Err(ReplayError::Malformed { .. })
        ));
    }

    #[test]
    fn unknown_action_token_is_reported() {
        let config = sample_config();
        let record = sample_record();
        let text = write_record(&config, &record).replace("turn 0 0 East", "turn 0 0 Teleport");
        assert!(matches!(
            parse_record(&text),
            Err(ReplayError::UnknownAction { .. })
        ));
    }

    #[test]
    fn seats_count_disagreeing_with_final_lines_is_malformed() {
        let config = sample_config();
        let record = sample_record();
        let text = write_record(&config, &record).replace("seats 2", "seats 3");
        assert!(matches!(
            parse_record(&text),
            Err(ReplayError::Malformed { .. })
        ));
    }

    /// Comment lines (`#`-prefixed) and blank lines are skipped by the
    /// parser.
    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let config = sample_config();
        let record = sample_record();
        let text = format!(
            "# a comment\n\n{}\n# trailing comment\n",
            write_record(&config, &record).trim_end()
        );
        let (_, parsed_record) = parse_record(&text).expect("comments/blanks must be skipped");
        assert_eq!(parsed_record, record);
    }
}

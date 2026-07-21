//! `ResultsScreen` — port of `Screens.jsx`'s `ResultsScreen` (design
//! `2026-07-22-render-results-screen` § *Approach*).
//!
//! Draw-only, caller-supplies-data (mirrors
//! [`crate::screens::setup::SetupScreen`]): the screen holds a
//! caller-supplied `&[StandingEntry]` slice (already rank-ordered), a
//! [`RaceSummary`], and an optional "Race again" icon handle — and emits the
//! player's chosen navigation intent ("Race again" / "Menu") via
//! `ResultsResponse`. It performs **no** ranking, timing, or counting — the
//! caller supplies already-finished outcome data (spec § Scope).

use crate::screens::race::CAR_NAMES;
use crate::tokens::color::{CAR_COLORS, car_color};
use crate::widgets::CarKind;
use egui::Color32;

/// One car's finished-race outcome, in caller-supplied rank order.
///
/// `car_index` is the car's **stable identity** (resolves *name* via
/// [`CAR_NAMES`] and *color* via [`car_color`]) — deliberately decoupled from
/// `rank`: a real player can finish P3 with `car_index == 0` (design §
/// *Resolving the Open question*).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StandingEntry {
    /// The car's stable identity (0 = the player's car by convention,
    /// though `kind` is the authoritative player/AI signal).
    pub car_index: usize,
    /// `You`/`Ai` (matches `CarChip`'s own prop).
    pub kind: CarKind,
    /// Finishing rank.
    pub rank: u32,
    /// Finish time, in seconds (formatted at draw time).
    pub finish_time: f32,
}

/// The race's summary metrics (fastest lap / tempo / crash count), numeric.
///
/// Formatted at draw time (`{:.1}` / `{:.2}` / `to_string`), mirroring
/// `lab.rs::oracle_tile_strings`'s numeric-in / string-at-draw contract.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaceSummary {
    /// Fastest lap time, in seconds.
    pub fastest_lap: f32,
    /// Average tempo.
    pub tempo: f32,
    /// Total crash count.
    pub crashes: u32,
}

/// One resolved Final-standings row — the [`standings_rows`] output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandingRow {
    /// The car's display name, resolved from [`CAR_NAMES`] via
    /// `entry.car_index` (total: out-of-range falls back to `"Car"`).
    pub name: &'static str,
    /// The car's ramp color, resolved via [`car_color`] (total: out-of-range
    /// falls back to `CAR_COLORS[0]`).
    pub color: Color32,
    /// `You`/`Ai`.
    pub kind: CarKind,
    /// Finishing rank.
    pub rank: u32,
    /// The formatted finish time (`"{:.1}s"`).
    pub finish_time: String,
}

/// The three summary-tile labels, in [`summary_tiles`] order (`Fastest lap`,
/// `Tempo`, `Crashes` — `Screens.jsx:223`).
pub const SUMMARY_LABELS: [&str; 3] = ["Fastest lap", "Tempo", "Crashes"];

/// The rank of the `You`-kind entry in `entries`, or `None` if absent (an
/// empty slice, or a slice with no `You` entry — the header then renders a
/// `P—` placeholder, never a panic).
///
/// Plain `fn` — `<[_]>::iter().find` is not const-stable.
#[must_use]
pub fn player_position(entries: &[StandingEntry]) -> Option<u32> {
    entries
        .iter()
        .find(|entry| entry.kind == CarKind::You)
        .map(|entry| entry.rank)
}

/// Resolves `entries` (in slice order) into their [`StandingRow`]s.
///
/// Name and color are looked up from **`entry.car_index`** (the car's stable
/// identity), NOT the loop/enumerate index within the slice — identity is
/// decoupled from rank/position by design (design § *Resolving the Open
/// question*). Both lookups are total (`CAR_NAMES.get(..).unwrap_or("Car")`,
/// `car_color(..).unwrap_or(CAR_COLORS[0])`), matching
/// [`crate::track::CarRender::color`]'s no-panic-on-bad-index posture.
///
/// Plain `fn` — allocates a `Vec` and calls `format!`/`car_color` (`car_color`
/// is documented non-`const`: `<[T]>::get` is not yet const-stable).
#[must_use]
pub fn standings_rows(entries: &[StandingEntry]) -> Vec<StandingRow> {
    entries
        .iter()
        .map(|entry| {
            let name = CAR_NAMES.get(entry.car_index).copied().unwrap_or("Car");
            let color = car_color(entry.car_index).unwrap_or(CAR_COLORS[0]);
            StandingRow {
                name,
                color,
                kind: entry.kind,
                rank: entry.rank,
                finish_time: format!("{:.1}s", entry.finish_time),
            }
        })
        .collect()
}

/// Formats `summary`'s three tile values, in [`SUMMARY_LABELS`] order:
/// `fastest_lap` at `{:.1}`, `tempo` at `{:.2}`, `crashes` via
/// `u32::to_string`.
///
/// Plain `fn` — calls `format!` (not const-stable).
#[must_use]
pub fn summary_tiles(summary: RaceSummary) -> [String; 3] {
    [
        format!("{:.1}", summary.fastest_lap),
        format!("{:.2}", summary.tempo),
        summary.crashes.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        RaceSummary, SUMMARY_LABELS, StandingEntry, player_position, standings_rows, summary_tiles,
    };
    use crate::tokens::color::{CAR_COLORS, car_color};
    use crate::widgets::CarKind;

    /// A rank-ordered 4-car fixture that **deliberately decouples
    /// `car_index` from slice position**: the `You` car sits at slice
    /// position 2 (rank 3) with `car_index == 0` — the exact
    /// positional-vs-identity discriminator the decoupling assertion below
    /// requires.
    fn fixture_standings() -> [StandingEntry; 4] {
        [
            StandingEntry {
                car_index: 2,
                kind: CarKind::Ai,
                rank: 1,
                finish_time: 38.0,
            },
            StandingEntry {
                car_index: 9,
                kind: CarKind::Ai,
                rank: 2,
                finish_time: 39.6,
            },
            StandingEntry {
                car_index: 0,
                kind: CarKind::You,
                rank: 3,
                finish_time: 41.2,
            },
            StandingEntry {
                car_index: 1,
                kind: CarKind::Ai,
                rank: 4,
                finish_time: 42.8,
            },
        ]
    }

    fn fixture_summary() -> RaceSummary {
        RaceSummary {
            fastest_lap: 12.4,
            tempo: 0.87,
            crashes: 1,
        }
    }

    /// AC2 — the `You` entry's rank (3, at a non-first slice position).
    #[test]
    fn player_position_returns_you_entry_rank() {
        assert_eq!(player_position(&fixture_standings()), Some(3));
    }

    /// AC2 — no `You` entry, and an empty slice, both return `None`.
    #[test]
    fn player_position_none_when_no_you_entry() {
        let all_ai: [StandingEntry; 2] = [
            StandingEntry {
                car_index: 0,
                kind: CarKind::Ai,
                rank: 1,
                finish_time: 10.0,
            },
            StandingEntry {
                car_index: 1,
                kind: CarKind::Ai,
                rank: 2,
                finish_time: 11.0,
            },
        ];
        assert_eq!(player_position(&all_ai), None);
        assert_eq!(player_position(&[]), None);
    }

    /// AC3 — chip count equals car count.
    #[test]
    fn standings_rows_length_matches_entry_count() {
        assert_eq!(standings_rows(&fixture_standings()).len(), 4);
    }

    /// AC3 — ranks appear in strictly ascending order.
    #[test]
    fn standings_rows_ranks_are_ascending() {
        let rows = standings_rows(&fixture_standings());
        assert!(rows.windows(2).all(|w| w[0].rank < w[1].rank));
    }

    /// AC3 — the headline decoupling property: the `You` car sits at slice
    /// position 2 yet carries `car_index == 0`; name/color track
    /// `entry.car_index` (0 → `"You"`/`car_color(0)`), NOT the loop index
    /// (2). A regression to positional lookup would resolve `rows[2]` from
    /// index 2 (`"Rival Green"`/`car_color(2)`), which this assert catches.
    #[test]
    fn standings_rows_resolves_identity_from_car_index_not_slice_position() {
        let rows = standings_rows(&fixture_standings());
        assert_eq!(rows[2].name, "You");
        assert_eq!(rows[2].color, car_color(0).unwrap_or(CAR_COLORS[0]));
        assert_eq!(rows[2].kind, CarKind::You);
    }

    /// AC3 — `car_index == 2` (slice position 0) resolves `"Rival Green"`;
    /// an out-of-range `car_index` (`9`, slice position 1) falls back to the
    /// total `"Car"` name — no panic.
    #[test]
    fn standings_rows_resolves_names_by_car_index() {
        let rows = standings_rows(&fixture_standings());
        assert_eq!(rows[0].name, "Rival Green");
        assert_eq!(rows[1].name, "Car");
    }

    /// AC3 — finish-time formatting: `38.0 -> "38.0s"`, `39.6 -> "39.6s"`.
    #[test]
    fn standings_rows_formats_finish_time() {
        let rows = standings_rows(&fixture_standings());
        assert_eq!(rows[0].finish_time, "38.0s");
        assert_eq!(rows[1].finish_time, "39.6s");
    }

    /// AC4 — tile values reflect the supplied data.
    #[test]
    fn summary_tiles_reflects_supplied_data() {
        assert_eq!(
            summary_tiles(fixture_summary()),
            ["12.4".to_owned(), "0.87".to_owned(), "1".to_owned()]
        );
    }

    /// AC4 — the three summary labels are present, in tile order.
    #[test]
    fn summary_labels_are_present_in_order() {
        assert_eq!(SUMMARY_LABELS, ["Fastest lap", "Tempo", "Crashes"]);
    }
}

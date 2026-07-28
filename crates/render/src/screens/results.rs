//! `ResultsScreen` — port of `Screens.jsx`'s `ResultsScreen` (design
//! `2026-07-22-render-results-screen` § *Approach*).
//!
//! Draw-only, caller-supplies-data (mirrors
//! [`crate::screens::setup::SetupScreen`]): the screen holds a
//! caller-supplied `&[StandingEntry]` slice (already rank-ordered), a
//! [`RaceSummary`], and an optional "Race again" icon handle — and emits the
//! player's chosen navigation intent ("Race again" / "Menu") via
//! [`ResultsResponse`]. It performs **no** ranking, timing, or counting — the
//! caller supplies already-finished outcome data (spec § Scope).

use crate::screens::race::CAR_NAMES;
use crate::tokens::color::{CAR_COLORS, car_color};
use crate::tokens::{color, spacing, typography};
use crate::widgets::{
    Button, ButtonVariant, CarChip, CarKind, Card, Size, Telemetry, TelemetryTone,
};
use egui::{Align2, Color32, FontFamily, FontId, Layout, Pos2, Response, Sense, TextureHandle, Ui};

/// The centered content column's fixed width (`Screens.jsx:207` `maxWidth:
/// 560`), equal to `setup.rs::CONTENT_MAX_W`.
const CONTENT_MAX_W: f32 = 560.0;
/// Placeholder rendered for a non-finisher's finish-turn column (issue #43
/// D3, design KD12) — a sentinel turn count would be a lie, so a
/// non-finisher renders this instead. Module-private, mirroring
/// `lab.rs::PLACEHOLDER` as a sibling-module idiom rather than a shared
/// constant (design KD12 — two independent screens each owning a one-line
/// display constant is cheaper than a cross-module coupling).
const PLACEHOLDER: &str = "—";
/// Gap below the header block (`Screens.jsx:207` `marginBottom: 28`).
const HEADER_GAP: f32 = 28.0;
/// Gap between the eyebrow and the title (`Screens.jsx:213` `marginTop: 6`).
const EYEBROW_TITLE_GAP: f32 = 6.0;
/// The title's display-face font size (`Screens.jsx:213` `fontSize: 34`) —
/// not an existing `typography` token (nearest are `FS_H1 = 40`/`FS_H2 = 30`).
const TITLE_FS: f32 = 34.0;
/// Gap between standings rows (`Screens.jsx:216` `gap: 10`).
const STANDINGS_ROW_GAP: f32 = 10.0;
/// Gap between the action row's two buttons (`Screens.jsx:229` `gap: 12`),
/// equals `spacing::SPACE_3`.
const ACTION_ROW_GAP: f32 = spacing::SPACE_3;
/// The header eyebrow text (`Screens.jsx:210`, uppercased at draw time,
/// mirroring `Card::paint`'s eyebrow).
const EYEBROW_TEXT: &str = "Race complete";
/// The Final-standings `Card`'s title (`Screens.jsx:214`).
const STANDINGS_TITLE: &str = "Final standings";
/// The title's ink-colored prefix (`Screens.jsx:213`).
const TITLE_PREFIX: &str = "You finished ";

/// One car's finished-race outcome, in caller-supplied rank order.
///
/// `car_index` is the car's **stable identity** (resolves *name* via
/// [`CAR_NAMES`] and *color* via [`car_color`]) — deliberately decoupled from
/// `rank`: a real player can finish P3 with `car_index == 0` (design §
/// *Resolving the Open question*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StandingEntry {
    /// The car's stable identity (0 = the player's car by convention,
    /// though `kind` is the authoritative player/AI signal).
    pub car_index: usize,
    /// `You`/`Ai` (matches `CarChip`'s own prop).
    pub kind: CarKind,
    /// Finishing rank.
    pub rank: u32,
    /// The global turn index this car finished on, or `None` if it never
    /// finished (design KD12; formatted at draw time — `None` renders the
    /// `PLACEHOLDER` em-dash).
    pub finish_turn: Option<u32>,
}

/// The race's summary metrics (fastest lap / tempo / crash count), numeric.
///
/// Formatted at draw time (`to_string` / `{:.2}` / `to_string`), mirroring
/// `lab.rs::oracle_tile_strings`'s numeric-in / string-at-draw contract.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaceSummary {
    /// The fewest turns any car spent on one lap (design KD13); `0` when
    /// no car completed a lap.
    pub fastest_lap: u32,
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
    /// The formatted finish turn (`"{n} turns"`, or the `PLACEHOLDER`
    /// em-dash for a non-finisher).
    pub finish_time: String,
}

/// The three summary-tile labels, in [`summary_tiles`] order (design §
/// *The exact Results label wording*, issue #43 D3 — turn-count units
/// replace the pre-#43 second-based wording).
pub const SUMMARY_LABELS: [&str; 3] = ["Fastest lap, turns", "Tempo, cells/turn", "Crashes"];

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
                finish_time: entry
                    .finish_turn
                    .map_or_else(|| PLACEHOLDER.to_owned(), |turn| format!("{turn} turns")),
            }
        })
        .collect()
}

/// Formats `summary`'s three tile values, in [`SUMMARY_LABELS`] order:
/// `fastest_lap` as an integer (`u32::to_string`), `tempo` at `{:.2}`,
/// `crashes` via `u32::to_string`.
///
/// Plain `fn` — calls `format!` (not const-stable).
#[must_use]
pub fn summary_tiles(summary: RaceSummary) -> [String; 3] {
    [
        summary.fastest_lap.to_string(),
        format!("{:.2}", summary.tempo),
        summary.crashes.to_string(),
    ]
}

/// The response of [`ResultsScreen::show`].
///
/// Carries the "Race again"/"Menu" click flags plus each button's row
/// `Response` (needed by an interaction test that has no AccessKit label to
/// query — mirrors [`crate::screens::lab::LabResponse`]). Not `Copy`/`Debug`
/// — carries `egui::Response`, which is neither.
pub struct ResultsResponse {
    /// `true` iff "Race again" was clicked this frame.
    pub again: bool,
    /// `true` iff "Menu" was clicked this frame.
    pub menu: bool,
    /// The "Race again" button's row `Response`.
    pub again_response: Response,
    /// The "Menu" button's row `Response`.
    pub menu_response: Response,
}

/// The required frame-immutable inputs for [`ResultsScreen::new`].
///
/// Bundles the rank-ordered standings slice and the race summary into one
/// cohesive value (design `2026-07-22-consolidate-render-inputs`).
#[derive(Clone, Copy, Debug)]
pub struct ResultsInput<'a> {
    /// The rank-ordered standings slice.
    pub standings: &'a [StandingEntry],
    /// The race's summary metrics (fastest lap / tempo / crashes).
    pub summary: RaceSummary,
}

/// `ResultsScreen` builder.
///
/// Holds the per-frame draw data (the required [`ResultsInput`]), and an
/// optional "Race again" icon handle. `Copy` (mirrors every other
/// screen/widget builder); not `Debug` — `Option<&TextureHandle>` holds
/// `egui::TextureHandle`, which has no `Debug` (`Button`'s reason,
/// `button.rs`).
#[derive(Clone, Copy)]
pub struct ResultsScreen<'a> {
    input: ResultsInput<'a>,
    again_icon: Option<&'a TextureHandle>,
}

impl<'a> ResultsScreen<'a> {
    /// Builds a `ResultsScreen` from the required [`ResultsInput`]. No icon
    /// by default (text-only "Race again" button, `rotate-ccw` is unvendored
    /// — design § *Icon handling*); set it via [`Self::again_icon`].
    #[must_use]
    pub const fn new(input: ResultsInput<'a>) -> Self {
        Self {
            input,
            again_icon: None,
        }
    }

    /// Sets the "Race again" button's leading icon (`rotate-ccw` glyph
    /// absent from the vendored set — `None` renders text-only, design §
    /// *Icon handling*).
    #[must_use]
    pub const fn again_icon(mut self, icon: &'a TextureHandle) -> Self {
        self.again_icon = Some(icon);
        self
    }

    /// Draws the centered results column (header / Final-standings `Card` /
    /// action row) and returns the player's chosen navigation intent this
    /// frame via [`ResultsResponse`].
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first (same precondition as every
    /// other screen/widget's `paint`/`show`).
    ///
    /// Installs its own `Order::Middle` layer before drawing any `Card`
    /// chrome, mirroring [`crate::screens::lab::LabScreen::show`]'s
    /// documented reason: `Card::show` paints its fill on
    /// `LayerId::background()`, which must render *behind* the caller's own
    /// layer.
    pub fn show(self, ui: &mut Ui) -> ResultsResponse {
        let layer_id = egui::LayerId::new(egui::Order::Middle, ui.id().with("results_screen"));
        let mut screen_ui = ui.new_child(
            egui::UiBuilder::new()
                .layer_id(layer_id)
                .max_rect(ui.max_rect()),
        );
        let ui = &mut screen_ui;

        ui.add_space(spacing::SPACE_12);
        let margin = ((ui.available_width() - CONTENT_MAX_W) / 2.0).max(spacing::SPACE_6);

        let (again_response, menu_response) = ui
            .horizontal(|ui| {
                ui.add_space(margin);
                ui.vertical(|ui| {
                    ui.set_width(CONTENT_MAX_W);

                    let position = player_position(self.input.standings);
                    draw_header(ui, position);
                    ui.add_space(HEADER_GAP);

                    let rows = standings_rows(self.input.standings);
                    let tiles = summary_tiles(self.input.summary);
                    draw_standings_card(ui, &rows, &tiles);

                    ui.add_space(spacing::SPACE_6);
                    draw_action_row(ui, self.again_icon)
                })
                .inner
            })
            .inner;

        ResultsResponse {
            again: again_response.clicked(),
            menu: menu_response.clicked(),
            again_response,
            menu_response,
        }
    }
}

/// Draws the centered header block: a mono uppercase "RACE COMPLETE"
/// eyebrow, then the "You finished P&lt;n&gt;" display-face title (`"You
/// finished "` in `TEXT_INK`, `"P<n>"` in `ACCENT`) — `<n>` from
/// `position` (`None` renders the `"P—"` placeholder, never a panic).
///
/// Uses the two-sequential-`text()`-call two-tone idiom
/// ([`crate::screens::setup::draw_wordmark`]'s precedent), not
/// `egui::text::LayoutJob` — both render pixel-identical output for this
/// case, and the sequential-text idiom is already established in this crate.
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first.
fn draw_header(ui: &mut Ui, position: Option<u32>) {
    ui.vertical_centered(|ui| {
        let eyebrow_font = FontId::new(
            typography::FS_XS,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
        );
        let eyebrow = EYEBROW_TEXT.to_uppercase();
        let eyebrow_galley = ui
            .painter()
            .layout_no_wrap(eyebrow, eyebrow_font, color::TEXT_MUTED);
        let eyebrow_w = eyebrow_galley.size().x;
        let (eyebrow_rect, _response) = ui.allocate_exact_size(
            egui::vec2(eyebrow_w, typography::FS_XS * typography::LH_SNUG),
            Sense::hover(),
        );
        crate::text::paint_galley(
            ui.painter(),
            eyebrow_rect.left_top(),
            Align2::LEFT_TOP,
            eyebrow_galley,
            color::TEXT_MUTED,
        );

        ui.add_space(EYEBROW_TITLE_GAP);

        let suffix = position.map_or_else(|| "P—".to_owned(), |rank| format!("P{rank}"));
        let title_font = FontId::new(TITLE_FS, FontFamily::Name(crate::fonts::ONEST_BOLD.into()));
        let prefix_galley = ui.painter().layout_no_wrap(
            TITLE_PREFIX.to_owned(),
            title_font.clone(),
            color::TEXT_INK,
        );
        let suffix_galley = ui
            .painter()
            .layout_no_wrap(suffix, title_font, color::ACCENT);
        let prefix_w = prefix_galley.size().x;
        let suffix_w = suffix_galley.size().x;
        let (title_rect, _response) =
            ui.allocate_exact_size(egui::vec2(prefix_w + suffix_w, TITLE_FS), Sense::hover());
        let prefix_rect = crate::text::paint_galley(
            ui.painter(),
            title_rect.left_top(),
            Align2::LEFT_TOP,
            prefix_galley,
            color::TEXT_INK,
        );
        crate::text::paint_galley(
            ui.painter(),
            Pos2::new(prefix_rect.max.x, title_rect.min.y),
            Align2::LEFT_TOP,
            suffix_galley,
            color::ACCENT,
        );
    });
}

/// Draws the Final-standings `Card`: one row per `StandingRow` (a `CarChip`
/// left, mono right-aligned finish time), a hairline divider, then the
/// summary `Telemetry` row (`Fastest lap` accent+`s`, `Tempo` default,
/// `Crashes` danger).
fn draw_standings_card(ui: &mut Ui, rows: &[StandingRow], tiles: &[String; 3]) {
    Card::new()
        .title(STANDINGS_TITLE)
        .grid(true)
        .padding(spacing::SPACE_6)
        .show(ui, None::<fn(&mut Ui)>, |ui| {
            for (index, row) in rows.iter().enumerate() {
                if index > 0 {
                    ui.add_space(STANDINGS_ROW_GAP);
                }
                ui.horizontal(|ui| {
                    CarChip::new(row.name)
                        .color(row.color)
                        .rank(row.rank)
                        .kind(row.kind)
                        .show(ui);
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(&row.finish_time)
                                .font(FontId::new(
                                    typography::FS_SM,
                                    FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
                                ))
                                .color(color::TEXT_MUTED),
                        );
                    });
                });
            }

            ui.add_space(spacing::SPACE_5);
            draw_divider(ui);
            ui.add_space(spacing::SPACE_4);

            ui.horizontal(|ui| {
                Telemetry::new(SUMMARY_LABELS[0], &tiles[0])
                    .tone(TelemetryTone::Accent)
                    .show(ui);
                ui.add_space(spacing::SPACE_6);
                Telemetry::new(SUMMARY_LABELS[1], &tiles[1]).show(ui);
                ui.add_space(spacing::SPACE_6);
                Telemetry::new(SUMMARY_LABELS[2], &tiles[2])
                    .tone(TelemetryTone::Danger)
                    .show(ui);
            });
        });
}

/// Draws a `BW_HAIR`/`BORDER_HAIRLINE` horizontal rule spanning the
/// available width (`Screens.jsx:223` `borderTop: '1px solid
/// var(--border-hairline)'`).
fn draw_divider(ui: &mut Ui) {
    let width = ui.available_width();
    let (rect, _response) =
        ui.allocate_exact_size(egui::vec2(width, spacing::BW_HAIR), Sense::hover());
    ui.painter().hline(
        egui::Rangef::new(rect.min.x, rect.max.x),
        rect.center().y,
        egui::Stroke::new(spacing::BW_HAIR, color::BORDER_HAIRLINE),
    );
}

/// Draws the centered action row: primary "Race again" (conditional
/// `again_icon`) + secondary "Menu" — returns their `Response`s in that
/// order.
fn draw_action_row(ui: &mut Ui, again_icon: Option<&TextureHandle>) -> (Response, Response) {
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            let mut again_btn = Button::new("Race again")
                .variant(ButtonVariant::Primary)
                .size(Size::Lg);
            if let Some(icon) = again_icon {
                again_btn = again_btn.icon_left(icon);
            }
            let again_response = again_btn.show(ui);

            ui.add_space(ACTION_ROW_GAP);

            let menu_response = Button::new("Menu")
                .variant(ButtonVariant::Secondary)
                .size(Size::Lg)
                .show(ui);

            (again_response, menu_response)
        })
        .inner
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::{
        PLACEHOLDER, RaceSummary, SUMMARY_LABELS, StandingEntry, player_position, standings_rows,
        summary_tiles,
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
                finish_turn: Some(38),
            },
            StandingEntry {
                car_index: 9,
                kind: CarKind::Ai,
                rank: 2,
                finish_turn: Some(40),
            },
            StandingEntry {
                car_index: 0,
                kind: CarKind::You,
                rank: 3,
                finish_turn: Some(41),
            },
            StandingEntry {
                car_index: 1,
                kind: CarKind::Ai,
                rank: 4,
                finish_turn: None,
            },
        ]
    }

    fn fixture_summary() -> RaceSummary {
        RaceSummary {
            fastest_lap: 12,
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
                finish_turn: Some(10),
            },
            StandingEntry {
                car_index: 1,
                kind: CarKind::Ai,
                rank: 2,
                finish_turn: Some(11),
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

    /// AC3 — finish-turn formatting: `Some(38) -> "38 turns"`, `Some(40) ->
    /// "40 turns"`.
    #[test]
    fn standings_rows_formats_finish_turn() {
        let rows = standings_rows(&fixture_standings());
        assert_eq!(rows[0].finish_time, "38 turns");
        assert_eq!(rows[1].finish_time, "40 turns");
    }

    /// Design KD12 — a non-finisher (`finish_turn: None`) renders
    /// [`PLACEHOLDER`], never a fabricated turn count.
    #[test]
    fn standings_rows_formats_non_finisher_as_placeholder() {
        let rows = standings_rows(&fixture_standings());
        assert_eq!(rows[3].finish_time, PLACEHOLDER);
    }

    /// AC4 — tile values reflect the supplied data.
    #[test]
    fn summary_tiles_reflects_supplied_data() {
        assert_eq!(
            summary_tiles(fixture_summary()),
            ["12".to_owned(), "0.87".to_owned(), "1".to_owned()]
        );
    }

    /// AC4 — the three summary labels are present, in tile order.
    #[test]
    fn summary_labels_are_present_in_order() {
        assert_eq!(
            SUMMARY_LABELS,
            ["Fastest lap, turns", "Tempo, cells/turn", "Crashes"]
        );
    }
}

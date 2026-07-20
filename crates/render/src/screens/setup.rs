//! `SetupScreen` — port of `Screens.jsx`'s `SetupScreen` (design
//! `2026-07-20-render-setup-screen` § *Emission mechanism* / *`show`
//! composition*).
//!
//! Composes the wordmark block, the inputs `Card` (2×`Stepper` +
//! `SegmentedControl` + `Slider`), the primary `Button`, and a footer
//! caption, snapped to the 4px spacing lattice (AC7). Unlike the widget
//! galleries, this module drives the real interactive `show` — there is no
//! separate manual-layout `paint` path to keep in sync.

use crate::screens::{DIFFICULTY_LABELS, Difficulty, RaceConfig};
use crate::tokens::{color, spacing, typography};
use crate::widgets::{Button, ButtonVariant, Card, SegmentedControl, Size, Slider, Stepper};
use egui::{Align2, FontFamily, FontId, Pos2, Response, Sense, Stroke, Ui};

/// The centered content column's fixed width (`Screens.jsx`'s `maxWidth:
/// 560` — a container bound, not an inter-widget gap; design § *`show`
/// composition*).
const CONTENT_MAX_W: f32 = 560.0;

/// The accent dot's diameter (`16×16` = `spacing::SPACE_4`).
const ACCENT_DOT_D: f32 = spacing::SPACE_4;

/// Minimum `cars` (AC1).
const MIN_CARS: i32 = 2;
/// Maximum `cars` (AC1).
const MAX_CARS: i32 = 6;
/// Minimum `laps` (AC2).
const MIN_LAPS: i32 = 1;
/// Maximum `laps` (AC2).
const MAX_LAPS: i32 = 9;
/// Minimum `v_target` (AC4).
const MIN_V_TARGET: f32 = 3.0;
/// Maximum `v_target` (AC4).
const MAX_V_TARGET: f32 = 10.0;
/// `v_target` slider step (AC4).
const V_TARGET_STEP: f32 = 1.0;

/// `SetupScreen` builder — holds the live [`RaceConfig`] state.
///
/// Design § *Emission mechanism*: mirrors the crate's widget-response idiom,
/// where the builder's input value doubles as the emitted output on change.
#[derive(Clone, Copy, Debug)]
pub struct SetupScreen {
    config: RaceConfig,
}

/// The response of [`SetupScreen::show`]: the primary button's `Response`,
/// this frame's live-updated `config`, and whether "Generate track" was
/// clicked this frame.
///
/// Not `Copy` — carries an `egui::Response`, unlike [`RaceConfig`] itself.
#[derive(Debug)]
pub struct SetupResponse {
    /// The primary "Generate track" button's row `Response`.
    pub response: Response,
    /// The live-updated config values this frame — always present, even on
    /// a non-generate frame (AC6).
    pub config: RaceConfig,
    /// `true` iff "Generate track" was clicked this frame (AC6 — emits
    /// nothing until pressed).
    pub generated: bool,
}

impl SetupScreen {
    /// Builds a `SetupScreen` starting from `config`.
    #[must_use]
    pub const fn new(config: RaceConfig) -> Self {
        Self { config }
    }

    /// Composes the wordmark block, the inputs `Card`, the primary button,
    /// and the footer (design § *`show` composition*, top → bottom, snapped
    /// to the 4px lattice), returning the assembled [`SetupResponse`].
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first.
    pub fn show(self, ui: &mut Ui) -> SetupResponse {
        let mut raw_cars = i32::try_from(self.config.cars).unwrap_or(MAX_CARS);
        let mut raw_laps = i32::try_from(self.config.laps).unwrap_or(MAX_LAPS);
        #[allow(
            clippy::cast_precision_loss,
            reason = "v_target is bounded to [3,10] by construction — no \
                      precision loss for such a small integer"
        )]
        let mut raw_v_target = self.config.v_target as f32;
        let mut raw_difficulty = self.config.difficulty;

        let mut generate_response = None;

        ui.add_space(spacing::SPACE_12);

        let margin = ((ui.available_width() - CONTENT_MAX_W) / 2.0).max(spacing::SPACE_6);
        ui.horizontal(|ui| {
            ui.add_space(margin);
            ui.vertical(|ui| {
                ui.set_width(CONTENT_MAX_W);

                draw_wordmark(ui);
                ui.add_space(spacing::SPACE_8);

                let card = Card::new()
                    .eyebrow("New race")
                    .title("Set up the grid")
                    .grid(true)
                    .padding(spacing::SPACE_6);
                card.show(ui, None::<fn(&mut Ui)>, |ui| {
                    ui.horizontal(|ui| {
                        let cars_resp = Stepper::new(raw_cars)
                            .min(MIN_CARS)
                            .max(MAX_CARS)
                            .label("Cars (m)")
                            .show(ui);
                        raw_cars = cars_resp.value;

                        ui.add_space(spacing::SPACE_8);

                        let laps_resp = Stepper::new(raw_laps)
                            .min(MIN_LAPS)
                            .max(MAX_LAPS)
                            .label("Laps")
                            .show(ui);
                        raw_laps = laps_resp.value;
                    });

                    ui.add_space(spacing::SPACE_6);

                    draw_mono_label(ui, "Difficulty (pilot temperature)");
                    ui.add_space(spacing::SPACE_2);

                    let seg_resp =
                        SegmentedControl::new(&DIFFICULTY_LABELS, raw_difficulty.label()).show(ui);
                    if let Some(index) = seg_resp.selected
                        && let Some(difficulty) = Difficulty::from_index(index)
                    {
                        raw_difficulty = difficulty;
                    }

                    ui.add_space(spacing::SPACE_6);

                    let slider_resp = Slider::new(raw_v_target)
                        .min(MIN_V_TARGET)
                        .max(MAX_V_TARGET)
                        .step(V_TARGET_STEP)
                        .label("V_target (design speed)")
                        .show(ui, format_v_target);
                    raw_v_target = slider_resp.value;
                });

                ui.add_space(spacing::SPACE_6);

                ui.vertical_centered(|ui| {
                    let response = Button::new("Generate track")
                        .variant(ButtonVariant::Primary)
                        .size(Size::Lg)
                        .show(ui);
                    generate_response = Some(response);
                });

                ui.add_space(spacing::SPACE_3);
                draw_footer(ui);
            });
        });

        let response = generate_response.expect("the button row unconditionally runs inside show");
        let generated = response.clicked();
        let config = assemble(raw_cars, raw_laps, raw_v_target, raw_difficulty);

        SetupResponse {
            response,
            config,
            generated,
        }
    }
}

/// Assembles a [`RaceConfig`] from raw widget values.
///
/// Defensively clamps each value into its bound (AC1/AC2/AC4) and performs
/// the total (non-panicking) integer conversions. Not `const fn` —
/// `TryFrom`/`f32::round`/`f32::clamp` are not const-stable (design §
/// *Const-ness*).
///
/// # Panics
///
/// Never panics at runtime: `cars`/`laps` are clamped into `[2, 6]`/`[1, 9]`
/// immediately above, so both `u32::try_from` calls always succeed — the
/// `expect`s document that invariant rather than a reachable failure.
#[must_use]
pub fn assemble(cars: i32, laps: i32, v_target: f32, difficulty: Difficulty) -> RaceConfig {
    let cars = cars.clamp(MIN_CARS, MAX_CARS);
    let laps = laps.clamp(MIN_LAPS, MAX_LAPS);
    let v_target = v_target.round().clamp(MIN_V_TARGET, MAX_V_TARGET);

    #[allow(
        clippy::cast_possible_truncation,
        reason = "clamped to [3,10] and rounded — a small finite \
                  integer-valued f32 (design § Risks)"
    )]
    let v_target = v_target as i32;

    RaceConfig {
        cars: u32::try_from(cars).expect("cars is clamped to [2,6] — always fits u32"),
        laps: u32::try_from(laps).expect("laps is clamped to [1,9] — always fits u32"),
        v_target,
        difficulty,
    }
}

/// Formats the slider's `v_target` readout (`Screens.jsx`'s `format={v =>
/// `${v} cells/turn`}`).
#[allow(
    clippy::cast_possible_truncation,
    reason = "the slider value is snapped to integer steps within [3,10] — \
              display-only, no data loss for this bounded domain"
)]
fn format_v_target(v: f32) -> String {
    format!("{} cells/turn", v as i32)
}

/// Draws the centered wordmark block: the accent dot + two-tone `GRAPHITE
/// GP` wordmark (display face, `GP` in `ACCENT`), then the mono uppercase
/// subtitle below.
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first.
fn draw_wordmark(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        let wordmark_font = FontId::new(
            typography::FS_H1,
            FontFamily::Name(crate::fonts::ONEST_BOLD.into()),
        );
        let graphite_w = ui
            .painter()
            .layout_no_wrap(
                "GRAPHITE ".to_owned(),
                wordmark_font.clone(),
                color::TEXT_INK,
            )
            .rect
            .width();
        let gp_w = ui
            .painter()
            .layout_no_wrap("GP".to_owned(), wordmark_font.clone(), color::ACCENT)
            .rect
            .width();
        let row_w = ACCENT_DOT_D + spacing::SPACE_3 + graphite_w + gp_w;
        let row_h = typography::FS_H1.max(ACCENT_DOT_D);

        let (rect, _response) = ui.allocate_exact_size(egui::vec2(row_w, row_h), Sense::hover());

        let dot_center = Pos2::new(rect.min.x + ACCENT_DOT_D / 2.0, rect.center().y);
        ui.painter()
            .circle_filled(dot_center, ACCENT_DOT_D / 2.0, color::ACCENT);
        ui.painter().circle_stroke(
            dot_center,
            ACCENT_DOT_D / 2.0,
            Stroke::new(spacing::BW_2, color::GRAPHITE_900),
        );

        let text_x = rect.min.x + ACCENT_DOT_D + spacing::SPACE_3;
        let text_y = rect.center().y - typography::FS_H1 / 2.0;
        let graphite_rect = ui.painter().text(
            Pos2::new(text_x, text_y),
            Align2::LEFT_TOP,
            "GRAPHITE ",
            wordmark_font.clone(),
            color::TEXT_INK,
        );
        ui.painter().text(
            Pos2::new(graphite_rect.max.x, text_y),
            Align2::LEFT_TOP,
            "GP",
            wordmark_font,
            color::ACCENT,
        );

        ui.add_space(spacing::SPACE_2);

        let subtitle_font = FontId::new(
            typography::FS_XS,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
        );
        let subtitle = "GRID VECTOR RACING".to_uppercase();
        let subtitle_w = ui
            .painter()
            .layout_no_wrap(subtitle.clone(), subtitle_font.clone(), color::TEXT_MUTED)
            .rect
            .width();
        let (subtitle_rect, _response) = ui.allocate_exact_size(
            egui::vec2(subtitle_w, typography::FS_XS * typography::LH_SNUG),
            Sense::hover(),
        );
        ui.painter().text(
            subtitle_rect.left_top(),
            Align2::LEFT_TOP,
            subtitle,
            subtitle_font,
            color::TEXT_MUTED,
        );
    });
}

/// Draws a left-aligned mono uppercase `TEXT_MUTED` label at `FS_XS`,
/// advancing the cursor by its measured height — the difficulty block's
/// caption (`Screens.jsx`'s form-label style, mirrored from
/// `widgets::common::paint_form_label` since `screens` cannot reach that
/// `widgets`-private helper).
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first.
fn draw_mono_label(ui: &mut Ui, text: &str) {
    let width = ui.available_width();
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(width, typography::FS_XS * typography::LH_SNUG),
        Sense::hover(),
    );
    ui.painter().text(
        rect.left_top(),
        Align2::LEFT_TOP,
        text.to_uppercase(),
        FontId::new(
            typography::FS_XS,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
        ),
        color::TEXT_MUTED,
    );
}

/// Draws the centered mono footer caption, `TEXT_FAINT`.
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first.
fn draw_footer(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        const FOOTER_TEXT: &str = "Procedural · closed loop · valid by construction";
        let font = FontId::new(
            typography::FS_XS,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
        );
        let width = ui
            .painter()
            .layout_no_wrap(FOOTER_TEXT.to_owned(), font.clone(), color::TEXT_FAINT)
            .rect
            .width();
        let (rect, _response) = ui.allocate_exact_size(
            egui::vec2(width, typography::FS_XS * typography::LH_SNUG),
            Sense::hover(),
        );
        ui.painter().text(
            rect.left_top(),
            Align2::LEFT_TOP,
            FOOTER_TEXT,
            font,
            color::TEXT_FAINT,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::assemble;
    use crate::screens::Difficulty;

    /// `AC8a` — happy path: mid-range widget values map through to a matching
    /// `RaceConfig`, including `difficulty → temperature`.
    #[test]
    fn assemble_happy_path_round_trips_values() {
        let config = assemble(4, 3, 5.0, Difficulty::Pro);
        assert_eq!(config.cars, 4);
        assert_eq!(config.laps, 3);
        assert_eq!(config.v_target, 5);
        assert_eq!(config.difficulty, Difficulty::Pro);
        crate::test_util::assert_f32(
            "temperature",
            config.temperature(),
            Difficulty::Pro.temperature(),
        );
    }

    /// AC1 — `cars` below/above bounds clamps to `[2, 6]`.
    #[test]
    fn assemble_clamps_cars_to_bounds() {
        assert_eq!(assemble(0, 3, 5.0, Difficulty::Pro).cars, 2);
        assert_eq!(assemble(99, 3, 5.0, Difficulty::Pro).cars, 6);
    }

    /// AC2 — `laps` below/above bounds clamps to `[1, 9]`.
    #[test]
    fn assemble_clamps_laps_to_bounds() {
        assert_eq!(assemble(4, 0, 5.0, Difficulty::Pro).laps, 1);
        assert_eq!(assemble(4, 99, 5.0, Difficulty::Pro).laps, 9);
    }

    /// AC4 — `v_target` rounds and clamps to `[3, 10]`.
    #[test]
    fn assemble_rounds_and_clamps_v_target() {
        assert_eq!(assemble(4, 3, 2.4, Difficulty::Pro).v_target, 3);
        assert_eq!(assemble(4, 3, 10.6, Difficulty::Pro).v_target, 10);
        assert_eq!(assemble(4, 3, 7.0, Difficulty::Pro).v_target, 7);
        assert_eq!(assemble(4, 3, -100.0, Difficulty::Pro).v_target, 3);
        assert_eq!(assemble(4, 3, 1_000.0, Difficulty::Pro).v_target, 10);
    }

    /// Type-conversion totality: extreme `i32` inputs never panic.
    #[test]
    fn assemble_extreme_inputs_do_not_panic() {
        let config = assemble(i32::MIN, i32::MAX, f32::NAN, Difficulty::Ace);
        assert_eq!(config.cars, 2);
        assert_eq!(config.laps, 9);
    }
}

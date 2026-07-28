//! `LabScreen` — port of `Screens.jsx`'s `LabScreen` (design
//! `2026-07-21-render-track-lab-screen` § *Approach*).
//!
//! Draw-only, caller-supplies-data (mirrors
//! [`crate::screens::setup::SetupScreen`]): the screen holds a
//! caller-supplied [`TrackArtifact`] (the canvas fixture, plus source of all
//! four oracle-report tiles), a caller-supplied `[PhaseStatus; 7]`, the
//! header `valid`/`seed`, and two optional icon handles — and emits which
//! action button was clicked this frame via [`LabResponse`]. It never calls
//! `gp_gen::generate`, owns no RNG, and buffers no state (spec § Key
//! decisions).

use crate::BakedTrackGeometry;
use crate::tokens::{color, spacing, typography};
use crate::widgets::{
    Badge, BadgeTone, Button, ButtonVariant, Card, Size, Tag, Telemetry, TelemetryTone,
};
use egui::{Align2, FontFamily, FontId, Layout, Pos2, Rect, Response, TextureHandle, Ui};
use gp_core::track::TrackArtifact;

/// Outer padding inset from the screen's `max_rect` (`Screens.jsx` layout —
/// spec § Technical constraints).
const PAD_OUTER: f32 = 20.0;
/// The right column's fixed width.
const COL_RIGHT_W: f32 = 320.0;
/// Gutter between the left and right columns.
const COL_GAP: f32 = 20.0;
/// Gap between the left column's header / canvas / action bands.
const COL_LEFT_GAP: f32 = 14.0;
/// Gap between the right column's two stacked `Card`s.
const COL_RIGHT_GAP: f32 = 16.0;
/// Gap between header-row items (title / Badge / Tag).
const HEADER_GAP: f32 = 12.0;
/// Gap between the two action-row buttons.
const ACTION_GAP: f32 = 12.0;
/// Gap between generation-phase rows.
const PHASE_ROW_GAP: f32 = 9.0;
/// Gap between the oracle report's 2×2 `Telemetry` tiles.
const ORACLE_GRID_GAP: f32 = 18.0;
/// Left column header band height — fixed at the medium control height (the
/// tallest control in the row, the header `Button` — spec § *header row*).
const HEADER_HEIGHT: f32 = spacing::CONTROL_H_MD;
/// Left column action band height — one medium `Button` row.
const ACTION_HEIGHT: f32 = spacing::CONTROL_H_MD;
/// Canvas border stroke width (spec § Technical constraints).
const CANVAS_BORDER_W: f32 = spacing::BW_1;
/// Canvas border corner radius.
const CANVAS_RADIUS: f32 = spacing::RADIUS_2;

/// The canvas overlays the lab screen always draws with — heatmap +
/// fastest-lap analytics **and** the grid, all on (AC1;
/// `Screens.jsx`'s `showGrid`/`showHeatmap`/`showFastestLap` all `true`).
pub(crate) const LAB_OVERLAYS: crate::Overlays = crate::Overlays {
    speed_heatmap: true,
    fastest_lap: true,
    grid: true,
};

/// Placeholder rendered for an absent `Option` oracle value (Vmax/Tempo
/// only — Width min / S/F width always render a real number).
const PLACEHOLDER: &str = "—";

/// The Ф1–Ф7 generation-pipeline phase ids (`docs/design.md` §2), fixed —
/// only the per-phase [`PhaseStatus`] varies per frame.
const PHASE_IDS: [&str; 7] = ["Ф1", "Ф2", "Ф3", "Ф4", "Ф5", "Ф6", "Ф7"];

/// The Ф1–Ф7 phase names, in [`PHASE_IDS`] order (`Screens.jsx`'s fixed
/// phase-row labels).
const PHASE_NAMES: [&str; 7] = [
    "Coarse ring (infield-first)",
    "Rasterize to points D",
    "Start / finish + grid",
    "Static validation",
    "Passability oracle",
    "Local repair",
    "Output artifact",
];

/// A single Ф1–Ф7 generation-pipeline phase's caller-supplied status
/// (gp-render-local — `gp-render` has no `gp-gen` dependency, spec § Key
/// decisions).
///
/// Declared in ascending severity so the derived [`Ord`] **is** the total
/// order spec § Phase-status ordering requires (`Pending < Skipped < Ok <
/// Repair < Failed`, `AC9b`) — no hand-written `cmp` to drift. "Worst across
/// attempts" is then a plain `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PhaseStatus {
    /// The run is still in flight and this phase has not been reached yet.
    Pending,
    /// The run (or attempt) finished without ever executing this phase.
    Skipped,
    /// The phase completed cleanly.
    Ok,
    /// The phase needed local repair.
    Repair,
    /// The phase produced a blocking issue on some attempt.
    Failed,
}

/// Maps a [`PhaseStatus`] to its status-`Badge` tone + label.
///
/// `Pending` → `Neutral`-tone "…", `Skipped` → `Neutral`-tone "skip", `Ok` →
/// `Ok`-tone "✓", `Repair` → `Warn`-tone "repair", `Failed` → `Danger`-tone
/// "failed" (spec § Key decisions). FORCED `const fn` — a pure `match` over
/// `Copy` values is const-eligible (`clippy::missing_const_for_fn`,
/// nursery = deny).
#[must_use]
pub const fn phase_badge(status: PhaseStatus) -> (BadgeTone, &'static str) {
    match status {
        PhaseStatus::Pending => (BadgeTone::Neutral, "…"),
        PhaseStatus::Skipped => (BadgeTone::Neutral, "skip"),
        PhaseStatus::Ok => (BadgeTone::Ok, "✓"),
        PhaseStatus::Repair => (BadgeTone::Warn, "repair"),
        PhaseStatus::Failed => (BadgeTone::Danger, "failed"),
    }
}

/// Formats the oracle-report tile strings straight off `track` (AC2, order:
/// Vmax, Tempo, Width min, S/F width) — no gp-render-local report struct.
///
/// Spec § Key decisions, round-2 amendment. Vmax/Tempo are `Option`s off
/// `track.metrics` (`None` → the em-dash placeholder); Width min (`u32`) and
/// S/F width ([`gp_core::track::StartFinish::width`], `usize`) are always
/// present and always format a real number.
#[must_use]
pub fn oracle_tile_strings(track: &TrackArtifact) -> [String; 4] {
    let vmax = track
        .metrics
        .vmax_attain
        .map_or_else(|| PLACEHOLDER.to_owned(), |v| v.to_string());
    let tempo = track
        .metrics
        .tempo
        .map_or_else(|| PLACEHOLDER.to_owned(), |v| format!("{v:.2}"));
    let width_min = track.width_min.to_string();
    let sf_width = track.sf.width().to_string();
    [vmax, tempo, width_min, sf_width]
}

/// The response of [`LabScreen::show`].
///
/// Which of the three action buttons (Regenerate / Test lap / Menu) were
/// clicked this frame, plus each button's row `Response` (needed by an
/// interaction test that has no AccessKit label to query — mirrors
/// [`crate::screens::setup::SetupResponse`]). Not `Copy`/`Debug` — carries
/// `egui::Response`, which is neither.
pub struct LabResponse {
    /// `true` iff "Regenerate" was clicked this frame.
    pub regenerate: bool,
    /// `true` iff "Test lap" was clicked this frame.
    pub test_lap: bool,
    /// `true` iff "← Menu" was clicked this frame.
    pub menu: bool,
    /// The "Regenerate" button's row `Response`.
    pub regenerate_response: Response,
    /// The "Test lap" button's row `Response`.
    pub test_lap_response: Response,
    /// The "← Menu" button's row `Response`.
    pub menu_response: Response,
}

/// The required frame-immutable inputs for [`LabScreen::new`].
///
/// Bundles the canvas fixture (also the source of all 4 oracle tiles), the 7
/// caller-supplied generation-phase statuses, and the header `valid`/`seed`
/// into one cohesive value (design `2026-07-22-consolidate-render-inputs`).
#[derive(Clone, Copy, Debug)]
pub struct LabInput<'a> {
    /// The track fixture — drives the canvas and all 4 oracle-report tiles.
    pub track: &'a TrackArtifact,
    /// The baked geometry for `track` (design
    /// `2026-07-22-cache-track-geometry`) — threaded into the canvas
    /// `Scene`.
    pub geometry: &'a BakedTrackGeometry,
    /// The Ф1–Ф7 generation-pipeline phase statuses, in `PHASE_IDS` order.
    pub phases: [PhaseStatus; 7],
    /// The header validity flag (`VALID`/`INVALID` badge).
    pub valid: bool,
    /// The header `seed <N>` tag value.
    pub seed: u64,
    /// The grid's seating outcome (issue #43 D2, spec Open question 3), or
    /// `None` when seating is not yet known. [`header_tag_labels`] only
    /// renders the "seated N of M" notice when `Some` **and** the grid
    /// seated fewer cars than requested — a `Some` at `seated == requested`
    /// is a legitimate no-op input.
    pub seated: Option<SeatedGrid>,
}

/// `LabScreen` builder.
///
/// Holds the per-frame draw data (the required [`LabInput`] plus two
/// optional icon handles). `Copy` (mirrors every other screen/widget
/// builder); not `Debug` — `Option<&TextureHandle>` holds
/// `egui::TextureHandle`, which has no `Debug` (`Button`'s reason,
/// `button.rs`).
#[derive(Clone, Copy)]
pub struct LabScreen<'a> {
    input: LabInput<'a>,
    regenerate_icon: Option<&'a TextureHandle>,
    test_lap_icon: Option<&'a TextureHandle>,
}

impl<'a> LabScreen<'a> {
    /// Builds a `LabScreen` from the required [`LabInput`]. No icons by
    /// default (text-only action buttons) — set them via
    /// [`Self::regenerate_icon`] / [`Self::test_lap_icon`].
    #[must_use]
    pub const fn new(input: LabInput<'a>) -> Self {
        Self {
            input,
            regenerate_icon: None,
            test_lap_icon: None,
        }
    }

    /// Sets the "Regenerate" button's leading icon (shuffle glyph absent
    /// from the vendored set — `None` renders text-only, design § Key
    /// decision 4).
    #[must_use]
    pub const fn regenerate_icon(mut self, icon: &'a TextureHandle) -> Self {
        self.regenerate_icon = Some(icon);
        self
    }

    /// Sets the "Test lap" button's leading icon (`icons::Icon::Play`).
    #[must_use]
    pub const fn test_lap_icon(mut self, icon: &'a TextureHandle) -> Self {
        self.test_lap_icon = Some(icon);
        self
    }

    /// Draws the two-column lab layout (header / canvas / action row on the
    /// left, the oracle-report + generation-phases `Card`s on the right)
    /// and returns which action button was clicked this frame.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first (same precondition as every
    /// other screen/widget's `paint`/`show`).
    ///
    /// Installs its own `Order::Middle` layer before drawing any `Card`
    /// chrome, mirroring [`crate::screens::setup::SetupScreen::show`]'s
    /// documented reason: `Card::show` paints its fill on
    /// `LayerId::background()`, which must render *behind* the caller's own
    /// layer.
    pub fn show(self, ui: &mut Ui) -> LabResponse {
        let layer_id = egui::LayerId::new(egui::Order::Middle, ui.id().with("lab_screen"));
        let mut screen_ui = ui.new_child(
            egui::UiBuilder::new()
                .layer_id(layer_id)
                .max_rect(ui.max_rect()),
        );
        let ui = &mut screen_ui;

        let full = ui.max_rect().shrink(PAD_OUTER);
        let col_right_rect =
            Rect::from_min_max(Pos2::new(full.max.x - COL_RIGHT_W, full.min.y), full.max);
        let col_left_rect = Rect::from_min_max(
            full.min,
            Pos2::new(col_right_rect.min.x - COL_GAP, full.max.y),
        );

        let header_rect = Rect::from_min_size(
            col_left_rect.min,
            egui::vec2(col_left_rect.width(), HEADER_HEIGHT),
        );
        let action_rect = Rect::from_min_max(
            Pos2::new(col_left_rect.min.x, col_left_rect.max.y - ACTION_HEIGHT),
            col_left_rect.max,
        );
        let canvas_rect = Rect::from_min_max(
            Pos2::new(col_left_rect.min.x, header_rect.max.y + COL_LEFT_GAP),
            Pos2::new(col_left_rect.max.x, action_rect.min.y - COL_LEFT_GAP),
        );

        let menu_response = draw_header(
            ui,
            header_rect,
            self.input.valid,
            self.input.seed,
            self.input.seated,
        );
        draw_canvas(ui, canvas_rect, self.input.track, self.input.geometry);
        let (regenerate_response, test_lap_response) =
            draw_action_row(ui, action_rect, self.regenerate_icon, self.test_lap_icon);
        draw_right_column(ui, col_right_rect, self.input.track, self.input.phases);

        LabResponse {
            regenerate: regenerate_response.clicked(),
            test_lap: test_lap_response.clicked(),
            menu: menu_response.clicked(),
            regenerate_response,
            test_lap_response,
            menu_response,
        }
    }
}

/// A short grid's seating outcome (issue #43 D2, spec Open question 3):
/// `seated` cars were actually seated out of `requested`
/// (`seated <= requested`, AC14's "seat fewer and race" floor).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeatedGrid {
    /// The number of cars actually seated.
    pub seated: u32,
    /// The number of cars requested (`--cars`).
    pub requested: u32,
}

/// Formats the header's `Tag` labels: `"seed <N>"` always, plus `"seated N
/// of M"` when short (issue #43 D1/D2).
///
/// `seed` is `u64`, widened from the pre-#43 `i32` so any master seed
/// round-trips without truncation. The "seated N of M" label appears only
/// when `seated` is `Some` **and** the grid seated fewer cars than
/// requested.
///
/// Pure formatter, the `oracle_tile_strings` precedent (`:127`) — no
/// `Ui`/`Context`, so it needs no Miri gate and is directly assertable
/// without a rendered frame. `draw_header` draws one [`Tag`] per returned
/// label. An absent (or non-degraded) seating notice allocates nothing
/// beyond the always-present seed label — the AC17 golden-safety condition
/// (D2 widens no existing Lab golden fixture, which all construct
/// `seated: None`).
#[must_use]
pub fn header_tag_labels(seed: u64, seated: Option<SeatedGrid>) -> Vec<String> {
    let mut labels = vec![format!("seed {seed}")];
    if let Some(SeatedGrid { seated, requested }) = seated
        && seated < requested
    {
        labels.push(format!("seated {seated} of {requested}"));
    }
    labels
}

/// Draws the header band: display-face "Track lab" title, a validity
/// `Badge`, one `Tag` per [`header_tag_labels`] entry, and a right-aligned
/// ghost "← Menu" `Button` — returns the Menu button's `Response`.
fn draw_header(
    ui: &mut Ui,
    rect: Rect,
    valid: bool,
    seed: u64,
    seated: Option<SeatedGrid>,
) -> Response {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal_centered(|ui| {
            let title_font = FontId::new(
                typography::FS_H2,
                FontFamily::Name(crate::fonts::ONEST_BOLD.into()),
            );
            let title_galley =
                ui.painter()
                    .layout_no_wrap("Track lab".to_owned(), title_font, color::TEXT_INK);
            let title_width = title_galley.size().x;
            let (title_rect, _resp) = ui.allocate_exact_size(
                egui::vec2(title_width, typography::FS_H2),
                egui::Sense::hover(),
            );
            crate::text::paint_galley(
                ui.painter(),
                Pos2::new(title_rect.min.x, title_rect.center().y),
                Align2::LEFT_CENTER,
                title_galley,
                color::TEXT_INK,
            );

            ui.add_space(HEADER_GAP);
            let tone = if valid {
                BadgeTone::Ok
            } else {
                BadgeTone::Warn
            };
            let label = if valid { "VALID" } else { "INVALID" };
            Badge::new(tone, label).show(ui);

            for label in header_tag_labels(seed, seated) {
                ui.add_space(HEADER_GAP);
                Tag::new(&label).selected(true).show(ui);
            }

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                Button::new("← Menu")
                    .variant(ButtonVariant::Ghost)
                    .size(Size::Sm)
                    .show(ui)
            })
            .inner
        })
        .inner
    })
    .inner
}

/// Draws the canvas border (rounded-rect stroke, clip inside) then renders
/// `track` via [`crate::render_frame`] with [`LAB_OVERLAYS`] and an empty
/// car slice (`Screens.jsx`'s `cars={[]}`, spec § *Key decisions*).
fn draw_canvas(ui: &mut Ui, rect: Rect, track: &TrackArtifact, geometry: &BakedTrackGeometry) {
    let painter = ui.painter();
    painter.rect_stroke(
        rect,
        egui::CornerRadius::from(CANVAS_RADIUS),
        egui::Stroke::new(CANVAS_BORDER_W, color::GRAPHITE_900),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(CANVAS_BORDER_W);
    crate::render_frame(
        painter,
        inner,
        crate::Scene {
            track,
            geometry,
            cars: &[],
            reduced_motion: false,
            overlays: LAB_OVERLAYS,
        },
    );
    ui.allocate_rect(rect, egui::Sense::hover());
}

/// Draws the action row: primary "Regenerate" (conditional shuffle icon) +
/// secondary "Test lap" (conditional play icon) — returns their `Response`s
/// in that order.
fn draw_action_row(
    ui: &mut Ui,
    rect: Rect,
    regenerate_icon: Option<&TextureHandle>,
    test_lap_icon: Option<&TextureHandle>,
) -> (Response, Response) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal_centered(|ui| {
            let mut regenerate_btn = Button::new("Regenerate").variant(ButtonVariant::Primary);
            if let Some(icon) = regenerate_icon {
                regenerate_btn = regenerate_btn.icon_left(icon);
            }
            let regenerate_response = regenerate_btn.show(ui);

            ui.add_space(ACTION_GAP);

            let mut test_lap_btn = Button::new("Test lap").variant(ButtonVariant::Secondary);
            if let Some(icon) = test_lap_icon {
                test_lap_btn = test_lap_btn.icon_left(icon);
            }
            let test_lap_response = test_lap_btn.show(ui);

            (regenerate_response, test_lap_response)
        })
        .inner
    })
    .inner
}

/// Draws the right column's two stacked `Card`s: the oracle report (2×2
/// `Telemetry` grid, AC2) and the generation-phases list (7 rows, AC3).
fn draw_right_column(ui: &mut Ui, rect: Rect, track: &TrackArtifact, phases: [PhaseStatus; 7]) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_width(COL_RIGHT_W);

            let [vmax, tempo, width_min, sf_width] = oracle_tile_strings(track);
            Card::new()
                .eyebrow("Passability + metrics")
                .show(ui, None::<fn(&mut Ui)>, |ui| {
                    ui.horizontal(|ui| {
                        Telemetry::new("VMAX", &vmax).unit("c/t").show(ui);
                        ui.add_space(ORACLE_GRID_GAP);
                        Telemetry::new("TEMPO", &tempo)
                            .tone(TelemetryTone::Accent)
                            .show(ui);
                    });
                    ui.add_space(ORACLE_GRID_GAP);
                    ui.horizontal(|ui| {
                        Telemetry::new("WIDTH MIN", &width_min).unit("pts").show(ui);
                        ui.add_space(ORACLE_GRID_GAP);
                        Telemetry::new("S/F WIDTH", &sf_width).unit("pts").show(ui);
                    });
                });

            ui.add_space(COL_RIGHT_GAP);

            Card::new()
                .eyebrow("Ф1 – Ф7")
                .show(ui, None::<fn(&mut Ui)>, |ui| {
                    for (row, (&id, &name)) in PHASE_IDS.iter().zip(PHASE_NAMES.iter()).enumerate()
                    {
                        if row > 0 {
                            ui.add_space(PHASE_ROW_GAP);
                        }
                        ui.horizontal(|ui| {
                            ui.painter().text(
                                ui.cursor().min,
                                Align2::LEFT_TOP,
                                id,
                                FontId::new(
                                    typography::FS_SM,
                                    FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
                                ),
                                color::TEXT_MUTED,
                            );
                            ui.add_space(spacing::SPACE_8);
                            ui.label(
                                egui::RichText::new(name)
                                    .font(FontId::new(
                                        typography::FS_SM,
                                        FontFamily::Name(crate::fonts::ONEST_REGULAR.into()),
                                    ))
                                    .color(color::TEXT_BODY),
                            );
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                let (tone, label) = phase_badge(phases[row]);
                                Badge::new(tone, label).show(ui);
                            });
                        });
                    }
                });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{
        LAB_OVERLAYS, PHASE_IDS, PHASE_NAMES, PhaseStatus, SeatedGrid, header_tag_labels,
        oracle_tile_strings, phase_badge,
    };
    use crate::widgets::BadgeTone;
    use gp_core::geom::{Corridor, Orient, Point, Side};
    use gp_core::track::{
        Centerline, RaceDir, SField, StartFinish, StartGrid, TimingGate, TrackArtifact,
        TrackMetrics,
    };

    /// A minimal `TrackArtifact` fixture — every field the oracle tiles /
    /// canvas don't read stays at its cheapest valid default. `metrics` and
    /// `sf.chord`/`width_min` are set by the caller per-test.
    fn fixture(metrics: TrackMetrics, width_min: u32, chord_len: usize) -> TrackArtifact {
        let corridor = Corridor::new(Point::new(0, 0), 1, 1);
        TrackArtifact {
            walls: vec![],
            sf: StartFinish {
                chord: vec![Point::new(0, 0); chord_len],
                orient: Orient::Horizontal,
                gate: TimingGate {
                    behind: vec![],
                    forward: Side::East,
                },
            },
            corridor,
            race_dir: RaceDir::Cw,
            s_field: SField::default(),
            start_grid: StartGrid::default(),
            centerline: Centerline::default(),
            metrics,
            width_min,
        }
    }

    /// AC1 — `LAB_OVERLAYS` has `speed_heatmap`/`fastest_lap`/`grid` all
    /// `true`. Context-free (no `egui::Context`) → un-gated.
    #[test]
    fn lab_overlays_are_all_on() {
        const {
            assert!(LAB_OVERLAYS.speed_heatmap);
            assert!(LAB_OVERLAYS.fastest_lap);
            assert!(LAB_OVERLAYS.grid);
        }
    }

    /// AC2 — a fully-populated artifact: Vmax/Tempo `Some`, Width min / S/F
    /// width always a real number (the JSX exemplars).
    #[test]
    fn oracle_tile_strings_full_values() {
        let track = fixture(
            TrackMetrics {
                vmax_attain: Some(7),
                tempo: Some(0.87),
                ..TrackMetrics::default()
            },
            3,
            4,
        );
        assert_eq!(
            oracle_tile_strings(&track),
            [
                "7".to_owned(),
                "0.87".to_owned(),
                "3".to_owned(),
                "4".to_owned()
            ]
        );
    }

    /// AC2 — Vmax/Tempo `None` → the "—" placeholder; Width min / S/F width
    /// are non-`Option` and still render a real number.
    #[test]
    fn oracle_tile_strings_absent_metrics_placeholder() {
        let track = fixture(TrackMetrics::default(), 3, 4);
        assert_eq!(
            oracle_tile_strings(&track),
            [
                "—".to_owned(),
                "—".to_owned(),
                "3".to_owned(),
                "4".to_owned()
            ]
        );
    }

    /// AC3 — exactly 7 phase ids/names, and the `Ok`→`Ok`-tone / `Repair`→
    /// `Warn`-tone badge mapping.
    #[test]
    fn phase_tables_have_seven_entries_and_correct_tones() {
        assert_eq!(PHASE_IDS.len(), 7);
        assert_eq!(PHASE_NAMES.len(), 7);
        assert_eq!(phase_badge(PhaseStatus::Ok), (BadgeTone::Ok, "✓"));
        assert_eq!(
            phase_badge(PhaseStatus::Repair),
            (BadgeTone::Warn, "repair")
        );
    }

    /// `AC9b` — `PhaseStatus`'s total order is `Pending < Skipped < Ok <
    /// Repair < Failed`, asserted pairwise, plus `max` over a mixed sequence
    /// (including `Ok > Skipped`, the ordering fact spec § Phase-status
    /// ordering calls out explicitly). Pure `Ord` — no `Context` → ungated.
    #[test]
    fn phase_status_total_order_is_declaration_order() {
        use PhaseStatus::{Failed, Ok, Pending, Repair, Skipped};

        let ascending = [Pending, Skipped, Ok, Repair, Failed];
        for pair in ascending.windows(2) {
            assert!(pair[0] < pair[1], "{pair:?} should be strictly ascending");
        }
        assert!(Ok > Skipped);
        assert_eq!(
            [Skipped, Failed, Ok, Pending].into_iter().max(),
            Some(Failed)
        );
        assert_eq!([Pending, Skipped, Ok].into_iter().max(), Some(Ok));
    }

    /// `AC15` — `header_tag_labels` is pure `u64` formatting: a value
    /// exceeding `i32::MAX` (the pre-D1 parameter type) round-trips without
    /// truncation. Context-free → ungated.
    #[test]
    fn header_tag_labels_formats_u64_seed_without_truncation() {
        assert_eq!(header_tag_labels(42, None), vec!["seed 42".to_owned()]);
        assert_eq!(
            header_tag_labels(2_147_483_648, None),
            vec!["seed 2147483648".to_owned()]
        );
    }

    /// `AC14` — the "seated N of M" notice: absent (`None`) and
    /// not-degraded (`seated == requested`) both yield a **one**-element
    /// vec (the AC17 golden-safety condition — an absent notice allocates
    /// nothing beyond the seed label); a genuinely short grid appends the
    /// second label.
    #[test]
    fn header_tag_labels_seated_notice_only_when_short() {
        assert_eq!(header_tag_labels(42, None), vec!["seed 42".to_owned()]);
        assert_eq!(
            header_tag_labels(
                42,
                Some(SeatedGrid {
                    seated: 6,
                    requested: 6
                })
            ),
            vec!["seed 42".to_owned()]
        );
        assert_eq!(
            header_tag_labels(
                42,
                Some(SeatedGrid {
                    seated: 3,
                    requested: 6
                })
            ),
            vec!["seed 42".to_owned(), "seated 3 of 6".to_owned()]
        );
    }

    /// `AC9b` — the ordering comes from `#[derive(Ord)]`, not a hand-written
    /// `cmp`. A `rg` scan over this file finds no manual `impl (Partial)?Ord
    /// for PhaseStatus`.
    #[test]
    fn phase_status_has_no_hand_written_ord_impl() {
        let src = include_str!("lab.rs");
        // Built from parts so this assertion's own source line doesn't trip
        // the scan it performs (the AC24 `include_str!` self-match trap).
        let needle_ord = format!("impl {} for PhaseStatus", "Ord");
        let needle_partial_ord = format!("impl Partial{} for PhaseStatus", "Ord");
        assert!(
            !src.contains(&needle_ord) && !src.contains(&needle_partial_ord),
            "PhaseStatus must derive Ord/PartialOrd, not hand-implement it"
        );
    }
}

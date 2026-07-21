//! `RaceScreen` — port of `Screens.jsx`'s `RaceScreen` (design
//! `2026-07-21-render-race-screen` § *Approach*).
//!
//! Draw-only, caller-supplies-data (mirrors [`crate::screens::lab::LabScreen`]
//! / [`crate::screens::setup::SetupScreen`]): the screen holds a
//! caller-supplied [`TrackArtifact`], a caller-supplied `&[CarRender<'a>]`
//! slice + active-car index, the current [`crate::Overlays`], and the
//! caller-tracked lap counters — and emits the player's selected [`Action`]
//! (if any), the toggled [`crate::Overlays`], and a `finish` click signal via
//! [`RaceResponse`]. It never calls `gp_core::sim::step`/`resolve_crash`/
//! `resolve_collisions`/`LapCounter` — those are `gp-game` orchestration
//! (spec § Scope).

use crate::tokens::{color, spacing};
use crate::widgets::{
    Button, ButtonVariant, CarChip, CarKind, Card, LapMeter, MovePad, MovePadResponse, Size,
    Switch, Telemetry, TelemetryTone,
};
use crate::{CarRender, Overlays, Scene};
use egui::{Layout, Pos2, Rect, Response, Ui};
use gp_core::sim::{Action, BitFlags, CarState, legal_mask};
use gp_core::track::TrackArtifact;

/// Outer padding inset from the screen's `max_rect` (`Screens.jsx:98`
/// `padding: 20`).
const PAD_OUTER: f32 = 20.0;
/// The right column's fixed width (`Screens.jsx:98`
/// `gridTemplateColumns: '1fr 300px'`).
const COL_RIGHT_W: f32 = 300.0;
/// Gutter between the left and right columns (`Screens.jsx:98` `gap: 20`).
const COL_GAP: f32 = 20.0;
/// Gap between the left column's HUD / toolbar / canvas bands
/// (`Screens.jsx:100` `gap: 14`).
const LEFT_STACK_GAP: f32 = 14.0;
/// Gap between the right column's two stacked `Card`s (`Screens.jsx`'s
/// `"Your move"`/`"Standings"` cards, not JSX-literal — mirrors
/// `lab.rs::COL_RIGHT_GAP`'s role).
const RIGHT_STACK_GAP: f32 = 16.0;

/// HUD band horizontal padding (`Screens.jsx:102` `padding: '14px 20px'`).
const HUD_PAD_X: f32 = 20.0;
/// HUD band vertical padding (`Screens.jsx:102` `padding: '14px 20px'`).
const HUD_PAD_Y: f32 = 14.0;
/// Gap between HUD tiles (`Screens.jsx:102` `gap: 28`).
const HUD_GAP: f32 = 28.0;
/// The right-aligned `LapMeter`'s fixed width (`Screens.jsx:107`
/// `minWidth: 130`).
const HUD_LAPMETER_W: f32 = 130.0;
/// HUD band corner radius (`Screens.jsx:102` `borderRadius: 'var(--radius-2)'`).
const HUD_RADIUS: f32 = spacing::RADIUS_2;
/// HUD band's fixed height — sized to fit the `lg` `SPEED` tile (label +
/// value stack) under `HUD_PAD_Y` top/bottom padding; not JSX-literal (the
/// JSX lets flexbox size the row).
const HUD_HEIGHT: f32 = 72.0;

/// Toolbar row height — the tallest control in the row (the `sm` "Finish →"
/// `Button`, `spacing::CONTROL_H_SM`); not JSX-literal (JSX lets flexbox size
/// the row).
const TOOLBAR_HEIGHT: f32 = spacing::CONTROL_H_SM;
/// Gap between toolbar `Switch`es (`Screens.jsx:113` `gap: 10`).
const TOOLBAR_GAP: f32 = 10.0;

/// Track canvas border stroke width (`Screens.jsx:120`
/// `border: '1.5px solid var(--graphite-900)'`), equals `spacing::BW_1`.
const CANVAS_BORDER_W: f32 = spacing::BW_1;
/// Track canvas border corner radius (`Screens.jsx:120`
/// `borderRadius: 'var(--radius-2)'`).
const CANVAS_RADIUS: f32 = spacing::RADIUS_2;

/// `MovePad` cell edge (`Screens.jsx:129` `size={52}`, overriding
/// `MovePad.jsx`'s own `48` default).
const MOVEPAD_SIZE: f32 = 52.0;
/// Gap between the `MovePad` and the helper caption (`Screens.jsx:128`
/// `margin: '4px 0 14px'`, second value).
const MOVEPAD_CAPTION_GAP: f32 = 14.0;
/// Gap between the helper caption and the "Coast (·)" `Button`
/// (`Screens.jsx:133` `marginTop: 12`).
const CAPTION_BUTTON_GAP: f32 = 12.0;
/// Gap between `CarChip` rows in the Standings `Card` (`Screens.jsx:139`
/// `gap: 8`).
const STANDINGS_GAP: f32 = 8.0;

/// The mono helper caption under the `MovePad` (`Screens.jsx:131-132`,
/// two lines joined by `<br />`).
const MOVE_CAPTION: &str = "±1 per axis · no diagonal accel\nsupercover ⊆ D";

/// The ported `CAR_NAMES` table (`Screens.jsx:8`), in car-index order.
///
/// Access is total via `CAR_NAMES.get(i).copied().unwrap_or("Car")` — a
/// slice longer than 6 cars never panics, consistent with
/// [`CarRender::color`](crate::track::CarRender::color)'s no-panic-on-bad-index
/// posture.
pub const CAR_NAMES: [&str; 6] = [
    "You",
    "Rival Blue",
    "Rival Green",
    "Rival Amber",
    "Rival Plum",
    "Rival Teal",
];

/// The response of [`RaceScreen::show`].
///
/// Carries the selected [`Action`] this frame (if any — Coast-button click
/// wins over a `MovePad` change, per the design's action precedence), the
/// toggled [`Overlays`], the `finish` click signal, and each interactive
/// widget's row `Response` (needed by an interaction test that has no
/// AccessKit label to query — mirrors
/// [`crate::screens::lab::LabResponse`]). Not `Copy`/`Debug` — carries
/// `egui::Response`, which is neither.
pub struct RaceResponse {
    /// The selected action this frame, or `None` if neither the `MovePad`
    /// nor the Coast shortcut was clicked.
    pub action: Option<Action>,
    /// The toggled overlays (mirrors the `Switch`/`SetupScreen` value-in /
    /// value-out idiom).
    pub overlays: Overlays,
    /// `true` iff the "Finish →" button was clicked this frame.
    pub finish: bool,
    /// The `MovePad`'s whole-pad response.
    pub movepad_response: Response,
    /// The "Coast (·)" shortcut button's response.
    pub coast_response: Response,
    /// The "Finish →" button's response.
    pub finish_response: Response,
}

/// The required frame-immutable inputs for [`RaceScreen::new`].
///
/// Bundles the canvas [`Scene`] (track, cars, reduced-motion, overlays) with
/// the active-car index and the caller-tracked lap counters into one
/// cohesive value (design `2026-07-22-consolidate-render-inputs`).
#[derive(Clone, Copy, Debug)]
pub struct RaceInput<'a> {
    /// The canvas scene — track, cars, reduced-motion, and the current
    /// overlays.
    pub scene: Scene<'a>,
    /// The active (player-controlled) car's index into `scene.cars`.
    pub active: usize,
    /// The caller-tracked completed-lap count.
    pub laps_done: i32,
    /// The caller-tracked total lap count.
    pub total_laps: i32,
}

/// `RaceScreen` builder.
///
/// Holds the per-frame draw data (the required [`RaceInput`]). `Copy`
/// (mirrors every other screen/widget builder).
#[derive(Clone, Copy)]
pub struct RaceScreen<'a> {
    input: RaceInput<'a>,
}

impl<'a> RaceScreen<'a> {
    /// Builds a `RaceScreen` from the required [`RaceInput`].
    #[must_use]
    pub const fn new(input: RaceInput<'a>) -> Self {
        Self { input }
    }

    /// Draws the two-column race layout (HUD strip / overlay toolbar /
    /// bordered track canvas on the left, "Your move" / "Standings" `Card`s
    /// on the right) and returns the player's selected action (if any), the
    /// toggled overlays, and the `finish` click signal this frame.
    ///
    /// Action precedence (design § *Approach*): a "Coast (·)" button click
    /// wins over a `MovePad` change; if neither fired this frame,
    /// `action` is `None`.
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
    pub fn show(self, ui: &mut Ui) -> RaceResponse {
        let layer_id = egui::LayerId::new(egui::Order::Middle, ui.id().with("race_screen"));
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

        let hud_rect = Rect::from_min_size(
            col_left_rect.min,
            egui::vec2(col_left_rect.width(), HUD_HEIGHT),
        );
        let toolbar_rect = Rect::from_min_size(
            Pos2::new(col_left_rect.min.x, hud_rect.max.y + LEFT_STACK_GAP),
            egui::vec2(col_left_rect.width(), TOOLBAR_HEIGHT),
        );
        let canvas_rect = Rect::from_min_max(
            Pos2::new(col_left_rect.min.x, toolbar_rect.max.y + LEFT_STACK_GAP),
            col_left_rect.max,
        );

        let active_car_state = active_state(self.input.scene.cars, self.input.active);
        draw_hud(
            ui,
            hud_rect,
            active_car_state,
            self.input.laps_done,
            self.input.total_laps,
        );

        let (overlays, finish_response) = draw_toolbar(ui, toolbar_rect, self.input.scene.overlays);

        draw_canvas(
            ui,
            canvas_rect,
            Scene {
                overlays,
                ..self.input.scene
            },
        );

        let legal = active_legal_mask(
            self.input.scene.track,
            self.input.scene.cars,
            self.input.active,
        );
        let (movepad_response, coast_response) = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .max_rect(col_right_rect)
                    .layout(Layout::top_down(egui::Align::Min)),
                |ui| {
                    ui.set_width(COL_RIGHT_W);
                    let (movepad_response, coast_response) = draw_your_move(ui, legal);
                    ui.add_space(RIGHT_STACK_GAP);
                    draw_standings(ui, self.input.scene.cars);
                    (movepad_response, coast_response)
                },
            )
            .inner;

        let action = if coast_response.clicked() {
            Some(Action::Coast)
        } else if movepad_response.changed {
            movepad_response.selected
        } else {
            None
        };

        RaceResponse {
            action,
            overlays,
            finish: finish_response.clicked(),
            movepad_response: movepad_response.response,
            coast_response,
            finish_response,
        }
    }
}

/// Draws the dark `GRAPHITE_900` HUD band: 3 on-ink `Telemetry` tiles bound
/// to `state` (`SPEED` accent/`lg` = `|v|`, `v` = `(vx, vy)`, `POS` =
/// `(x, y)`) plus a right-aligned on-ink `LapMeter` (`laps_done`/`total_laps`).
fn draw_hud(ui: &mut Ui, rect: Rect, state: CarState, laps_done: i32, total_laps: i32) {
    ui.painter()
        .rect_filled(rect, HUD_RADIUS, color::GRAPHITE_900);

    let content = rect.shrink2(egui::vec2(HUD_PAD_X, HUD_PAD_Y));
    let lapmeter_rect = Rect::from_min_max(
        Pos2::new(content.max.x - HUD_LAPMETER_W, content.min.y),
        content.max,
    );
    let tiles_rect = Rect::from_min_max(
        content.min,
        Pos2::new(lapmeter_rect.min.x - HUD_GAP, content.max.y),
    );

    let (speed, v, pos) = hud_readouts(state);
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(tiles_rect)
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| {
            Telemetry::new("SPEED", &speed)
                .tone(TelemetryTone::Accent)
                .size(Size::Lg)
                .on_ink(true)
                .show(ui);
            ui.add_space(HUD_GAP);
            Telemetry::new("v", &v).on_ink(true).show(ui);
            ui.add_space(HUD_GAP);
            Telemetry::new("POS", &pos).on_ink(true).show(ui);
        },
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(lapmeter_rect), |ui| {
        LapMeter::new(laps_done, total_laps).on_ink(true).show(ui);
    });
}

/// Draws the overlay toolbar: 3 `Switch`es (Grid / Heatmap / Fastest lap)
/// bound to `overlays`, plus a right-aligned ghost "Finish →" `Button`.
/// Returns the toggled overlays (via [`overlays_from_switches`]) and the
/// Finish button's response.
fn draw_toolbar(ui: &mut Ui, rect: Rect, overlays: Overlays) -> (Overlays, Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| {
            let grid = Switch::new(overlays.grid).label("Grid").show(ui);
            ui.add_space(TOOLBAR_GAP);
            let heatmap = Switch::new(overlays.speed_heatmap)
                .label("Heatmap")
                .show(ui);
            ui.add_space(TOOLBAR_GAP);
            let fastest = Switch::new(overlays.fastest_lap)
                .label("Fastest lap")
                .show(ui);

            let finish_response = ui
                .with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    Button::new("Finish →")
                        .variant(ButtonVariant::Ghost)
                        .size(Size::Sm)
                        .show(ui)
                })
                .inner;

            let new_overlays =
                overlays_from_switches(grid.checked, heatmap.checked, fastest.checked);
            (new_overlays, finish_response)
        },
    )
    .inner
}

/// Draws the canvas border (`1.5px` `GRAPHITE_900` stroke, `radius-2`) then
/// renders `scene` via [`crate::render_frame`].
///
/// `scene.overlays` is the **live** overlays for this frame — the caller
/// reconstructs `scene` from the toolbar's return value (not
/// `self.input.scene` wholesale) so a same-frame toolbar toggle is reflected
/// on the canvas (design § Risks — interactive-toggle threading).
fn draw_canvas(ui: &mut Ui, rect: Rect, scene: Scene<'_>) {
    let painter = ui.painter();
    painter.rect_stroke(
        rect,
        egui::CornerRadius::from(CANVAS_RADIUS),
        egui::Stroke::new(CANVAS_BORDER_W, color::GRAPHITE_900),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(CANVAS_BORDER_W);
    crate::render_frame(painter, inner, scene);
    ui.allocate_rect(rect, egui::Sense::hover());
}

/// Draws the "Your move" `Card`: a `MovePad` (size `52`) bound to `legal`,
/// the mono helper caption, and a full-width secondary "Coast (·)" `Button`
/// shortcut. Returns the `MovePad`'s response and the Coast button's
/// response.
fn draw_your_move(ui: &mut Ui, legal: BitFlags<Action>) -> (MovePadResponse, Response) {
    let mut movepad_response = None;
    let mut coast_response = None;
    Card::new()
        .title("Your move")
        .eyebrow("Turn — choose acceleration")
        .show(ui, None::<fn(&mut Ui)>, |ui| {
            ui.vertical_centered(|ui| {
                movepad_response = Some(MovePad::new(legal).size(MOVEPAD_SIZE).show(ui));
            });
            ui.add_space(MOVEPAD_CAPTION_GAP);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(MOVE_CAPTION)
                        .font(egui::FontId::new(
                            crate::tokens::typography::FS_XS,
                            egui::FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
                        ))
                        .color(color::TEXT_MUTED),
                );
            });
            ui.add_space(CAPTION_BUTTON_GAP);
            coast_response = Some(
                Button::new("Coast (·)")
                    .variant(ButtonVariant::Secondary)
                    .size(Size::Sm)
                    .full_width(true)
                    .show(ui),
            );
        });
    (
        movepad_response.expect("Card::show always invokes add_contents"),
        coast_response.expect("Card::show always invokes add_contents"),
    )
}

/// Draws the "Standings" `Card`: one `CarChip` per car
/// ([`standings_entry`] derives name/kind/rank/active).
fn draw_standings(ui: &mut Ui, cars: &[CarRender<'_>]) {
    Card::new()
        .title("Standings")
        .show(ui, None::<fn(&mut Ui)>, |ui| {
            for (index, car) in cars.iter().enumerate() {
                if index > 0 {
                    ui.add_space(STANDINGS_GAP);
                }
                let (name, kind, rank, active) = standings_entry(index);
                CarChip::new(name)
                    .color(car.color())
                    .rank(rank)
                    .kind(kind)
                    .active(active)
                    .show(ui);
            }
        });
}

/// The active car's `(x, y, vx, vy)` state, falling back to
/// [`CarState::default`] for an empty slice or out-of-range `active` — never
/// a panic (design § *Internal data-shape decisions*).
fn active_state(cars: &[CarRender<'_>], active: usize) -> CarState {
    cars.get(active)
        .map(|render| render.state)
        .unwrap_or_default()
}

/// Formats the HUD readout strings off the active car's `CarState` (AC1,
/// order: `SPEED` = `|v|` 2dp, `v` = `(vx, vy)`, `POS` = `(x, y)`).
///
/// Plain `fn` — allocates `String`s and calls `f32::hypot` (not
/// const-stable), so `missing_const_for_fn` does not force `const`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "cell/velocity coordinates are grid-realistic i32s, far below f32's \
              exact-integer range; precedent: track/car.rs::lerp_pos"
)]
pub fn hud_readouts(state: CarState) -> (String, String, String) {
    let speed = f32::hypot(state.vx as f32, state.vy as f32);
    (
        format!("{speed:.2}"),
        format!("({}, {})", state.vx, state.vy),
        format!("({}, {})", state.x, state.y),
    )
}

/// The active car's legal-action mask (AC3).
///
/// `gp_core::sim::legal_mask` over `track`'s corridor at `cars[active]`'s
/// state, falling back to `CarState::default()` for an empty slice or
/// out-of-range `active` — never a panic (design § *Internal data-shape
/// decisions*).
///
/// Plain `fn` — calls `legal_mask` (not `const`), so `missing_const_for_fn`
/// does not force `const`.
#[must_use]
pub fn active_legal_mask(
    track: &TrackArtifact,
    cars: &[CarRender<'_>],
    active: usize,
) -> BitFlags<Action> {
    legal_mask(&track.corridor, active_state(cars, active))
}

/// Maps the three toolbar `Switch`es' checked states to an [`Overlays`]
/// value (AC2): each `bool` drives exactly its own field.
///
/// FORCED `const fn` — a pure struct literal over `bool`s is const-eligible
/// (`clippy::missing_const_for_fn`, nursery = deny).
#[must_use]
pub const fn overlays_from_switches(grid: bool, heatmap: bool, fastest_lap: bool) -> Overlays {
    Overlays {
        grid,
        speed_heatmap: heatmap,
        fastest_lap,
    }
}

/// One Standings row's derived (name, kind, rank, active) tuple (AC5).
///
/// Index `0` is the player (`CarKind::You`, active, rank `1`); every other
/// index is `CarKind::Ai` (inactive, `rank = index + 1`). Name access is
/// total via [`CAR_NAMES`] (`.get(i).copied().unwrap_or("Car")`) — an
/// out-of-range index never panics.
///
/// Plain `fn`, not `const` — `u32::try_from` is not yet a const-stable
/// `TryFrom` impl, matching `movepad.rs::cell_rect`'s `f32::from(u8)`
/// precedent for the same class of gap.
#[must_use]
pub fn standings_entry(index: usize) -> (&'static str, CarKind, u32, bool) {
    let name = CAR_NAMES.get(index).copied().unwrap_or("Car");
    let kind = if index == 0 {
        CarKind::You
    } else {
        CarKind::Ai
    };
    let active = index == 0;
    let rank = u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX);
    (name, kind, rank, active)
}

#[cfg(test)]
mod tests {
    use super::{
        CAR_NAMES, active_legal_mask, hud_readouts, overlays_from_switches, standings_entry,
    };
    use crate::CarRender;
    use crate::widgets::CarKind;
    use gp_core::geom::{Corridor, Point};
    use gp_core::sim::CarState;
    use gp_core::track::{
        Centerline, RaceDir, SField, StartFinish, StartGrid, TrackArtifact, TrackMetrics,
    };

    fn car(x: i32, y: i32, vx: i32, vy: i32) -> CarState {
        CarState { x, y, vx, vy }
    }

    /// A minimal drivable-rectangle `TrackArtifact` fixture; only
    /// `corridor` is exercised by [`active_legal_mask`].
    fn fixture_track(w: usize, h: usize) -> TrackArtifact {
        let mut corridor = Corridor::new(Point::new(0, 0), w, h);
        for y in 0..i32::try_from(h).unwrap() {
            for x in 0..i32::try_from(w).unwrap() {
                corridor.set(Point::new(x, y), true);
            }
        }
        TrackArtifact {
            walls: vec![],
            sf: StartFinish {
                chord: vec![],
                orient: gp_core::geom::Orient::Horizontal,
                gate: gp_core::track::TimingGate {
                    behind: vec![],
                    forward: gp_core::geom::Side::East,
                },
            },
            corridor,
            race_dir: RaceDir::Cw,
            s_field: SField::default(),
            start_grid: StartGrid::default(),
            centerline: Centerline::default(),
            metrics: TrackMetrics::default(),
            width_min: 0,
        }
    }

    /// AC1 — `(3, 4)` velocity yields speed `"5.00"`.
    #[test]
    fn hud_readouts_computes_hypot_speed() {
        let (speed, v, pos) = hud_readouts(car(0, 0, 3, 4));
        assert_eq!(speed, "5.00");
        assert_eq!(v, "(3, 4)");
        assert_eq!(pos, "(0, 0)");
    }

    /// AC1 — zero velocity yields `"0.00"` and `"(0, 0)"`.
    #[test]
    fn hud_readouts_zero_velocity() {
        let (speed, v, _pos) = hud_readouts(car(5, 5, 0, 0));
        assert_eq!(speed, "0.00");
        assert_eq!(v, "(0, 0)");
    }

    /// AC1 — negative coordinates/velocity format with signs.
    #[test]
    fn hud_readouts_negative_values_format_with_signs() {
        let (_speed, v, pos) = hud_readouts(car(-2, -3, -1, 2));
        assert_eq!(v, "(-1, 2)");
        assert_eq!(pos, "(-2, -3)");
    }

    /// AC3 — `active_legal_mask` equals `legal_mask(&track.corridor,
    /// cars[active].state)` for an in-range active index.
    #[test]
    fn active_legal_mask_matches_core_legal_mask() {
        let track = fixture_track(5, 5);
        let state = car(2, 2, 0, 0);
        let trail: [Point; 0] = [];
        let cars = [CarRender::new(state, 0, &trail, true, 0.0)];
        let mask = active_legal_mask(&track, &cars, 0);
        assert_eq!(mask, gp_core::sim::legal_mask(&track.corridor, state));
        assert!(!mask.is_empty());
    }

    /// AC3 — an out-of-range `active` (or empty slice) falls back to
    /// `CarState::default()` — no panic.
    #[test]
    fn active_legal_mask_out_of_range_falls_back_to_default_state() {
        let track = fixture_track(5, 5);
        let cars: [CarRender<'_>; 0] = [];
        let mask = active_legal_mask(&track, &cars, 0);
        assert_eq!(
            mask,
            gp_core::sim::legal_mask(&track.corridor, CarState::default())
        );

        let trail: [Point; 0] = [];
        let one_car = [CarRender::new(car(1, 1, 0, 0), 0, &trail, true, 0.0)];
        let mask2 = active_legal_mask(&track, &one_car, 7);
        assert_eq!(
            mask2,
            gp_core::sim::legal_mask(&track.corridor, CarState::default())
        );
    }

    /// AC2 — each switch flag maps to exactly its own `Overlays` field.
    #[test]
    fn overlays_from_switches_maps_each_flag_independently() {
        let all_off = overlays_from_switches(false, false, false);
        assert!(!all_off.grid);
        assert!(!all_off.speed_heatmap);
        assert!(!all_off.fastest_lap);

        let grid_only = overlays_from_switches(true, false, false);
        assert!(grid_only.grid);
        assert!(!grid_only.speed_heatmap);
        assert!(!grid_only.fastest_lap);

        let heatmap_only = overlays_from_switches(false, true, false);
        assert!(!heatmap_only.grid);
        assert!(heatmap_only.speed_heatmap);
        assert!(!heatmap_only.fastest_lap);

        let fastest_only = overlays_from_switches(false, false, true);
        assert!(!fastest_only.grid);
        assert!(!fastest_only.speed_heatmap);
        assert!(fastest_only.fastest_lap);

        let all_on = overlays_from_switches(true, true, true);
        assert!(all_on.grid);
        assert!(all_on.speed_heatmap);
        assert!(all_on.fastest_lap);
    }

    /// AC2 — the JSX initial default `(grid=true, heatmap=false,
    /// fastest=false)` matches `overlays_from_switches(true, false, false)`.
    #[test]
    fn overlays_from_switches_initial_default_matches_jsx() {
        let default = overlays_from_switches(true, false, false);
        assert!(default.grid);
        assert!(!default.speed_heatmap);
        assert!(!default.fastest_lap);
    }

    /// AC5 — index 0 is the player (`You`, active, rank 1); index k>0 is
    /// `Ai` (inactive, `rank = k+1`).
    #[test]
    fn standings_entry_index_zero_is_player() {
        let (name, kind, rank, active) = standings_entry(0);
        assert_eq!(name, "You");
        assert_eq!(kind, CarKind::You);
        assert_eq!(rank, 1);
        assert!(active);
    }

    /// AC5 — a non-zero index is an inactive `Ai` with `rank = index + 1`.
    #[test]
    fn standings_entry_nonzero_index_is_ai() {
        let (name, kind, rank, active) = standings_entry(2);
        assert_eq!(name, "Rival Green");
        assert_eq!(kind, CarKind::Ai);
        assert_eq!(rank, 3);
        assert!(!active);
    }

    /// AC5 — `CAR_NAMES` has exactly 6 entries, `CAR_NAMES[0] == "You"`.
    #[test]
    fn car_names_length_and_first_entry() {
        assert_eq!(CAR_NAMES.len(), 6);
        assert_eq!(CAR_NAMES[0], "You");
    }

    /// AC5 — an out-of-range standings index still returns a name (the
    /// `"Car"` fallback), never a panic.
    #[test]
    fn standings_entry_out_of_range_index_falls_back_to_car() {
        let (name, kind, rank, active) = standings_entry(9);
        assert_eq!(name, "Car");
        assert_eq!(kind, CarKind::Ai);
        assert_eq!(rank, 10);
        assert!(!active);
    }
}

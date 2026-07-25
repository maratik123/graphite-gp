//! `AppShell` — the draw-only router + top bar (design § *Approach*, issue
//! #23), ported from `docs/design-system/ui_kits/game/App.jsx`.
//!
//! Composes the four existing screens (`SetupScreen`/`LabScreen`/
//! `RaceScreen`/`ResultsScreen`) into one navigable flow. The shell **owns**
//! the minimal cross-transition state the spec fixes — the current
//! [`Screen`], the [`RaceConfig`], the [`Overlays`], and a `has_generated`
//! latch — and **borrows** all externally-sourced session data per frame
//! through [`ShellSession`] (`gp-render` is draw-only and has no dependency
//! on `gp-gen`/`gp-ai` — `ai-docs/key-decisions.md`; it does depend on
//! `gp-core`, including `gp_core::sim::Action` for [`ShellResponse::action`]).
//!
//! The transition logic is split from drawing: [`AppShell::apply`] and
//! [`AppShell::can_nav`] touch no `egui` type, so AC1/AC2/AC5/AC6/AC7 are
//! plain state-machine unit tests with no `egui::Context` — un-Miri-gated
//! (AGENTS.md § *Rust Test Conventions* — the gate's trigger is *constructs a
//! Context/painter*, which these do not).

use crate::screens::setup::SetupScreen;
use crate::screens::{LabInput, LabScreen, RaceInput, RaceScreen, ResultsInput, ResultsScreen};
use crate::tokens::{color, spacing, typography};
use crate::{
    BakedTrackGeometry, CarRender, Overlays, PhaseStatus, RaceConfig, RaceSummary, Scene,
    StandingEntry,
};
use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Ui};
use gp_core::sim::Action;
use gp_core::track::TrackArtifact;

/// Top bar band height — not JSX-literal (`App.jsx`'s header lets flexbox
/// size the row); sized to fit the header's top/bottom padding around its
/// tallest content (a padded nav item), mirroring `race.rs::HUD_HEIGHT`'s
/// fixed-height convention.
pub(crate) const TOP_BAR_H: f32 = HEADER_PAD_Y * 2.0 + NAV_ITEM_H;

/// Header horizontal padding (`App.jsx`'s header `padding: '10px 18px'`,
/// second value) — not a `spacing` token (nearest are `SPACE_4`/`SPACE_5`).
const HEADER_PAD_X: f32 = 18.0;
/// Header vertical padding (`App.jsx`'s header `padding: '10px 18px'`, first
/// value) — not a `spacing` token (nearest are `SPACE_2`/`SPACE_3`).
const HEADER_PAD_Y: f32 = 10.0;
/// Gap between the accent dot and the wordmark text (`App.jsx`'s wordmark
/// block `gap: 9`) — not a `spacing` token (nearest is `SPACE_2`).
const WORDMARK_GAP: f32 = 9.0;
/// Gap from the wordmark block to the nav row (`App.jsx`'s `nav` `marginLeft:
/// 8`), equals `spacing::SPACE_2`.
const NAV_MARGIN_L: f32 = spacing::SPACE_2;
/// Gap between nav items (`App.jsx`'s `nav` `gap: 4`), equals
/// `spacing::SPACE_1`.
const NAV_GAP: f32 = spacing::SPACE_1;
/// Accent dot diameter (`App.jsx`'s wordmark dot `13×13`) — not a `spacing`
/// token (nearest is `CELL_SM = 16`).
const ACCENT_DOT_D: f32 = 13.0;
/// Wordmark font size (`App.jsx`'s wordmark `fontSize: 19`) — not a
/// `typography` token (nearest are `FS_TITLE = 18`/`FS_H3 = 22`).
const WORDMARK_FS: f32 = 19.0;
/// Nav item horizontal padding (`App.jsx`'s `NavItem` `padding: '8px 14px'`,
/// second value) — not a `spacing` token (nearest are `SPACE_3`/`SPACE_4`).
const NAV_PAD_X: f32 = 14.0;
/// Nav item fixed height — the `8px` top/bottom padding
/// (`App.jsx`'s `NavItem` `padding: '8px 14px'`, first value, equals
/// `spacing::SPACE_2`) around one `FS_BODY` text line; not JSX-literal (JSX
/// lets flexbox size the button).
const NAV_ITEM_H: f32 = 34.0;
/// Nav item disabled-state opacity multiplier. Mirrors
/// `widgets::common::DISABLED_OPACITY`'s value — that const is private to
/// `widgets` (`mod common;`), unreachable from this top-level module, so the
/// value is redeclared here rather than imported.
const NAV_DISABLED_OPACITY: f32 = 0.45;

/// The four navigable screens (`App.jsx`'s `setup | race | lab | results`).
///
/// "Menu" is not a distinct screen — it routes to `Setup` (design § Key
/// decisions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// The new-race configuration screen.
    Setup,
    /// The generated-track lab/inspection screen.
    Lab,
    /// The live race screen.
    Race,
    /// The finished-race results screen.
    Results,
}

/// The nav items shown in the top bar, in display order (`App.jsx`'s three
/// `NavItem`s — `Results` is reached only via `RaceResponse.finish` /
/// `Nav::Finish`, never a nav item).
const NAV_ITEMS: [(Screen, &str); 3] = [
    (Screen::Setup, "New race"),
    (Screen::Race, "Race"),
    (Screen::Lab, "Track lab"),
];

/// One frame's navigation intent, derived from a screen's `*Response` or a
/// top-bar nav click (design § *Navigation intent enum*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    /// `SetupResponse.generated` — advances to [`Screen::Lab`], latches
    /// `has_generated`.
    Generate,
    /// `LabResponse.test_lap` — advances to [`Screen::Race`].
    TestLap,
    /// `LabResponse.menu` / `ResultsResponse.menu` — returns to
    /// [`Screen::Setup`].
    Menu,
    /// `LabResponse.regenerate` — a no-op; stays on [`Screen::Lab`] (AC5).
    Regenerate,
    /// `RaceResponse.finish` — advances to [`Screen::Results`].
    Finish,
    /// `ResultsResponse.again` — returns to [`Screen::Race`].
    Again,
    /// A top-bar nav click — jumps directly to the target screen, subject
    /// to [`AppShell::can_nav`] (AC6/AC7).
    JumpTo(Screen),
}

/// The draw-only router (design § *Approach*).
///
/// Owns only `Copy`, `gp-render`-local state: the current [`Screen`], the
/// [`RaceConfig`], the [`Overlays`], and the `has_generated` latch.
/// Everything sourced from `gp-gen`/`gp-ai` is borrowed per frame through
/// [`ShellSession`] (design § *Owned vs borrowed*), as is the inbound
/// [`gp_core::sim::CarState`] inside each [`ShellSession::cars`] entry. The
/// shell's one *outbound* `gp_core::sim` value is [`ShellResponse::action`].
#[derive(Clone, Copy, Debug)]
pub struct AppShell {
    screen: Screen,
    config: RaceConfig,
    overlays: Overlays,
    has_generated: bool,
}

impl AppShell {
    /// Builds a fresh shell on [`Screen::Setup`] with `config` as the
    /// starting `RaceConfig` (design § Key decisions — the mock's startup
    /// default is the caller's job to supply). FORCED `const fn` — a pure
    /// struct literal over `Copy` values (`clippy::missing_const_for_fn`,
    /// nursery = deny).
    #[must_use]
    pub const fn new(config: RaceConfig) -> Self {
        Self {
            screen: Screen::Setup,
            config,
            overlays: Overlays {
                speed_heatmap: false,
                fastest_lap: false,
                grid: false,
            },
            has_generated: false,
        }
    }

    /// The current screen.
    #[must_use]
    pub const fn screen(&self) -> Screen {
        self.screen
    }

    /// The current `RaceConfig`.
    #[must_use]
    pub const fn config(&self) -> RaceConfig {
        self.config
    }

    /// The current `Overlays`.
    #[must_use]
    pub const fn overlays(&self) -> Overlays {
        self.overlays
    }

    /// Whether the first `Generate` intent has fired (AC7's guard latch).
    #[must_use]
    pub const fn has_generated(&self) -> bool {
        self.has_generated
    }

    /// Replaces the owned `RaceConfig` (the shell's Setup-frame refresh
    /// path, design § *Owned vs borrowed*).
    pub const fn set_config(&mut self, config: RaceConfig) {
        self.config = config;
    }

    /// Replaces the owned `Overlays` (the shell's Race-frame refresh path).
    pub const fn set_overlays(&mut self, overlays: Overlays) {
        self.overlays = overlays;
    }

    /// Whether a top-bar jump to `target` is currently allowed (AC7):
    /// `Setup` is always enabled; `Race`/`Lab` require `has_generated`.
    /// FORCED `const fn` — a pure `matches!`/`||` over `Copy` values.
    #[must_use]
    pub const fn can_nav(&self, target: Screen) -> bool {
        matches!(target, Screen::Setup) || self.has_generated
    }

    /// Applies one frame's navigation intent (design § *Navigation intent
    /// enum*). A guarded [`Nav::JumpTo`] whose target fails [`Self::can_nav`]
    /// is a no-op (AC7); [`Nav::Regenerate`] is always a no-op (AC5). FORCED
    /// `const fn` — a pure `match` assigning `Copy` values through
    /// `&mut self` (`clippy::missing_const_for_fn`, nursery = deny).
    pub const fn apply(&mut self, nav: Nav) {
        match nav {
            Nav::Generate => {
                self.has_generated = true;
                self.screen = Screen::Lab;
            }
            // `TestLap` and `Again` both return to `Race` (from Lab / from
            // Results respectively) — one merged arm, not a duplicate.
            Nav::TestLap | Nav::Again => self.screen = Screen::Race,
            Nav::Menu => self.screen = Screen::Setup,
            Nav::Finish => self.screen = Screen::Results,
            Nav::JumpTo(target) if self.can_nav(target) => self.screen = target,
            // No-ops: `Regenerate` stays on the current screen; a `JumpTo` to a
            // not-yet-reachable screen is ignored.
            Nav::Regenerate | Nav::JumpTo(_) => {}
        }
    }

    /// Draws the top bar plus the current screen's body into `ui`'s
    /// `max_rect`, applies the derived [`Nav`] (screen intent takes
    /// precedence over a same-frame nav-bar click — design § *Navigation
    /// intent enum*; the two are disjoint hit regions, so both firing in one
    /// frame is unreachable, but the precedence keeps `apply` single-valued),
    /// and returns the resulting [`ShellResponse`].
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first (the same precondition every
    /// screen's own `show` documents).
    pub fn show(&mut self, ui: &mut Ui, session: ShellSession<'_>) -> ShellResponse {
        let full = ui.max_rect();
        let top_bar_rect = Rect::from_min_size(full.min, egui::vec2(full.width(), TOP_BAR_H));
        let body_rect = Rect::from_min_max(Pos2::new(full.min.x, top_bar_rect.max.y), full.max);

        // The app's page background (`App.jsx`'s root `--surface-page`),
        // painted once for the whole shell so the app reads as one fixed
        // design-token palette independent of egui's ambient light/dark
        // visuals (PR #118 round 3 — the body previously inherited whatever
        // `Visuals` the host set, which was black in `gp-game`). The top
        // bar's own `PAPER_0` fill below paints over the header band.
        ui.painter().rect_filled(full, 0, color::SURFACE_PAGE);

        let jump = ui
            .scope_builder(egui::UiBuilder::new().max_rect(top_bar_rect), |ui| {
                draw_top_bar(ui, self.screen, self.has_generated)
            })
            .inner;

        let (screen_nav, advance_rect, action) = ui
            .scope_builder(egui::UiBuilder::new().max_rect(body_rect), |ui| {
                self.show_body(ui, session)
            })
            .inner;

        let nav = screen_nav.or_else(|| jump.map(Nav::JumpTo));
        if let Some(nav) = nav {
            self.apply(nav);
        }

        ShellResponse {
            screen: self.screen,
            advance_rect,
            action,
        }
    }

    /// Draws the current screen's body, threading the owned `config`/
    /// `overlays` and the borrowed `session` into the matching screen's
    /// input struct. Returns the screen-derived `Nav` (if any), the forward
    /// control's rect, and — for [`Screen::Race`] only — the player's
    /// selected [`Action`] this frame (every other screen yields `None`).
    fn show_body(
        &mut self,
        ui: &mut Ui,
        session: ShellSession<'_>,
    ) -> (Option<Nav>, Rect, Option<Action>) {
        match self.screen {
            Screen::Setup => {
                let resp = SetupScreen::new(self.config).show(ui);
                self.config = resp.config;
                (
                    resp.generated.then_some(Nav::Generate),
                    resp.response.rect,
                    None,
                )
            }
            Screen::Lab => {
                let input = LabInput {
                    track: session.track,
                    geometry: session.geometry,
                    phases: session.phases,
                    valid: session.valid,
                    seed: session.seed,
                };
                let resp = LabScreen::new(input).show(ui);
                let nav = if resp.test_lap {
                    Some(Nav::TestLap)
                } else if resp.menu {
                    Some(Nav::Menu)
                } else if resp.regenerate {
                    Some(Nav::Regenerate)
                } else {
                    None
                };
                (nav, resp.test_lap_response.rect, None)
            }
            Screen::Race => {
                let input = RaceInput {
                    scene: Scene {
                        track: session.track,
                        geometry: session.geometry,
                        cars: session.cars,
                        reduced_motion: session.reduced_motion,
                        overlays: self.overlays,
                    },
                    active: session.active,
                    laps_done: session.laps_done,
                    total_laps: session.total_laps,
                };
                let resp = RaceScreen::new(input).show(ui);
                self.overlays = resp.overlays;
                (
                    resp.finish.then_some(Nav::Finish),
                    resp.finish_response.rect,
                    resp.action,
                )
            }
            Screen::Results => {
                let input = ResultsInput {
                    standings: session.standings,
                    summary: session.summary,
                };
                let resp = ResultsScreen::new(input).show(ui);
                let nav = if resp.again {
                    Some(Nav::Again)
                } else if resp.menu {
                    Some(Nav::Menu)
                } else {
                    None
                };
                (nav, resp.menu_response.rect, None)
            }
        }
    }
}

/// The frame-immutable, externally-sourced session data [`AppShell::show`]
/// borrows (design § *Owned vs borrowed*).
///
/// `gp-render` cannot own any of this, since it has no dependency on
/// `gp-gen`/`gp-ai`. The shell selects the fields the current screen needs;
/// fields for a screen not currently active are simply unread that frame.
#[derive(Clone, Copy, Debug)]
pub struct ShellSession<'a> {
    /// The track fixture — `Lab`'s canvas + oracle tiles, `Race`'s canvas.
    pub track: &'a TrackArtifact,
    /// The baked geometry for `track` (design
    /// `2026-07-22-cache-track-geometry`) — `Lab`'s and `Race`'s canvas.
    pub geometry: &'a BakedTrackGeometry,
    /// Per-frame car render input — `Race`'s canvas.
    pub cars: &'a [CarRender<'a>],
    /// Snaps every car's move animation to its final position — `Race`'s
    /// canvas.
    pub reduced_motion: bool,
    /// The active (player-controlled) car's index — `Race`.
    pub active: usize,
    /// The caller-tracked completed-lap count — `Race`.
    pub laps_done: i32,
    /// The caller-tracked total lap count — `Race`.
    pub total_laps: i32,
    /// The Ф1–Ф7 generation-pipeline phase statuses — `Lab`.
    pub phases: [PhaseStatus; 7],
    /// The header validity flag — `Lab`.
    pub valid: bool,
    /// The header `seed <N>` tag value — `Lab`.
    pub seed: i32,
    /// The rank-ordered standings slice — `Results`.
    pub standings: &'a [StandingEntry],
    /// The race summary metrics — `Results`.
    pub summary: RaceSummary,
}

/// The response of [`AppShell::show`]: the resulting current [`Screen`] plus
/// the current screen's forward control's rect (the click target an
/// interaction test drives — design § *Rejected alternatives*).
#[derive(Clone, Copy, Debug)]
pub struct ShellResponse {
    /// The screen the shell is on after this frame's `Nav` was applied.
    pub screen: Screen,
    /// The current screen's forward control's rect (`SetupScreen`'s
    /// Generate button / `LabScreen`'s Test-lap button /
    /// `RaceScreen`'s Finish button / `ResultsScreen`'s Menu button).
    pub advance_rect: Rect,
    /// The player's selected [`Action`] this frame, forwarded from
    /// `RaceResponse::action` when [`Screen::Race`] is active; `None` on
    /// every other screen and on any frame the player has not yet decided
    /// (spec `2026-07-25-game-controller-player` Scope 6). This is the
    /// `MovePad`/Coast-button path's only route out of the shell.
    pub action: Option<Action>,
}

/// Draws the top bar (wordmark + interactive nav row) into `ui`'s
/// `max_rect`, returning the nav item clicked this frame (if any and
/// enabled).
fn draw_top_bar(ui: &mut Ui, current: Screen, has_generated: bool) -> Option<Screen> {
    let full_rect = ui.max_rect();
    ui.painter().rect_filled(full_rect, 0, color::PAPER_0);
    ui.painter().hline(
        full_rect.x_range(),
        full_rect.max.y - spacing::BW_HAIR,
        Stroke::new(spacing::BW_HAIR, color::BORDER_HAIRLINE),
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(full_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.add_space(HEADER_PAD_X);
            draw_wordmark(ui);
            ui.add_space(NAV_MARGIN_L);

            let mut jump = None;
            for &(screen, label) in &NAV_ITEMS {
                let active = current == screen;
                let enabled = matches!(screen, Screen::Setup) || has_generated;
                let (_response, clicked) = nav_item(ui, label, active, enabled);
                if clicked {
                    jump = Some(screen);
                }
                ui.add_space(NAV_GAP);
            }
            jump
        },
    )
    .inner
}

/// Draws the accent-dot + two-tone `GRAPHITE GP` wordmark (`App.jsx`'s
/// header wordmark block — a smaller sibling of
/// `crate::screens::setup::draw_wordmark`'s `FS_H1` display).
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first.
fn draw_wordmark(ui: &mut Ui) {
    let font = FontId::new(
        WORDMARK_FS,
        FontFamily::Name(crate::fonts::ONEST_BOLD.into()),
    );
    let graphite_galley =
        ui.painter()
            .layout_no_wrap("GRAPHITE ".to_owned(), font.clone(), color::TEXT_INK);
    let gp_galley = ui
        .painter()
        .layout_no_wrap("GP".to_owned(), font, color::ACCENT);
    let graphite_w = graphite_galley.size().x;
    let gp_w = gp_galley.size().x;
    let row_w = ACCENT_DOT_D + WORDMARK_GAP + graphite_w + gp_w;
    let row_h = WORDMARK_FS.max(ACCENT_DOT_D);
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(row_w, row_h), Sense::hover());

    let dot_center = Pos2::new(rect.min.x + ACCENT_DOT_D / 2.0, rect.center().y);
    ui.painter()
        .circle_filled(dot_center, ACCENT_DOT_D / 2.0, color::ACCENT);
    ui.painter().circle_stroke(
        dot_center,
        ACCENT_DOT_D / 2.0,
        Stroke::new(spacing::BW_2, color::GRAPHITE_900),
    );

    let text_x = rect.min.x + ACCENT_DOT_D + WORDMARK_GAP;
    let text_y = rect.center().y - WORDMARK_FS / 2.0;
    let graphite_rect = crate::text::paint_galley(
        ui.painter(),
        Pos2::new(text_x, text_y),
        Align2::LEFT_TOP,
        graphite_galley,
        color::TEXT_INK,
    );
    crate::text::paint_galley(
        ui.painter(),
        Pos2::new(graphite_rect.max.x, text_y),
        Align2::LEFT_TOP,
        gp_galley,
        color::ACCENT,
    );
}

/// Draws one top-bar nav item pill (`App.jsx`'s `NavItem`): active =
/// `GRAPHITE_900` fill + `PAPER_0` text; inactive = transparent fill +
/// `TEXT_BODY` text; disabled dims both via [`NAV_DISABLED_OPACITY`] and
/// only senses hover (never a click). Returns the item's `Response` plus
/// whether it was clicked (always `false` when disabled).
///
/// # Panics
///
/// Panics at layout time if the caller has not installed
/// [`crate::fonts::definitions`] first.
fn nav_item(ui: &mut Ui, label: &str, active: bool, enabled: bool) -> (egui::Response, bool) {
    let font = FontId::new(
        typography::FS_BODY,
        FontFamily::Name(crate::fonts::ONEST_MEDIUM.into()),
    );
    let label_galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, color::TEXT_BODY);
    let text_w = label_galley.size().x;
    let width = NAV_PAD_X.mul_add(2.0, text_w);

    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, NAV_ITEM_H), sense);

    let opacity = if enabled { 1.0 } else { NAV_DISABLED_OPACITY };
    let (bg, fg) = if active {
        (color::GRAPHITE_900, color::PAPER_0)
    } else {
        (Color32::TRANSPARENT, color::TEXT_BODY)
    };

    if ui.is_rect_visible(rect) {
        ui.painter()
            .rect_filled(rect, spacing::RADIUS_2, bg.gamma_multiply(opacity));
        crate::text::paint_galley_override(
            ui.painter(),
            rect.center(),
            Align2::CENTER_CENTER,
            label_galley,
            fg.gamma_multiply(opacity),
        );
    }

    let clicked = enabled && response.clicked();
    (response, clicked)
}

#[cfg(test)]
mod tests {
    use super::{AppShell, Nav, Screen};
    use crate::{Difficulty, Overlays, RaceConfig};

    /// The mock's startup default (design § Key decisions).
    const FIXED_CONFIG: RaceConfig = RaceConfig {
        cars: 4,
        laps: 5,
        v_target: 7,
        difficulty: Difficulty::Pro,
    };

    /// AC1 — the linear flow: Setup → Lab → Race → Results on the
    /// corresponding intents, and Results/Lab → Menu (Setup) on `Menu`.
    #[test]
    fn linear_flow_setup_lab_race_results_menu() {
        let mut shell = AppShell::new(FIXED_CONFIG);
        assert_eq!(shell.screen(), Screen::Setup);

        shell.apply(Nav::Generate);
        assert_eq!(shell.screen(), Screen::Lab);

        shell.apply(Nav::TestLap);
        assert_eq!(shell.screen(), Screen::Race);

        shell.apply(Nav::Finish);
        assert_eq!(shell.screen(), Screen::Results);

        shell.apply(Nav::Menu);
        assert_eq!(shell.screen(), Screen::Setup);

        // Lab → Menu (Setup) too.
        let mut from_lab = AppShell::new(FIXED_CONFIG);
        from_lab.apply(Nav::Generate);
        assert_eq!(from_lab.screen(), Screen::Lab);
        from_lab.apply(Nav::Menu);
        assert_eq!(from_lab.screen(), Screen::Setup);
    }

    /// AC2 — `RaceConfig` set on Setup is unchanged on Lab/Race/Results;
    /// `Overlays` set on Race persists while the shell stays in the race
    /// sub-flow.
    #[test]
    fn config_and_overlays_persist_across_transitions() {
        let non_default = RaceConfig {
            cars: 6,
            laps: 9,
            v_target: 10,
            difficulty: Difficulty::Ace,
        };
        let mut shell = AppShell::new(non_default);

        shell.apply(Nav::Generate);
        assert_eq!(shell.config(), non_default, "config unchanged on Lab");

        shell.apply(Nav::TestLap);
        assert_eq!(shell.config(), non_default, "config unchanged on Race");

        let overlays = Overlays {
            speed_heatmap: true,
            fastest_lap: true,
            grid: false,
        };
        shell.set_overlays(overlays);
        assert_eq!(shell.overlays(), overlays);
        assert_eq!(
            shell.screen(),
            Screen::Race,
            "overlays persist while still on Race"
        );

        shell.apply(Nav::Finish);
        assert_eq!(shell.config(), non_default, "config unchanged on Results");
    }

    /// AC5 — `Regenerate` on Lab does not change the current screen.
    #[test]
    fn regenerate_does_not_change_screen() {
        let mut shell = AppShell::new(FIXED_CONFIG);
        shell.apply(Nav::Generate);
        assert_eq!(shell.screen(), Screen::Lab);

        shell.apply(Nav::Regenerate);
        assert_eq!(shell.screen(), Screen::Lab);
    }

    /// AC6 — nav-bar jumps from an arbitrary current screen, once
    /// `has_generated` is latched.
    #[test]
    fn nav_jump_from_arbitrary_screen() {
        let mut shell = AppShell::new(FIXED_CONFIG);
        shell.apply(Nav::Generate);
        assert!(shell.has_generated());

        // JumpTo(Race) from Setup.
        shell.apply(Nav::JumpTo(Screen::Setup));
        assert_eq!(shell.screen(), Screen::Setup);
        shell.apply(Nav::JumpTo(Screen::Race));
        assert_eq!(shell.screen(), Screen::Race);

        // JumpTo(Setup) from Race.
        shell.apply(Nav::JumpTo(Screen::Setup));
        assert_eq!(shell.screen(), Screen::Setup);

        // JumpTo(Lab) from Results.
        shell.apply(Nav::JumpTo(Screen::Race));
        shell.apply(Nav::Finish);
        assert_eq!(shell.screen(), Screen::Results);
        shell.apply(Nav::JumpTo(Screen::Lab));
        assert_eq!(shell.screen(), Screen::Lab);
    }

    /// AC7 — the nav guard: `Race`/`Lab` jumps are a no-op before the first
    /// `Generate`; `Setup` is always allowed; all three succeed after.
    #[test]
    fn nav_guard_before_first_generate() {
        let mut shell = AppShell::new(FIXED_CONFIG);
        assert!(!shell.has_generated());

        shell.apply(Nav::JumpTo(Screen::Race));
        assert_eq!(
            shell.screen(),
            Screen::Setup,
            "Race jump is a no-op before the first generate"
        );
        shell.apply(Nav::JumpTo(Screen::Lab));
        assert_eq!(
            shell.screen(),
            Screen::Setup,
            "Lab jump is a no-op before the first generate"
        );
        shell.apply(Nav::JumpTo(Screen::Setup));
        assert_eq!(shell.screen(), Screen::Setup, "Setup jump always allowed");

        shell.apply(Nav::Generate);
        assert!(shell.has_generated());
        assert_eq!(shell.screen(), Screen::Lab);

        shell.apply(Nav::JumpTo(Screen::Race));
        assert_eq!(shell.screen(), Screen::Race, "Race jump now allowed");
        shell.apply(Nav::JumpTo(Screen::Lab));
        assert_eq!(shell.screen(), Screen::Lab, "Lab jump now allowed");
        shell.apply(Nav::JumpTo(Screen::Setup));
        assert_eq!(shell.screen(), Screen::Setup, "Setup jump still allowed");
    }
}

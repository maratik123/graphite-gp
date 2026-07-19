//! `MovePad` — port of `MovePad.jsx`/`MovePad.d.ts` (design § *The three
//! layers*, AC1–AC6).
//!
//! The 5 von-Neumann accelerations as a plus-shaped keypad: Coast (`·`)
//! center, `↑ ↓ ← →` around it. Diagonal acceleration does not exist — the
//! 4 corners of the 3×3 grid are structurally empty.

use crate::tokens::{color, spacing};
use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Ui};
use gp_core::sim::{Action, BitFlags};

/// Default cell edge (`MovePad.jsx` default; `Screens.jsx:129` overrides to
/// `52`).
const SIZE: f32 = 48.0;
/// Inter-cell gap, `spacing::SPACE_1` (design § *Cell gap*, owner-confirmed).
const GAP: f32 = spacing::SPACE_1;
/// Arrow glyph font-size factor (`MovePad.jsx:39` `fontSize: round(size*0.42)`).
const ARROW_FS_FACTOR: f32 = 0.42;
/// Sublabel font-size factor (`MovePad.jsx:40` `fontSize: round(size*0.19)`).
const SUBLABEL_FS_FACTOR: f32 = 0.19;
/// Sublabel opacity (`MovePad.jsx:40` `opacity: 0.7`).
const SUBLABEL_OPACITY: f32 = 0.7;
/// Gap between the arrow glyph and the sublabel (`MovePad.jsx:40`
/// `marginTop: 2`).
const SUBLABEL_GAP: f32 = 2.0;

/// One `MOVES` table entry: the action, its glyph, and its plus-grid
/// position (`row`/`col`, 0-based, screen-space: row 0 = top, col 0 = left).
struct MoveCell {
    /// The `gp_core` action this cell represents.
    action: Action,
    /// The glyph drawn large (Coast `·`, else an arrow).
    glyph: &'static str,
    /// Grid row: `0` top, `1` middle, `2` bottom.
    row: u8,
    /// Grid column: `0` left, `1` center, `2` right.
    col: u8,
}

/// The plus-layout table, in `Action` declaration order (`Coast, East, West,
/// North, South`, `gp_core::sim::mod.rs`) — single source of truth for
/// per-cell glyph + grid position (design § *`MOVES` layout table*). Exactly
/// 5 cells; the 4 corners are unused, so diagonals are structurally
/// impossible (AC1).
const MOVES: [MoveCell; 5] = [
    MoveCell {
        action: Action::Coast,
        glyph: "·",
        row: 1,
        col: 1,
    },
    MoveCell {
        action: Action::East,
        glyph: "→",
        row: 1,
        col: 2,
    },
    MoveCell {
        action: Action::West,
        glyph: "←",
        row: 1,
        col: 0,
    },
    MoveCell {
        action: Action::North,
        glyph: "↑",
        row: 0,
        col: 1,
    },
    MoveCell {
        action: Action::South,
        glyph: "↓",
        row: 2,
        col: 1,
    },
];

/// The pure style-resolution output of [`MovePad::resolve`] (AC5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveCellStyle {
    /// Cell fill.
    pub bg: Color32,
    /// Glyph/sublabel color.
    pub fg: Color32,
    /// Cell border color.
    pub border: Color32,
}

/// The response of [`MovePad::show`]: the whole-pad `Response`, the
/// post-click `selected` action, and whether a click changed it.
#[derive(Debug)]
pub struct MovePadResponse {
    /// The whole-pad interaction response.
    pub response: Response,
    /// `Some(action)` = the currently-chosen action after this frame.
    pub selected: Option<Action>,
    /// Whether a click selected a (legal) action this frame.
    pub changed: bool,
}

/// `MovePad` props (`MovePad.d.ts`): `legal`, `value` → `selected`, `size`.
#[derive(Clone, Copy, Debug)]
pub struct MovePad {
    /// The legal-action mask, consumed directly from `gp_core::sim::legal_mask`.
    pub legal: BitFlags<Action>,
    /// The currently-chosen action (`.jsx` `value`); `None` = nothing chosen.
    pub selected: Option<Action>,
    /// Cell edge.
    pub size: f32,
}

impl MovePad {
    /// Builds an unselected, default-size (`SIZE` = 48.0) pad over `legal`.
    #[must_use]
    pub const fn new(legal: BitFlags<Action>) -> Self {
        Self {
            legal,
            selected: None,
            size: SIZE,
        }
    }

    /// Sets `selected`.
    #[must_use]
    pub const fn selected(mut self, selected: Action) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Sets `size`.
    #[must_use]
    pub const fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// The pure style-resolution layer (AC5): `(legal, selected)` → cell
    /// style. No `egui::Ui`, no allocation — Miri-clean. `selected` takes
    /// precedence over `legal` (`MovePad.jsx:31-33`).
    #[must_use]
    pub const fn resolve(legal: bool, selected: bool) -> MoveCellStyle {
        if selected {
            MoveCellStyle {
                bg: color::ACCENT,
                fg: color::PAPER_0,
                border: color::ACCENT,
            }
        } else if legal {
            MoveCellStyle {
                bg: color::PAPER_0,
                fg: color::GRAPHITE_900,
                border: color::GRAPHITE_900,
            }
        } else {
            MoveCellStyle {
                bg: color::PAPER_2,
                fg: color::TEXT_FAINT,
                border: color::BORDER_SOFT,
            }
        }
    }

    /// Draws every `MOVES` cell into `pad_rect`: chrome via
    /// `common::paint_surface`, press darkening for `pressed`, and the arrow
    /// glyph + `a,b` sublabel.
    ///
    /// # Panics
    ///
    /// Panics at layout time if the caller has not installed
    /// [`crate::fonts::definitions`] first.
    #[allow(
        clippy::too_many_arguments,
        reason = "paint layer takes every resolved input explicitly, per the design's 3-layer split"
    )]
    pub(crate) fn paint(
        painter: &Painter,
        pad_rect: Rect,
        legal: BitFlags<Action>,
        selected: Option<Action>,
        pressed: Option<Action>,
        size: f32,
    ) {
        let arrow_font_size = (size * ARROW_FS_FACTOR).round();
        let sublabel_font_size = (size * SUBLABEL_FS_FACTOR).round();
        let arrow_font = FontId::new(
            arrow_font_size,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_BOLD.into()),
        );
        let sublabel_font = FontId::new(
            sublabel_font_size,
            FontFamily::Name(crate::fonts::JETBRAINS_MONO_REGULAR.into()),
        );
        let content_height = SUBLABEL_GAP.mul_add(1.0, arrow_font_size + sublabel_font_size);

        for cell in &MOVES {
            let is_legal = legal.contains(cell.action);
            let is_selected = selected == Some(cell.action);
            let style = Self::resolve(is_legal, is_selected);
            let rect = cell_rect(pad_rect, cell.row, cell.col, size, GAP);

            super::common::paint_surface(
                painter,
                rect,
                spacing::RADIUS_0,
                style.bg,
                style.border,
                spacing::BW_1,
            );

            if is_legal && pressed == Some(cell.action) {
                painter.rect_filled(rect, spacing::RADIUS_0, super::common::GHOST_PRESS_OVERLAY);
            }

            let top_y = rect.center().y - content_height / 2.0;
            let arrow_pos = Pos2::new(rect.center().x, top_y + arrow_font_size / 2.0);
            painter.text(
                arrow_pos,
                Align2::CENTER_CENTER,
                cell.glyph,
                arrow_font.clone(),
                style.fg,
            );

            let sublabel_y = top_y + arrow_font_size + SUBLABEL_GAP + sublabel_font_size / 2.0;
            let (a, b) = cell.action.accel();
            painter.text(
                Pos2::new(rect.center().x, sublabel_y),
                Align2::CENTER_CENTER,
                format!("{a},{b}"),
                sublabel_font.clone(),
                style.fg.gamma_multiply(SUBLABEL_OPACITY),
            );
        }
    }

    /// Allocates the `3×3`-grid rect (`SPACE_1`-spaced), reads live pointer
    /// input over every **legal** cell (illegal cells never receive
    /// `Sense::click`, so clicking one is a structural no-op — AC2/AC6),
    /// resolves styles, paints, and returns a [`MovePadResponse`].
    ///
    /// # Panics
    ///
    /// See the private `paint` layer's panics.
    pub fn show(self, ui: &mut Ui) -> MovePadResponse {
        let extent = self.size.mul_add(3.0, GAP * 2.0);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(extent, extent), Sense::hover());

        let mut clicked = None;
        let mut pressed = None;
        for cell in &MOVES {
            if !self.legal.contains(cell.action) {
                continue;
            }
            let cell_rect = cell_rect(rect, cell.row, cell.col, self.size, GAP);
            let cell_response =
                ui.interact(cell_rect, response.id.with(cell.action), Sense::click());
            if cell_response.clicked() {
                clicked = Some(cell.action);
            }
            if cell_response.is_pointer_button_down_on() {
                pressed = Some(cell.action);
            }
        }

        let selected = clicked.or(self.selected);
        if ui.is_rect_visible(rect) {
            Self::paint(ui.painter(), rect, self.legal, selected, pressed, self.size);
        }

        MovePadResponse {
            response,
            selected,
            changed: clicked.is_some(),
        }
    }
}

/// The rect of the cell at `(row, col)` within `pad_rect`, given a shared
/// `size`/`gap`. Shared by [`MovePad::paint`] and [`MovePad::show`] so the
/// drawn cells and the interactive hit rects never drift apart.
///
/// Plain `fn` — `missing_const_for_fn` declines: `f32::from(u8)` is not yet a
/// const-stable `From` impl.
fn cell_rect(pad_rect: Rect, row: u8, col: u8, size: f32, gap: f32) -> Rect {
    let x0 = f32::from(col).mul_add(size + gap, pad_rect.min.x);
    let y0 = f32::from(row).mul_add(size + gap, pad_rect.min.y);
    Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x0 + size, y0 + size))
}

#[cfg(test)]
mod tests {
    use super::{MOVES, MovePad};
    use crate::tokens::color;
    use gp_core::sim::{Action, BitFlags};

    /// AC5 — the three-row style table: selected wins over legal; legal
    /// unselected uses the resting trio; illegal uses the disabled trio.
    #[test]
    fn resolve_selected_legal_illegal_colors() {
        for legal in [false, true] {
            let style = MovePad::resolve(legal, true);
            assert_eq!(style.bg, color::ACCENT);
            assert_eq!(style.fg, color::PAPER_0);
            assert_eq!(style.border, color::ACCENT);
        }

        let legal_unselected = MovePad::resolve(true, false);
        assert_eq!(legal_unselected.bg, color::PAPER_0);
        assert_eq!(legal_unselected.fg, color::GRAPHITE_900);
        assert_eq!(legal_unselected.border, color::GRAPHITE_900);

        let illegal = MovePad::resolve(false, false);
        assert_eq!(illegal.bg, color::PAPER_2);
        assert_eq!(illegal.fg, color::TEXT_FAINT);
        assert_eq!(illegal.border, color::BORDER_SOFT);
    }

    /// AC1/AC5 — `MOVES` is exactly the 5 actions in `Action` declaration
    /// order, each entry's `accel()`/glyph/grid position matches the `.jsx`
    /// ground truth — pins "exactly 5, plus layout, no diagonals".
    #[test]
    fn moves_table_matches_action_order_and_accel() {
        /// `(action, glyph, accel, (row, col))` per expected `MOVES` entry.
        type ExpectedCell = (Action, &'static str, (i32, i32), (u8, u8));

        let expected: [ExpectedCell; 5] = [
            (Action::Coast, "·", (0, 0), (1, 1)),
            (Action::East, "→", (1, 0), (1, 2)),
            (Action::West, "←", (-1, 0), (1, 0)),
            (Action::North, "↑", (0, 1), (0, 1)),
            (Action::South, "↓", (0, -1), (2, 1)),
        ];

        assert_eq!(MOVES.len(), expected.len());
        for (cell, (action, glyph, accel, (row, col))) in MOVES.iter().zip(expected) {
            assert_eq!(cell.action, action);
            assert_eq!(cell.glyph, glyph);
            assert_eq!(cell.action.accel(), accel);
            assert_eq!(cell.row, row);
            assert_eq!(cell.col, col);
        }
    }

    /// AC6 — an all-illegal mask (`BitFlags::empty()`) yields every cell
    /// disabled, deterministically.
    #[test]
    fn all_illegal_mask_yields_all_disabled() {
        let legal: BitFlags<Action> = BitFlags::empty();
        for cell in &MOVES {
            assert!(!legal.contains(cell.action));
            let style = MovePad::resolve(false, false);
            assert_eq!(style.bg, color::PAPER_2);
            assert_eq!(style.fg, color::TEXT_FAINT);
            assert_eq!(style.border, color::BORDER_SOFT);
        }
    }

    /// Builder defaults: `new` starts unselected at the default size.
    #[test]
    fn new_has_expected_defaults() {
        let pad = MovePad::new(BitFlags::all());
        assert!(pad.selected.is_none());
        crate::test_util::assert_f32("MovePad::new size", pad.size, super::SIZE);

        let selected = pad.selected(Action::North).size(52.0);
        assert_eq!(selected.selected, Some(Action::North));
        crate::test_util::assert_f32("MovePad::size setter", selected.size, 52.0);
    }
}

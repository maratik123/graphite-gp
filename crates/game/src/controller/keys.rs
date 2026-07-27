//! Keyboard input, implemented in `gp-game` (spec Scope 3 / owner ruling
//! R1-Q3).
//!
//! `gp-render` gains no keyboard handling and no knowledge of
//! controllers — the key map lives here, and is masked against `legal`
//! exactly like a `MovePad` cell.

use eframe::egui::Key;
use gp_core::sim::{Action, Actions};

/// The key → action map (spec Key decisions — R1-Q3), in [`Action`]
/// declaration order, arrow key before its letter-key alias within a pair.
///
/// `crates/render/src/track/transform.rs`'s documented `y`-flip and
/// `movepad.rs`'s cell table both place `North` visually up, so `↑`/`W` map
/// to [`Action::North`] with no orientation flip needed.
pub const KEY_ORDER: [(Key, Action); 9] = [
    (Key::Space, Action::Coast),
    (Key::ArrowRight, Action::East),
    (Key::D, Action::East),
    (Key::ArrowLeft, Action::West),
    (Key::A, Action::West),
    (Key::ArrowUp, Action::North),
    (Key::W, Action::North),
    (Key::ArrowDown, Action::South),
    (Key::S, Action::South),
];

/// Maps a single key to its [`Action`], per [`KEY_ORDER`]. `None` for any
/// key not in the table.
#[must_use]
pub fn action_for_key(key: Key) -> Option<Action> {
    KEY_ORDER.iter().find_map(|&(k, a)| (k == key).then_some(a))
}

/// Reads the keyboard half of a frame's input.
///
/// Scans [`KEY_ORDER`] in declaration order and returns the first key's
/// action that is both currently pressed (per `pressed`) and a member of
/// `legal`. `None` if no pressed key names a legal action.
///
/// Takes a predicate rather than an `&egui::InputState` so this stays
/// Miri-clean and needs no `egui::Context` to test (design § *The `gp-game`
/// seam*). A production caller passes
/// `|k| i.key_pressed(k)` from inside `ui.input(|i| ...)`.
#[must_use]
pub fn keyboard_action(legal: Actions, pressed: impl Fn(Key) -> bool) -> Option<Action> {
    KEY_ORDER
        .iter()
        .find(|&&(k, a)| pressed(k) && legal.contains(a))
        .map(|&(_, a)| a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_map_table() {
        assert_eq!(action_for_key(Key::ArrowUp), Some(Action::North));
        assert_eq!(action_for_key(Key::W), Some(Action::North));
        assert_eq!(action_for_key(Key::ArrowDown), Some(Action::South));
        assert_eq!(action_for_key(Key::S), Some(Action::South));
        assert_eq!(action_for_key(Key::ArrowLeft), Some(Action::West));
        assert_eq!(action_for_key(Key::A), Some(Action::West));
        assert_eq!(action_for_key(Key::ArrowRight), Some(Action::East));
        assert_eq!(action_for_key(Key::D), Some(Action::East));
        assert_eq!(action_for_key(Key::Space), Some(Action::Coast));
        // An unmapped key.
        assert_eq!(action_for_key(Key::Escape), None);
    }

    #[test]
    fn keyboard_action_masks_illegal_keys() {
        let mut legal = Actions::all();
        legal.remove(Action::North);
        let action = keyboard_action(legal, |k| k == Key::ArrowUp);
        assert_eq!(action, None);
    }

    #[test]
    fn keyboard_action_scans_in_action_declaration_order() {
        let legal = Actions::all();
        let action = keyboard_action(legal, |k| k == Key::Space || k == Key::ArrowUp);
        assert_eq!(action, Some(Action::Coast));
    }

    #[test]
    fn keyboard_action_skips_illegal_and_takes_the_next_legal_key() {
        let mut legal = Actions::all();
        legal.remove(Action::Coast);
        let action = keyboard_action(legal, |k| k == Key::Space || k == Key::ArrowUp);
        assert_eq!(action, Some(Action::North));
    }
}

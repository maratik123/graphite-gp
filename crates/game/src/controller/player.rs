//! The player controller (spec Scope 2) — the concrete [`Controller`]
//! implementation fed by the on-screen `MovePad` and the keyboard.
//!
//! Deterministic: no RNG (spec Key decisions — *Determinism*). A pure
//! function of `legal` plus the frame's [`FrameInput`], so
//! [`PlayerController::decide`] is exposed standalone for the
//! replay-determinism test (AC9).

use super::{Controller, FrameInput, PollContext};
use gp_core::sim::{Action, BitFlags};

/// The player seat. Stateless — every decision is a pure function of the
/// [`PollContext`] it is handed.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerController;

impl PlayerController {
    /// The pure decision core `poll` delegates to (spec § *Same-frame input
    /// precedence*).
    ///
    /// A singleton `{Coast}` mask auto-resolves to `Some(Action::Coast)`
    /// before consulting any input (AC5). Otherwise, `input.shell_action`
    /// and `input.key_action` are scanned in that order; the first one that
    /// is a member of `legal` wins (AC2/AC3). Total: an empty `legal` (out
    /// of the [`PollContext`] precondition) makes the scan vacuously
    /// `None`, never an illegal `Some`.
    #[must_use]
    pub fn decide(legal: BitFlags<Action>, input: FrameInput) -> Option<Action> {
        if legal == BitFlags::from(Action::Coast) {
            return Some(Action::Coast);
        }
        [input.shell_action, input.key_action]
            .into_iter()
            .flatten()
            .find(|&a| legal.contains(a))
    }
}

impl Controller for PlayerController {
    fn poll(&mut self, ctx: PollContext<'_>) -> Option<Action> {
        Self::decide(ctx.legal, ctx.input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::test_fixtures::{
        crash_prone_state, fast_approach_excludes_coast_state, fixture_states_with_masks,
        fixture_track, mid_corridor_state, wall_adjacent_state,
    };
    use gp_core::sim::{legal_move, resolve_crash};

    /// All 5 actions, plus `None` — the candidate space AC2's table sweeps
    /// each input source over.
    const CANDIDATE_ACTIONS: [Option<Action>; 6] = [
        Some(Action::Coast),
        Some(Action::East),
        Some(Action::West),
        Some(Action::North),
        Some(Action::South),
        None,
    ];

    #[test]
    fn never_yields_an_action_outside_the_mask() {
        let track = fixture_track();
        for (state, legal) in fixture_states_with_masks(&track) {
            if legal.is_empty() {
                // Out of the documented precondition — not this test's case
                // (AC6 covers it).
                continue;
            }
            for shell_action in CANDIDATE_ACTIONS {
                for key_action in CANDIDATE_ACTIONS {
                    let input = FrameInput {
                        shell_action,
                        key_action,
                    };
                    if let Some(a) = PlayerController::decide(legal, input) {
                        assert!(legal.contains(a));
                        assert!(
                            legal_move(&track.corridor, state, a),
                            "decide yielded {a:?} for {state:?}, not legal_move-legal"
                        );
                    }
                }
            }
        }

        // REQUIRED case: the unconditional "Coast (·)" shell button can
        // carry an illegal Coast when the mask excludes it — the single
        // real illegal-input path in the product (`race.rs:372-405`,
        // `:252`). PlayerController must mask it out.
        let state = fast_approach_excludes_coast_state();
        let legal = gp_core::sim::legal_mask(&track.corridor, state);
        assert!(
            !legal.contains(Action::Coast),
            "fixture (c) must exclude Coast from its mask"
        );
        let input = FrameInput {
            shell_action: Some(Action::Coast),
            key_action: None,
        };
        assert_eq!(PlayerController::decide(legal, input), None);
    }

    #[test]
    fn illegal_inputs_are_no_ops_on_both_paths() {
        let track = fixture_track();
        let state = wall_adjacent_state();
        let legal = gp_core::sim::legal_mask(&track.corridor, state);
        assert!(
            !legal.contains(Action::West),
            "fixture (b) must exclude West from its mask"
        );

        // Illegal shell_action alone -> None.
        let input = FrameInput {
            shell_action: Some(Action::West),
            key_action: None,
        };
        assert_eq!(PlayerController::decide(legal, input), None);

        // Illegal key_action alone -> None.
        let input = FrameInput {
            shell_action: None,
            key_action: Some(Action::West),
        };
        assert_eq!(PlayerController::decide(legal, input), None);

        // Illegal shell_action + legal key_action -> the key's action
        // (documented fall-through).
        let input = FrameInput {
            shell_action: Some(Action::West),
            key_action: Some(Action::East),
        };
        assert_eq!(PlayerController::decide(legal, input), Some(Action::East));
    }

    #[test]
    fn singleton_coast_mask_resolves_on_the_first_poll() {
        let track = fixture_track();

        // Scenario 1: a hand-built singleton mask.
        let legal = BitFlags::from(Action::Coast);
        let action = PlayerController::decide(legal, FrameInput::default());
        assert_eq!(action, Some(Action::Coast));

        // Scenario 2: the real CrashOutcome::action_mask during a scrub
        // tick.
        let crash_state = crash_prone_state();
        assert!(gp_core::sim::legal_mask(&track.corridor, crash_state).is_empty());
        let outcome = resolve_crash(&track.corridor, crash_state);
        assert!(outcome.scrub);
        let scrub_mask = outcome.action_mask(&track.corridor);
        assert_eq!(scrub_mask, BitFlags::from(Action::Coast));
        let action = PlayerController::decide(scrub_mask, FrameInput::default());
        assert_eq!(action, Some(Action::Coast));
        assert!(legal_move(&track.corridor, outcome.state, Action::Coast));
    }

    #[test]
    fn replaying_the_same_inputs_yields_the_same_actions() {
        let track = fixture_track();
        let mid = mid_corridor_state();
        let wall = wall_adjacent_state();
        let mid_legal = gp_core::sim::legal_mask(&track.corridor, mid);
        let wall_legal = gp_core::sim::legal_mask(&track.corridor, wall);

        let script = vec![
            (
                mid,
                mid_legal,
                FrameInput {
                    shell_action: Some(Action::East),
                    key_action: None,
                },
            ),
            (
                wall,
                wall_legal,
                FrameInput {
                    shell_action: Some(Action::West),
                    key_action: None,
                },
            ),
            (mid, mid_legal, FrameInput::default()),
        ];

        let run = |script: &[(gp_core::sim::CarState, BitFlags<Action>, FrameInput)]| {
            let mut controller = PlayerController;
            script
                .iter()
                .map(|&(state, legal, input)| {
                    controller.poll(PollContext {
                        track: &track,
                        state,
                        legal,
                        input,
                    })
                })
                .collect::<Vec<_>>()
        };

        let first = run(&script);
        let second = run(&script);
        assert_eq!(first, second);
        assert!(first.iter().any(Option::is_some));
        assert!(first.iter().any(Option::is_none));
    }
}

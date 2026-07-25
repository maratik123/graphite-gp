//! The controller abstraction (spec `2026-07-25-game-controller-player`
//! Scope 1) — the single seam through which every car's per-turn
//! [`gp_core::sim::Action`] is produced.
//!
//! **Poll-shaped**, per owner ruling R1-Q1: [`Controller::poll`] returns
//! `Option<Action>`, where `None` means "no answer yet — ask again next
//! frame". The player seat ([`player::PlayerController`]) returns `None` on
//! every frame before it has input; a future AI seat (#158) always returns
//! `Some`. The trait computes no legality of its own (AC7) — every
//! `Some(a)` an implementation returns must already be a member of the
//! `legal` mask it was given.

use gp_core::sim::{Action, BitFlags, CarState};
use gp_core::track::TrackArtifact;

pub mod keys;
pub mod player;

/// One frame's input candidates.
///
/// From the two sources a player seat reads (spec Scope 2/3): the on-screen
/// `MovePad` (via `gp_render`'s shell) and the keyboard (read in `gp-game`,
/// per owner ruling R1-Q3). A seat that reads no UI input (an AI seat)
/// simply ignores this (AC8's no-branching call site passes it uniformly to
/// every seat).
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameInput {
    /// The action, if any, the on-screen shell (`MovePad` / Coast button)
    /// selected this frame — forwarded from
    /// `gp_render::app::ShellResponse::action`. **Not** pre-masked against
    /// `legal`: the shell's "Coast (·)" button is built unconditionally and
    /// can carry an action outside the legal mask.
    pub shell_action: Option<Action>,
    /// The action, if any, the keyboard read
    /// ([`keys::keyboard_action`]) selected this frame.
    pub key_action: Option<Action>,
}

/// The per-poll context a [`Controller`] is asked to decide from.
///
/// **Precondition:** `legal` is **non-empty** (spec Scope 4 / owner ruling
/// R1-Q2). An empty legal mask is a genuine crash; the caller (#43's game
/// loop) pre-checks with `gp_core::sim::legal_mask` and routes an empty
/// result straight to `gp_core::sim::resolve_crash` **without** calling any
/// controller. A [`Controller`] implementation is never asked to handle an
/// empty mask, and must remain total (never panic) if handed one anyway.
pub struct PollContext<'a> {
    /// The track this car races on — the feature source for a future AI
    /// seat (#158); the player seat does not read it.
    pub track: &'a TrackArtifact,
    /// The car's current kinematic state.
    pub state: CarState,
    /// The legal-action mask for `state`, computed by the caller via
    /// `gp_core::sim::legal_mask` (or, during a scrub tick, by
    /// `gp_core::sim::CrashOutcome::action_mask`). **Precondition:
    /// non-empty** — see this struct's own doc.
    pub legal: BitFlags<Action>,
    /// This frame's input candidates. A seat that reads no UI input (an AI
    /// seat) ignores this field.
    pub input: FrameInput,
}

/// The single seam through which every car's per-turn action is produced
/// (spec Scope 1). Implementable by an AI seat with no change to this
/// interface.
pub trait Controller {
    /// Decides this car's action for the current frame/tick, or `None` if
    /// no decision is ready yet ("ask again next frame"). Every `Some(a)`
    /// returned **must** satisfy `ctx.legal.contains(a)` (AC1/AC2); the
    /// implementation computes no legality of its own (AC7) — `ctx.legal`
    /// is the only source of truth. **Precondition:** `ctx.legal` is
    /// non-empty — see [`PollContext`]'s own doc.
    fn poll(&mut self, ctx: PollContext<'_>) -> Option<Action>;
}

/// A heterogeneous collection of seats (spec Scope 5), driven through one
/// uniform call site with no seat-kind branching (AC8) — a mix of the
/// player controller and future AI seats (#158).
pub struct Roster {
    seats: Vec<Box<dyn Controller>>,
}

impl Roster {
    /// An empty roster.
    #[must_use]
    pub const fn new() -> Self {
        Self { seats: Vec::new() }
    }

    /// Adds a seat to the end of the roster.
    pub fn push(&mut self, seat: Box<dyn Controller>) {
        self.seats.push(seat);
    }

    /// Polls the seat at `index`. Total: an out-of-range `index` yields
    /// `None` rather than panicking (the reason `Roster` is a newtype
    /// rather than a `pub type` alias over `Vec` — see the design doc's Q1).
    /// **Precondition:** `ctx.legal` is non-empty — see [`PollContext`]'s
    /// own doc.
    pub fn poll(&mut self, index: usize, ctx: PollContext<'_>) -> Option<Action> {
        self.seats.get_mut(index).and_then(|seat| seat.poll(ctx))
    }

    /// The number of seats in the roster.
    #[must_use]
    pub fn len(&self) -> usize {
        self.seats.len()
    }

    /// Whether the roster holds no seats.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seats.is_empty()
    }
}

impl Default for Roster {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared `#[cfg(test)]` fixtures reused by every `controller` submodule's
/// test module (design § Test Design — the four-state fixture table).
/// `pub(crate)` so `controller::player`'s tests can reach it via
/// `crate::controller::test_fixtures::*`.
#[cfg(test)]
pub(crate) mod test_fixtures {
    use super::{Action, BitFlags, CarState, TrackArtifact};
    use gp_core::geom::{Corridor, Orient, Point, Side, walls_from_boundary};
    use gp_core::sim::{legal_mask, resolve_crash};
    use gp_core::track::{
        Centerline, RaceDir, SField, StartFinish, StartGrid, TimingGate, TrackMetrics,
    };

    /// The shared fixture corridor's width/height — a fully-drivable square
    /// large enough that a near-wall state on one edge does not interact
    /// with any other fixture state.
    const CORRIDOR_SIDE: usize = 30;

    /// A fully-drivable `30×30` corridor with a placeholder (unused by
    /// controller logic — AC7 forbids the controller layer from reading
    /// anything but `legal`) start/finish chord, mirroring
    /// `crates/game/src/main.rs::fixture_track`'s pattern.
    pub(crate) fn fixture_track() -> TrackArtifact {
        let corridor = Corridor::filled(Point::new(0, 0), CORRIDOR_SIDE, CORRIDOR_SIDE);
        let walls = walls_from_boundary(&corridor);
        TrackArtifact {
            walls,
            sf: StartFinish {
                chord: vec![Point::new(15, 0), Point::new(15, 1)],
                orient: Orient::Vertical,
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
            metrics: TrackMetrics {
                vmax_attain: None,
                tempo: None,
                fastest_lap: vec![],
                speed_heatmap: vec![],
            },
            width_min: CORRIDOR_SIDE.try_into().unwrap_or(u32::MAX),
        }
    }

    /// Fixture (a): all-legal mid-corridor state — at rest, far from every
    /// wall, so every one of the 5 actions is legal.
    pub(crate) fn mid_corridor_state() -> CarState {
        CarState {
            x: 15,
            y: 15,
            vx: 0,
            vy: 0,
        }
    }

    /// Fixture (b): wall-adjacent state with a restricted mask — at rest on
    /// the corridor's west edge, so `West` (which would leave the corridor)
    /// is illegal while the other 4 actions stay legal.
    pub(crate) fn wall_adjacent_state() -> CarState {
        CarState {
            x: 0,
            y: 15,
            vx: 0,
            vy: 0,
        }
    }

    /// Fixture (c): fast approach to the east wall whose mask **excludes**
    /// `Coast` — `Coast` (unchanged velocity) would leave the corridor, and
    /// so would every action but the braking `West`.
    pub(crate) fn fast_approach_excludes_coast_state() -> CarState {
        CarState {
            x: 27,
            y: 15,
            vx: 3,
            vy: 0,
        }
    }

    /// Fixture (d): a genuine crash — every action leaves the corridor, so
    /// `legal_mask` is empty and `gp_core::sim::resolve_crash` is the only
    /// legitimate next step (never a [`Controller`] call, per the
    /// non-empty-mask precondition).
    pub(crate) fn crash_prone_state() -> CarState {
        CarState {
            x: 15,
            y: 15,
            vx: 100,
            vy: 100,
        }
    }

    /// Fixture (e): the REQUIRED singleton-`{Coast}` mask row (spec AC2;
    /// design § Test Design fixture (d)) — a genuine scrub tick reached via
    /// `gp_core::sim::resolve_crash` on fixture (d)
    /// ([`crash_prone_state`]), whose `action_mask` collapses to exactly
    /// `{Coast}` (mirrors
    /// `player::tests::singleton_coast_mask_resolves_on_the_first_poll`'s
    /// Scenario 2). Distinct from `crash_prone_state`'s own row in this
    /// table, which stays the **empty**-mask fixture AC6 needs.
    pub(crate) fn scrub_state_with_singleton_coast_mask(
        track: &TrackArtifact,
    ) -> (CarState, BitFlags<Action>) {
        let outcome = resolve_crash(&track.corridor, crash_prone_state());
        let mask = outcome.action_mask(&track.corridor);
        (outcome.state, mask)
    }

    /// The fixture states, paired with their real `legal_mask` (or, for the
    /// scrub row, real `CrashOutcome::action_mask`) against
    /// [`fixture_track`]'s corridor — computed once so every consumer test
    /// asserts against the mask the state actually produces, rather than a
    /// hand-guessed one.
    pub(crate) fn fixture_states_with_masks(
        track: &TrackArtifact,
    ) -> Vec<(CarState, BitFlags<Action>)> {
        let mut fixtures: Vec<(CarState, BitFlags<Action>)> = [
            mid_corridor_state(),
            wall_adjacent_state(),
            fast_approach_excludes_coast_state(),
            crash_prone_state(),
        ]
        .into_iter()
        .map(|state| (state, legal_mask(&track.corridor, state)))
        .collect();
        fixtures.push(scrub_state_with_singleton_coast_mask(track));
        fixtures
    }

    /// A stub [`super::Controller`] returning a fixed, mask-respecting
    /// answer: `Some(Action::Coast)` when `Coast` is legal, `None`
    /// otherwise. Deliberately mask-respecting (not "always `Coast`
    /// regardless") so AC7's "no relaxation for any seat kind" is exercised
    /// by the stub too, and reused for both AC1 (a stub seat returning a
    /// fixed action) and AC8 (a heterogeneous, non-player seat).
    pub(crate) struct AlwaysCoastStub;

    impl super::Controller for AlwaysCoastStub {
        fn poll(&mut self, ctx: super::PollContext<'_>) -> Option<Action> {
            ctx.legal.contains(Action::Coast).then_some(Action::Coast)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_fixtures::{AlwaysCoastStub, fixture_states_with_masks, fixture_track};
    use super::{Controller, FrameInput, PollContext};
    use gp_core::sim::{BitFlags, legal_move};

    #[test]
    fn poll_yields_only_legal_actions_for_the_state_it_was_asked_about() {
        let track = fixture_track();
        let mut stub = AlwaysCoastStub;
        for (state, legal) in fixture_states_with_masks(&track) {
            // The precondition (non-empty `legal`) does not hold for the
            // crash-prone fixture; skip it here — it is exercised by AC6's
            // empty-mask test below instead.
            if legal.is_empty() {
                continue;
            }
            let ctx = PollContext {
                track: &track,
                state,
                legal,
                input: FrameInput::default(),
            };
            if let Some(a) = stub.poll(ctx) {
                assert!(
                    legal_move(&track.corridor, state, a),
                    "stub yielded {a:?} for {state:?}, not legal_move-legal"
                );
            }
        }
    }

    #[test]
    fn empty_mask_yields_none_and_never_an_illegal_some() {
        let track = fixture_track();
        let state = super::test_fixtures::crash_prone_state();
        let mut stub = AlwaysCoastStub;
        let mut player = crate::controller::player::PlayerController;
        let inputs = [
            FrameInput::default(),
            FrameInput {
                shell_action: None,
                key_action: None,
            },
            FrameInput {
                shell_action: Some(gp_core::sim::Action::Coast),
                key_action: None,
            },
            FrameInput {
                shell_action: None,
                key_action: Some(gp_core::sim::Action::Coast),
            },
            FrameInput {
                shell_action: Some(gp_core::sim::Action::East),
                key_action: Some(gp_core::sim::Action::West),
            },
        ];
        for input in inputs {
            let ctx = PollContext {
                track: &track,
                state,
                legal: BitFlags::empty(),
                input,
            };
            assert_eq!(stub.poll(ctx), None);

            // AC6 must hold for the real player seat too, not only the
            // AlwaysCoastStub above — the real seat is the one a shipped
            // scrub tick actually polls.
            let ctx = PollContext {
                track: &track,
                state,
                legal: BitFlags::empty(),
                input,
            };
            assert_eq!(player.poll(ctx), None);
        }
    }

    #[test]
    fn fixture_table_includes_a_required_singleton_coast_mask_row() {
        let track = fixture_track();
        let fixtures = fixture_states_with_masks(&track);
        let singleton_coast = BitFlags::from(gp_core::sim::Action::Coast);
        assert!(
            fixtures.iter().any(|&(_, mask)| mask == singleton_coast),
            "fixture_states_with_masks must include a singleton-{{Coast}} mask row \
             (spec AC2 REQUIRED case), got masks: {:?}",
            fixtures.iter().map(|&(_, mask)| mask).collect::<Vec<_>>()
        );
    }

    #[test]
    fn heterogeneous_roster_drives_every_seat_through_one_call_site() {
        let track = fixture_track();
        let state = super::test_fixtures::mid_corridor_state();
        let legal = gp_core::sim::legal_mask(&track.corridor, state);
        assert!(!legal.is_empty());
        // Fixture (a) is the all-legal mid-corridor state, not the
        // singleton-{Coast} row — so PlayerController::decide must consult
        // input.shell_action rather than auto-resolving (AC5 does not
        // apply here).
        assert_ne!(legal, BitFlags::from(gp_core::sim::Action::Coast));

        let mut roster = super::Roster::new();
        roster.push(Box::new(crate::controller::player::PlayerController));
        roster.push(Box::new(AlwaysCoastStub));
        assert_eq!(roster.len(), 2);

        let input = FrameInput {
            shell_action: Some(gp_core::sim::Action::East),
            key_action: None,
        };

        // Seat 0 (the player seat): decide picks input.shell_action (East)
        // first, since it's a member of the all-legal mask.
        let player_ctx = PollContext {
            track: &track,
            state,
            legal,
            input,
        };
        let player_answer = roster.poll(0, player_ctx);
        assert_eq!(
            player_answer,
            Some(gp_core::sim::Action::East),
            "seat 0 (PlayerController) must actually answer with its shell_action"
        );
        assert!(legal_move(
            &track.corridor,
            state,
            player_answer.expect("asserted Some above")
        ));

        // Seat 1 (AlwaysCoastStub): answers Coast unconditionally whenever
        // Coast is legal, ignoring `input` entirely.
        let stub_ctx = PollContext {
            track: &track,
            state,
            legal,
            input,
        };
        let stub_answer = roster.poll(1, stub_ctx);
        assert_eq!(
            stub_answer,
            Some(gp_core::sim::Action::Coast),
            "seat 1 (AlwaysCoastStub) must actually answer Coast"
        );
        assert!(legal_move(
            &track.corridor,
            state,
            stub_answer.expect("asserted Some above")
        ));
    }

    /// AC7's structural no-physics scan. Written here (subtask 4) rather
    /// than subtask 2, where the design's Test Design table nominally
    /// places it — `include_str!` on `keys.rs`/`player.rs` cannot compile
    /// until both files exist, which is only true from subtask 4 onward
    /// (see the progress-file Decisions log).
    #[test]
    fn controller_module_calls_no_physics() {
        /// Strips the `#[cfg(test)]` region and every doc/comment/`use`
        /// line, so only production code lines remain to be scanned — a
        /// doc sentence naming `legal_move` (which AC1/AC6 require) must
        /// not itself trip this test.
        fn code_lines(src: &str) -> String {
            let mut out = String::new();
            for line in src.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("#[cfg(test)]") {
                    break;
                }
                if trimmed.starts_with("///")
                    || trimmed.starts_with("//!")
                    || trimmed.starts_with("//")
                {
                    continue;
                }
                if trimmed.starts_with("use ") {
                    continue;
                }
                out.push_str(line);
                out.push('\n');
            }
            out
        }

        let haystack = code_lines(include_str!("mod.rs"))
            + &code_lines(include_str!("keys.rs"))
            + &code_lines(include_str!("player.rs"));

        for forbidden in ["step(", "legal_move(", "supercover("] {
            assert!(
                !haystack.contains(forbidden),
                "controller/{{mod,keys,player}}.rs production code calls {forbidden}, \
                 violating AC7's no-independent-physics rule"
            );
        }
    }
}

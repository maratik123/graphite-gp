//! Test-only fixtures and helpers shared across the gallery modules
//! (`app_gallery`, `screens::results_gallery`, `screens::lab_gallery`) —
//! hoisted out of the per-gallery copies the `JetBrains` duplicate-code
//! inspection flagged. A crate-root `#[cfg(test)]` module (reachable by both
//! `app_gallery` and the `screens` galleries), mirroring `track::test_support`
//! for track fixtures.

use crate::widgets::CarKind;
use crate::{RaceSummary, StandingEntry};
use std::cell::Cell;

/// The fixed 4-car standings fixture (issue #43 D3 — turn counts, no longer
/// the JSX's second-based exemplar), in the JSX's own `car_index == slice
/// position` order (`k == 0` is `You`, `k > 0` is `Ai`).
pub(crate) fn fixture_standings() -> [StandingEntry; 4] {
    [
        StandingEntry {
            car_index: 0,
            kind: CarKind::You,
            rank: 1,
            finish_turn: Some(38),
        },
        StandingEntry {
            car_index: 1,
            kind: CarKind::Ai,
            rank: 2,
            finish_turn: Some(40),
        },
        StandingEntry {
            car_index: 2,
            kind: CarKind::Ai,
            rank: 3,
            finish_turn: Some(41),
        },
        StandingEntry {
            car_index: 3,
            kind: CarKind::Ai,
            rank: 4,
            finish_turn: Some(43),
        },
    ]
}

/// The fixed race summary (issue #43 D3 — `fastest_lap` is now a turn
/// count, no longer the JSX's second-based exemplar).
pub(crate) const FIXED_SUMMARY: RaceSummary = RaceSummary {
    fastest_lap: 12,
    tempo: 0.87,
    crashes: 1,
};

/// Drives a full hover → drag → drop click (three `step()`s) at the center of
/// the button rect captured in `rect` during a prior rest frame — the shared
/// interaction-test click gesture.
pub(crate) fn click<State>(
    harness: &mut egui_kittest::Harness<'_, State>,
    rect: &Cell<Option<egui::Rect>>,
) {
    let center = rect
        .get()
        .expect("rest frame captured the button rect")
        .center();
    harness.hover_at(center);
    harness.step();
    harness.drag_at(center);
    harness.step();
    harness.drop_at(center);
    harness.step();
}

//! Native `gp-render` core widgets (issue #13).
//!
//! Ports the five design-system core components — Button, `IconButton`,
//! Badge, Tag, Card — to `egui`. Each widget's `.d.ts` prop contract +
//! `.jsx` style tables are the port ground truth; style is sourced entirely
//! from `crate::tokens`.
//!
//! Each widget is split into three layers (design § Approach):
//!
//! 1. A pure `const fn resolve(...)` style-resolution layer (AC7) — Miri-clean,
//!    no `egui::Ui`, no allocation.
//! 2. A private `paint(painter, rect, &style, …)` layer drawing the resolved
//!    style.
//! 3. A public `show(self, ui) -> Response` interaction shell (egui builder
//!    idiom) reading live pointer input.

mod common;

pub use common::Size;

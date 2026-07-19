//! Native `gp-render` core + forms widgets (issues #13, #14).
//!
//! Ports the five design-system core components — Button, `IconButton`,
//! Badge, Tag, Card — plus the four forms components — Slider, Switch,
//! `SegmentedControl`, Stepper — to `egui`. Each widget's `.d.ts` prop
//! contract + `.jsx` style tables are the port ground truth; style is
//! sourced entirely from `crate::tokens`.
//!
//! Each widget is split into three layers (design § Approach):
//!
//! 1. A pure `const fn resolve(...)` style-resolution layer (AC7) — Miri-clean,
//!    no `egui::Ui`, no allocation.
//! 2. A private `paint(painter, rect, &style, …)` layer drawing the resolved
//!    style.
//! 3. A public `show(self, ui) -> Response` interaction shell (egui builder
//!    idiom) reading live pointer input.

pub mod badge;
pub mod button;
pub mod card;
mod common;
#[cfg(test)]
mod forms_gallery;
#[cfg(test)]
mod gallery;
pub mod icon_button;
pub mod segmented_control;
pub mod slider;
pub mod stepper;
pub mod switch;
pub mod tag;
pub mod telemetry;

pub use badge::{Badge, Tone as BadgeTone};
pub use button::{Button, Variant as ButtonVariant};
pub use card::{Card, Elevation};
pub use common::Size;
pub use icon_button::{IconButton, Variant as IconButtonVariant};
pub use segmented_control::{SegmentedControl, SegmentedControlResponse};
pub use slider::{Slider, SliderResponse};
pub use stepper::{StepDir, Stepper, StepperResponse};
pub use switch::{Switch, SwitchResponse};
pub use tag::{Tag, TagResponse};
pub use telemetry::{Align as TelemetryAlign, Telemetry, Tone as TelemetryTone};

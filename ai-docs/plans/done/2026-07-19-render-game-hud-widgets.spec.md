# gp-render: game HUD widgets — Telemetry, LapMeter, CarChip

**Source:** issue #15
**Date:** 2026-07-19
**Tracked in:** #15

## Scope

Port three game-specific readout widgets from the design system to the native
`egui` GUI in the `gp-render` crate (crate directory `crates/render`, package
`gp-render`; widgets live under `crates/render/src/widgets/`). The `.d.ts` prop
contract and `.jsx` style tables in `docs/design-system/components/game/` are
the port ground truth; all style is sourced from `crate::tokens` (colors,
spacing, typography). Follow the established three-layer widget pattern from
issues #13/#14 (see *Technical constraints*).

1. **Telemetry** (`Telemetry.d.ts` / `.jsx`) — a mono-face readout of one
   labelled metric. Props: `label`, `value` (mono string), optional `unit`,
   `tone` (`default | accent | ok | warn | danger | muted`), `size`
   (`sm | md | lg`), `align` (`left | right`). The uppercase `label` is drawn
   in the mono face at `FS_XS`/`TEXT_MUTED`; the `value` in the mono face at a
   size that depends on `size` (`lg → FS_H2`, `md → FS_H3`, `sm → FS_TITLE`),
   `FW_BOLD`, colored by `tone`; the optional `unit` trails at `FS_SM`/
   `FW_REGULAR`/`TEXT_MUTED`. Composed into the race HUD strip (SPEED, v, POS,
   LAP, TEMPO). Carries an **on-ink render mode** (Q1) for the HUD strip, which
   sits on a `GRAPHITE_900` panel in `game.card.html`: on-ink swaps the
   `default` tone → a light on-ink ink (`#FBF8F0` = `PAPER_0`, the card's
   override) and the `label`/`muted` → a lighter on-ink muted (`#A69D8C` =
   `GRAPHITE_400`, == `TEXT_FAINT`, the card's override); the semantic tones
   (`accent`/`ok`/`warn`/`danger`) are unchanged (accent vermilion et al. read
   on graphite as-is, matching the card). On-ink is Telemetry-only — LapMeter
   and the CarChip standings render on the paper page in the card, not on ink.
2. **LapMeter** (`LapMeter.d.ts` / `.jsx`) — lap progress. Props: `lap`,
   `total`, `label` (default `"LAP"`). Draws a mono `done/total` readout (the
   `/total` suffix in `TEXT_FAINT`) above a row of `total` equal-width cells;
   the first `done = clamp(lap, 0, total)` cells fill `ACCENT`, the rest
   `PAPER_3`, each with a `BW_HAIR` `GRAPHITE_900` border and square corners
   (`RADIUS_0`).
3. **CarChip** (`CarChip.d.ts` / `.jsx`) — a car token for rosters/standings.
   Props: `color` (a car-ramp color), `name`, optional `rank`, `kind`
   (`you | ai`), `active`. Draws an optional mono `rank`, a colored dot
   (16 px, `2 px` `GRAPHITE_900` border), the `name` in the UI face
   (`FS_BODY`/`FW_MEDIUM`), and an optional pill `kind` tag (`YOU` in `ACCENT`,
   `AI` in `TEXT_MUTED`). `active` raises the border (`BW_2`/`GRAPHITE_900`) and
   background (`PAPER_2`) vs the resting chip (`BW_HAIR`/`BORDER_HAIRLINE`,
   `PAPER_0`).

Each widget follows the #13/#14 layering: a pure `const fn resolve(...)`
style-resolution layer producing a `*Style` struct (Miri-clean; no `egui::Ui`,
no allocation), a private `paint(painter, rect, &style, …)` layer, and a public
`show(self, ui) -> Response` interaction shell. All three are non-interactive
(no `onClick` in their `.d.ts`), so `show` returns a `Sense::hover()`
`Response`, matching `Badge`.

A HUD specimen golden (`egui_kittest` wgpu snapshot, exact-compare, Miri-ignored
per the `widget_gallery`/`forms_gallery` precedent) renders the composed HUD
strip + LapMeter + CarChip standings laid out to match
`docs/design-system/components/game/game.card.html`.

## Out of scope

- **MovePad** — present in `game.card.html` and `docs/design-system/components/
  game/MovePad.*`, but issue #15 enumerates only Telemetry / LapMeter / CarChip.
  MovePad is a separate widget/issue; the specimen omits its region.
- Wiring these widgets to live `gp-core` game state (the signed lap counter,
  real car positions/standings). They take their values as props; integration
  into the running game loop is a later block.
- Porting the React `style?: React.CSSProperties` passthrough as a general
  escape hatch — dropped per the #13/#14 precedent (the Rust `Badge`/`Tag`/…
  carry no `style` field; style comes only from `crate::tokens`). The one
  specimen-driven use of `style` in the card — the on-graphite text inversion —
  is instead realized as an explicit Telemetry `on_ink` mode (Q1 decision, see
  Key decisions), not a general CSS passthrough.
- The heatmap / fastest-lap analytic overlays and track-layer rendering of
  `docs/design.md` §4 (separate issues).

## Deferred

- Live-game integration of the widgets | needs the game-loop/state block | yes — later Block 2 issue.

## Key decisions

| Question | Decision |
|---|---|
| Which crate / module layout | New modules `telemetry.rs`, `lap_meter.rs`, `car_chip.rs` under `crates/render/src/widgets/`, exported from `widgets/mod.rs` and the crate root — mirrors the one-file-per-widget layout of #13/#14. Exact naming to `design`. |
| Tone set for Telemetry | Six tones (`default | accent | ok | warn | danger | muted`) → `TEXT_INK | ACCENT | OK | WARN | DANGER | TEXT_MUTED`. This is a distinct enum from `BadgeTone` (Badge has 5 tones with tint/fg semantics; Telemetry colors solid text). |
| Size→font-size mapping for Telemetry value | `sm → FS_TITLE (18)`, `md → FS_H3 (22)`, `lg → FS_H2 (30)`, per `Telemetry.jsx`. The `common::Size` enum (`Sm/Md/Lg`) is reusable for the variant set; the size→px table is Telemetry-specific. |
| Fonts | Mono value/label/rank via `JETBRAINS_MONO_BOLD`/`_MEDIUM` families; CarChip `name` via the UI face (`ONEST_MEDIUM`). All families already exist in `crate::fonts`. |
| CarChip color input | Take a `Color32` (defaulting to `CAR_1`); the specimen and unit tests source it from `crate::tokens::color::CAR_COLORS` / `car_color(index)`. `CAR_1 == ACCENT` (both `#E24A2B`), so "car 1 = accent" holds. Exact shape (raw `Color32` vs ramp index) is a `design` call; the test-note requirement ("color indexes into the car-color ramp, car 1 = accent") is satisfied either way. |
| LapMeter `lap` type / clamp | Accept a signed count and clamp to `[0, total]` (`done = clamp(lap, 0, total)`), honoring the "signed lap counter, clamped ≥ 0" contract in `LapMeter.d.ts`. |
| Telemetry HUD-strip legibility on graphite (Q1) | **On-ink mode** (round-1 answer). Telemetry gains an `on_ink` render mode (a bool field / `.on_ink(true)` builder, exact shape to `design`) beyond the `.d.ts`. When set: `default` tone → light on-ink text and `label`/`muted` → lighter on-ink muted, matching the card's inverted values (`#FBF8F0`, `#A69D8C`); semantic tones stay as-is; on-ink is Telemetry-only. **Token binding to `design`:** the card's default-ink override is `#FBF8F0` = `PAPER_0`, but the `TEXT_ON_INK` token = `PAPER_1` (`#F5F1E6`) — reconcile this paper-0/paper-1 discrepancy against the golden; the muted override `#A69D8C` = `GRAPHITE_400` (== `TEXT_FAINT`). Since the golden is self-minted, the chosen bytes just need to be token-sourced and match the card's look. |
| Letter-spacing (`ls-label` / `ls-mono`) | Not applied — `egui::FontId` has no letter-spacing; the #13/#14 ports already omit it (e.g. `Badge` draws mono text with no tracking). Cosmetic only; does not affect the exact-compare golden since the golden is minted from this same code. |

## Technical constraints

- **Three-layer widget pattern (AC7 precedent, #13/#14).** Each widget exposes a
  pure `const fn resolve(...) -> *Style` (no `egui::Ui`, no allocation, Miri-
  clean), a private `paint(...)`, and a public `show(self, ui) -> Response`.
  Style is sourced entirely from `crate::tokens::{color, spacing, typography}`;
  new numeric literals with semantic meaning become module-level
  `SCREAMING_SNAKE_CASE` consts, not inline magic numbers.
- **Tokens already present.** All needed tokens exist: tones
  (`ACCENT/OK/WARN/DANGER/TEXT_MUTED/TEXT_INK/TEXT_FAINT`), car ramp
  (`CAR_COLORS`, `car_color`), font sizes/weights/families, spacing/radius/
  border-width consts. No new tokens are required for the port; a per-widget
  non-token color (as `Badge`'s `BADGE_OK_FG` did) is allowed only if a `.jsx`
  literal has no token — flag any such case in the design.
- **Golden specimen** uses the `widget_gallery`/`forms_gallery` harness pattern:
  `egui_kittest::wgpu` CPU adapter, frame-1-install-fonts / frame-2-draw,
  `SnapshotOptions` with `threshold(1.0)` + `failed_pixel_count_threshold(0)`
  (exact-compare; the 1.0 color threshold only absorbs cross-renderer AA
  rounding), and `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan
  ICD")]`. The golden PNG is minted during implementation (code-writer +
  `image-check`), not pre-committed.
- **Non-interactive.** No pointer state is read; `show` allocates the widget's
  rect and returns a `Sense::hover()` `Response` for layout uniformity.
- **Dependencies #11 (egui backend) and #12 (tokens) are satisfied** — the
  `gp-render` crate builds on `egui 0.35` with `crate::tokens`, `crate::fonts`
  in place.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `Telemetry` ports `TelemetryProps`: a mono `label`+`value` with semantic `tone` (6 tones), `size` (sm/md/lg → value font size per the table), optional `unit`, `align` (left/right), and an `on_ink` mode. In on-ink mode the `default` tone and the `label`/`muted` colors switch to their on-ink equivalents (light ink / lighter muted, matching the card); semantic tones are unchanged. |
| AC2 | `Telemetry` exposes a pure `const fn resolve(tone, size, on_ink) -> TelemetryStyle`; unit tests assert every `tone → value color` and `size → value font size` mapping against the token table, plus the on-ink overrides (`default`/`label`/`muted` swap on ink; `accent`/`ok`/`warn`/`danger` do not). |
| AC3 | `LapMeter` ports `LapMeterProps`: a mono `done/total` readout (with `/total` in `TEXT_FAINT`) plus `total` cells, the first `done = clamp(lap, 0, total)` filled `ACCENT` and the rest `PAPER_3`, each `GRAPHITE_900`-bordered with square corners; `label` defaults to `"LAP"`. |
| AC4 | `LapMeter`'s fill logic is unit-tested: `done` clamps to `[0, total]`; cell `i` is filled iff `i < done`; boundary cases `lap ≤ 0`, `lap ≥ total`, and an intermediate value. |
| AC5 | `CarChip` ports `CarChipProps`: colored dot from the car ramp, `name` in the UI face, optional mono `rank`, `kind` tag (`YOU` in `ACCENT` / `AI` in `TEXT_MUTED`), and `active` state (raised border + `PAPER_2` bg vs resting hairline + `PAPER_0`). |
| AC6 | `CarChip` exposes a pure `const fn resolve(...) -> CarChipStyle`; unit tests assert the color indexes into `CAR_COLORS` (index 0 / car 1 = `ACCENT`), `active` vs resting border+bg, and `kind → tag color`. |
| AC7 | All three widgets follow the #13/#14 three-layer pattern (pure `const fn resolve` → private `paint` → public `show`); style is sourced entirely from `crate::tokens`; the `resolve` layers are Miri-clean. |
| AC8 | A HUD specimen golden (`egui_kittest` wgpu, exact-compare, `#[cfg_attr(miri, ignore)]`) renders the Telemetry HUD strip (SPEED / v / POS / TEMPO) + `LapMeter` + a `CarChip` standings column, laid out to match `game.card.html` (MovePad region omitted, out of scope). |
| AC9 | Props mirror the `.d.ts` contracts, translated to Rust idioms (`ReactNode` → `&str`, `?`-optional → `Option<_>` / builder default, union → enum); the React `style` passthrough is dropped, with the card's on-graphite text inversion realized instead as Telemetry's explicit `on_ink` mode (the one field beyond the `.d.ts`, per Q1). |
| AC10 | New public items (`Telemetry` + its `Tone`, `LapMeter`, `CarChip` + its `CarKind`) are exported from `widgets/mod.rs` and the crate root, each with a `///` doc; `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `cargo test -p gp-render` are clean. |

## Open questions

- **Q1 — RESOLVED (round 1): on-ink mode.** Telemetry gains an `on_ink` render
  mode (one field beyond the `.d.ts`) so the HUD strip reads on the card's
  `GRAPHITE_900` panel: `default`/`label`/`muted` switch to on-ink light
  values, semantic tones unchanged, Telemetry-only. See the Key-decisions row
  for the token-binding note (`PAPER_0` vs `TEXT_ON_INK`/`PAPER_1` discrepancy,
  `GRAPHITE_400` muted) left to `design`. No open questions remain.

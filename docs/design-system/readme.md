# Graphite GP — Design System

A design system for **Graphite GP**: a grid-based **vector-racing game** — the
classic "Racetrack" pencil-and-paper game (integer position + velocity,
accelerate ±1 per axis per turn) with procedurally generated closed tracks and
self-taught AI opponents.

> **Core product invariant:** *a point is the center of a unit cell; a wall is a
> dual edge on the half-grid.* The whole game — and this design language — reads
> geometry through that duality. The medium is graph paper; the mark is graphite.

---

## Sources

This system was built from the project's open-source repository. Nothing here was
invented about the brand beyond what the product concept implies — the codebase is
**scaffold-only** (module structure + a rich design doc; the rendering backend is
not yet chosen and no UI, colors, fonts, or logo exist yet).

- **GitHub:** https://github.com/maratik123/graphite-gp
  - `README.md` — product summary + the 4-block architecture.
  - `docs/design.md` — the full design spec (in Russian): track model, generation
    pipeline (Ф1–Ф7), physics core (supercover, crash, collisions, lap counter),
    AI training (features/reward/architecture), and **§4 Rendering + UX** — the one
    concrete visual spec, which anchors this system.
  - `docs/design-review.md` — the multi-round design review.
  - `crates/render/src/lib.rs` — `render_frame` stub + `Overlays { speed_heatmap,
    fastest_lap, grid }` — the only rendering surface, still `todo!()`.

Explore the repository above to build richer, more faithful designs than this
system alone captures.

### Because there was no existing visual design, this system is an *interpretation*

Grounded strictly in the product: the "pencil game on graph paper" concept, the
name **Graphite**, the doc's *«тетрадный лист»* (notebook/graph-paper sheet), and
the §4 render layers (outfield / infield / asphalt / walls / S-F line / graph-paper
grid + dots / speed-heatmap overlay / fastest-lap line / cars-as-points). Treat the
visual choices below as a strong proposed direction, not a recreation of shipped UI.

---

## Content Fundamentals

**Voice: precise, technical, unpretentious — an engineer explaining a clean idea.**
The product's own writing is dense with exact definitions and derives properties
"by construction." Copy should feel *specified*, not *marketed*.

- **Casing.** Sentence case for body and UI labels. UPPERCASE with wide tracking for
  small eyebrow labels and telemetry field names (`LAP`, `SPEED`, `V·MAX`).
  The wordmark is **GRAPHITE GP** in caps.
- **Person.** Second person for player-facing instruction ("Choose your acceleration."),
  neutral third person for system/spec descriptions ("A wall never crosses a point.").
- **Numbers are first-class.** Coordinates `(x, y)`, velocity vectors `(vx, vy)`,
  laps, tempo, `Vmax` — always set in the mono face, always exact. Prefer a real
  value over a vague adjective.
- **Terminology (use verbatim):** *corridor* `D`, *wall*, *dual edge / half-grid*,
  *supercover*, *run-out*, *S/F line* (start-finish), *lap counter*, *centerline*,
  *speed heatmap*, *fastest lap*, *von Neumann move* (the 5 accelerations), *crash*,
  *coast* `(0,0)`, *oracle*, *tempo*.
- **Tone.** Confident and terse. "Valid by construction." "The car never touches a
  wall." No hype words, no exclamation marks in system copy. A dry wit is welcome in
  player feedback ("Braking distance: you didn't have one.").
- **No emoji.** The icon vocabulary is geometric (dots, vectors, checkered flag), not
  emoji. Unicode arrows (`↑ ↓ ← →`, `·` for coast) are acceptable inline in mono.

**Examples**
- Eyebrow: `TRACK 04 · PROCEDURAL`
- Telemetry: `SPEED |v| = 3.61   v = (2, 3)   LAP 2/5`
- Button: `Generate track`, `Coast`, `Accelerate +1`
- Empty/valid state: `Track valid — closes a lap by construction.`
- Difficulty label: `Pilot temperature — low T = clean & fast, high T = noisy.`

---

## Visual Foundations

The look is **engineering graph paper marked in graphite, with racing telemetry.**
Flat, precise, lattice-aligned. Not skeuomorphic notebook kitsch — crisp and modern,
but the paper + grid + pencil-stroke logic is always present.

- **Color.** Warm graph-paper cream (`--paper-1 #F5F1E6`) is the ground. Ink is warm
  near-black graphite (`--graphite-900 #201E1A`) through a full grey ramp — these do
  most of the work. The single brand accent is **GP vermilion** (`--accent #E24A2B`),
  used sparingly for the primary car, key actions, and the hot end of the heatmap.
  Cars each get a distinct **chalk hue** (vermilion, blue, green, amber, plum, teal).
  Asphalt is a desaturated warm grey corridor. Grid ruling is a faint engineering
  blue (`--grid-line #C3CEDD`). It is a restrained, near-duotone palette with color
  reserved for *meaning* (which car, how fast).
- **Type.** Two families. **Onest** (geometric grotesk, Cyrillic support) for display + UI;
  **JetBrains Mono** for all coordinates, vectors, and telemetry. Display is tight
  (`--ls-display -0.02em`) and heavy; small labels are UPPERCASE at `+0.06em`. Big
  numerals (speed, lap) are a signature — set them large in mono or Grotesk 700.
- **Backgrounds.** The graph-paper grid is the primary background motif — a
  quad-ruled `--bg-grid` (1px faint-blue lines at the `--cell` = 24px pitch), often
  with a dotted lattice `--bg-dots` for the "points." Full-bleed on the track view;
  a subtle watermark grid behind panels. **No photographic imagery, no gradients**
  beyond the flat heatmap ramp. The track itself (asphalt corridor, walls, S/F,
  cars) is the hero illustration and is drawn as vector geometry on the grid.
- **Layout.** Everything snaps to the 4px lattice / 24px cell. Panels are rectangular
  with hairline pencil borders. Generous alignment, orthogonal composition; diagonals
  appear only as velocity vectors and racing lines. Fixed HUD chrome (lap/speed/pos)
  frames a scrollable/zoomable track canvas.
- **Borders.** Pencil hairlines: `1px --graphite-300` for soft dividers, `2px
  --graphite-900` for emphasis/selected, `3px --wall` for wall emphasis. Borders
  carry structure — preferred over shadow.
- **Corner radii.** Crisp. `--radius-0 0px` for anything sitting *on* the grid
  (chips, cells, the move pad), `--radius-2 6px` default for cards/inputs, up to
  `--radius-3 10px` for floating panels. Pills (`--radius-pill`) only for status
  tags and toggles.
- **Shadows.** Minimal and warm. `--shadow-1/2` for lifted cards, `--shadow-pop` for
  modals/menus. Pressed controls use `--shadow-inset` (a graphite darkening).
  Paper is flat by default — elevation is the exception, not the rule.
- **Motion.** Purposeful, mechanical. The signature motion is the **move animation:
  a car slides linearly along the chord** `(x,y) → (x+vx, y+vy)` — linear easing,
  because the physics is linear. UI transitions use `--ease-standard` over
  `--dur-fast/med`. Hover = quick tint; no bounce, no float loops. Track generation
  can "draw in" phase by phase. Respect `prefers-reduced-motion`.
- **Hover states.** Subtle darkening/tint toward the accent or a graphite step, never
  a scale-up. Buttons darken (`--accent-hover`); ghost/secondary get a `--paper-2`
  wash. Cards raise one shadow step.
- **Press states.** Color deepens (`--accent-press`) and an inset pencil-press shadow
  appears; the move-pad cell darkens like a pressed graphite key. No shrink beyond a
  1px optical nudge.
- **Transparency & blur.** Sparing. Modal scrims are a warm graphite wash
  (`rgba(32,30,26,.5)`), optionally with a slight backdrop blur. The heatmap overlay
  and racing-line overlay sit semi-transparent over the asphalt. Otherwise surfaces
  are opaque paper.
- **Cards.** Paper face (`--surface-card #FBF8F0`), hairline border, `--radius-2`,
  `--shadow-1`. A card may carry a faint grid watermark. Selected/active cards get a
  `2px --graphite-900` border or an accent left-marker — not a colored glow.

---

## Iconography

**The codebase ships no icons** (no icon font, sprite, or SVGs — the render backend
is unchosen). The brand's *native* iconography is **geometric game primitives**,
which this system draws as first-class marks:

- **Lattice point / car** — a filled dot on a grid intersection.
- **Velocity vector** — an arrow from the car along `(vx, vy)`; length ∝ speed.
- **Von Neumann move pad** — the 5 accelerations `·  ↑ ↓ ← →` as a plus-shaped keypad.
- **Wall** — a heavy segment on the half-grid (never through a point).
- **S/F line** — a checkered/dashed segment across the corridor.
- **Fastest-lap line** — a thin smooth spline overlay.

For **UI chrome** (play/pause, settings, close, chevrons, etc.) there is no source
set, so this system **substitutes [Lucide](https://lucide.dev)** — a crisp 2px-stroke
open-source line set that matches the technical, hairline aesthetic. Loaded from CDN
(`lucide@latest`). **FLAGGED:** this is a substitution; swap for an official set if
one is adopted. Use Lucide at 1.75–2px stroke, `currentColor`, sized on the 4px grid
(16 / 20 / 24). Do **not** use emoji. Inline Unicode arrows/middot are acceptable
inside mono telemetry only.

---

## Contents / Manifest

Root:
- `styles.css` — global entry (import lines only). Consumers link this.
- `tokens/` — `colors.css`, `fonts.css`, `typography.css`, `spacing.css`, `effects.css`.
- `readme.md` — this guide.
- `SKILL.md` — Agent-Skills-compatible entry point.

Foundations (Design System tab cards): under `guidelines/` — color, type, spacing,
grid/paper, motion, and brand specimen cards.

Components (`components/`, namespace `window.GraphiteGPDesignSystem_1b43c8`):
- `core/` — **Button**, **IconButton**, **Badge**, **Tag**, **Card**
- `forms/` — **Slider**, **Switch**, **SegmentedControl**, **Stepper**
- `game/` — **Telemetry**, **MovePad**, **CarChip**, **LapMeter**

> **Intentional additions.** The source defines *no* UI components, so this is an
> authored set. Beyond the standard primitives, the **game/** group adds product-
> specific controls the game genuinely needs: **MovePad** (the 5-action von Neumann
> accelerator — the signature control), **Telemetry** (mono coordinate/vector/metric
> readout), **CarChip** (colored car token), and **LapMeter** (lap progress). These
> exist because they are the core interaction surface of a vector-racing game.

UI kit (`ui_kits/game/`): high-fidelity, interactive recreation of the game —
new-race setup → the race view (graph-paper track + HUD + move pad) → the track lab
(generation params + oracle metrics) → results.

---

## Caveats / substitutions

- **Fonts substituted** (Onest + JetBrains Mono via Google Fonts) — no fonts
  ship with the source.
- **Icons substituted** (Lucide via CDN) — no icon set in the source.
- **No logo exists** — the wordmark is set in plain type (**GRAPHITE GP**). No mark
  was drawn.
- The entire visual language is an **interpretation** of a scaffold-only project.
  Confirm direction before treating any of it as canonical.

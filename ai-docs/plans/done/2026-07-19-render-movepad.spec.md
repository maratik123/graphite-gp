# MovePad widget (gp-render)

**Source:** issue #16
**Date:** 2026-07-19
**Tracked in:** #16

The signature game control: the 5 von-Neumann accelerations as a plus-shaped
keypad — Coast (`·`) in the center, `↑ ↓ ← →` around it. Each cell is one
`gp_core::sim::Action`. It consumes the legal mask from `legal_mask`, disables
illegal moves, marks the chosen one, and reports the selection back to the
caller. Diagonal acceleration does not exist — only the 5 actions do.

This is the last Block 2 game widget (#16, build-order 13/40), the **6th** widget
on the established `gp_render::widgets` three-layer pattern (see #13 core, #14
forms, #15 game-HUD — all merged). The port ground truth is
`docs/design-system/components/game/MovePad.{jsx,d.ts,prompt.md}`; style is
sourced entirely from `crate::tokens`.

## Scope

1. Add a `MovePad` widget under `crates/render/src/widgets/movepad.rs`, exported
   from `gp_render::widgets` (module + `pub use` in `widgets/mod.rs`), on the
   same three-layer pattern every prior widget uses:
   - **Pure layer** — a `const fn resolve(...)` mapping per-cell logical state
     (legal / illegal / selected) to a style struct. Miri-clean: no
     `egui::Ui`, no allocation, no text. This is the AC5 unit-test backbone.
   - **Paint layer** — a private `paint(painter, rect, &style, …)` drawing the
     3×3 plus grid, per-cell arrow glyph + `a,b` sublabel, and press darkening.
   - **Show layer** — a public `show(self, ui) -> MovePadResponse` builder that
     allocates the grid, reads live pointer input (hover/press/click), resolves
     styles, paints, and returns the response.
2. **Input contract** (maps the `.jsx`/`.d.ts` props to Rust):
   - `legal: BitFlags<Action>` — the legal-action mask, consumed directly from
     `gp_core::sim::legal_mask`. All-legal is `BitFlags::all()` (the `.jsx`
     `legal={null}` sentinel has no Rust equivalent — the caller passes the real
     mask). Reuses the **existing** `gp-core` dependency (already imported in
     `crates/render/src/lib.rs`) — no new crate edge.
   - `selected: Option<Action>` — the currently-chosen action (`.jsx` `value`);
     `None` = nothing chosen.
   - `size: f32` — cell edge, builder-configurable with a const default of `48.0`
     (`.jsx` default; `Screens.jsx:129` overrides to `52`).
3. **Output contract** — `show` returns a `#[derive(Debug)] MovePadResponse {
   response: egui::Response, selected: Option<Action>, changed: bool }` mirroring
   `SegmentedControlResponse`/`SwitchResponse`. Clicking a **legal** cell sets
   `selected = Some(action)` and `changed = true`; clicking an illegal cell or
   empty corner is a no-op. State is caller-owned (immediate-mode idiom).
4. **Cell visual states** (from `MovePad.jsx:31-33`, all tokens already in
   `crate::tokens`):
   - selected → bg `ACCENT`, fg `PAPER_0`, border `ACCENT`
   - legal (unselected) → bg `PAPER_0`, fg `GRAPHITE_900`, border `GRAPHITE_900`
   - illegal → bg `PAPER_2`, fg `TEXT_FAINT`, border `BORDER_SOFT`
   - border width `BW_1`, corner radius `RADIUS_0`, `SPACE_1`-spaced cells
     (`4.0`px inter-cell gap, matching the `.jsx` `gap: 4` ground truth).
5. **Pressed darkening** (AC4, beyond the `.jsx`): while the pointer is held
   over a legal cell, overlay the established `common::GHOST_PRESS_OVERLAY`
   (`rgba(32,30,26,0.12)` — the design's flagged graphite darkening) — the same
   "graphite-key" press cue Button/IconButton use. No new color.
6. **Glyphs + sublabel** — each cell draws its arrow (`↑ ↓ ← →`, Coast `·`)
   large and mono, plus a smaller mono `a,b` sublabel (from `Action::accel()`),
   per `MovePad.jsx:39-40`.
7. **Tests** — AC5 `resolve` unit tests (Miri-clean) asserting the
   mask→disabled/legal/selected mapping in `Action` declaration order
   (`Coast, East, West, North, South`), the emit-Action-per-cell mapping, and
   the all-illegal case; plus one `#[cfg(test)]` wgpu golden over the state
   matrix, `#[cfg_attr(miri, ignore)]`, `image-check`-verified at mint.

## Out of scope

- Any window/event-loop ownership — `gp-render` stays draw-only; `gp-game` owns
  the window (design §6, #11).
- Wiring the MovePad into the actual game screen / turn loop (that lands with the
  game-screen assembly tasks, later in Block 2).
- New design tokens or a token audit — MovePad uses only tokens already ported
  by #12.
- Changes to `gp_core::sim` (`Action`, `legal_mask`, `legal_move`) — consumed
  as-is.

## Deferred

- Keyboard/gamepad activation of cells (arrow keys → actions) | pointer-only
  matches the `.jsx`; input mapping is a game-screen concern | separate issue if
  wanted, not blocking.
- Animated selection transition (`.jsx` `transition: background …`) | egui
  immediate-mode has no free CSS transition; static states match every prior
  widget port | no issue needed.

## Key decisions

| Question | Decision |
|---|---|
| Legal-mask input type | `BitFlags<Action>` consumed directly from `gp_core::sim` (re-exported `BitFlags`); reuses gp-render's existing gp-core dep. Matches the issue's "takes the legal mask (from core `legal_mask`)" and the test-note ordering. |
| Selected value + emit | Builder takes `selected: Option<Action>`; `show` returns `MovePadResponse { response, selected, changed }`. Caller-owned state, `SegmentedControl`/`Switch` precedent. `Action::accel()` supplies the `(a,b)` the issue's `onSelect` payload carries. |
| Layout | 3×3 grid, plus-shaped: Coast center, North top-center, South bottom-center, West mid-left, East mid-right; 4 corners empty. Exactly 5 cells; diagonals structurally impossible. |
| Pressed darkening (AC4) | Reuse `common::GHOST_PRESS_OVERLAY` on pointer-down over a legal cell — the in-tree "graphite darkening" convention (Button/IconButton). No new color/token. |
| All-illegal behavior | Empty mask (`BitFlags::empty()`) → every cell disabled, nothing selectable, no-op on click (test-note requirement). |
| Cell size | `f32` builder field, const default `48.0`; caller may override (`Screens.jsx` uses `52`). |
| Disabled opacity | Illegal cells are styled by their own token trio (`PAPER_2`/`TEXT_FAINT`/`BORDER_SOFT`), matching `MovePad.jsx` — not the `DISABLED_OPACITY`/`FORMS_DISABLED_OPACITY` gamma dim used by Switch/Slider. |

## Technical constraints

- Style comes **only** from `crate::tokens`; the sole non-token color is the
  shared `common::GHOST_PRESS_OVERLAY` (already a flagged exception).
- `gp-render` must keep **zero production panics**: any `# Panics` doc is the
  egui font-layout caller-precondition class (caller must install
  `crate::fonts::definitions` first), same as `switch.rs`/`button.rs`; every
  `expect`/`panic!` lives inside `#[cfg(test)]`.
- The golden test drives wgpu + `dlopen`s the Vulkan ICD, so it **must** be
  `#[cfg_attr(miri, ignore = "…")]` or it reds the workspace Miri job (a red
  Miri blocks merge, #76). The `resolve` unit tests stay Miri-clean.
- Golden exactness: `threshold(0.0)` **and** `failed_pixel_count_threshold(0)`
  both overridden (the 0.6 default is a trap), per the established golden
  convention.
- Gates: `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
  --workspace` all pass; every public item carries a `///`.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `MovePad::show` renders exactly 5 cells (Coast center + North/South/East/West) in a 3×3 plus layout; the 4 corners are empty — no diagonal cells exist. |
| AC2 | Illegal actions (per the passed `BitFlags<Action>` mask) render disabled (`PAPER_2` bg / `TEXT_FAINT` fg / `BORDER_SOFT` border) and are non-selectable (click is a no-op). |
| AC3 | Clicking a legal cell selects the corresponding `Action` and marks it chosen (`ACCENT` bg / `PAPER_0` fg); the returned `MovePadResponse` carries `selected = Some(action)` and `changed = true`, and the action's `accel()` is the correct `(a,b)`. |
| AC4 | Pressing a legal cell (pointer held down) darkens it via `common::GHOST_PRESS_OVERLAY` — the graphite-key darkening. |
| AC5 | A pure `const fn resolve(...)` maps per-cell state to style; unit tests (Miri-clean) assert the mask→state mapping in `Action` declaration order (`Coast, East, West, North, South`) and the per-cell action→`(a,b)` mapping. |
| AC6 | An all-illegal mask (`BitFlags::empty()`) yields every cell disabled and no selectable move (deterministic test). |
| AC7 | A `#[cfg(test)]` wgpu golden covers the state matrix (legal / illegal / selected / pressed), is `#[cfg_attr(miri, ignore)]`, uses exact compare, and is `image-check`-verified at mint. |
| AC8 | Style is sourced entirely from `crate::tokens`; no new tokens and no per-widget non-token colors beyond the shared `common::GHOST_PRESS_OVERLAY`. |
| AC9 | `gp-render` keeps zero production panics (all `expect`/`panic!` inside `#[cfg(test)]`; any `# Panics` doc is the font-layout precondition class) and still owns no window. |
| AC10 | `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and the doc gate all pass; the widget is exported from `gp_render::widgets`. |

## Open questions

- **Arrow/coast glyph rendering.** The `.jsx` draws Unicode `↑ ↓ ← → ·`. Verify
  these glyphs are present in the vendored Onest / JetBrains Mono faces at
  design/impl time — the `render-onest-font-swap` work found `✓` (U+2713) is
  tofu in **all** vendored faces. If any arrow is tofu, fall back to
  painter-drawn arrow shapes; otherwise use the glyphs. Default: glyphs if
  present. Design-resolvable, not blocking.
- **Golden placement.** New `movepad_gallery` `#[cfg(test)]` module vs extending
  the existing `game_gallery` (which explicitly excluded MovePad). Test-design
  detail for the `design` Subagent to choose; either satisfies AC7.

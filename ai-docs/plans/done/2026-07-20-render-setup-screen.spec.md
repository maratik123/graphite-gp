# gp-render: Setup screen (cars / laps / difficulty / V_target → generate)

**Source:** issue #19
**Date:** 2026-07-20
**Tracked in:** #19

## Scope

Build the **SetupScreen** in `gp-render` (`crates/render`) — the first screen the
player sees. It ports `docs/design-system/ui_kits/game/Screens.jsx`'s
`SetupScreen` to `egui`, reusing the already-shipped widgets, and **emits an
assembled race-configuration value** when the primary button is pressed.

1. **Wordmark block** — an accent dot, the `GRAPHITE GP` wordmark rendered in the
   display face with `GP` in the accent color, and a mono uppercase subtitle
   (`GRID VECTOR RACING`), matching the `SetupScreen` header.
2. **A `Card`** (eyebrow "New race", title "Set up the grid", grid backdrop)
   containing the four inputs:
   - **Cars (m)** — a `Stepper` bounded **2–6**.
   - **Laps** — a `Stepper` bounded **1–9**.
   - **Difficulty (pilot temperature)** — a `SegmentedControl` with
     `Rookie / Pro / Ace`, mapped to a pilot **temperature** (the softmax skill
     dial, `docs/design.md` §5).
   - **V_target (design speed)** — a `Slider` over integer cells/turn, range
     **3–10**, step **1**, formatted `"{v} cells/turn"`.
3. **A primary `Button`** ("Generate track", `lg`, primary variant) that, when
   pressed, **emits the assembled config** built from the current widget values
   (cars, laps, difficulty→temperature, V_target).
4. A mono footer caption (`Procedural · closed loop · valid by construction`).

The four widgets (`Stepper`, `SegmentedControl`, `Slider`, `Card`, `Button`)
already exist under `crates/render/src/widgets/`; this task **composes** them,
it does not re-port them.

## Out of scope

- **Wiring the emitted config into generation.** `gp-gen`'s `GenParams` does not
  yet carry `V_target`, and `gp_gen::generate` is stubbed (`todo!`). Mapping the
  race config to generation inputs and triggering a track build is downstream
  (Block 3b) work.
- **Screen transition / app-state machine** (Setup → Race). `gp-render` is
  draw-only; `gp-game` owns the window, loop, and screen orchestration
  (`ai-docs/key-decisions.md`, `docs/design.md` §6). The `RaceScreen` is a
  separate issue.
- **Actual consumption of `temperature` by the pilot.** `gp_ai::policy_action`
  is a `todo!` stub; the emitted temperature is carried, not yet consumed.
- **Seed / `min_straight` / `block_size` / `v_ceiling` inputs.** These are
  generation-pipeline parameters, not user-facing on this screen. In particular
  the slider is **`V_target`** (a design input, `docs/design.md` §2 [D3]), *not*
  `V_ceil` (the oracle's floating search bound = `GenParams.v_ceiling`) — the two
  must not be conflated.

## Deferred

- Final `Rookie / Pro / Ace` → temperature values | pilot behaviour is only
  observable once `gp-ai` (Block 4) is implemented; values are tuning-empirical
  (`docs/design.md` §5) | tracked with the Block-4 AI work, no separate issue
  needed now.

## Key decisions

| Question | Decision |
|---|---|
| Widget bounds | Adopt the `Screens.jsx` reference verbatim: cars 2–6, laps 1–9, V_target 3–10 step 1, format `"{v} cells/turn"`. |
| What "emits" means | On "Generate track" press, the screen produces the assembled race-config value from the current widget values. The exact emission mechanism (return an `Option<Config>` from the screen's `show`, a response struct with a `generate` flag, or a callback) mirrors the crate's existing widget idiom — left to the `design` Subagent. |
| Config value types | `cars: u32` (matches `gp_gen::GenParams.cars`); laps an integer; **`V_target` an integer** (whole cells/turn — the `Slider` value is snapped to integer steps via `step = 1`, and the config carries the snapped integer, honoring the integer-only physics/generation domain, `docs/design.md` §3a); temperature carried as `gp_ai::policy_action`'s temperature type. |
| Difficulty representation | The config represents difficulty as an enum (`Rookie / Pro / Ace`) with a pure `→ temperature` mapping, OR stores the resolved temperature directly — the `design` Subagent chooses. Direction is fixed by `docs/design.md` §5: **Ace = lowest temperature** (strong, smooth pilot), **Rookie = highest** (noisy). |
| Placeholder temperature values | Since no pilot consumes them yet, adopt defensible placeholders (e.g. Rookie ≈ 1.5, Pro ≈ 1.0, Ace ≈ 0.6) so the config-assembly test has concrete expected values. Marked tunable (see Deferred / Open questions). |
| Config type placement | Its only current consumer is `gp-render` (emit-only), so a `gp-render`-local type suffices for this issue. Anticipated future consumers span `gp-gen` (cars, V_target), `gp-ai` (temperature), and `gp-game` (owns/routes it) — the `design` Subagent decides whether to site the type in `gp-render` now or in a shared location, per AGENTS.md's cross-crate-duplication guidance. |
| Screen module placement | A new module in `crates/render/src` (e.g. `widgets/setup_screen.rs`, or a new `screens` module). Exact file layout left to the `design` Subagent; follow the existing widget module structure (`resolve` / `paint` / `show` layers, `pub use` re-export in `widgets/mod.rs`). |

## Technical constraints

- **`gp-render` is draw-only.** The screen renders and returns/emits data; it
  does not open windows, own state transitions, or trigger generation.
- **Reuse existing widgets** from `crates/render/src/widgets/` (`Stepper`,
  `SegmentedControl`, `Slider`, `Card`, `Button`); do not re-port their visuals.
  `Stepper` is `i32`-based with `.min`/`.max`; `SegmentedControl::new(options,
  value)` with a `selected_index` helper; `Slider` value/min/max/step with a
  `format` closure at `show`; `Card` supports `eyebrow`/`title`/`grid`/`padding`;
  `Button` supports `variant`/`size`/`icon_left`/`full_width`.
- **Style from `crate::tokens` only**; the layout must snap to the **4px
  spacing lattice** (all paddings/gaps multiples of 4, sourced from the spacing
  tokens — cf. `Screens.jsx`'s `space-*` / explicit 4-multiple values).
- **Fonts** via `gp_render::fonts`: the display face for the wordmark, the mono
  face for the subtitle/footer.
- **Golden test discipline.** Any `egui_kittest` / wgpu snapshot test of the
  screen MUST be `#[cfg_attr(miri, ignore = "...")]` (it drives wgpu, which
  `dlopen`s the Vulkan ICD — FFI aborts under Miri; a red workspace Miri blocks
  merge). Follow `crates/render/src/widgets/game_gallery.rs` as the pattern:
  in-crate `#[cfg(test)]` module, exact-compare snapshot options.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | The **Cars (m)** stepper is bounded **2–6**: it cannot step below 2 or above 6, and the emitted config's `cars` is always within `[2, 6]`. |
| AC2 | The **Laps** stepper is bounded **1–9** and the emitted config's laps is always within `[1, 9]`. |
| AC3 | The **Difficulty** `SegmentedControl` offers exactly `Rookie / Pro / Ace`, and each maps to a pilot **temperature** via a pure, unit-testable function with Ace = lowest, Rookie = highest (per `docs/design.md` §5). |
| AC4 | The **V_target** slider ranges **3–10**, step **1**, displays `"{v} cells/turn"`, and the emitted config carries an integer V_target in `[3, 10]`. |
| AC5 | The **GRAPHITE GP** wordmark renders in the display face with `GP` in the accent color, alongside the accent dot and the mono uppercase `GRID VECTOR RACING` subtitle. |
| AC6 | Pressing **"Generate track"** emits a race-config value whose `cars`, `laps`, `V_target`, and difficulty-derived temperature equal the current widget values. Emitting nothing until the button is pressed. |
| AC7 | The layout snaps to the **4px lattice** — all inter-widget gaps and card paddings are multiples of 4, sourced from `crate::tokens` spacing. |
| AC8 | Tests cover: (a) a pure config-assembly test asserting the emitted config reflects the widget values, including `difficulty → temperature`; (b) a wgpu golden snapshot of the rendered screen against the `Screens.jsx` `SetupScreen` reference, Miri-ignored per the discipline above. |

## Open questions

- **Exact `Rookie / Pro / Ace` temperature values.** Placeholders are used now
  (see Key decisions); the real values are empirical and set once `gp-ai`
  (Block 4) exists. The product owner may revisit them then.
- **Config-type shape and placement** (enum-with-mapping vs resolved
  temperature; `gp-render`-local vs shared crate) — a `design`-Subagent choice
  with defensible defaults recorded above; the product owner may prefer a
  specific home once the downstream (gp-game / gp-gen / gp-ai) consumers land.

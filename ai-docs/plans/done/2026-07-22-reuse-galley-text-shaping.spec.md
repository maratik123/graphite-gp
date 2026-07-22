# Reuse a single Galley for text shaping in gp-render

**Source:** issue #96
**Date:** 2026-07-22
**Tracked in:** #96

## Scope

A behavior-preserving refactor of `gp-render` so each text run is **shaped once**
and that one `egui::Galley` is reused for both measurement and painting, instead
of the current pattern that shapes the same run 2–3 times.

The current pattern (see `crates/render/src/widgets/telemetry.rs` `show`, the
clearest case):

1. `show` calls `Painter::layout_no_wrap` to read a run's **width**, then calls
   it **again** on the same run to read its **height** — two shaping passes for
   one measurement.
2. The `pub(crate) paint` layer then draws the same strings via `Painter::text`
   (which shapes them a **third** time internally).

`layout_no_wrap` returns an `Arc<Galley>` that already carries **both**
dimensions (`galley.size()` / `galley.rect`) and can be drawn directly via
`Painter::galley(pos, galley, fallback_color)`. Each run should therefore be
shaped once, its width **and** height read off the one `Galley`, and that same
handle reused to paint.

**Per text-rendering component in the chosen scope (see Q1):**

- In `show` (or the equivalent measure step), build the `Galley` for each text
  run **once**, keeping the `Arc<Galley>` handle.
- Derive desired size from `galley.size()` / `galley.rect` — width and height
  from the one galley, never a second `layout_no_wrap` on the same run.
- Draw by reusing the same handle (`Painter::galley(...)`) rather than
  re-shaping. Where the three-layer `resolve → paint → show` split keeps `paint`
  a separate `pub(crate)` layer, pass the pre-built galley handles into `paint`
  instead of raw `&str`, so each run is shaped exactly once end-to-end.

**Surfaces in scope (Q1 = whole `gp-render`).** Two code shapes, both covered:

1. **Widgets** with the `resolve → paint → show` split, under
   `crates/render/src/widgets/`. Candidates (illustrative, from a `.text(` /
   `layout_no_wrap` grep): `telemetry`, `lap_meter`, `car_chip`, `badge`,
   `button`, `tag`, `switch`, `segmented_control`, plus `card`, `slider`,
   `stepper`, `movepad`, and any shared text helper in `common.rs`.
2. **Screens + app shell** — inline free functions with **no** 3-layer split:
   `crates/render/src/screens/{setup,results,lab}.rs` and
   `crates/render/src/app.rs`. Here the fix is local (build the galley once in
   the same function, reuse the handle for `Painter::galley`); there is no
   `paint`-layer signature to thread.

**The design phase owns the authoritative list.** The issue directs the design
phase to survey the then-current set rather than treat any list as fixed. The
design phase MUST re-run the survey (`grep -lE '\.text\(|\.galley\(|layout_no_wrap'
crates/render/src/`) and apply the change wherever the shape-more-than-once
pattern occurs across both surfaces; it MUST NOT treat the lists above as
authoritative or exhaustive.

## Out of scope

- Any change to rendered output. This is behavior-preserving: the golden
  snapshots must stay byte-identical (see AC4).
- Public-API changes beyond internal `pub(crate)` `paint`-layer signatures.
- Changing which font / color / position any run draws with.
- Introducing wrapping or multi-line layout (`layout_no_wrap` semantics stay).
- **Other workspace crates — cross-crate audit, NEGATIVE (verified 2026-07-22).**
  The measure-then-reshape pattern can only exist where `egui` is used, which is
  `gp-render` alone:
  - `gp-core`, `gp-gen`, `gp-ai`: no `egui`/`eframe` dependency
    (`rg -l 'egui|eframe' crates/*/Cargo.toml` → only `render` + `game`) — zero
    text rendering.
  - `gp-game`: depends on `eframe` but draws no text of its own — `main.rs`
    installs fonts/visuals and delegates every draw to
    `gp_render::app::AppShell::show`. No `layout_no_wrap` / `Painter::galley` /
    `Galley` / `RichText` / `.label(` in `crates/game/src`
    (`rg` → NONE).

  Conclusion: "Whole `gp-render`" already captures the pattern fully; no other
  crate is in scope. The design phase and reviewer need not re-litigate this.

## Deferred

- None. Q1 resolved to "Whole `gp-render`" — the screens and app shell are
  in scope, not deferred to a follow-up.

## Key decisions

| Question | Decision |
|---|---|
| Mechanism | Shape once via `layout_no_wrap`; read width+height off the one `Galley`; reuse the handle via `Painter::galley`. Per the issue. |
| `paint`-layer signature | Widgets with a `pub(crate) paint` layer receive pre-built `Arc<Galley>` handles instead of raw `&str` for runs shaped in `show`. `pub(crate)` only — no API-stability concern (AGENTS.md § *API Stability*). |
| Target list authority | Design phase re-surveys `crates/render/src/` and applies the change wherever the pattern occurs; the candidate list above is illustrative, per the issue. |
| Shared helper vs per-widget | Left to the design phase. This is a single crate, so the AGENTS.md ≥3-crate shared-crate rule does not trigger; design chooses a shared helper in `common.rs` vs per-widget inlining. |
| Scope boundary (Q1, round 1) | **Whole `gp-render`.** Both the `resolve/paint/show` widget family and the inline screen/app-shell free functions are in scope; nothing is left half-done. |

## Technical constraints

- **Byte-identical goldens.** The `gp-render` goldens are exact-compare wgpu
  snapshots (threshold 0, failed-pixel-count 0). Reuse must reproduce the exact
  pixels `Painter::text` currently produces. Affected golden set
  (`crates/render/tests/snapshots/*.png`): widget goldens `widget_gallery`,
  `forms_gallery`, `movepad_gallery`, `game_gallery`; and — since Q1 resolved to
  whole-crate — the screen / app-shell goldens `setup_screen`, `results_screen`,
  `lab_screen`, `app_shell`, `app_shell_race`, `app_shell_lab`, `race_screen`.
  No snapshot regeneration is permitted — a diff means the refactor changed
  output and is a defect.
- **Anchoring must be reproduced by hand.** `Painter::galley(pos, galley, _)`
  draws at a top-left `pos` — it does **not** apply `Align2` anchoring, and it
  does not return the drawn rect. The current code relies on both: `Painter::text`
  takes an `Align2` (LEFT/RIGHT/CENTER) and returns a rect that downstream layout
  reads (e.g. `telemetry` positions the value row off the label rect's
  `max.y`; `car_chip` advances its cursor by the measured name width). The
  refactor must compute anchored top-left positions and downstream rects from
  `galley.size()` / `galley.rect` so pixels match `Painter::text` exactly
  (including any rounding) — the primary golden-drift risk.
- **Color is baked into the galley.** `layout_no_wrap(text, font, color)` colors
  the whole run; `Painter::galley`'s `fallback_color` only fills
  `Color32::PLACEHOLDER` sections, so reuse preserves color with no regression.
  Use the same color already passed at measurement time.
- **String allocations collapse.** Per-run `to_owned()` / `to_uppercase()` (e.g.
  `telemetry`'s label, done thrice today) is done once when the galley is built.
- **Miri.** The gallery/screen goldens already carry `#[cfg_attr(miri, ignore)]`
  (wgpu FFI + `vello_cpu` cast). No new FFI/Context-constructing test is expected;
  if any is added it must carry the gate per AGENTS.md § *Rust Test Conventions*.
- **`layout_no_wrap` vs `Painter::text` equivalence.** `Painter::text`
  internally shapes via the same `layout_no_wrap` path then calls
  `Painter::galley`; splitting the two must be a pure factoring with no visible
  change. Verify by the golden safety net.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | For every in-scope text-rendering component across **both** surfaces (the `resolve/paint/show` widgets under `crates/render/src/widgets/` **and** the inline screen/app-shell free functions in `crates/render/src/screens/*.rs` + `crates/render/src/app.rs`), per the design survey, each text run is shaped exactly **once** end-to-end: no `layout_no_wrap` call whose sole purpose is to re-measure a run already shaped, and no re-shaping (via `Painter::text` or a second `layout_no_wrap`) of a run already built earlier in the same draw. |
| AC2 | In-scope components derive desired size from `galley.size()` / `galley.rect` (width **and** height from the one galley), not from a second `layout_no_wrap` on the same run. |
| AC3 | Widgets with a `pub(crate) paint` layer receive pre-built galley handles (not raw `&str`) for runs shaped in `show`, and draw them via `Painter::galley`. Screen / app-shell free functions build the galley once locally and reuse the handle (no signature change needed there). |
| AC4 | All affected golden snapshots remain **byte-identical** — `cargo test -p gp-render` is green with **no** snapshot regeneration. |
| AC5 | Existing resolve-layer unit tests pass unchanged (no behavioral edit to `resolve`). |
| AC6 | `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` are clean. |
| AC7 | No public-API change; only internal `pub(crate)` `paint` signatures may change. |

## Open questions

- None. Q1 (scope boundary) resolved in round 1 to whole `gp-render`; no
  design-affecting ambiguity remains.

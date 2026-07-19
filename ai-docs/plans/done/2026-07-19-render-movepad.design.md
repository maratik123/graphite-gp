# Design: MovePad widget (gp-render)

**Issue:** #16
**Date:** 2026-07-19

## Approach

MovePad is the 6th widget on the established `gp_render::widgets` three-layer
pattern (const `resolve` → `pub(crate) paint` → `pub show`), ported from
`docs/design-system/components/game/MovePad.jsx`. It reuses every convention
already set by `segmented_control.rs`, `switch.rs`, and the game-HUD widgets
(`car_chip.rs`, `game_gallery.rs`) — no new infrastructure. It consumes
`gp_core::sim::{Action, BitFlags}` (Action/`legal_mask` contract, `docs/design.md`
§3) **as-is**; no `gp_core` change.

### The three layers

**Pure layer — `const fn resolve(legal: bool, selected: bool) -> MoveCellStyle`.**
A per-cell style map, Miri-clean (no `egui::Ui`, no allocation, no text). Ports
the `MovePad.jsx:31-33` ternary directly, `selected` taking precedence over
`legal`:

| input | bg | fg | border |
|---|---|---|---|
| `selected` (any legal) | `ACCENT` | `PAPER_0` | `ACCENT` |
| `legal` unselected | `PAPER_0` | `GRAPHITE_900` | `GRAPHITE_900` |
| illegal | `PAPER_2` | `TEXT_FAINT` | `BORDER_SOFT` |

`MoveCellStyle { bg, fg, border }` is a 3-`Color32` struct →
`#[derive(Clone, Copy, Debug, PartialEq, Eq)]` (Eq is legal — no `f32` field;
border width is the fixed `BW_1` const, not part of the resolved style, since
all three states share it per `MovePad.jsx:31`). All colors are `crate::tokens`
consts — verified present [measured: `rg 'pub const' crates/render/src/tokens/*.rs`
→ `PAPER_0` `color.rs:11`, `PAPER_2` `:15`, `GRAPHITE_900` `:22`, `ACCENT`
`:59`, `TEXT_FAINT` `:131`, `BORDER_SOFT` `:146`]. `resolve` is a body of
struct-literals over const `Color32`s with no non-const call →
**`missing_const_for_fn` (nursery, `deny`) FORCES `const fn`**
[measured: `Cargo.toml [workspace.lints.clippy] nursery = { level = "deny" }`];
matches `SegmentedControl::resolve`/`Switch::resolve`/`CarChip::resolve`, all
`pub const fn`. The **press darkening stays OUT of `resolve`** (it is a
composited paint-time overlay, not a resolved base color) — consistent with
`common.rs`'s note that opacity/overlay ops are deferred from `resolve`.

**`MOVES` layout table** (single source of truth for the plus layout + glyphs,
mirroring the `.jsx` `MOVES` object), in `Action` **declaration order**
(`Coast, East, West, North, South`) [measured: `sim/mod.rs:50-60`]:

```
struct MoveCell { action: Action, glyph: &'static str, row: u8, col: u8 }
// row: 0=top 1=mid 2=bottom ; col: 0=left 1=center 2=right (screen-space)
Coast "·" (1,1) · East "→" (1,2) · West "←" (1,0) · North "↑" (0,1) · South "↓" (2,1)
```

Grid positions transcribe `MovePad.jsx:11-15` `grid` (`row / col`, 1-based → 0-based):
North=top-center, South=bottom-center, West=mid-left, East=mid-right, Coast=center;
the 4 corners are unused → exactly 5 cells, diagonals structurally impossible (AC1).
The glyph per action pairs with `Action::accel()` [measured: `sim/mod.rs:66-74`:
Coast(0,0) East(1,0) West(-1,0) North(0,1) South(0,-1)] — North(0,+1)="↑",
South(0,-1)="↓" is correct for screen-space (top row = smallest y = North/up).

**Glyphs (open question 1 — RESOLVED: use Unicode glyphs).** All five glyphs are
present in **both** vendored faces
[measured: skrifa 0.42 `charmap().map(ch)` probe over the in-tree
`Onest[wght].ttf` + `JetBrainsMono[wght].ttf` →
`Onest: U+2191 ↑ YES  U+2193 ↓ YES  U+2190 ← YES  U+2192 → YES  U+00B7 · YES`;
`JetBrainsMono: U+2191 ↑ YES  U+2193 ↓ YES  U+2190 ← YES  U+2192 → YES  U+00B7 · YES`].
The `.jsx` draws them in `--font-mono`, so MovePad draws the arrow in
`JETBRAINS_MONO_BOLD` (`.jsx` `fontWeight: 700`) and the `a,b` sublabel in
`JETBRAINS_MONO_REGULAR`. **No painter-drawn arrow fallback is needed** — the
`✓` (U+2713) tofu that motivated the font-swap caution does not extend to these
glyphs; the existing `fonts.rs` test already pins `·` and `→` present in
JetBrains Mono [measured: `fonts.rs:291-296`], and this probe extends the
guarantee to `↑ ↓ ←`.

**Paint layer — `pub(crate) fn paint(painter, pad_rect, legal: BitFlags<Action>,
selected: Option<Action>, pressed: Option<Action>, size: f32)`** (6 args, under
the `too_many_arguments` 7-limit). Loops `MOVES`; per cell computes
`is_legal = legal.contains(cell.action)` [measured: enumflags2-0.7.12
`src/lib.rs:849 pub fn contains<B: Into<BitFlags<T>>>`; `Action: Into<BitFlags>`
via `#[bitflags]`] and `is_selected = selected == Some(cell.action)`, then:

1. `common::paint_surface(painter, rect, RADIUS_0, style.bg, style.border, BW_1)`
   — reuses the shared fill+inside-stroke helper [measured: `common.rs:56`
   `pub(crate) fn paint_surface`]; `RADIUS_0 = 0.0` → square cell, `BW_1 = 1.5`
   [measured: `spacing.rs:49,68`].
2. if `is_legal && pressed == Some(cell.action)`: overlay
   `painter.rect_filled(rect, RADIUS_0, common::GHOST_PRESS_OVERLAY)` — the
   translucent (`alpha 31`) graphite-key darkening composited over the cell
   fill, drawn **before** the glyph so text stays crisp (AC4). Same non-token
   overlay Button/IconButton use [measured: `common.rs:30`, `button.rs:172`,
   `icon_button.rs:124`] — no new color.
3. arrow glyph (`JETBRAINS_MONO_BOLD`, size `(size * ARROW_FS_FACTOR).round()`)
   centered upper; `a,b` sublabel (`JETBRAINS_MONO_REGULAR`, size
   `(size * SUBLABEL_FS_FACTOR).round()`, tinted `fg.gamma_multiply(SUBLABEL_OPACITY)`)
   centered lower with a `SUBLABEL_GAP` gap — ports `MovePad.jsx:39-40`
   (`fontSize: round(size*0.42)` / `round(size*0.19)`, `opacity:0.7`,
   `marginTop:2`). `gamma_multiply` is a paint-time op (not const), matching the
   `common.rs` opacity convention; the sublabel string is `format!("{},{}", a, b)`
   from `cell.action.accel()`.

**Show layer — `pub fn show(self, ui) -> MovePadResponse`.** Allocates a
`(size*3 + gap*2)`-square hover rect (`gap = SPACE_1 = 4.0`, see below), then per
`MOVES` cell that is **legal** calls `ui.interact(cell_rect(..), response.id.with(cell.action), Sense::click())`
(`Action: Hash` [measured: `sim/mod.rs:45`], so `id.with(action)` is valid, as
`segmented_control.rs:208` does with `usize`); records `clicked`/`pressed` via
`.clicked()` / `.is_pointer_button_down_on()` [measured: `button.rs:348`,
`icon_button.rs:206`]. **Illegal cells are skipped** (never receive
`Sense::click`) → clicking one is a structural no-op (AC2/AC6). Selection:
`selected = clicked.or(self.selected)` (fresh click overrides the caller's
value, `SegmentedControl` precedent `:214`). Paints, returns
`MovePadResponse { response, selected, changed: clicked.is_some() }` (AC3).

### Builder + response

```
pub struct MovePad { legal: BitFlags<Action>, selected: Option<Action>, size: f32 }
MovePad::new(legal)          // const, defaults selected=None, size=SIZE (48.0)
    .selected(action)        // const setter
    .size(f32)               // const setter

#[derive(Debug)]
pub struct MovePadResponse { response: egui::Response, selected: Option<Action>, changed: bool }
```

`BitFlags<Action>` is named via the `gp_core::sim` re-export [measured:
`sim/mod.rs:15 pub use enumflags2::BitFlags;`], so gp-render needs **no** direct
`enumflags2` edge — reuses the existing `gp-core` dep [measured:
`crates/render/Cargo.toml:14`]. All-legal is `BitFlags::all()`, all-illegal is
`BitFlags::empty()` [measured: enumflags2 `src/lib.rs:785 all`, `:757 empty`].
`new`/setters are const (struct-literal bodies, no non-const call) — same
`missing_const_for_fn` force + `Switch::new`/`.label`/`.enabled` precedent.
`MovePadResponse` derives `Debug` (all fields `Debug`: `egui::Response` per
`SegmentedControlResponse`, `Option<Action>`, `bool`) → mirrors
`SegmentedControlResponse`/`SwitchResponse`.

### Cell gap (`SPACE_1` = 4.0, owner-confirmed — settled)

The `.jsx` grid uses `gap: 4` [measured: `MovePad.jsx:50` `gap: 4`], which equals
`spacing::SPACE_1 = 4.0` [measured: `spacing.rs:15`]. **The product owner confirmed
`SPACE_1` (4.0) during Step-7 design-review resolution, and the spec's Scope item 4
was amended from "`BW_1`-spaced cells" to "`SPACE_1`-spaced"** [measured:
`2026-07-19-render-movepad.spec.md:52` → "border width `BW_1`, corner radius
`RADIUS_0`, `SPACE_1`-spaced cells (`4.0`px inter-cell gap, matching the `.jsx`
`gap: 4` ground truth)"]. Design and spec now agree; **no owner flag remains** — the
`show` layer allocates its grid on `gap = SPACE_1 = 4.0`. (`BW_1 = 1.5` is the
border **width**, a separate concern, unchanged.) Both are `crate::tokens` consts,
so AC8 holds.

### Rejected alternatives

- **Painter-drawn arrow shapes** — rejected: all five glyphs verified present in
  both faces (measured above); glyphs match the `.jsx` and every prior widget's
  text-draw path.
- **Extending `game_gallery` for the golden** — rejected: `game_gallery.rs`'s
  own docstring scopes it to exclude MovePad [measured: `game_gallery.rs:8-9`
  "MovePad region omitted, out of scope"], and extending it would re-mint an
  already-merged, image-check-verified golden. See § Test Design (open question 2).
- **A 3-variant `CellState` enum for `resolve`** — rejected (YAGNI): two bools
  `(legal, selected)` port the `.jsx` ternary directly and match the
  bool/enum-param precedent of every prior `resolve`.
- **`Action::iter()` (strum) to enumerate cells** — rejected in favour of the
  `MOVES` const table: the table is the single source of truth for per-cell
  layout metadata (glyph, row, col) the iterator cannot carry, and pins the
  declaration order the AC5 test asserts. (strum *is* available
  [measured: `crates/render/Cargo.toml:19 strum`], so this is a clarity choice,
  not a dependency constraint.)

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Add the `MovePad` widget: `MOVES` table, `MoveCellStyle`, `const fn resolve`, `MovePad` builder + `MovePadResponse`, `paint` (chrome via `paint_surface`, press overlay, glyph + sublabel), `show`; TDD unit tests first (AC1/AC5/AC6, Miri-clean); export `pub mod movepad` + `pub use movepad::{MovePad, MovePadResponse}` in `widgets/mod.rs`. Gates: clippy `-D warnings`, `fmt --check`, doc gate. | `crates/render/src/widgets/movepad.rs`, `crates/render/src/widgets/mod.rs` | — |
| 2 | Add the wgpu state-matrix golden `movepad_gallery` (legal / illegal / selected / pressed), driven through `MovePad::paint` with forced values; `#[cfg_attr(miri, ignore)]`, exact compare; register `#[cfg(test)] mod movepad_gallery;` in `mod.rs`; **mint golden + spawn `image-check`** at mint. | `crates/render/src/widgets/movepad_gallery.rs`, `crates/render/src/widgets/mod.rs` | 1 |

M = 2. Both subtasks are **code** change-type (`*.rs` + the minted golden PNG,
which is a code-group artifact produced by `code-writer` + `image-check`).
Within the 15-task cap; no issue-split needed.

## Handoff plan

Per § Rules → handoff-grouping, mandatory for every M ≥ 1. **(a)** M = 2 → one
group. **(e)** Both subtasks change **code** (Rust `*.rs`) → homogeneous, no
change-type boundary. **(f)** Already the fewest groups possible (1). **(b/d)**
Group A holds 2 consecutive subtasks — within the `≤ 10` cap and the terminal
`1..=10` range. **(h)** 1 group ≤ 4.

- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent, 1M-token window — subtasks 1–2 (code change-type:
  `*.rs`). **Terminal group** (2 subtasks; within `1..=10`).
- **Entry into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) — the
  every-group handoff fires on the first group too. Being the single, terminal
  group, Group A completes /task Step 8 in its own `/context-reset` subagent; no
  inter-group handoff follows. **(c)** destination named. **(g)** code group →
  `subagent_type="code-writer"` (frontmatter-pinned `model: sonnet` +
  `effort: medium`; no inline override). The `design`/`design-review`/
  `self-review` gates stay on Opus.

## Risks

- **Golden reds the workspace Miri job** (a red Miri blocks merge, #76): the
  wgpu golden `dlopen`s the Vulkan ICD. Mitigation: `#[cfg_attr(miri, ignore =
  "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)")]`, verbatim from
  the established pattern — `[measured: game_gallery.rs:192-195]`. The `resolve`/
  `MOVES` unit tests are text-free/allocation-free → Miri-clean.
  `[derived → MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace, Group A gate]`
- **`arithmetic_side_effects` (deny) on layout math**: cell rects, extent, and
  proportional font sizes are all `f32` arithmetic, which the lint does **not**
  fire on (only integer ops can panic). Evidence: `switch.rs` paint does `f32`
  `+`/`/` under this crate's `[lints] workspace = true` on green main
  `[measured: crates/render/Cargo.toml:10-11; switch.rs:136,147]`. Row/col are
  cast via `f32::from(u8)` before any multiply, so there is **no** integer
  arithmetic in `paint`/`show`. `[derived → cargo clippy --workspace --all-targets -- -D warnings, Group A gate]`
- **Zero-production-panic invariant (AC9)**: `paint`'s `painter.text` panics only
  under the font-layout caller-precondition class (caller must install
  `crate::fonts::definitions` first) — documented as `# Panics`, identical to
  `switch.rs`/`car_chip.rs` `[measured: switch.rs:115-118; car_chip.rs:166-169]`.
  Every `expect`/`panic!` lives in `#[cfg(test)]`. gp-render owns no window
  (draw-only, design §6). `[derived → RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace + clippy, Group A gate]`
- **Caller passes a `selected` action that is now illegal**: `resolve(false, true)`
  → selected (accent) style, and the cell is non-interactive (skipped in the
  `show` interact loop). This matches `MovePad.jsx:31-33` (`selected` wins over
  `isLegal`); the caller owns state and normally passes a legal selection. No
  panic, defined behaviour. `[measured: MovePad.jsx:31-33]`

## Test Design

**Subtask 1 — `movepad.rs` `#[cfg(test)] mod tests`** (Miri-clean; every prior
widget covers `resolve` + table + builder here and defers interaction to the
golden — no prior widget headless-tests `show`'s click
`[measured: switch.rs:227-265, segmented_control.rs:277-337 test modules]`):

- `resolve_selected_legal_illegal_colors` (AC5) — asserts the three-row style
  table above (`resolve(_, true)` = ACCENT/PAPER_0/ACCENT; `resolve(true, false)`
  = PAPER_0/GRAPHITE_900/GRAPHITE_900; `resolve(false, false)` =
  PAPER_2/TEXT_FAINT/BORDER_SOFT), all via `crate::tokens::color::*`.
- `moves_table_matches_action_order_and_accel` (AC1/AC5) — asserts `MOVES` is
  exactly the 5 actions in `Coast, East, West, North, South` order, each entry's
  `action.accel()` equals the `.jsx` `(a,b)` (`Coast(0,0) East(1,0) West(-1,0)
  North(0,1) South(0,-1)`), each glyph is the expected `↑↓←→·`, and the plus grid
  positions are `Coast(1,1) East(1,2) West(1,0) North(0,1) South(2,1)` — pins
  AC1's "exactly 5, plus layout, no diagonals".
- `all_illegal_mask_yields_all_disabled` (AC6) — for `BitFlags::empty()`, every
  cell's `legal.contains(action)` is `false`, so each resolves to the illegal
  style trio (deterministic). (The click-no-op half of AC6 is structural — illegal
  cells never receive `Sense::click` in `show` — and is visually covered by the
  Pad C golden.)
- `new_has_expected_defaults` — `MovePad::new(BitFlags::all())` → `selected None`,
  `size == SIZE (48.0)`; `.selected(a)`/`.size(x)` set their fields.

- Location: `crates/render/src/widgets/movepad.rs` `#[cfg(test)] mod tests`.
- Entry points: `MovePad::resolve`, the `MOVES` const, `MovePad::new`/setters.
- Fixtures: none beyond `BitFlags::{all,empty}` and literal `Action`s.

**Subtask 2 — `movepad_gallery.rs` wgpu golden** (AC7; open question 2 —
**RESOLVED: new module**). Mirrors `game_gallery.rs`'s harness **verbatim**
[measured: `game_gallery.rs:184-244`]: `CANVAS_RECT` const sized to hold the
matrix, `RendererOptions::PREDICTABLE` render_state, CPU-adapter assertion,
frame-1-install-fonts / frame-2-draw closure, `harness.run_steps(1)`,
`try_image_snapshot_options`. Draws **through `MovePad::paint`** (pub(crate),
reachable as a `widgets` submodule — `game_gallery` precedent) with forced values
across the state matrix:

- **Pad A** — `legal = BitFlags::all()`, `selected = Some(Action::North)`,
  `pressed = None` → all-legal + one selected (ACCENT).
- **Pad B** — `legal = Coast|East|West`, `selected = None`,
  `pressed = Some(Action::East)` → North/South illegal (disabled) + East pressed
  (GHOST_PRESS_OVERLAY darkening).
- **Pad C** — `legal = BitFlags::empty()`, `selected = None`, `pressed = None`
  → all disabled (AC6 visual).

Covers legal / illegal / selected / pressed in one frame. `size = 52.0`
(`Screens.jsx:129` override). `#[cfg_attr(miri, ignore = "...")]`.

- **Compare exactness**: `failed_pixel_count_threshold(0)` is the invariant
  (never the 0.6 default). The per-pixel `threshold` starts at `0.0`
  (spec item) but MovePad renders glyphs, and the two prior **text-bearing**
  goldens both empirically require `threshold(1.0)` for 1-level cross-renderer
  AA-text channel rounding [measured: `game_gallery.rs:237` `.threshold(1.0)`
  with the "AA text pixels" comment]. The code-writer determines the minimal
  passing per-pixel threshold at mint (`0.0` preferred; raise to `1.0` only if
  AA-text noise appears, exactly as `gallery.rs`/`game_gallery.rs` did); either
  is the project's "exact compare" convention because
  `failed_pixel_count_threshold` stays `0`.
- **Mint protocol**: `code-writer` mints the golden and spawns the `image-check`
  subagent to verify the PNG against the drawing code (never in CI) before
  committing (AC7).

## Deferred

- **Shared in-crate wgpu golden test helper** (design-review Note 2 — deferred by
  owner, **out of scope for this widget task; do NOT refactor here**). The golden
  harness — `create_render_state` + `RendererOptions::PREDICTABLE` + CPU-adapter
  assert + frame-1-install-fonts closure + `run_steps(1)` +
  `try_image_snapshot_options` — is copy-pasted across the three existing gallery
  test modules [measured: `rg -c -e PREDICTABLE -e run_steps -e
  try_image_snapshot_options -e create_render_state` over each → `gallery.rs` 4,
  `forms_gallery.rs` 4, `game_gallery.rs` 4]. Subtask 2's `movepad_gallery.rs`
  becomes the **4th** copy, crossing the **≥3-site duplication threshold**
  (design § Rules → *≥3-site duplication → shared helper*). Because all four are
  `#[cfg(test)]` modules **inside the same crate** (`gp-render`), the correct
  lift is an **in-crate** `#[cfg(test)]` helper (e.g. `fn run_golden(canvas_rect,
  draw_fn, snapshot_name)`) in a shared `widgets` test module — **not** a new
  workspace crate — with identical per-test-binary linkage semantics, so no
  behavioural cost. **This task deliberately keeps the copy-paste** per owner
  direction: subtask 2 mirrors the existing harness verbatim and does **not**
  pre-emptively extract it. Recorded here (with the 4-site count) so the
  trade-off is auditable and a follow-up cleanup issue can pick it up.

## Open questions

Both spec open questions are **resolved in this design**; listed for the record:

1. **Arrow/coast glyph rendering** — RESOLVED: use the Unicode glyphs `↑ ↓ ← → ·`
   directly (drawn in JetBrains Mono, bold for the arrow). Verified present in
   both vendored faces (skrifa probe, § Approach). No painter-drawn fallback.
2. **Golden placement** — RESOLVED: a new `movepad_gallery` `#[cfg(test)]`
   module, **not** an extension of `game_gallery` (whose docstring explicitly
   excludes MovePad and whose specimen layout does not fit the required state
   matrix). Follows the per-widget-group gallery split
   (`gallery` / `forms_gallery` / `game_gallery` / `movepad_gallery`).

- **Cell gap** — RESOLVED and **settled**: owner confirmed `SPACE_1` (4.0) and the
  spec's Scope item 4 was amended to "`SPACE_1`-spaced" (§ Approach → *Cell gap*).
  No open owner flag remains.

No open questions remain.

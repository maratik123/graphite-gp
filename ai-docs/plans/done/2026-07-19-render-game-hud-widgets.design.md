# Design: gp-render game HUD widgets — Telemetry, LapMeter, CarChip

**Issue:** #15
**Date:** 2026-07-19

## Approach

Port the three game-specific readout widgets from `docs/design-system/components/game/`
(`Telemetry`, `LapMeter`, `CarChip`) to `egui` in `gp-render`, following the
three-layer widget pattern established by #13/#14 verbatim: a pure
`const fn resolve(...) -> *Style` style-resolution layer (Miri-clean, no
`egui::Ui`, no allocation), a private `pub(crate) fn paint(painter, rect, &style, …)`
draw layer, and a public `show(self, ui) -> Response` shell. All three widgets
are non-interactive (their `.d.ts` carry no `onClick`), so — exactly like
`Badge` — `show` allocates the rect with `Sense::hover()` and returns a bare
`Response` (no custom `*Response` struct is needed) `[measured: Read crates/render/src/widgets/badge.rs → show returns Response via allocate_exact_size(_, Sense::hover())]`.

One file per widget under `crates/render/src/widgets/` (`telemetry.rs`,
`lap_meter.rs`, `car_chip.rs`), each re-exported from `widgets/mod.rs`,
mirroring the one-file-per-widget layout `[measured: ls crates/render/src/widgets/ → badge.rs, tag.rs, switch.rs, slider.rs, stepper.rs, segmented_control.rs each one file]`.
A fourth file, `game_gallery.rs`, adds the HUD-specimen golden, mirroring
`gallery.rs`/`forms_gallery.rs` (in-crate `#[cfg(test)] mod`, driving each
widget's crate-visible `paint` with forced values) `[measured: Read crates/render/src/widgets/gallery.rs + forms_gallery.rs → both are #[cfg(test)] modules calling `Widget::paint` directly on a wgpu CPU adapter]`.

### Key decisions

1. **Six-variant `Tone` enum for Telemetry, distinct from `BadgeTone`.**
   `Default | Accent | Ok | Warn | Danger | Muted` → `TEXT_INK | ACCENT | OK | WARN | DANGER | TEXT_MUTED`.
   Telemetry colors solid text (one `value_color`), unlike `BadgeTone`'s
   tint/fg pair, so a separate enum is correct (spec Key-decisions row).
   Declared as a plain `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum`
   — no `strum`/`enum-map`, matching `badge::Tone`
   `[measured: rg 'derive|enum Tone' badge.rs → #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Tone]`.

2. **`size` reuses `common::Size` (`Sm/Md/Lg`); the size→px table is
   Telemetry-local.** `sm → FS_TITLE (18)`, `md → FS_H3 (22)`, `lg → FS_H2 (30)`
   `[measured: Read typography.rs → FS_TITLE=18.0, FS_H3=22.0, FS_H2=30.0]`,
   as a pure `const fn value_font_size(Size) -> f32`. `common::Size` already
   backs `Button`/`IconButton`/`SegmentedControl`
   `[measured: Read common.rs + forms_gallery.rs → Size used by SegmentedControl::resolve(_, size)]`;
   its doc comment lists only "Button, IconButton" (already stale re
   SegmentedControl) — the implementor MAY extend that list to include the new
   users, but it is not load-bearing.

3. **`align` becomes a Telemetry-local `Align { Left, Right }` enum**, per AC9's
   "union → enum" rule and the #13/#14 idiom of one enum per union prop
   (`Button::Variant`, `Badge::Tone`). Re-exported as `TelemetryAlign` to avoid
   colliding with `egui::Align`. This is **one public enum beyond AC10's
   parenthetical export list** — see § Open questions (AC9 vs AC10
   reconciliation). `align` is a paint/layout concern only; it is **not** a
   `resolve` parameter, keeping `resolve(tone, size, on_ink)` exactly as AC2
   prescribes.

4. **On-ink token binding (resolves the spec's Q1 design-phase note).**
   Telemetry gains an `on_ink: bool` field (the one field beyond the `.d.ts`,
   per Q1). `resolve(tone, size, on_ink)` produces:
   - `value_color` = `match tone { Default => on_ink ? PAPER_0 : TEXT_INK, Accent => ACCENT, Ok => OK, Warn => WARN, Danger => DANGER, Muted => muted_color }`
   - `muted_color` = `on_ink ? TEXT_FAINT : TEXT_MUTED` (used for the label, the
     unit, **and** the `Muted`-tone value)
   - `value_size` = the size→px table above.

   **Binding chosen and why:**
   - on-ink `Default` value → **`PAPER_0`** (`#FBF8F0`), **not** `TEXT_ON_INK`
     (which = `PAPER_1` = `#F5F1E6`). The card's specimen `--text-ink` override
     is literally `#FBF8F0`, and `PAPER_0` matches those bytes exactly while
     `TEXT_ON_INK`/`PAPER_1` does not; both are token-sourced, so the tie-break
     is "match the card"
     `[measured: Read color.rs → PAPER_0=(FB,F8,F0); TEXT_ON_INK=PAPER_1=(F5,F1,E6); Read game.card.html:32-33 → --text-ink:#FBF8F0]`.
   - on-ink muted → **`TEXT_FAINT`** (= `GRAPHITE_400` = `#A69D8C`), the card's
     `--text-muted` override. `TEXT_FAINT` and `GRAPHITE_400` are byte-identical;
     binding to the semantic alias `TEXT_FAINT`
     `[measured: Read color.rs → TEXT_FAINT=GRAPHITE_400=(A6,9D,8C); Read game.card.html:29,34 → --text-muted:#A69D8C]`.

   The card applies the two `style` overrides **inconsistently** per-widget
   (SPEED gets only `--text-muted`; v/POS get only `--text-ink`; TEMPO only
   `--text-muted`) `[measured: Read game.card.html:28-35]`. Our `on_ink` mode
   applies **both** uniformly (default value → `PAPER_0`, label+unit+muted-value
   → `TEXT_FAINT`, semantic tones unchanged). The spec explicitly sanctions this
   ("Since the golden is self-minted, the chosen bytes just need to be
   token-sourced and match the card's look"). Semantic tones
   (`Accent`/`Ok`/`Warn`/`Danger`) are unchanged on ink.

5. **`LapMeter` uses `i32` for both `lap` and `total`; `resolve` clamps in
   const with comparisons only (no casts).**
   ```
   pub const fn resolve(lap: i32, total: i32) -> LapMeterStyle {
       let total = if total < 0 { 0 } else { total };
       let done  = if lap < 0 { 0 } else if lap > total { total } else { lap };
       LapMeterStyle { done, total }
   }
   ```
   This keeps `resolve` a **pure `const fn`** (AC7): only integer comparisons,
   which are const-stable, so `missing_const_for_fn` fires and const is correct
   `[derived → cargo clippy --workspace --all-targets -D warnings]`. Using
   `i32/i32` (rather than a signed/unsigned mix) sidesteps every `as` cast —
   the pedantic `cast_sign_loss`/`cast_possible_truncation` denies never arise
   inside `resolve`. `done`/`total` are non-negative after clamp; `i32→f32`
   conversions for cell geometry happen later in `paint` via the in-crate
   `f32::from(u16::try_from(x).unwrap_or(u16::MAX))` idiom
   `[measured: Read gallery.rs:35-37 → col/row converted with f32::from(u16::try_from(...).unwrap_or(u16::MAX))]`.

6. **`CarChip::resolve(active, kind: Option<CarKind>) -> CarChipStyle`** carries
   the chip chrome plus an `Option<KindTagStyle>` for the pill:
   ```
   pub struct KindTagStyle { pub fg: Color32, pub border: Color32 }
   pub struct CarChipStyle {
       pub bg: Color32, pub border: Color32, pub border_width: f32,
       pub tag: Option<KindTagStyle>,
   }
   ```
   `active` → `(PAPER_2, GRAPHITE_900, BW_2)`; resting → `(PAPER_0, BORDER_HAIRLINE, BW_HAIR)`.
   `kind` → tag: `You → (ACCENT, ACCENT)`, `Ai → (TEXT_MUTED, BORDER_HAIRLINE)`,
   `None → None` `[measured: Read CarChip.jsx:14-31 → bg active?paper-2:paper-0, border active?bw-2 graphite-900:bw-hair border-hairline, tag color you?accent:text-muted, tag border you?accent:border-hairline]`.
   The dot **color** is a prop (`Color32`, default `CAR_1`), passed straight to
   `paint` — not resolved. `CarKind` gets a `const fn label(self) -> &'static str`
   (`You → "YOU"`, `Ai → "AI"`). `Option`/struct literals are const-constructible,
   so `resolve` stays `const`.

7. **No non-token colors are needed.** Every `.jsx` color maps to an existing
   token (verified per-widget in § Style-mapping ground truth below); unlike
   `Badge` (which needed `BADGE_OK_FG`/`BADGE_WARN_FG`) `[measured: Read badge.rs:27-29]`,
   these three widgets require zero per-widget non-token colors. Several
   non-token **dimensions** become module-level `const`s per the magic-number
   rule (see § Non-token dimensions), mirroring `tag.rs`'s local
   `HEIGHT`/`PAD_X`/`DOT_DIAMETER` `[measured: Read tag.rs:10-23]`.

### Style-mapping ground truth (per `.jsx`)

**Telemetry** `[measured: Read Telemetry.jsx]`:
- column: `flex-direction:column`, `gap:3`, `align-items` = align.
- label: `font-mono`, `FS_XS`, uppercase, `TEXT_MUTED`, line-height 1.
- value: `font-mono`, `value_size`, `FW_BOLD` → **`JETBRAINS_MONO_BOLD`**,
  `tones[tone]`, baseline row with unit, `gap:4`.
- unit: `FS_SM`, `FW_REGULAR` → **`JETBRAINS_MONO_REGULAR`**, `TEXT_MUTED`.
- label has no explicit weight → CSS 400 → **`JETBRAINS_MONO_REGULAR`**.

**LapMeter** `[measured: Read LapMeter.jsx]`:
- column `gap:6`; header row `space-between`, `gap:12`, baseline.
- label: `font-mono`, `FS_XS`, uppercase, `TEXT_MUTED`; default `"LAP"`.
- readout: `font-mono`, `FS_TITLE`, `FW_BOLD` → `JETBRAINS_MONO_BOLD`; `done`
  in `TEXT_INK`, `/total` in `TEXT_FAINT`.
- cells row: `gap:3`; each cell `flex:1`, `height:8`, fill `i<done ? ACCENT : PAPER_3`,
  `border: bw-hair GRAPHITE_900`, `radius-0` (square).

**CarChip** `[measured: Read CarChip.jsx]`:
- inline row: `gap:10`, `height:34`, `padding:0 12px 0 8px`, radius `radius-1`,
  bg/border per `active`.
- rank (if `Some`): `font-mono`, `FS_TITLE`, `FW_BOLD` → `JETBRAINS_MONO_BOLD`,
  `TEXT_INK`, `min-width:18`, centered.
- dot: 16×16 circle, prop color, `border: 2px GRAPHITE_900` (`BW_2`).
- name: `font-ui`, `FS_BODY`, `FW_MEDIUM` → **`ONEST_MEDIUM`**, `TEXT_INK`.
- kind pill (if `Some`): `font-mono`, `FS_MICRO`, uppercase, `radius-pill`,
  `padding:1px 6px`, `bw-hair` border; colors per `KindTagStyle`.

**Font families** — all three required faces exist (among the 7 registered font
instances): `JETBRAINS_MONO_BOLD`, `JETBRAINS_MONO_REGULAR`, `ONEST_MEDIUM`
`[measured: Read fonts.rs:38-42 → JETBRAINS_MONO_REGULAR/_MEDIUM/_BOLD; :32 → ONEST_MEDIUM]`.
Note: the spec Fonts row ("value/label/rank via `JETBRAINS_MONO_BOLD`/`_MEDIUM`")
is a loose summary; the faithful per-run binding uses `_BOLD` for the weighted
runs (value / readout / rank, all `FW_BOLD`) and `_REGULAR` for the unweighted
runs (label / unit / kind-pill, CSS 400). Labels are **uppercased in `paint`**
via `to_uppercase()` to honor each `.jsx`'s `textTransform:uppercase` (the card
passes `label="v"` and renders "V") `[measured: Read game.card.html:32 → <Telemetry label="v" ...>]`
— the allocation lives in `paint`, never in the Miri-clean `resolve`.

### Non-token dimensions (module-level consts, magic-number rule)

Bound to a `spacing` token where one matches exactly; otherwise a local const:
- Telemetry: `LABEL_VALUE_GAP = 3.0` (no token; `SPACE_1=4`), value↔unit gap =
  `SPACE_1` (4).
- LapMeter: `ROW_GAP = 6.0` (no token), header gap = `SPACE_3` (12), `CELL_GAP = 3.0`
  (no token), cell height = `SPACE_2` (8, exact match)
  `[measured: Read spacing.rs → SPACE_1=4, SPACE_2=8, SPACE_3=12]`.
- CarChip: `HEIGHT = 34.0`, `GAP = 10.0`, `DOT_DIAMETER = 16.0`, `RANK_MIN_W = 18.0`,
  `TAG_PAD_X = 6.0`, `TAG_PAD_Y = 1.0` (none are tokens); pad-left = `SPACE_2` (8),
  pad-right = `SPACE_3` (12), dot border = `BW_2`, chip radius = `RADIUS_1`, tag
  radius = `RADIUS_PILL`, tag border = `BW_HAIR`
  `[measured: Read spacing.rs → BW_2=2.0, RADIUS_1=3.0, RADIUS_PILL=999.0, BW_HAIR=1.0]`.

### Rejected alternatives

- **`align` as a `bool` field** — rejected: AC9 mandates union → enum, and every
  #13/#14 union prop got its own enum.
- **`align` reusing `egui::Align` (Min/Center/Max)** — rejected: 3 variants for a
  2-value union is lossy and less faithful than a dedicated `Left/Right` enum.
- **on-ink `Default` → `TEXT_ON_INK`** — rejected: `TEXT_ON_INK`=`PAPER_1`=`#F5F1E6`
  does not match the card's `#FBF8F0`; `PAPER_0` does (Key decision 4).
- **`LapMeter` signed/unsigned mix (`lap: i32, total: u32`)** — rejected: forces
  an `as`/`try_from` inside `resolve`, breaking either const-ness or the pedantic
  cast denies. `i32/i32` is clean in both.
- **Adding lib.rs crate-root re-exports** — rejected: no #13/#14 widget is
  re-exported at `lib.rs`; `widgets/mod.rs` IS the widget crate-root
  `[measured: rg 'pub use|pub mod widgets' lib.rs → only `pub mod widgets;`]`.
  See § Open questions.

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | `Telemetry`: `Tone`/`Align` enums, `TelemetryStyle`, `const fn resolve(tone,size,on_ink)`, `paint`, `show` (label uppercased, align, stacked label/value+unit, on-ink mode), builder (`new(label,value)` + `.unit/.tone/.size/.align/.on_ink`), `#[cfg(test)]` unit tests (AC1/AC2/AC7/AC9); re-export `Telemetry`, `Tone as TelemetryTone`, `Align as TelemetryAlign` from `widgets/mod.rs`, each `///`-documented | `crates/render/src/widgets/telemetry.rs`, `crates/render/src/widgets/mod.rs` | — |
| 2 | `LapMeter`: `LapMeterStyle`, `const fn resolve(lap,total)` (clamp), `paint` (header row + equal-width cells), `show`, builder (`new(lap,total)` + `.label` default `"LAP"`), unit tests (AC3/AC4/AC7); re-export `LapMeter` from `widgets/mod.rs`, `///`-documented | `crates/render/src/widgets/lap_meter.rs`, `crates/render/src/widgets/mod.rs` | — |
| 3 | `CarChip`: `CarKind` enum (+`const fn label`), `KindTagStyle`/`CarChipStyle`, `const fn resolve(active,kind)`, `paint` (rank/dot/name/kind-pill row), `show`, builder (`new(name)` + `.color`/`.rank`/`.kind`/`.active`, default color `CAR_1`), unit tests (AC5/AC6/AC7); re-export `CarChip`, `CarKind` from `widgets/mod.rs`, `///`-documented | `crates/render/src/widgets/car_chip.rs`, `crates/render/src/widgets/mod.rs` | — |
| 4 | HUD specimen golden `game_gallery.rs` (mirror `gallery.rs`): draw the Telemetry HUD strip (SPEED/v/POS/TEMPO, on-ink) on a `GRAPHITE_900` `RADIUS_2` panel + `LapMeter(lap=2,total=5)` + a 3-row `CarChip` standings column, laid out to `game.card.html` (MovePad omitted); `#[cfg(test)] mod game_gallery;` in `mod.rs`; **mint** `game_gallery.png` (code-writer + `image-check`); then run the full AC10 gate sweep (`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test -p gp-render`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p gp-render`) | `crates/render/src/widgets/game_gallery.rs`, `crates/render/src/widgets/mod.rs`, `crates/render/tests/snapshots/game_gallery.png` | 1, 2, 3 |

`M = 4`. All four subtasks are Rust `*.rs` (subtask 4 also mints a test-asset
`.png`, produced by code — still the code change-type).

## Handoff plan

Grouping is required for every `M ≥ 1` **(a)**. All four subtasks are the **code**
change-type (`*.rs` + a code-minted `.png` test asset) — none touch
`*.md`/`.claude/**`/`AGENTS.md`/`ai-docs/**` — so they are homogeneous **(e)**
and cluster into the **fewest possible groups**: one **(f)**. Dependency order
(1,2,3 independent → 4 depends on 1–3) is preserved within the group. Four
subtasks is within the `≤ 10` size cap **(b)** and the terminal-group `1..=10`
range **(d)**; one group is under the default 4-group cap **(h)**.

- **Group A** — implementor model **`sonnet`** (sonnet-5), effort **`medium` (pinned)**,
  1M-token window, via the **`code-writer`** subagent — subtasks 1, 2, 3, 4
  (code change-type: `*.rs` + minted `.png`). **Terminal group** (4 subtasks;
  within `1..=10`). Per the marker→implementor routing **(g)**, this code group
  routes to `subagent_type="code-writer"` (its `model: sonnet` + `effort: medium`
  are frontmatter-pinned — no inline override). The `design`/`design-review`/
  `self-review` gates stay on Opus.
- **Entry into Group A (the first and only group):** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry) **(c)**.
  The single group completes /task Step 8 in its own `/context-reset` subagent;
  there is no inter-group handoff.

## Risks

- **`resolve` inadvertently non-`const` (AC2/AC7).** Telemetry/CarChip `resolve`
  bodies are `match` → token consts / `Option`/struct literals; LapMeter's is
  integer comparisons. All are const-eligible, so `missing_const_for_fn` (nursery,
  deny) **forces** `const` — a plain `fn` would fail the gate. Mitigation:
  declare all three `resolve` as `const fn` (Key decisions 4–6) —
  `[derived → cargo clippy --workspace --all-targets -- -D warnings]`.
- **f32-comparison `float_cmp` in unit tests (pedantic, deny).** Asserting
  `value_size == FS_H2`, `border_width == BW_2`, etc. with `assert_eq!` on `f32`
  fires `clippy::float_cmp`. Mitigation: use `crate::tokens::css::assert_f32(label, got, want)`
  for every `f32` assertion (Color32 stays `assert_eq!` — it is `Eq`), as
  `badge.rs`/`tag.rs` tests do
  `[measured: rg 'pub(crate) fn assert_f32' tokens/mod.rs → assert_f32(label,got,want) at :105; Read badge.rs:239-248 → uses assert_f32 for radius]`.
- **Integer→f32 casts in geometry (pedantic `cast_precision_loss`/`_truncation`,
  deny).** Cell count, rank, and cell-index → f32 for positioning. Mitigation:
  convert via `f32::from(u16::try_from(x).unwrap_or(u16::MAX))`, never a bare
  `as` — the in-crate gallery idiom `[measured: Read gallery.rs:35-37]`. Scalar
  `f32` arithmetic (`+ - * / +=`) is **not** flagged by `arithmetic_side_effects`
  (integer-only lint) — proven by committed `tag.rs` using it freely under the
  `-D warnings` gate `[measured: rg on tag.rs → `cursor_x += DOT_DIAMETER + GAP`, `rect.max.x - PAD_X - REMOVE_SIZE`, `/ 2.0` all committed & clippy-clean]`.
- **Golden red on a non-CPU wgpu adapter / Miri.** The `game_gallery` test must
  assert the resolved adapter is `DeviceType::Cpu` and carry
  `#[cfg_attr(miri, ignore = "drives wgpu; dlopens the Vulkan ICD (no FFI under Miri)")]`,
  or it reds the workspace Miri gate (which now blocks merge). Mitigation: copy
  `gallery.rs`'s harness verbatim (CPU-adapter assert, `SnapshotOptions::new().threshold(1.0).failed_pixel_count_threshold(0)`,
  frame-1-install-fonts/frame-2-draw) `[measured: Read gallery.rs:303-363]`.
- **Golden regen loop.** The golden PNG cannot be authored blind; it is minted by
  `code-writer` and verified by the `image-check` subagent against the drawing
  code before commit `[measured: Read gallery.rs:8 header → golden minted, not hand-drawn]`.
  Mitigation: subtask 4 explicitly mints + `image-check`s.
- **`mod.rs` three-way edit churn.** Subtasks 1–3 each append a `pub mod` + `pub use`
  to `widgets/mod.rs`, and subtask 4 appends the `#[cfg(test)] mod game_gallery;`.
  Sequential same-group execution (one `code-writer`, ordered 1→4) avoids
  concurrent edits — `[derived → code-writer Mode A executes subtasks sequentially per group]`.

## Test Design

**Subtask 1 — Telemetry** (`crates/render/src/widgets/telemetry.rs` `#[cfg(test)] mod tests`):
- Entry point: `Telemetry::resolve(tone, size, on_ink)`, plus builder-default checks.
- Scenarios (AC2):
  - tone→value color off-ink: `Default→TEXT_INK`, `Accent→ACCENT`, `Ok→OK`,
    `Warn→WARN`, `Danger→DANGER`, `Muted→TEXT_MUTED` (`assert_eq!`, Color32).
  - size→value size: `Sm→FS_TITLE`, `Md→FS_H3`, `Lg→FS_H2` (`assert_f32`).
  - on-ink overrides: `resolve(Default,_,true).value_color == PAPER_0`;
    `resolve(Muted,_,true).value_color == TEXT_FAINT`; `resolve(_,_,true).muted_color == TEXT_FAINT`;
    off-ink `muted_color == TEXT_MUTED`.
  - on-ink leaves semantic tones: `resolve(Accent/Ok/Warn/Danger,_,true).value_color`
    == `ACCENT/OK/WARN/DANGER` (unchanged).
  - builder defaults: `new(label,value)` → `tone==Default`, `size==Md`,
    `align==Left`, `unit==None`, `on_ink==false`.
- Fixtures: none beyond `crate::tokens::{color, typography}` and
  `crate::tokens::css::assert_f32`.

**Subtask 2 — LapMeter** (`lap_meter.rs` `#[cfg(test)] mod tests`):
- Entry point: `LapMeter::resolve(lap, total)`.
- Scenarios (AC4): `resolve(-3, 5).done == 0` (`lap ≤ 0`); `resolve(9, 5).done == 5`
  (`lap ≥ total`); `resolve(2, 5).done == 2` (intermediate); `resolve(_, -1).total == 0`
  (negative-total clamp); "cell `i` filled iff `i < done`" — for `resolve(2,5)`
  assert the boolean series `[true,true,false,false,false]` via `i < style.done`.
  Builder default: `new(l,t).label == "LAP"`; `.label("TOUR")` overrides.
- Fixtures: none.

**Subtask 3 — CarChip** (`car_chip.rs` `#[cfg(test)] mod tests`):
- Entry point: `CarChip::resolve(active, kind)`, `CarKind::label`, builder defaults.
- Scenarios (AC6):
  - ramp identity: `CAR_COLORS[0] == ACCENT`, `CarChip::new("You").color == CAR_1`,
    `CAR_1 == ACCENT` `[measured: Read color.rs:70,154,312 → CAR_1=(E2,4A,2B); CAR_COLORS[0]==ACCENT tested]`.
  - active vs resting: `resolve(true,None) → bg PAPER_2, border GRAPHITE_900, border_width BW_2`
    (via `assert_f32` for width); `resolve(false,None) → bg PAPER_0, border BORDER_HAIRLINE, border_width BW_HAIR`.
  - kind→tag: `resolve(_,Some(You)).tag == Some(KindTagStyle{fg:ACCENT,border:ACCENT})`;
    `resolve(_,Some(Ai)).tag == Some(KindTagStyle{fg:TEXT_MUTED,border:BORDER_HAIRLINE})`;
    `resolve(_,None).tag == None`.
  - `CarKind::You.label() == "YOU"`, `CarKind::Ai.label() == "AI"`.
  - builder defaults: `new(name)` → `color==CAR_1`, `rank==None`, `kind==None`, `active==false`.
- Fixtures: none. (`KindTagStyle`/`CarChipStyle` derive `PartialEq` + `Debug` so
  `assert_eq!` on the `Option<KindTagStyle>` works; `Color32`/`f32` fields are
  fine — no `assert_matches!` needed, so no `Debug`-supertrait concern.)

**Subtask 4 — HUD golden** (`game_gallery.rs` `#[cfg(test)] mod tests`):
- Entry point: `game_gallery_matches_golden` — one wgpu CPU frame vs the minted
  `game_gallery.png`.
- Scenario (AC8): draw `GRAPHITE_900` panel; SPEED = `resolve(Accent,Lg,true)`,
  v = `resolve(Default,Md,true)`, POS = `resolve(Default,Md,true)`,
  TEMPO = `resolve(Muted,Md,true)` with unit `"c/t"`; `LapMeter::new(2,5)`;
  three `CarChip` rows (`CAR_1`/"You"/You/rank 1/active, `CAR_2`/"Rival Blue"/Ai/rank 2,
  `CAR_3`/"Rival Green"/Ai/rank 3) `[measured: Read game.card.html:26-54]`.
- Harness/fixtures: exact copy of `gallery.rs`'s test — CPU-adapter assert,
  frame-1-install-fonts/frame-2-draw, `threshold(1.0)+failed_pixel_count_threshold(0)`,
  `#[cfg_attr(miri, ignore = "…")]`. PNG minted at `crates/render/tests/snapshots/game_gallery.png`
  `[measured: ls crates/render/tests/snapshots/ → widget_gallery.png, forms_gallery.png present; egui_kittest default snapshot dir = <manifest>/tests/snapshots]`.
- **`image-check` target — verify the UNIFORM on-ink version, not the card's
  literal inconsistency.** The self-minted golden applies `on_ink` **uniformly**:
  every `Default`-tone value → `PAPER_0`, and **every** label/unit/muted value →
  `TEXT_FAINT`. This intentionally differs from `game.card.html`, which applies
  the two `style` overrides inconsistently and so keeps the **v** and **POS**
  labels at the dark default `--text-muted` (no `--text-muted` override on those
  two widgets) `[measured: Read game.card.html:32-33 → v/POS carry only --text-ink, not --text-muted]`.
  When `image-check` derives the expected frame from the drawing code, it MUST
  compare against the design's coherent uniform version (v/POS labels →
  `TEXT_FAINT`), **NOT** the card's literal per-widget inconsistency. The spec
  sanctions this because the golden is self-minted (Key decision 4).

## Open questions

- **SETTLED (design-review GO, note 4) — AC9 vs AC10 export list (Telemetry
  `Align`).** AC9 requires union → enum; `align='left'|'right'` therefore becomes
  a public `Align` enum, **exported as `TelemetryAlign` from `widgets/mod.rs`
  alongside `Telemetry` / `Tone` (as `TelemetryTone`) / `LapMeter` / `CarChip` /
  `CarKind`**. AC10's parenthetical export list omits it but is **illustrative,
  not exhaustive**; the design's resolution (add the documented `TelemetryAlign`
  export) is AC-compliant. No further owner confirmation required.
- **SETTLED (design-review GO, note 5) — "crate root" export scope (AC10) =
  `widgets/mod.rs`, not `lib.rs`.** No #13/#14 widget is re-exported at `lib.rs`;
  the established crate-root for widgets is `widgets/mod.rs`
  (`gp_render::widgets::Badge`) `[measured: rg lib.rs → only `pub mod widgets;`]`.
  The design re-exports the new widgets from `widgets/mod.rs`, matching precedent.
  A literal `lib.rs` re-export would be a cross-widget consistency change (out of
  scope) and is **not** what AC10 means. Verified and resolved.
- **SETTLED (design-review GO, note 3) — HUD-strip membership: SPEED/v/POS/TEMPO
  (4), not "…/LAP/…" (5).** Binding AC8 and `game.card.html` both show a
  **4-widget** HUD strip (SPEED/v/POS/TEMPO); "LAP" is the separate `LapMeter`,
  **not** a Telemetry strip cell `[measured: Read game.card.html:28-45 → HUD strip = SPEED,v,POS,TEMPO; LapMeter rendered separately]`.
  The spec Scope prose's 5-item "(SPEED, v, POS, LAP, TEMPO)" list is imprecise;
  the design correctly follows AC8 + the card (4 Telemetries). Reconciliation
  resolved — no 5th "LAP" Telemetry.

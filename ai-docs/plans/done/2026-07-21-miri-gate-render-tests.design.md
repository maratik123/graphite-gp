# Design: Gate egui-Context/tessellation-driving gp-render unit tests under Miri

**Issue:** #106
**Date:** 2026-07-21

## Approach

**Problem.** Under the workspace Miri job
(`MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace`), every
`gp-render` unit test that stands up an `egui::Context` and drives a `run_ui`
paint pass is executed by the Miri interpreter. Those passes are pure
wall-clock cost (interpreted egui layout/paint), and #101's overlay/track tests
added a batch of them, materially increasing the Miri job's wall-clock. Gating
them measurably reduces it; the residual non-Context Miri cost is out of scope
and tracked in #107 (per amended AC4 — the fix's success is the *measured*
reduction, not a fixed wall-clock target). Native `cargo test` is unaffected.
[measured: `MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace` → BEFORE 24m48s (185 passed / 13 ignored) → AFTER 17m02s (170 passed / 28 ignored), Δ +15 newly-`ignored` = exact gate set]

**Chosen solution — per-test `#[cfg_attr(miri, ignore = "…")]` on every
un-gated Context/painter-driving test, plus a brief module `//!` note**,
exactly mirroring the crate's existing wgpu-golden gate convention
(`track/golden.rs`, the `widgets::*_gallery` goldens, `placeholder.rs`,
`icons.rs::icon_set_bakes_all_five`). This is the mechanism the spec Key
Decisions row fixes ("Per-item `#[cfg_attr(miri, ignore = "…")]`, mirroring the
existing golden gates (module `//!` note + per-test attribute)"). Native
`cargo test` still runs all of them (the `miri` cfg is only set under the
interpreter), so AC3's coverage is preserved.

**The gate is per-`#[test]`, never on the shared helper.** The four capture
helpers (`track::mod::render_shapes`, `track::grid::painted_shape_count`,
`track::fastest_lap::painted_shape_count`, `track::heatmap::painted_meshes`) are
plain `fn`s, not `#[test]`s — `#[cfg_attr(miri, ignore)]` only suppresses a
`#[test]`. So each **calling** `#[test]` carries its own attribute. This is the
convention (AGENTS.md § Rust Test Conventions: "Per-test, **never** a
crate-level `--exclude`").

**Reason strings — same-helper tests share one honest cause; the "don't copy a
sibling" rule is about *different* causes.** AGENTS.md says: "Write the reason
for **that test's own** abort; do not copy a sibling's — a wrong reason is a
false justification for **a different failure**." The hazard it names is the
`golden_guard` (FFI `dlopen`) vs `tessellation_smoke` (`vello_cpu` checked-cast
panic) case: two *unrelated* causes. The tests gated here are **cost, not
abort** (see Risks — none call `tessellate` or rasterise glyphs), and tests that
funnel through the *same* helper genuinely share *one* cause, so a reason shared
within a helper-group is **accurate, not a copy-violation**. Reasons differ
*across* helper-groups (each names its own module's paint entry point). The
design mandates one reason per helper-group; the implementer confirms
cost-vs-abort against the before-measurement Miri run (AC4) and, if any single
test is observed to *abort* rather than merely slow, names that abort for that
test per the convention.

**`icons.rs::draw_icon_emits_tinted_textured_mesh` is gated too — decided by
AC1+AC6, not by measurement.** Its current doc comment calls it "Miri-clean …
(no tessellation)" and it is deliberately left un-gated. But AC1 is
unconditional ("**Every** … test that constructs an `egui::Context`/painter
carries the gate; **verified by a grep audit**") and AC6/B.1 require a
**mechanically-checkable trigger** ("constructs an `egui::Context`/painter").
A grep audit cannot distinguish "cheap & clean" from "slow" — that is the point
of a mechanical trigger — so `draw_icon` must carry the gate or the audit and
the codified rule are self-contradictory. Its doc comment is reconciled: it is
Miri-clean (no abort — texture-only, bypasses resvg/tiny-skia and tessellation),
but gated under the uniform Context-test convention for wall-clock + audit
uniformity. The two `svg_to_color_image_*` tests that carry "Not Miri-gated:
verified via …" markers are **not** counterexamples: they call
`svg_to_color_image` directly and construct **no** `Context`, so they are
pure-logic and out of AC1's scope. [measured: `rg -Un 'Context' crates/render/src/icons.rs` → the only `Context::default()` sites are lines 351 (already gated) and 376 (`draw_icon`)]

**Rejected alternatives.**
- *Gate the shared helpers instead of the tests* — impossible: `cfg_attr(miri,
  ignore)` is a `#[test]`-only attribute; a plain `fn` has nothing to ignore.
- *Crate-level `--exclude gp-render` from the Miri job* — this is the spec's
  rejected **B.3**; it would drop UB coverage on the pure-logic render tests
  that (A)+(B.1) preserve, and it touches `ci.yml` (AC7 forbids). Out of scope.
- *Exempt `draw_icon` because it is Miri-clean* — makes the trigger
  non-mechanical ("constructs a Context **and** is slow"), which a grep audit
  (AC1's verification) cannot check; violates AC6. Rejected.

**Files audited, nothing to gate (recorded so the crate-wide claim is
auditable):**
- `screens/setup.rs` — its `#[cfg(test)]` tests are the pure `assemble_*`
  clamp/round tests; they build **no** `Context`. The #105 setup *capture*
  golden lives in `screens/setup_gallery.rs` and is **already gated** (lines
  43, 120). [measured: `rg -Un 'Context::default|Harness::builder' crates/render/src/screens/setup.rs` → no match (the broader `…|Harness` pattern hits only setup.rs:86, a `Harness::build_ui` doc comment — not a test)]
- `widgets/card.rs` — its tests exercise `CardStyle::resolve`/`header_height`
  style logic; **no** `Context`/`Painter`. [measured: `rg -Un 'Context|Painter::|Harness|tessellate' crates/render/src/widgets/card.rs` → no match]
- `track/{car,walls,sf,transform}.rs` — define production `paint(painter:
  &Painter, …)` fns but their tests build no `Context` (no `run_ui`/
  `layer_painter` anywhere in them). [measured: `rg -Un 'Context|run_ui|layer_painter' crates/render/src/track/{car,walls,sf,transform}.rs` → only production `Painter` param types, no Context construction]
- `placeholder.rs` — `tessellation_smoke` + `golden_guard` **already gated**
  (lines 299, 384). [measured: `rg -Un --multiline 'cfg_attr\(\s*miri' crates/render/src/placeholder.rs` → lines 299, 384]

## Decomposition

Complete set of tests to gate: **15 tests across 6 files** (all currently
un-gated; each transitively constructs an `egui::Context`).
[measured: `rg -Un 'egui::Context::default' crates/render/src` cross-referenced with `rg -Un --multiline 'cfg_attr\(\s*miri'`]

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Measure Miri BEFORE.** On the branch tip *before* any gate edit, run `MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace` and record wall-clock (the post-#101 BEFORE baseline; ~24m48s this session). Records to `.progress.md` / PR body. | — (measurement only) | — |
| 2 | Gate `track/mod.rs` — add `#[cfg_attr(miri, ignore = "…")]` to the **7** `render_shapes`-driven tests (`render_frame_draws_without_panicking`, `each_overlay_changes_output_when_on`, `all_overlay_combinations_render_without_panic`, `all_off_equals_metrics_independent_baseline`, `heatmap_is_noop_on_empty_metrics`, `fastest_lap_is_noop_on_empty_metrics`, `fastest_lap_paint_does_not_mutate`); brief `//!` Miri note; leave `layer_order_is_documented` **un-gated**. | `crates/render/src/track/mod.rs` | 1 |
| 3 | Gate `track/grid.rs` — the **2** `painted_shape_count`-driven tests (`paint_emits_ruling_and_dots`, `paint_is_noop_on_degenerate_transform`); `//!` note; leave the 5 `line_coords_*` tests un-gated. | `crates/render/src/track/grid.rs` | 1 |
| 4 | Gate `track/fastest_lap.rs` — the **2** `painted_shape_count`-driven tests (`paint_is_noop_on_empty_path`, `paint_draws_populated_path`); `//!` note; leave the 3 `catmull_rom_*` tests un-gated. | `crates/render/src/track/fastest_lap.rs` | 1 |
| 5 | Gate `track/heatmap.rs` — the **2** `painted_meshes`-driven tests (`paint_emits_per_cell_meshes_plus_infield_recut`, `paint_is_noop_on_empty_heatmap`); `//!` note; leave the `speed_bounds_*`/`normalize_*`/`ramp_color_*` tests un-gated. | `crates/render/src/track/heatmap.rs` | 1 |
| 6 | Gate `track/regions.rs` — the **1** inline-Context test `fill_emits_asphalt_mesh_then_infield_mesh`; `//!` note; leave the `classify_loops_*`/`triangulate_*`/`asphalt_equals_corridor_contains`/`infield_hole_and_outfield_are_disjoint`/… tests un-gated. | `crates/render/src/track/regions.rs` | 1 |
| 7 | Gate `icons.rs` — the **1** inline-Context test `draw_icon_emits_tinted_textured_mesh`; reconcile its doc comment (Miri-clean/no-abort but gated for audit uniformity); leave the pure `svg_to_color_image_*` tests un-gated (they build no Context). `icon_set_bakes_all_five` is already gated — do not touch. | `crates/render/src/icons.rs` | 1 |
| 8 | **Measure Miri AFTER + green gates.** Re-run the exact Miri workspace command; record after-value and confirm the amended AC4: (a) AFTER wall-clock materially < BEFORE, and (b) Δ newly-`ignored` (AFTER `ignored` − BEFORE `ignored`) = the gate set (15). The residual non-Context Miri cost is tracked in #107, **not** a success gate here. Run `cargo test -p gp-render` + `cargo test --workspace` (AC3 count unchanged, green), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` (AC5). Record all outputs. [measured this session: BEFORE 24m48s (13 ignored) → AFTER 17m02s (28 ignored), Δ +15 = gate set] | — (measurement/verification) | 2–7 |
| 9 | **AGENTS.md convention rule (B.1).** Add to § Rust Test Conventions, alongside the existing golden Miri-gate bullet, a rule naming the mechanical trigger: any `gp-render` unit test constructing an `egui::Context`/painter carries `#[cfg_attr(miri, ignore = "…")]`, with a per-helper-group cost/abort reason (same-cause siblings may share a reason; the "don't copy a sibling" rule is about *different* causes). | `AGENTS.md` | 8 |
| 10 | **Propagation Rule sweep.** Run `grep -rn` for the changed keywords (`cfg_attr(miri`, `egui::Context`, Miri-gate) across `.claude/agents/`, `.claude/skills/`, `.claude/rules/`, `AGENTS.md`, `ai-docs/`; apply the corresponding change to every enforcement sibling. Record the sweep command + result even if no sibling edit is required (a negative result is only valid when produced by the command). | `.claude/**`, `ai-docs/**` as the sweep dictates | 9 |

## Handoff plan

Grouping is required for every `M ≥ 1` (here `M = 10`). Two homogeneous groups,
minimized (code and instructions cannot share a group — change-type
homogeneity), 2 ≤ 4 max-groups. Handoff destination for **every** group
(including entry into the first) is `/context-reset` per
`.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).

- **Entry into Group A:** spawn `/context-reset` (re-entry) before starting.
- **Group A** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the
  `code-writer` subagent, 1M-token window — subtasks **1–8** (code change-type:
  `*.rs`, plus measurement/gate commands). 8 subtasks (≤ 10). Measurement
  subtasks 1 and 8 bracket the six gate edits: subtask 1 must run on the
  un-gated tree, subtask 8 after all gates land.
- **Handoff after Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
  Parent `/task` resumes in Group B with fresh context.
- **Group B** — model `opus`, effort **inherited from the orchestrator
  (typically xHigh) — NOT pinned**, via the `general-purpose` subagent,
  1M-token window — subtasks **9–10** (instructions/harness change-type:
  `AGENTS.md`, `.claude/**`, `ai-docs/**`). Terminal group (2 subtasks; within
  `1..=10`).

Routing note: the `design`, `design-review`, and `self-review` subagents stay
on Opus regardless of the per-group marker — only the per-group *implementor*
model/effort varies.

## Risks

- **The gated tests are cost, not abort — so the reason strings must say
  *cost*, not fabricate an abort.** All four helpers call `ctx.run_ui(…)` +
  `layer_painter` and capture `output.shapes` (the *tessellation-independent*
  shape list); none call `ctx.tessellate` and none `set_fonts`/rasterise glyphs,
  so none reach the `vello_cpu` checked-cast abort that `placeholder.rs`'s
  `tessellation_smoke` hits. — `[measured: rg -Un 'tessellate|set_fonts' crates/render/src/track/{mod,grid,fastest_lap,heatmap,regions}.rs crates/render/src/icons.rs → single hit at track/mod.rs:171, a doc comment ("the track canvas draws no text, so no set_fonts install is needed") — NOT a call; no tessellate/set_fonts call in any of the 6 gated files; only run_ui/layer_painter present]`
- **"They run (slowly), not abort" is the premise for the cost-wording.** If a
  #101 track Context test aborted under the required Miri gate, #101 could not
  have merged (a red Miri blocks merge, #76). It merged → they run. —
  `[derived → subtask 1's before-measurement Miri run discharges cost-vs-abort per test; any test observed to abort gets an abort-naming reason per AGENTS.md]`
- **`draw_icon` gating loses a sliver of genuine Miri UB coverage** (the
  `load_texture`/`run_ui` path currently runs Miri-clean). Accepted: it mirrors
  the crate's already-documented placeholder coverage-loss note ("Under Miri,
  gp-render then exercises only the `fonts.rs` + `tokens` tests" — an accepted
  coverage loss, not an oversight), and native `cargo test` coverage is
  unchanged (AC3). — `[measured: crates/render/src/placeholder.rs — the "accepted coverage loss" phrase is at lines 382-383, within the sentence spanning 381-383 ("Under Miri, gp-render then exercises only the fonts.rs + tokens tests … an accepted coverage loss (design § Risks), not an oversight"); lines 378-380 are the separate tessellation_smoke-abort sentence, not the coverage-loss posture]`
- **AGENTS.md size cap.** Adding the B.1 rule must keep AGENTS.md under the
  35 000-char early-warning. Current size 33 010; a ~500–800-char rule lands
  ~33.5–33.8k, under 35k. — `[measured: wc -c AGENTS.md → 33010]`
- **Propagation sweep may find no enforcement sibling.** `self-review.md` /
  `review-findings.md` / `project-review` currently contain **no** Miri mention,
  so the render-Context Miri-gate convention has no existing enforcement sibling
  to sync; the sweep is still mandatory and its (likely negative) result must be
  command-produced, not assumed. — `[measured: rg -Un -i 'miri' .claude/agents/self-review.md .claude/agents/review-findings.md .claude/skills/project-review/SKILL.md → no output]`
- **AC7 (no `ci.yml` change).** No subtask touches `.github/workflows/**`; B.2
  budget guard stays deferred. — `[derived → subtask 10's grep sweep and the final `git diff --name-only` will show no `.github/workflows/**` path]`

## Test Design

This task **gates existing tests**; it writes no new test logic. The
"test design" is the audit that verifies the gate set and that pure-logic
coverage is preserved.

- **AC1 grep audit (the acceptance check itself).** After the edits, every
  `egui::Context::default()` / `Context::run_ui` construction site inside a
  `#[cfg(test)]` module must sit under a `#[test]` carrying
  `#[cfg_attr(miri, ignore)]`. Audit command:
  `rg -Un 'egui::Context::default' crates/render/src` (enumerate the sites) then
  confirm each site's enclosing `#[test]` is gated via
  `rg -Un --multiline 'cfg_attr\(\s*miri' crates/render/src`. Expected gated
  count after this task: the 15 new + the pre-existing golden/icon/placeholder
  gates.
- **AC2 (pure-logic stays under Miri).** Confirm the enumerated pure tests are
  **not** ignored: `grid::line_coords_*`, `fastest_lap::catmull_rom_*`,
  `heatmap::{speed_bounds_*,normalize_*,ramp_color_*}`,
  `regions::{classify_loops_*,triangulate_*,asphalt_equals_corridor_contains,
  infield_hole_and_outfield_are_disjoint}`, `mod::layer_order_is_documented`,
  `icons::svg_to_color_image_*`. Verify they run under Miri in subtask 8's run
  (they appear in the Miri `test result` list, not the `ignored` list).
- **AC3 (native coverage unchanged).** `cargo test -p gp-render` and
  `cargo test --workspace` report the same test count as before (the `ignore`
  is `miri`-cfg-gated, inert under native `cargo test`); full suite green.
- **AC4 (measured reduction).** Before (subtask 1) and after (subtask 8) Miri
  wall-clock recorded in the PR. Success is the *measured* outcome, per the
  amended AC4: (a) AFTER materially < BEFORE, and (b) Δ newly-`ignored` (AFTER
  `ignored` − BEFORE `ignored`) = the gate set (15). No fixed wall-clock target —
  the ~5-min figure is retired (refuted attribution: the residual is non-Context
  cost — gp-core integer-physics tests that must stay under Miri, plus the Miri
  interpret/compile baseline). Residual reduction is tracked in #107, not a gate
  here. [measured: BEFORE 24m48s (185 passed / 13 ignored) → AFTER 17m02s
  (170 passed / 28 ignored), Δ +15 newly-`ignored` = exact gate set]
- **AC5.** `cargo clippy --workspace --all-targets -- -D warnings` +
  `cargo fmt --check` clean.
- **AC6.** AGENTS.md § Rust Test Conventions states the rule with the mechanical
  trigger; Propagation sweep run + result recorded.

Reason-string plan (one per helper-group; honest cost cause; cross-group
distinct):

| Helper-group | Tests | Reason theme (cost, not abort) |
|---|---|---|
| `track::mod::render_shapes` | 7 (mod.rs) | fontless `Context` `run_ui` full-frame pass over the `TrackArtifact` fixture; captures the tessellation-independent `Shape` list — interpreted-pass wall-clock |
| `track::grid::painted_shape_count` | 2 (grid.rs) | `Context` `run_ui` + `layer_painter` driving `grid::paint`, counting emitted ruling/dot shapes |
| `track::fastest_lap::painted_shape_count` | 2 (fastest_lap.rs) | `Context` `run_ui` driving `fastest_lap::paint` over the Catmull-Rom path |
| `track::heatmap::painted_meshes` | 2 (heatmap.rs) | `Context` `run_ui` driving `heatmap::paint`, capturing per-cell meshes |
| `regions` inline | 1 (regions.rs) | `Context` `run_ui` driving `regions::fill`, capturing asphalt/infield meshes |
| `icons` inline | 1 (icons.rs) | `Context` + `load_texture` + `run_ui` driving `draw_icon`; **no abort** (texture-only, no resvg/tessellation) — gated for wall-clock + AC1 audit uniformity |

## Open questions

- None design-blocking. The spec's open question ("whether `icons.rs` /
  `placeholder.rs` / `widgets/card.rs` Context tests must be gated") is resolved
  by this audit: `placeholder.rs` is already gated; `widgets/card.rs` has no
  Context test; `icons.rs::draw_icon` is gated by AC1+AC6 (mechanical trigger),
  independent of the wall-clock measurement. The measurement (AC4) confirms the
  *measured reduction* (AFTER materially < BEFORE, Δ newly-`ignored` = gate set;
  no fixed target — residual tracked in #107) but is not the gate-set selector —
  AC1's "every" is.

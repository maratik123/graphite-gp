# Design: gp-render — choose + scaffold the native Rust GUI backend

**Issue:** #11
**Date:** 2026-07-16
**Round:** 4 (post *Exact-compare amendment* + *Addendum* (AC14/AC15 `image-check` subagent) + *Font-proof amendment*)

## Approach

The backend pick (**eframe/egui 0.35**), the `gp-game`-owns-the-loop override, the crate/dep split, the "golden ships now, text-free" call, the **font drop**, the **exact bit-to-bit compare**, and the **file-based `image-check` subagent** are all settled by the spec and are not reopened here. This design resolves the open mechanisms: the placeholder drawing path, AC9's pixel guard, AC10(b)'s adapter wiring, AC14's agent contract + invocation, AC15's propagation, and AC12's `ci.yml` step — plus the AC5 signature sanity-check.

Everything marked *(verified)* was checked against real 0.35.0 sources, the live repo, or by reproduction, per AGENTS.md § *Dependency Versions* AXIOM.

### Round-4 changes

| # | Change | Driver |
|---|---|---|
| 1 | **BLOCKER fixed — AC16.** `code-writer.md`'s `## Invariants` forbade the very `image-check` spawn round 3 routed through it. New **subtask 2** lands AC16 (a) the mode-aware golden-spawn rule + (b) the narrow artifact-validity carve-out, **together**; subtask 9 now depends on it. Round 3 verified **tool capability** where **instruction permission** was required. | design-review round 2 (`major`) + spec AC16 |
| 2 | **`code-writer.md` added to Group A** (instructions) — group stays homogeneous (`.claude/agents/**` + `ai-docs/**`), size 3 → 4. `M` 12 → **13**. | AC16 |
| 3 | **AC15(a) now names TWO hierarchy rows** — the new `image-check` row **and** the existing `code-writer` row's appended obligation. | spec AC15 |
| 4 | Summary-table "Regressions" cell qualified — *"in flat regions; AA-classified edge pixels are exempt"*. The one cell that laundered the bound finding 13 forbids laundering. | design-review (`minor`) |
| 5 | AC-coverage: `AC8 → 9 + 5` (its `.gitignore` clause is the wiring subtask); **AC16 row added** (16 rows). **Plus a full prose cross-reference sweep** — inserting subtask 2 shifted every subtask ≥2 by +1; the tables were renumbered but ~13 prose pointers were not. Navigational only: every authoritative cell carries its own instruction, so no work was lost. | design-review (`note` + `minor`) |
| 8 | `### Key decision — AC15 propagation` rationale now names **both** hierarchy rows — it described only the `image-check` row, i.e. the exact miss AC15 warns about, inside the design's own rationale. | design-review (`note`) |
| 9 | AC8's `Harness::new_ui(..)` → `builder()…build_ui(..)` substitution documented beside the existing `snapshot_options` one. | design-review (`note`) |
| 10 | Subtask 11: implementor runs `cargo build -p gp-game`, **not** `cargo run` (hangs a subagent); AC3's window check is human-only + must run from a local desktop session, not SSH X11. | design-review (recommendation) |
| 6 | *(verified)* inventory row corrected — **6** agents are `model: opus`, **2** set no `model` key; **none** pins `effort` but `code-writer`. | design-review (`note`) |
| 7 | Hot-reload risk **assessed and closed** with documentation + an empirical loud-failure check; the mid-session concern is **not** a hazard. | Coordinator question |

### Round-3 changes

| # | Change | Driver |
|---|---|---|
| 1 | **Eyes-on checkpoint REMOVED** — round 2's "surface the minted PNG for human review" is gone, in no form. | Product-owner reversal (*"golden visual inspection is not driveable via llm"*) |
| 2 | **`image-check` subagent** (AC14) + **inventory propagation** (AC15) designed in as two new Group A subtasks — the *replacement* for that step. | Spec *Addendum*, AC14/AC15 |
| 3 | **AC10(a) reversed → exact bit-to-bit**: `threshold(0.0)` + `failed_pixel_count_threshold(0)`. Round 2's `0.6`/`0` tolerance plumbing is gone. | Spec *Exact-compare amendment* |
| 4 | New **finding 13** — dify's identical-pixel short-circuit (proves `0.0` is safe) **and** its non-disableable AA exemption (qualifies "bit-to-bit"). | Round-3 verification |
| 5 | `PREDICTABLE`-by-default **and** finding 4's `.renderer(..)` bypass stated **together**, so no reader concludes the default saves them. | Coordinator directive |
| 6 | mesa/LLVM drift recorded as **accepted** (*"if it bites, revisit — do not pre-emptively loosen"*); **tolerance and finding 13's AA exemption deferred *together* on the one dx12/metal trigger** — accepted now, revisited when a second OS lane lands. | Spec *Exact-compare amendment* + product-owner call on finding 13 |
| 7 | **Group order inverted — instructions FIRST, code second.** Forced by a hard dependency the addendum's plan did not anticipate; see *Handoff plan*. Still 2 groups, still homogeneous. | Round-3 dependency analysis |

`M` 10 → **12**. AC-coverage table is now **15 rows**. Round 2's accepted work is unchanged: finding 10's brace reproduction, the mandatory prose-pointer fold-in, the single `CANVAS_RECT`, the risk downgrade, the `HarnessBuilder::wgpu()` footgun, and the font removal.

### Live re-verification (2026-07-16, PROC-1)

| Claim | Verified value | How |
|---|---|---|
| `egui` / `eframe` / `egui_kittest` / `egui-wgpu` max stable | **0.35.0** (all four) | `crates.io/api/v1/crates/<c>` |
| `RenderState.adapter` is a **public** `wgpu::Adapter` field | confirmed → `.get_info().device_type` reachable | `egui-wgpu-0.35.0/src/lib.rs` |
| `create_render_state(setup, options: egui_wgpu::RendererOptions)` | confirmed — second param is `RendererOptions` | `egui_kittest-0.35.0/src/wgpu.rs` |
| `UPDATE_SNAPSHOTS` is the only env var; `.diff/.new/.old.png` | confirmed (`Mode::from_env`; no skip var) | `egui_kittest-0.35.0/src/snapshot.rs` |
| `wgpu::DeviceType` derives `Debug, Eq, PartialEq` | confirmed → `assert_eq!` legal | `wgpu-types-29.0.0/src/adapter.rs` |
| `actions/upload-artifact` latest major | **v7** (v7.0.1); `if-no-files-found: ignore` valid | releases API + `v7/action.yml` |
| `actionlint` present locally | v1.7.12 | `actionlint --version` |
| **`threshold(0.0)` is safe — dify short-circuits identical pixels** | **confirmed — finding 13** | `dify-0.8.0/src/diff.rs:54-88` |
| **dify exempts AA-classified pixels; egui_kittest hardcodes it on** | **confirmed — finding 13** | `dify-0.8.0/src/{diff.rs,lib.rs}`, `snapshot.rs:475` |
| **`code-writer.md` is the only agent pinning `effort`**; of the other 8, **6 are `model: opus` and 2 (`review-findings`, `self-review`) set no `model` key at all** (they inherit); **none pins `effort`** | confirmed — house pattern for `image-check`. *(Round 3 stated "other 8 are `model: opus`, no effort" — wrong on the model half, corrected here. The load-bearing half — `code-writer` is the sole effort-pinned precedent — was and is true.)* | `.claude/agents/*.md` |
| **`claude-tools-hierarchy.md` has a `## Subagents` table with a `code-writer` row to mirror** | confirmed (3,415 chars — far under the 40k AXIOM) | `ai-docs/claude-tools-hierarchy.md` |
| **git fnmatch has no brace expansion** | **reproduced — finding 10** | `git check-ignore` on a scratch repo |
| **`@actions/glob` sets `nobrace: true`** | **reproduced — finding 11** | v7 shipped `dist/upload/index.js` |
| **winit's Linux backends are dlopen-based** | **confirmed — finding 12** | `winit-0.30.13/Cargo.toml` |

### Findings that change the design (would have been wrong from memory)

Findings 1–8 were independently re-derived and confirmed by design-review in round 2 and stand unchanged. Finding 13 is new in round 3.

1. **`eframe 0.35`'s `App` trait has no `update()`.** It is `fn logic(&mut self, ctx: &egui::Context, frame: &mut Frame)` (default impl, painting forbidden) + **`fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame)` (required)**. The classic `update(..)` + `CentralPanel::default().show(ctx, ..)` idiom **does not compile on 0.35**. *(verified `eframe-0.35.0/src/epi.rs:152-175`)*
2. **`eframe::egui` is re-exported** (`pub use {egui, egui::emath, egui::epaint};`) → `gp-game` needs **no direct `egui` dep**, exactly as the spec's split requires. *(verified `eframe-0.35.0/src/lib.rs:156`)*
3. **`run_native` is `#[cfg(any(feature = "glow", feature = "wgpu_no_default_features"))]`**, and eframe's default set names neither — but `wgpu = ["wgpu_no_default_features", "egui-wgpu/default"]`, so **defaults do satisfy the gate**. *(verified `eframe-0.35.0/Cargo.toml`)*
4. **`RendererOptions::PREDICTABLE ≠ RendererOptions::default()`, AND the builder's default does not save us.** Two facts that must be read **together**:
   - `PREDICTABLE = { msaa_samples: 1, depth_stencil_format: None, dithering: false, predictable_texture_filtering: true }`; plain `default() = { msaa_samples: 0, dithering: true, predictable_texture_filtering: false }`. Upstream on `predictable_texture_filtering`: *"useful when you want predictable rendering across different hardware, e.g. for kittest snapshots."*
   - **`HarnessBuilder`'s `render_options` field already defaults to `PREDICTABLE`** — but **`HarnessBuilder::renderer(..)` bypasses that field entirely**: `from_builder` literally destructures it as `render_options: _`. AC10(b)'s checked-render-state flow **must** use `.renderer(..)`, so the builder default is **inert on our path** and `PREDICTABLE` must be passed explicitly to `create_render_state` or it is silently lost — taking `dithering: true` with it, which alone would make bit-exactness unattainable.

   *Both are true; neither retires the other.* *(verified `egui-wgpu-0.35.0/src/renderer.rs:216-231`, `egui_kittest-0.35.0/src/builder.rs:45,144`, `lib.rs:120`)*
5. **The harness wraps the closure in `Frame::central_panel(ui.style()).outer_margin(8.0).inner_margin(0.0)`**, so `ui.painter()`'s clip is inset 8 px — and **`Painter::with_clip_rect` *intersects*, it cannot widen**. Drawing via `ui.painter()` would leave an 8-px transparent ring in the golden and make AC9(c) pass trivially. *(verified `egui_kittest-0.35.0/src/app_kind.rs:54-66`, `egui-0.35.0/src/painter.rs:67-75`)*
6. **The root `Ui` is on `LayerId::background()`**, and `Context::layer_painter` clips to **`content_rect()`** = `viewport_rect - safe_area_insets`, insets zero off-mobile ⇒ equal to `screen_rect`. So `ctx.layer_painter(LayerId::background())` yields a **full-canvas, same-layer** painter whose shapes append *after* the panel fill → painted on top. The escape from (5). *(verified `egui-0.35.0/src/context.rs:780-795,1519-1522,2805`)*
7. **The render target is `Rgba8Unorm` (non-sRGB)** — `create_render_state` passes `compatible_surface: None`, so `RenderState::create` uses `vec![TextureFormat::Rgba8Unorm]`, selecting **`fs_main_gamma_framebuffer`** (no gamma conversion). With `dithering: false` and an opaque fill, **a `rect_filled(#F5F1E6)` interior pixel lands exactly as `(245, 241, 230, 255)`** — AC9(a)'s exact assertion is sound. *(verified `egui-wgpu-0.35.0/src/lib.rs:267-276`, `renderer.rs:406-411`, `egui.wgsl:150-161`)*
8. **`clippy::arithmetic_side_effects` does not fire on `f32`**, and `missing_const_for_fn` does not force `const fn` on a float-math helper — verified **empirically** on a scratch crate carrying the workspace's exact lint table on toolchain 1.97.0. ⇒ **the placeholder's screen-space math needs no `#[allow]` and no `const fn`.**
9. **`egui::FontDefinitions` is the API AC2(d) must cite** — applied via `Context::set_fonts(FontDefinitions)`; `Context::add_font(FontInsert)` is the incremental variant. Verified so the rationale cites a name that exists on 0.35, since **no code in this unit exercises it**. *(verified `egui-0.35.0/src/context.rs:2038,2061`)*
10. **Git's fnmatch has no brace expansion — `*.{diff,new,old}.png` ignores NOTHING.** *(reproduced)* The brace form left **all four** files unignored; the three-line form ignored exactly the three artifacts while keeping `placeholder.png` trackable. **AC8's literal wording was already correct** — follow it verbatim.
11. **`@actions/glob` sets `nobrace: true`, so `path: *.{diff,new}.png` matches nothing** — with `if-no-files-found: ignore` the upload becomes a **silent no-op**, defeating AC12 while CI stays green. **`actionlint` does not validate glob semantics.** Fix: multiline `path: |`. *(verified in upload-artifact v7's shipped `dist/upload/index.js`)*
12. **winit's Linux backends are dlopen-based**, so `cargo build`/`cargo test` link without X11/wayland dev packages (`wayland-dlopen` is *in* winit's default set; `x11-dl`; `x11rb/dl-libxcb`; `xkbcommon-dl`). Exposure is **runtime window creation**, which CI never performs. *(verified `winit-0.30.13/Cargo.toml`)*
13. **`threshold(0.0)` is safe — but "bit-to-bit" has a real, non-disableable exception.** *(new; verified `dify-0.8.0/src/diff.rs:54-88` + `lib.rs:59-105`, `egui_kittest-0.35.0/src/snapshot.rs:475`)*
    - **`0.0` is not degenerate — confirmed structurally, not just by upstream's idiom.** dify classifies `if left_pixel == right_pixel { Identical }` **before** any threshold math, so identical pixels can never be counted whatever the threshold; the delta test `if delta.abs() > threshold` is reached only by already-differing pixels. The `>=`-vs-`>` worry that would make `0.0` fail every pixel does not arise.
    - **But differing pixels can still be exempted as anti-aliased.** After the delta cut, dify branches to `DiffResult::AntiAliased` (painted **yellow**, `diffs` **not** incremented) when `detect_anti_aliased_pixels && (antialiased(left,..) || antialiased(right,..))`. **`egui_kittest` hardcodes `detect_anti_aliased_pixels = true`** and exposes **no `SnapshotOptions` knob**. With `blend_factor_of_unchanged_pixels = None`, `get_results` returns `None` when `diffs == 0` ⇒ `try_image_snapshot_options` returns `Ok(())` early and **no diff image is written at all**.
    - **Consequence:** `threshold(0.0) + failed_pixel_count_threshold(0)` is **the strictest comparison `egui_kittest` 0.35 can express**, and it is bit-exact **in flat regions** — but it is *not literally bit-to-bit*. The heuristic is pixelmatch's: it inspects the 3×3 diagonal neighbourhood and **returns `false` as soon as more than two neighbours share the centre's brightness**, so a pixel inside a flat fill (all 8 neighbours identical) is **never** AA-exempt, while a feathered-edge pixel may be. The exemption lands **exactly on our card-rect stroke, hairline, and grid-line edges**, never on the paper interior.
    - This does not change the decision — `0.0`/`0` is right and is what to write — but it **bounds two claims** the design must not launder: "any single differing pixel fails" holds only outside AA-classified edges, and "the failure is loud, never silent" holds for flat-region drift, not edge drift. Net effect: it makes the exactness bet **safer**, since edge rasterisation is the most likely mesa/LLVM variance and is where the residual give sits — the exemption absorbs precisely the drift that would otherwise cause false failures, which is why the product owner **accepted it rather than treating it as a defect**. Recorded in *Risks*, in a code comment, and **bundled with tolerance into the dx12/metal Deferred trigger** — the hole only matters if edge pixels actually drift, which nothing evidences until a second OS lane exists.

### Key decision — the placeholder drawing path (spec: "Design's call")

**One `pub fn` in a new `gp-render` module, taking the draw context *and an explicit target rect*:**

```
crates/render/src/placeholder.rs
    pub fn draw_placeholder(painter: &egui::Painter, rect: egui::Rect)
```

- **(a) no `TrackArtifact`** — the signature has no track parameter; nothing in the body reads one. `gp-gen` stays `todo!()` and is never called.
- **(b) not `render_frame`** — `render_frame`'s body stays `todo!()` per AC5, so the placeholder is a *separate* function. A `todo!()` body would panic AC6/AC8/AC9.
- **one path, three call sites** — `gp-game`'s `App::ui` (AC3), the tessellation test (AC6), and the golden+guard test (AC8/AC9) all call **this same function**. Nothing else draws.

**Why an explicit `rect` rather than `painter.clip_rect()`** (which `render_frame` will eventually use): the clip rect depends on the painter's *provenance* — `ui.painter()` is inset 8 px in the harness (finding 5), `ctx.layer_painter(..)` is not. An explicit rect makes the output a pure function of `(rect)`, which is what lets AC9's guard derive its probes instead of hardcoding them. A *testability* choice for a scaffold #17 deletes — not a divergence from `render_frame`'s contract.

**Geometry SSOT, and how AC9's coordinates stay valid.** Two consts pin everything:

- **`CANVAS_RECT`** — the single canvas definition (192×128 @ ppp 1.0). `with_size(CANVAS_RECT.size())` derives from it; `draw_placeholder` is passed `CANVAS_RECT` itself. *(A single `debug_assert_eq!(CANVAS_RECT, ctx.content_rect())` documents the equality rather than depending on it.)*
- **`geometry(rect) -> PlaceholderGeometry`** — a private fn, the sole source of the paper/card/hairline positions. `draw_placeholder` draws from it; the guard derives its probes from it. No probe coordinate is written twice.

Both stay **private** — the guard lives in the same file's `#[cfg(test)] mod tests`, so there is no `pub`-for-test API (YAGNI).

**Colours are scaffold-local, not the token module.** #12 owns "design tokens → Rust consts". This unit defines only the `Color32` consts the placeholder needs, as **private module-level `const SCREAMING_SNAKE_CASE`** (AGENTS.md § *Code Style* magic-number rule), commented as scaffold-local and superseded by #12.

**Drawn content** (satisfies AC3 + the amended AC4 (a)(b)(c)):

| Layer | Token | Purpose |
|---|---|---|
| paper fill over the whole `rect` | `--paper-1 #F5F1E6` | AC3 "cleared to the paper background"; AC9(a) probe target |
| graph-paper ruling + dots | `--grid-line #C3CEDD`, `--grid-dot #93A2B8` | **AC4(c)** motif |
| one card rect (fill + crisp radius + stroke) | `--paper-2 #ECE6D6`, `--graphite-900 #201E1A` | AC3 "at least one rectangle"; AC4(a) crisp shapes |
| one hairline stroke | `--graphite-300 #C4BBAA` (the token literally named *hairline*) | AC3 "one hairline stroke"; AC4(b); AC9(b) probe target |

**No text is drawn anywhere in this unit** — text shaping, the dominant churn source, is absent by construction, a load-bearing premise of the exact-compare bet.

**Distinct, separately-probeable regions are load-bearing at 192×128** — AC9's probes need a paper pixel far from any edge, and `image-check` needs enough frame to tell the rect from the hairline from the ruling. `geometry()` must keep them clearly apart rather than packing them.

**Hairline placement is load-bearing**: the stroke sits at a **half-integer** coordinate so a 1.0-wide stroke covers exactly one pixel row at ppp 1.0. At an integer coordinate it straddles two rows at ~50 % each, halving AC9(b)'s margin. egui feathers edges (~1 px) regardless, so AC9(b) asserts a **darkness margin**, never an exact ink colour.

**Rejected:** *a temporary `render_frame` body*; *deriving the target from `painter.clip_rect()`*; *three per-site drawings* — each defeats AC5, AC9's probes, or AC6/AC8/AC9's shared-path purpose.

### Key decision — three guards, one render

| Mechanism | Catches | Where it runs | Subtask |
|---|---|---|---|
| Exact compare (AC10a) | **Regressions** — drift from the minted pixels **in flat regions**; AA-classified edge pixels are exempt (finding 13) | CI gate | 9 |
| AC9 pixel probes | **Degeneracy** — the quartzite all-black failure | CI gate | 8 |
| **`image-check` (AC14)** | **A golden that was wrong from birth** | **Mint/regen only — never CI** | 1 (file) + 2 (permission) + 9 (invoked) |

The third closes a gap neither of the others covers: **a black triangle minted as the golden compares bit-exact against itself forever**, and could pass AC9's three probes if they land plausibly. Exactness proves *"identical to what we minted"*, never *"what we minted was right"*; AC9 proves *"not degenerate"*, never *"the right shapes in the right places"*. **AC9 is not weakened by AC14** — `image-check` runs once at mint, AC9 runs on every CI push.

**AC8/AC9/AC10 share exactly one rasterisation.** `Harness::snapshot_options` internally calls `self.render()` (verified `snapshot.rs:621-629`), so `render()`-then-`snapshot_options()` would rasterise **twice** and the guard would inspect a different image object than the golden compares. Therefore:

```
let image = harness.render()?;              // exactly one rasterisation
  … AC9 guard assertions on `image` …
egui_kittest::try_image_snapshot_options(&image, "placeholder", &options)   // AC8/AC10(a), same image
```

*Note on AC10(a)'s wording:* the AC says "via `Harness::snapshot_options`". `try_image_snapshot_options` is the **same comparison function underneath** — `snapshot_options` → `try_snapshot_options` → `try_image_snapshot_options` — and is public at the crate root (`pub use crate::snapshot::*`). Using it directly is what makes the single-image property provable rather than merely probable; the thresholds AC10(a) mandates are unchanged. Design-review endorsed this construction in round 2.

*Note on AC8's other wording:* the AC's parenthetical says `Harness::new_ui(..)`, but this design uses `Harness::builder()…build_ui(..)`. **The substitution is forced, not a preference:** `new_ui(app: impl FnMut(&mut Ui))` takes **only a closure**, so it cannot express `with_size`, `with_pixels_per_point`, or `.renderer(..)` — and AC8 itself mandates "a pinned canvas size and `pixels_per_point = 1.0`" while AC10(b) mandates the checked render state, so `new_ui` **cannot satisfy the ACs that name it**. The spec's own Key-decisions row names `HarnessBuilder::with_size` + `with_pixels_per_point`, corroborating the builder path. `new_ui` is literally `Self::builder().build_ui(app)` (verified `egui_kittest-0.35.0/src/lib.rs:866`) — same construction, minus the knobs. Recorded so a reader does not read it as drift.

**Guard-before-golden ordering is deliberate.** A degenerate frame should fail with *"the frame is uniform"* (points at the drawing code) rather than *"images differ"* (points at the golden). `SnapshotResults::drop` skips its panic when already panicking (verified `snapshot.rs:862-868`), so a guard failure reports cleanly.

**AC9's four checks** (a/b/c plus one strengthening):

| Check | Assertion | Why |
|---|---|---|
| AC9(a) | pixel at the geometry-derived **paper probe** `== (245, 241, 230, 255)` | exact equality is sound per finding 7; and per finding 13 the flat interior is the one region dify's AA exemption can never reach |
| — | the image's **most common colour** is the paper colour | strengthens (a): proves the paper *fill* drew, not a lucky pixel. Coordinate-free. |
| AC9(b) | pixel at the geometry-derived **hairline probe** darker than the paper probe by ≥ `HAIRLINE_MIN_DARKENING` (a named const) | feathering + lavapipe make an exact ink match unreliable; `#C4BBAA` vs `#F5F1E6` is ≈ 50/channel, so ~16 is safe and meaningful |
| AC9(c) | distinct colour count `> 1` | coordinate-free; the literal quartzite anti-case |

A comment cites the quartzite finding: five CI-enforced goldens, byte-identical, uniformly `(0,0,0,255)`, would pass with the renderer deleted.

**Pixel indexing and the lint gate.** `RgbaImage::get_pixel` takes `u32`; `geometry()` returns `Pos2` (`f32`). One private test helper carries a single justified `#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "…")]` — `CANVAS_RECT` is 192×128 at ppp 1.0 and `geometry()` returns positions inside it, so the truncation is in-domain and total.

### Key decision — exact bit-to-bit compare (AC10a)

`SnapshotOptions::new().threshold(OsThreshold::new(0.0)).failed_pixel_count_threshold(OsThreshold::new(0))`.

- **Both fallbacks must be overridden.** `threshold`'s fallback is **`0.6`** (upstream: *"enough for most egui tests to pass across different wgpu backends"*) — a live trap that would silently permit per-pixel colour distance; `failed_pixel_count_threshold`'s fallback is already `0`. Writing both explicitly also makes them immune to a future `kittest.toml` or an upstream default change. `config()` degrades to `Config::default()` without a `kittest.toml`; we deliberately add none — the values belong in code next to their reason-comment.
- **`threshold(0.0)` is not degenerate** — proven structurally (finding 13), and it is upstream's own idiom: `egui_kittest` uses `0.0` under *"Produce diff for any error, however small"* in `Mode::UpdateAll`.
- **The gate is a pixel *count*, never a mean** — structural, not our choice: `below_threshold = num_wrong_pixels <= failed_pixel_count_threshold`. quartzite's mean pathology (`report.mean > 0.05` on 64×64, untrippable by a 1-px error) is **impossible here**. `FLIP_TOLERANCE = 0.05` stays rejected as unvalidated precedent.
- **Two reason-comments are required**: (i) the premise + deferral trigger — llvmpipe is expected stable on the single Linux/lavapipe lane; revisit when dx12 (Windows) / metal (macOS) lanes join the matrix; (ii) finding 13's AA caveat — this is the strictest setting the library can express, and edge pixels retain residual give. A future reader must not have to re-derive either from source.
- **Tolerance and the AA exemption are deferred *together*, not deleted** — both reopen on the **same** trigger: a second OS lane (dx12/Windows, metal/macOS) joining the CI matrix. `OsThreshold`'s per-OS variants exist precisely for that matrix and are unused here. The bundling is deliberate (product-owner call on finding 13): the AA hole only matters **if edge pixels actually drift**, and there is no evidence either way until cross-platform variance is real — the same condition that reopens tolerance. Do **not** design for either now. **If it bites, revisit — do not pre-emptively loosen.**

### Key decision — the `image-check` subagent (AC14)

**A file at `.claude/agents/image-check.md`, following `code-writer.md`'s house pattern exactly** (verified: `code-writer` is the **only** one of nine existing agents with a pinned `effort`; the other eight are `model: opus` with none).

| Field | Value | Why |
|---|---|---|
| `name` | `image-check` | spec-fixed; deliberately avoids the `review` token, which denotes the Review sync group here |
| `description` | dispatch-accurate one-liner naming the job + the pinned tier + the mint/regen-only scope | Claude routes delegation on this field |
| `model` | `sonnet` | AC14 |
| `effort` | `medium` | AC14 — **frontmatter is the only lever**; there is no per-invocation `effort` parameter on the Agent tool, which is exactly why an inline `Agent(model="sonnet", …)` cannot satisfy AC14. `code-writer.md` already records this rationale. |
| `tools` | **omitted** → inherit-all | `Read` must render the PNG; restricting `tools` would break the check |

**The body's contract — derive-then-look is the mechanism, not a detail.**

1. **Inputs** (from the spawn prompt): the path to the drawing code (`crates/render/src/placeholder.rs`, fn `draw_placeholder` + `geometry`) and the path to the minted PNG.
2. **Read the drawing code FIRST and write the expected frame down** *before opening the image*: paper `#F5F1E6` fill; graph-paper ruling + dots; one card rect (fill `#ECE6D6`, `#201E1A` stroke, crisp radius) at `geometry()`'s position; one `#C4BBAA` hairline at `geometry()`'s position; **nothing else**.
3. **Then `Read` the PNG.**
4. **Compare** against that written expectation: paper background present; rect and hairline where the code puts them; graph-paper motif present; **no unexplained shapes or colours**.
5. **Verdict: PASS / FAIL**, with specifics on a FAIL.
6. **On FAIL the caller fixes the drawing code and re-mints — never re-interpret the image, never adjust the expectation to fit it.**

Step 2's ordering is the whole point. A model shown a black triangle and asked *"is this consistent?"* will find a story; a model that has already written down "one rounded rect and one hairline on paper" and then sees a black triangle cannot. Looking first is what would make this check a rubber stamp, so the file mandates the order.

**It is NOT a CI gate and must not become one.** A definition file in `.claude/agents/` *looks installed* in a way an inline spawn did not, so the boundary is restated in the file itself: `image-check` is spawned at **mint/regen time only**; CI has no model and never invokes it. **AC13's gate list is exhaustive and names AC14 as deliberately absent** — subtasks 12 and 13 both carry an explicit "no `image-check` step" instruction. Forcing it into CI would make it flaky or a silent no-op — quartzite's `SKIP_RENDER_SNAPSHOT` failure mode exactly.

**Who spawns it:** the minting subtask's implementor, which is `code-writer`. The spawn is `subagent_type="image-check"` with **no inline `model=`/effort override** — the frontmatter is the enforcement, and an override would be the very thing AC14 forbids.

> **Round 3 got this wrong, and the correction is AC16.** Round 3 justified the spawn with *"`tools` inherit-all ⇒ it has the `Agent` tool"* — that establishes **tool capability**, but **instruction permission** was what mattered, and `code-writer.md`'s `## Invariants (both modes)` (*"These hold in EVERY invocation, regardless of mode"*) says verbatim: **"Do not spawn `self-review`; do not spawn any other reviewer."** `image-check` returns a PASS/FAIL verdict gating a commit, which matches that clause literally — so the contract **forbids the very spawn the design routes through it**. See *Key decision — AC16*.

**It gates regens too, not just the first mint** (AC11): every later `UPDATE_SNAPSHOTS=true` run re-runs it. #17 inherits this against its own drawing code, and the agent file carries forward unchanged.

**Keep the file small** — AC15(c) / the AGENTS.md 40,000-char AXIOM covers *"every `.claude/agents/**.md`"*. It has one job; a page is plenty.

### Key decision — AC16: the calling contract + the invariant carve-out

**AC14's file half is worthless without its invocation half, and the invocation half is currently forbidden.** AC16 fixes that with **two edits to one file** — `.claude/agents/code-writer.md` — and **(a) is unimplementable without (b); do not land (a) alone.**

**(a) The golden-spawn rule — mode-aware, standing, not #11-specific.** When a subtask **mints or regenerates a golden image**, spawn `image-check` and do not proceed until it confirms image↔code consistency. Matched to that file's two contracts:

| Mode | Its contract | The rule's bite |
|---|---|---|
| **Mode A** (`/task` group-implementor — commits per subtask) | commits | must **not commit** the PNG until `image-check` passes |
| **Mode B** (single-fix delegate — returns without committing) | returns | must **not return** until `image-check` passes |

Worded as a **standing rule for any golden** so #17/#18 inherit it — not "the #11 placeholder golden".

**(b) The carve-out — a category the invariant never meant to cover, not a hole punched in it.** The clause today reads *"Do not spawn `self-review`; **do not spawn any other reviewer**."* The carve-out:

- **Permits:** a **subtask-named artifact-validity check** that verifies a *generated artifact* against the code that generated it. `image-check` is the only instance today.
- **Still forbids, explicitly:** `self-review`, and **any approval-gate reviewer judging the quality or correctness of the writer's own work**.
- **The discriminator is artifact vs. work**, and it must be **stated, not implied** — a future reader must be able to place a new subagent on one side without re-deriving this decision.

**Why this is principled:** `image-check` is the same category as **`cargo test`, which `code-writer` already runs freely** — it judges *the artifact*, never *the work*. The invariant's own stated rationale — *"The orchestrator owns self-review — it must be able to review the work before it is committed/pushed"* — is **untouched**: `self-review` of the whole diff stays with the orchestrator, exactly as before. Nothing about the trust boundary moves; the carve-out only names a class that was always on the other side of it.

**Scope is exactly this one file.** No `self-review` edit (would fire the Review sync group); no `/task` SKILL.md edit (would fire the Task/Design group). Verified independently: **`code-writer` appears nowhere in `AGENTS.md`**, so editing it has **zero sync-group fan-out**.

**No sibling claim goes stale.** *"Never runs self-review, never pushes."* appears in **three** places — the `claude-tools-hierarchy.md` row, `code-writer.md`'s frontmatter `description`, and Mode A step 4 — plus Mode B's *"the orchestrator owns self-review and the single commit/push"*. **All four stay true**, because each names `self-review` *specifically* rather than claiming the agent spawns nothing. Exactly one clause changes (verified by reading all four).

**File-size (AC15(c)):** `code-writer.md` is **6,935 chars** (verified) — **28,065 below the 35,000 early-warning rung**, so the insertion is comfortably safe. It must still be **tight**: an instruction file, not a tutorial. Author both edits in that file's existing invariant voice (cross-mode `NEVER` bullets with mode-specific bite) — no bolted-on foreign block.

**The ordering property that makes this work — load-bearing and non-obvious.** Group A edits `code-writer.md`; Group B's `code-writer` spawn then reads the **amended** file. The instructions-first inversion, originally forced by the `image-check.md` file dependency, now **carries the permission dependency too** — the same ordering satisfies both. Had the doc group stayed last (rounds 1–2's order), Group B would have spawned a non-existent agent *under a contract still forbidding the spawn* — two independent failures at once.

### Key decision — AC15 propagation

Adding a subagent changes the Subagent inventory ⇒ **`ai-docs/claude-tools-hierarchy.md` MUST be updated in this same PR** (AGENTS.md § *Propagation Rule*). Concretely, **two rows change in its `## Subagents (.claude/agents/)` table** (verified present, 3,415 chars — far under the AXIOM):

- **(i)** **add** an `image-check` row, **mirroring the `code-writer` row's shape** — Name / Spawned by / Role, recording the frontmatter-pinned `model: sonnet` + `effort: medium` and the mint-time-only, never-CI scope.
- **(ii)** **update the existing `code-writer` row** to append its new AC16 obligation (spawns `image-check` on any golden mint/regen; Mode A must not commit the PNG, Mode B must not return, until it confirms). Its closing *"Never runs self-review, never pushes."* **stays true** — it names `self-review` specifically.

**(ii) is the easy one to lose**, precisely because `code-writer` is *already listed* and the row therefore looks done — which is the miss AC15 explicitly warns about (*"Do not let (ii) be missed because `code-writer` is already listed"*). Subtask 3 states both halves for that reason.

**No other sync-group sibling is implicated** (AC15(b)), and I verified this independently against AGENTS.md's Propagation table rather than taking it on report: the enumerated groups — Review (`project-review`/`review-findings`/`self-review`), Interview (`interview`/`spec-writer`), Triage (`triage`/`triage-runner`/`next`), Task/Design (`task`/`design`/`design-review`/`context-reset`), Spec-Amendment, Learning-Log — each name **specific existing** files. `image-check` joins none of them. It is spawned *inside* a subtask by the group implementor, not by `/task`'s own Step-8 logic, so the Task/Design group's contract is untouched.

### Key decision — AC10(b) adapter wiring (more load-bearing under exact compare)

AC10(b) **forces** the `create_render_state` path: `HarnessBuilder::wgpu()` would give `PREDICTABLE` for free but constructs the `RenderState` internally and never exposes it, so the adapter cannot be inspected. Hence:

1. `let render_state = egui_kittest::wgpu::create_render_state(egui_kittest::wgpu::default_wgpu_setup(), egui_wgpu::RendererOptions::PREDICTABLE);`
2. **AC10(b):** `assert_eq!(render_state.adapter.get_info().device_type, egui_wgpu::wgpu::DeviceType::Cpu, "<loud message naming lavapipe / mesa-vulkan-drivers>")`
3. `let renderer = egui_kittest::wgpu::WgpuTestRenderer::from_render_state(render_state);` (panics if the state was used — ours is fresh)
4. `Harness::builder().with_size(CANVAS_RECT.size()).with_pixels_per_point(1.0).with_theme(egui::Theme::Light).renderer(renderer).build_ui(|ui| { let p = ui.ctx().layer_painter(egui::LayerId::background()); draw_placeholder(&p, CANVAS_RECT); })`

**`PREDICTABLE` at step 1 is mandatory** — the builder's `render_options` default *is* `PREDICTABLE`, but `.renderer(..)` at step 4 bypasses that field (finding 4). Omitting it silently restores `dithering: true` and hardware texture filtering, which alone would make bit-exactness unattainable — and the failure would read as "exact compare is too strict" rather than "wrong options". **The default does not save us on this path.**

**Under exact compare, step 2 is more load-bearing than under a tolerance:** a bit-exact golden minted against a discrete GPU fails on every other machine. A tolerance might have absorbed that; exactness cannot. It also makes AC11's "CI is authoritative" near-vacuous in practice — both sides are guaranteed CPU adapters, so a local/CI difference is a hard failure pointing at a real environment divergence (e.g. a mesa/LLVM delta), not a judgement call to be talked away.

**`with_theme` is pinned** because the builder's default is `Theme::Dark`; the paper rect covers the canvas so it cannot currently show through, but pinning removes a churn class for free.

**This forces `egui-wgpu = "0.35"` as a dev-dep** — both of `create_render_state`'s parameter types are `egui_wgpu` types, and `RendererOptions::PREDICTABLE` cannot be named otherwise. Preferring `egui-wgpu` over a bare `wgpu = "29"` dev-dep matters: `egui-wgpu` does `pub use wgpu;` (verified `src/lib.rs:19`), so `egui_wgpu::wgpu::DeviceType` is *the same wgpu version `egui_kittest` resolves*.

**Footgun recorded for #17/#18 (not live here):** `HarnessBuilder::wgpu()` snapshots `self.render_options` **at call time** — `pub fn wgpu(self) -> Self { let r = WgpuTestRenderer::with_render_options(self.render_options); self.renderer(r) }` (verified `builder.rs:153-156`). So `.wgpu().with_render_options(..)` silently does **not** apply; the order must be `.with_render_options(..).wgpu()`. Same class as finding 4.

### Key decision — AC4's font capability: nothing to do here

Per the spec's **⚠ Font-proof amendment**, AC4 no longer carries a font clause. Round 1's `epaint_default_fonts` route was the design's own recommendation and was **rejected** in favour of dropping the capability outright. This design adds **no** `epaint_default_fonts` dev-dep (dev-deps are `egui_kittest` + `egui-wgpu`), writes **no** font test, draws **no** text. The only font obligation is **AC2(d)**: cite `egui::FontDefinitions` (finding 9). The accepted cost — *the backend's font capability goes unexercised by any code until #12* — is in *Risks*.

### Key decision — AC5's signature vs the §4 layer stack (sanity-check)

`render_frame(&egui::Painter, &TrackArtifact, &[CarState], Overlays)` **does not preclude** `docs/design.md` §4's stack:

| §4 layer | Data source under this signature |
|---|---|
| outfield / infield / asphalt | derived from `track.corridor` (§4: asphalt is *derived* from `D`) |
| walls | `track.walls` |
| S/F | `track.sf` |
| graph-paper grid + dots | needs only a target area → `painter.clip_rect()` |
| overlays (`speed_heatmap`, `fastest_lap`) | `Overlays` flags + `track.metrics` / `track.centerline` |
| cars | `&[CarState]` |

The one thing not passed is a **world→screen transform**. It is *derivable* (`painter.clip_rect()` + `track` ⇒ a fit-to-view mapping), so §4 is reachable — but the signature **pins the policy to auto-fit**: a future pan/zoom camera needs an extra parameter or an `Overlays` field. Recorded for #17/#18; **not** a reason to reopen AC5.

`Painter` is `Clone` and cheap; `&egui::Painter` borrows without owning/constructing/storing, satisfying AC5.

### Key decision — test placement

Everything lands in **`crates/render/src/placeholder.rs`'s `#[cfg(test)] mod tests`** (dev-deps are available to unit tests; `SnapshotOptions::output_path` defaults to `tests/snapshots` relative to CWD = the package root, so the golden lands at **`crates/render/tests/snapshots/placeholder.png`** exactly as AC8 requires). This keeps `geometry()` and the palette private — the anti-drift property with **zero** `pub`-for-test surface. Estimated ~330 lines incl. tests, inside the 800 incl.-test soft limit.

The trade-off — `cargo test -p gp-render --lib` links wgpu — is the crate-level cost the spec **already accepted**; AC6's "no GPU adapter" property is scoped *per-test* by the spec's own note, and holds.

### AC coverage (re-checked against the re-amended spec — 16 ACs)

| AC | Covered by |
|---|---|
| AC1 | 5 |
| AC2 (a)–(e) | 4 |
| AC3 | 7 (content) + 11 (window) |
| AC4 (a) crisp, (b) hairline, (c) graph-paper | 7 |
| AC5 | 10 |
| AC6 | 6 + 7 |
| AC7 | 5 (`--edges no-dev`) + 13 (re-verify) |
| AC8 | 9 (golden test + committed PNG) + 5 (`.gitignore`, three separate lines — AC8's own clause) |
| AC9 | 8 |
| AC10 (a) exact compare, (b) adapter | 9 (a) + 8 (b) |
| AC11 | 4 |
| AC12 | 12 |
| AC13 (exhaustive gate list) | 13 |
| **AC14** (`image-check` agent — **not** in AC13's list, **not** in CI) | **1** (file exists) + **9** (spawned at mint) |
| **AC15** (propagation + file-size) | **3** (both hierarchy rows) + 1 (`image-check.md` small) + 2 (`code-writer.md` insertion stays tight) |
| **AC16** (calling contract + invariant carve-out — (a) and (b) together, in one file) | **2** |

## Decomposition

TDD-ordered per AGENTS.md § *Workflow* within the code group. **One honest caveat:** the golden PNG (#9) is **not** TDD-able — an expected image cannot be authored by hand; it is *minted* from the implementation. `image-check` is precisely what compensates: it checks the minted image against the code's intent, which is the assertion a hand-written expectation would otherwise have encoded.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **AC14 — author `.claude/agents/image-check.md`.** Frontmatter `name: image-check`, dispatch-accurate `description`, **`model: sonnet`**, **`effort: medium`**, `tools` **omitted** (inherit-all — `Read` must render the PNG). Body: the derive-then-look contract (read the drawing code and write the expected frame **first**, `Read` the PNG **second**, compare, PASS/FAIL, on FAIL fix the code and re-mint — never re-interpret the image), and the **mint/regen-only, NEVER-a-CI-gate** boundary restated in the file. Mirror `.claude/agents/code-writer.md`'s house shape + its frontmatter-is-the-only-effort-lever rationale. **Keep it small** (AGENTS.md 40k AXIOM covers every `.claude/agents/**.md`). | `.claude/agents/image-check.md` | — |
| 2 | **AC16 — the calling contract, TWO edits to `.claude/agents/code-writer.md`; (a) is unimplementable without (b), do NOT land (a) alone.** **(a)** the **golden-spawn rule**, worded as a standing rule for **any** golden (#17/#18 inherit it): on any mint/regen, spawn `image-check`; **Mode A must not commit** the PNG and **Mode B must not return** until it confirms. **(b)** a **narrow carve-out** to the `## Invariants (both modes)` clause *"Do not spawn `self-review`; do not spawn any other reviewer"* — **permits** a *subtask-named artifact-validity check verifying a generated artifact against the code that generated it* (`image-check` is the only instance today); **still forbids, explicitly**, `self-review` and any *approval-gate reviewer judging the quality/correctness of the writer's own work*. State the **artifact-vs-work discriminator explicitly** so a future reader can place a new subagent without re-deriving this. Author both in the file's existing cross-mode `NEVER`-bullet voice — no bolted-on block. **Keep it tight**: 6,935 chars today, 28k under the 35k rung, but it is an instruction file, not a tutorial. Scope = this file only (no `self-review`, no `/task` SKILL.md — those fire sync groups). | `.claude/agents/code-writer.md` | 1 |
| 3 | **AC15(a) — propagation; TWO rows change, not one.** In `ai-docs/claude-tools-hierarchy.md`'s `## Subagents (.claude/agents/)` table: **(i)** add an `image-check` row (Name / Spawned by: `code-writer` at golden mint+regen — never CI / Role: image↔code consistency, frontmatter-pinned `model: sonnet` + `effort: medium`); **(ii)** update the **existing `code-writer` row** to append its new AC16 obligation (spawns `image-check` on any golden mint/regen; Mode A must not commit the PNG, Mode B must not return, until it confirms). **Do not skip (ii) just because `code-writer` is already listed.** That row's closing *"Never runs self-review, never pushes."* **stays true and needs no correction** — it names `self-review` specifically. AC15(b): no sync-group sibling is implicated by either edit — confirm with `grep -rn "image-check\|code-writer" .claude/ AGENTS.md ai-docs/` per the Propagation Rule Procedure (`code-writer` appears nowhere in `AGENTS.md` — verified). | `ai-docs/claude-tools-hierarchy.md` | 1, 2 |
| 4 | **AC2 + AC11 rationale** in `ai-docs/key-decisions.md` (`### YYYY-MM-DD — <title>` → **Context/Decision/Consequences**): (a) eframe/egui ↔ the flat-vector/lattice aesthetic + the 13-components-across-4-screens cost argument; (b) rejected alternatives — macroquad, raw wgpu+winit+tessellator, vello; (c) each **amended** AC4 capability (crisp shapes, hairline strokes, graph-paper motif) incl. the cited `egui` API; (d) **cite `egui::FontDefinitions`** as the backend's custom-font evidence, noting that **no face is loaded and no code exercises font loading here**; (e) the **§6-over-#11 ownership override** and why. Plus AC11's contributor workflow — golden path; `UPDATE_SNAPSHOTS=true cargo test`; **a regen is not complete until the `image-check` subagent confirms image↔code** (spawn the agent — never an inline `Agent(model="sonnet", …)`, which cannot enforce the effort tier); lavapipe/`mesa-vulkan-drivers` required, **no skip hatch**; **CI is authoritative** (near-vacuous in practice — AC10(b) puts both sides on a CPU adapter, so a divergence is a hard environment failure). `docs/design.md` is **not** amended. | `ai-docs/key-decisions.md` | 1, 2 |
| 5 | Dep + repo wiring. `crates/render`: `egui = "0.35"` normal; `[dev-dependencies]` `egui_kittest = { version = "0.35", features = ["wgpu","snapshot"] }` + `egui-wgpu = "0.35"`; delete the `TODO(2)` comment. `crates/game`: `eframe = "0.35"` (no `egui` — use `eframe::egui`). `.gitignore` gains the **three AC8 patterns as three separate lines** — `**/tests/snapshots/**/*.diff.png`, `**/tests/snapshots/**/*.new.png`, `**/tests/snapshots/**/*.old.png`. **No brace form** (finding 10). Verify with `git check-ignore -v` on all four names (three ignored, `placeholder.png` NOT). `cargo update` + `cargo build`. Pin against the version **observed now**; verify AC7 via `cargo tree -p gp-render --edges no-dev`. | `crates/render/Cargo.toml`, `crates/game/Cargo.toml`, `.gitignore`, `Cargo.lock` | — |
| 6 | **(test-first)** AC6 tessellation smoke test: `RawInput{screen_rect}` → `run_ui` → `tessellate(shapes, 1.0)`; assert non-empty **and** total vertex/index counts > 0. Fails to compile (no `draw_placeholder` yet). | `crates/render/src/placeholder.rs` | 5 |
| 7 | Implement `draw_placeholder(&Painter, Rect)` + `CANVAS_RECT` + private palette consts + `geometry(rect)`; register `pub mod placeholder` in `lib.rs`. AC6 goes green. Satisfies AC3's content + AC4(a)(b)(c). No `#[allow]` needed (f32 — finding 8). Draws no text. Keep the card and hairline in **distinct, separately-probeable regions** (AC9's probes and `image-check` both depend on it). | `crates/render/src/placeholder.rs`, `crates/render/src/lib.rs` | 6 |
| 8 | **(test-first)** AC10(b) + AC9 GPU test: `create_render_state(default_wgpu_setup(), RendererOptions::PREDICTABLE)` — **explicitly**; the builder default is bypassed by `.renderer(..)` (finding 4) — → `assert_eq!(adapter.device_type, Cpu)` → `from_render_state` → harness (`with_size(CANVAS_RECT.size())`, ppp 1.0, `Theme::Light`, `layer_painter(background)` + `CANVAS_RECT`) → **one** `render()` → the four guard checks. | `crates/render/src/placeholder.rs` | 7 |
| 9 | AC8 + AC10(a) + AC14-invocation. Add `try_image_snapshot_options(&image, "placeholder", &opts)` to the **same** test with `threshold(0.0)` + `failed_pixel_count_threshold(0)` — **both overridden**, `0.6` is a live trap (finding 13) — plus the two reason-comments (premise + dx12/metal deferral trigger; the AA caveat). Mint via `UPDATE_SNAPSHOTS=true cargo test -p gp-render`. **Then spawn `subagent_type="image-check"` with NO inline `model=`/effort override** (frontmatter is the enforcement), passing the drawing-code path + the minted-PNG path; **commit the PNG only on PASS** — on FAIL fix the drawing code and re-mint. **This spawn is permitted only because subtask 2 landed AC16's carve-out** — without it `code-writer`'s `## Invariants` forbids it and a contract-obeying implementor would silently skip the check (hence the `1, 2` dependency). Confirm a clean (no-env) `cargo test` re-passes. | `crates/render/tests/snapshots/placeholder.png`, `crates/render/src/placeholder.rs` | 1, 2, 8 |
| 10 | AC5: re-signature `render_frame` to `(&egui::Painter, &TrackArtifact, &[CarState], Overlays)`; body stays `todo!()`; refresh the rustdoc (backtick any `[Xn]` design markers — 2026-07-16 doc-gate learning). **Also add the one-line backticked-path prose pointer to `ai-docs/key-decisions.md`** in the crate docs — folded in here so the doc group stays purely `ai-docs/**` + `.claude/**`. Backticked prose, deliberately **not** an intra-doc link, so `broken_intra_doc_links` cannot fire. | `crates/render/src/lib.rs` | 5 |
| 11 | AC3: `gp-game` window + loop — `impl eframe::App` using **`fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)`** (*not* `update`, finding 1) delegating to `draw_placeholder(ui.painter(), ui.max_rect())`, plus `eframe::run_native(..) -> eframe::Result`. **The implementor confirms `cargo build -p gp-game` only — do NOT run `cargo run -p gp-game`**: a GUI binary never exits, so a foreground bash call blocks until timeout. The window check is **AC3's human verification** and surfaces to the orchestrator (see *Test Design 4*). Keep `main.rs` thin glue. | `crates/game/src/main.rs` | 7, 10 |
| 12 | AC12: `ci.yml` `test` job gains an `if: failure()` `actions/upload-artifact@v7` step with **multiline `path: \|`, one pattern per line** — `crates/render/tests/snapshots/**/*.diff.png` and `crates/render/tests/snapshots/**/*.new.png`. **No brace form**: `@actions/glob` sets `nobrace: true` (finding 11). Keep `if-no-files-found: ignore` — the step fires on *any* test failure. **`actionlint .github/workflows/ci.yml` MUST pass before `git add`** (AGENTS.md AXIOM) — but actionlint does **not** validate glob semantics, so green actionlint is not evidence the path matches. **No `image-check` step** — AC14 must never enter CI. No Vulkan step (`vulkaninfo --summary` already at line 102). | `.github/workflows/ci.yml` | 9 |
| 13 | AC13 full gate re-run after cleanup — **exactly** `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`. **This list is exhaustive; AC14/`image-check` is deliberately absent — do not add it.** Re-verify AC7 `cargo tree -p gp-render --edges no-dev`. Budgeted because a `-D warnings` gate aborts on the first failure and masks later ones. | — | 5–12 |

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping. `M = 13`; **2 groups** (the minimum: exactly two change-types are present, and neither exceeds the size cap). Every group — **including the first** — is entered via a `/context-reset` handoff.

- **Handoff into Group A:** spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
- **Group A** — model `opus`, effort **inherited from the orchestrator** (typically xHigh) — **NOT pinned** — via `general-purpose`, 1M-token window — subtasks **1–4** (instructions/harness change-type: `.claude/agents/image-check.md`, **`.claude/agents/code-writer.md`**, `ai-docs/claude-tools-hierarchy.md`, `ai-docs/key-decisions.md`). 4 subtasks, within the size cap `≤ 10`. Adding AC16's `code-writer.md` edit here **keeps the group homogeneous** — it is `.claude/agents/**`, the same change-type as the rest.
- **Handoff after Group A:** spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). Parent `/task` resumes in Group B with fresh context.
- **Group B** — model `sonnet` (sonnet-5), effort **`medium` (pinned)** via the `code-writer` subagent, 1M-token window — subtasks **5–13** (code change-type: `*.rs`, `Cargo.toml`, `Cargo.lock`, `.gitignore`, `.github/workflows/ci.yml`, the committed `.png`). 9 subtasks, within the size cap `≤ 10`. **Terminal group** (9; within `1..=10`).

**Why the order inverted — a hard dependency the addendum's plan did not anticipate.** The addendum placed the `image-check` file in "Group B" on the assumption that the doc group stays last (as rounds 1–2 had it). That does not work: **subtask 9 spawns `image-check`, so the agent file must already exist** — a code-group subtask depends on an instructions-group subtask. § Rules (f) bounds group-minimization by *"task dependencies — never break dependency order"*, so the instructions group must run **first**.

Three orderings were considered:

| Option | Groups | Verdict |
|---|---|---|
| agent file first (own group) → code → key-decisions last | 3 | Rejected — avoidable non-minimized count |
| **all instructions first → all code** | **2** | **Chosen** — minimal, homogeneous, dependency-correct |
| code first → instructions last (rounds 1–2's order) | 2 | **Impossible** — subtask 9 would spawn a non-existent agent, under a contract still forbidding the spawn |

Nothing is lost by moving `key-decisions.md` earlier. Rounds 1–2 placed it last because "AC11 documents the golden's path, name, and refresh loop, which exist only once the golden is minted" — but on inspection that dependency was always **soft**: the path (`crates/render/tests/snapshots/placeholder.png`), the name, the regen command, and every AC2 clause are **design-determined here**, not discovered at mint. They are promises the design already pins, and `self-review` at Step 10 checks the diff against spec + design, so a deviation is caught. The agent-file dependency, by contrast, is **hard**.

Both groups stay **homogeneous**: Group A is `.claude/**` + `ai-docs/**` (both instructions/harness); Group B is code. The prose pointer stays **mandatorily** folded into subtask 10 (Group B, already editing `crates/render/src/lib.rs`), so no `*.rs` file leaks into Group A.

**Model marking is per-group and unchanged in kind** — only the order moved: instructions → `opus`/inherited, code → `code-writer` (`sonnet`/`medium`, frontmatter-pinned). Note `code-writer`'s pinned `sonnet` is *also* what makes subtask 9's spawn of `image-check` a sonnet→sonnet delegation, which costs nothing and is not relied upon: `image-check`'s **own** frontmatter is what enforces its tier.

Group count 2 ≤ the default max of 4 → **no user gate needed** (§ Rules (h)).

## Risks

- **mesa/LLVM drift on `ubuntu-latest` breaks bit-exactness — the live risk under exact compare, accepted.** CI installs mesa/lavapipe from apt, so an upgrade could shift pixels on the *same* platform. Accepted because: the frame is **text-free flat fills + hairlines** at ppp 1.0 under `PREDICTABLE` (no MSAA, `dithering: false`, software texture filtering), so flat-fill/hairline rasterisation is the only variable; quartzite's one observed real drift (`99099ae`) was **text sub-pixel variance across backends** — neither condition applies; and the failure is **loud** (golden diff + the AC12 upload). Finding 13 refines this in our favour: dify's AA exemption means **edge-pixel** drift — the most likely mesa/LLVM variance — is absorbed rather than failing. **Standing instruction: if it bites, revisit — do not pre-emptively loosen.** Tolerance deferred; trigger is dx12/metal lanes.
- **"Bit-to-bit" is not literally bit-to-bit (finding 13) — accepted now, revisited with dx12/metal.** dify exempts AA-classified pixels, `egui_kittest` hardcodes that on, and there is no `SnapshotOptions` knob. Bounded to feathered edges (flat interiors are never AA-exempt) and, when it triggers, **no diff image is written and the test passes silently**. **Accepted** on a product-owner call, and not as a grudging loss: the exemption lands on **edge rasterisation, the likeliest mesa/LLVM variance**, so it absorbs precisely the drift that would otherwise cause *false* failures on the single lavapipe lane — that is the reason it was accepted, and it is why the exactness bet is safer than the flat claim suggests. The hole only bites **if edge pixels actually drift**, and there is no evidence either way until cross-platform variance exists — so it is **bundled into the existing dx12/metal Deferred trigger** and revisited *together with* tolerance, not separately. *Mitigation meanwhile:* it is the strictest setting the library can express, so there is nothing to change — a code comment records it (subtask 9) so a future reader does not over-trust the word "exact", and AC9's paper probe covers the flat region the exemption can never reach.
- **`RendererOptions::PREDICTABLE` omitted at `create_render_state`.** The builder default is `PREDICTABLE` but `.renderer(..)` bypasses it (finding 4); omitting it silently restores `dithering: true` ⇒ bit-exactness unattainable, and the failure reads as "exact compare is too strict". *Mitigation:* spelled out in subtask 8 and finding 4; AC9(a)'s exact paper-pixel assertion also fails immediately under dithering, so it fails loudly rather than rotting.
- **`code-writer`'s own contract forbids the `image-check` spawn — the blocker AC16 fixes, and the reason (a) cannot land without (b).** `.claude/agents/code-writer.md`'s `## Invariants (both modes)` — *"These hold in EVERY invocation, regardless of mode"* — says **"Do not spawn `self-review`; do not spawn any other reviewer."** `image-check` returns a PASS/FAIL verdict gating a commit and matches that clause literally. **The failure is silent, which is what makes it severe:** an implementor obeying its own contract simply **skips the spawn**; AC14's invocation half is dropped, the PNG commits unchecked, and **every gate stays green** — subtask 1's agent file exists, so an AC14 spot-check passes. That is quartzite's `SKIP_RENDER_SNAPSHOT` no-op reproduced inside this spec's own remedy. *Root cause worth naming:* round 3 verified **tool capability** (`code-writer` inherits `Agent`) where **instruction permission** was required — the same class as findings 4/5/10/11/13, missed on the harness surface rather than the library one. *Mitigation:* subtask 2 lands AC16 (a)+(b) **together**; subtask 9 depends on it (`1, 2`); Group A runs first so the amended file is in force before Group B spawns.
- **Agent-file hot-reload mid-session — assessed, NOT a hazard.** Group A creates `image-check.md` and edits `code-writer.md`; Group B spawns both **in the same session**. **Documented fact:** *"Claude Code watches `~/.claude/agents/` and `.claude/agents/`. When you **add or edit** a subagent file on disk … Claude Code detects the change **within a few seconds** and the **next delegation uses the updated definition**, with no restart needed."* Both halves are covered — the new file **and** the amended `code-writer.md`. The documented restart caveat applies **only** when the `agents` **directory** did not exist at session start; `.claude/agents/` exists with 9 files, so it does not apply here. **The failure mode is loud, verified empirically** (not from docs): spawning a missing agent type returns `Agent type 'image-check' not found. Available agents: …` — a hard error, never a silent no-op. *Residual, narrow:* (i) a spawn within the watcher's ~few-second window could race — retry; the error is loud. (ii) a session started with `--disable-slash-commands` does not watch these directories at all — not this workflow's mode. *Fallback if it ever fires:* the check runs at the group boundary or in a later session — cheap and local. **No design change is warranted; do not design around this speculatively.**
- **`image-check` drifting into CI.** A file in `.claude/agents/` *looks installed*, which is a new failure mode an inline spawn did not have — CI has no model, so it would be flaky or a silent no-op (quartzite's `SKIP_RENDER_SNAPSHOT` exactly). *Mitigation:* the never-CI boundary is stated in the agent file itself, in AC13's exhaustive gate list, and as an explicit "no `image-check` step" instruction in **both** subtask 12 (`ci.yml`) and subtask 13 (gate list).
- **`image-check` becoming a rubber stamp.** A model shown an image and asked "consistent?" will rationalise whatever is there. *Mitigation:* the file **mandates derive-then-look** — the expectation is written from the drawing code before the PNG is opened — and FAIL requires fixing the code, never re-reading the image more charitably.
- **`image-check` invoked inline instead of by `subagent_type`.** An inline `Agent(model="sonnet", …)` cannot set effort (no per-invocation parameter), silently losing the medium tier and violating AC14. *Mitigation:* subtask 9 says "NO inline `model=`/effort override"; AC11's contributor doc repeats it.
- **Brace expansion is inert in both `.gitignore` and `@actions/glob`** *(round-1 bug, fixed; retained because both sites fail silently)*. Neither errors, and `actionlint` does not validate glob semantics. *Mitigation:* three separate `.gitignore` lines + multiline `path: |`, with `git check-ignore -v` on all four names as subtask 5's acceptance check.
- **Golden minted against the wrong adapter.** *Mitigation:* AC10(b) — under exact compare a hard failure on every other machine rather than something a tolerance absorbs.
- **The backend's font capability goes unexercised by any code until #12** — the spec's verbatim accepted cost. A deliberate product-owner trade. AC2(d)'s `FontDefinitions` citation is the only evidence carried forward.
- **CI system libraries for `eframe`/`winit` — downgraded from round 1's "highest risk".** Finding 12: every winit Linux backend dep is dlopen-based, so `cargo build`/`cargo test` link **without** X11/wayland dev packages. Real exposure is **runtime window creation**, which CI never performs; any failure would be a loud linker error. *Mitigation if it fires:* add apt packages to the existing `Install Vulkan software stack` step (preferred), or trim to `eframe = { version = "0.35", default-features = false, features = ["wgpu", "x11", "default_fonts", "accesskit"] }`. Surface to the orchestrator rather than absorbing a silent feature-trim.
- **Harness paints a mouse-cursor triangle** in `render()` when `pointer.hover_pos()` is `Some` (verified `lib.rs:653-676`). We send no pointer events ⇒ `None`. *Mitigation:* `Harness::remove_cursor()` exists if one appears.
- **Upstream changes `default_wgpu_setup`'s CPU-first sort.** *Mitigation:* AC10(b) turns this into a failing assert.
- **`placeholder.rs` file-size ladder.** ~330 lines incl. tests vs the 800 incl.-test soft limit. *Mitigation:* if it grows, split the GPU suite to `crates/render/tests/placeholder_golden.rs` — but that forces `geometry()` to become `pub`; last resort, record the trade-off.
- **First committed binary asset.** Plain git, no LFS, per the spec. Expect single-digit KB at 192×128; a materially larger PNG means the canvas or `pixels_per_point` was not pinned as designed.
- **`cargo test -p gp-render` now needs a Vulkan ICD.** Accepted by the spec; no skip hatch. Contributors get a loud failure + AC11's instructions.
- **Panic/unsafe surface.** No `unsafe`. No new **production** panics: `draw_placeholder` is total (egui clips out-of-range rects; no indexing, no division by a possibly-zero value — a degenerate `rect` yields an empty draw, not a panic). `gp-core`'s zero-production-panics invariant is untouched — this unit adds no `gp-core` code. All `expect`/`unwrap`/`assert` introduced live in `#[cfg(test)]`, where they are the assertion mechanism. `create_render_state` itself `expect`s on failure (upstream's code, dev-dep, test path only).

## Test Design

**1. AC6 — tessellation smoke (CPU-only, no adapter, no display server)**
- Location: `crates/render/src/placeholder.rs` `#[cfg(test)] mod tests`
- Entry point: `draw_placeholder`, driven through `egui::Context::run_ui` → `Context::tessellate`
- Scenarios: *happy* — a full pass over the placeholder path yields a non-empty `Vec<ClippedPrimitive>` **and** total vertices > 0 and total indices > 0. The vertex/index strengthening is deliberate: a non-empty primitive vector can still carry **zero-geometry meshes**, which is precisely quartzite's actual defect ("widgets that existed but had zero-size geometry") and what AC6-alone would otherwise miss.
- Fixtures: `CANVAS_RECT`; `RawInput { screen_rect: Some(CANVAS_RECT), ..Default::default() }`. No `TrackArtifact`.

**2. AC8 + AC9 + AC10 — golden, guard, adapter (requires a Vulkan software ICD)**
- Location: same `mod tests`, **one** test fn so the guard and the golden provably share one image
- Entry point: `create_render_state` → `WgpuTestRenderer::from_render_state` → `Harness::render` → `try_image_snapshot_options`
- Scenarios, in order:
  - *AC10(b) adapter* — `device_type == DeviceType::Cpu`; on failure the message names lavapipe / `mesa-vulkan-drivers`.
  - *AC9 guard, before the golden* — paper probe exact; paper is the modal colour; hairline probe darker by ≥ the named margin; distinct colours > 1.
  - *AC10(a) + AC8 golden* — `try_image_snapshot_options` with `threshold(0.0)` + `failed_pixel_count_threshold(0)`; on `Err`, panic with the error's **`Display`** (it carries the diff path + the `UPDATE_SNAPSHOTS` hint), not `Debug`.
- Fixtures: `CANVAS_RECT` (192×128) @ ppp 1.0, `Theme::Light`, `layer_painter(LayerId::background())`, `RendererOptions::PREDICTABLE` passed explicitly; probes derived from `geometry(CANVAS_RECT)`; one private `f32 Pos2 → (u32, u32)` helper with the single justified cast `#[allow]`; a `debug_assert_eq!(CANVAS_RECT, ctx.content_rect())` documenting the equality the design relies on.
- Regeneration: `UPDATE_SNAPSHOTS=true cargo test -p gp-render`, followed by the **`image-check` spawn** before the image is committed.

**3. AC14 — `image-check` (a subagent, not a test; never a CI gate)**
- Location: `.claude/agents/image-check.md`; invoked from subtask 9 at mint and from every later regen (AC11).
- Entry point: `subagent_type="image-check"`, no inline model/effort override; inputs = the drawing-code path + the minted-PNG path.
- Scenarios: expectation derived from the code **first**; PNG `Read` **second**; consistency confirmed on paper background, rect and hairline placement, graph-paper motif, and absence of unexplained shapes/colours ⇒ PASS. Mismatch ⇒ FAIL with specifics; caller fixes the code and re-mints.
- Fixtures: none. Verified at review time, as AC2 and AC11 are — all three are deliverables no CI checks.
- **No meta-test.** Per the spec's *"Considered, not applicable"* note, upstream's harness is upstream's to test and our local test-side logic is plain assertions; `image-check` is a prose contract, not code, so it inherits no meta-test obligation either.

**4. `gp-game` (AC3)** — no automated test. There is **no display server on the runner** (verified: no Xvfb, no `DISPLAY`), so a windowed test would fail in CI; the spec defers Xvfb and requires any windowed test to be `#[ignore]`d with a justifying comment. `main.rs` is kept to thin delegation so AGENTS.md's "~50+ lines of substantial logic ⇒ `#[cfg(test)] mod tests`" rule does not bite; AC3 is verified by the human running `cargo run -p gp-game`, and the drawing it shows is the **same** `draw_placeholder` that tests 1 and 2 cover.
- **The implementor must not run it.** A GUI binary never exits, so `cargo run -p gp-game` in a subagent's foreground bash call blocks to timeout. Subtask 11 confirms `cargo build -p gp-game`; the window check surfaces to the orchestrator/human.
- **AC3 must be run from a local desktop session, not this shell** (verified live on this workstation): `DISPLAY=localhost:10.0` with `XDG_SESSION_TYPE=tty` and `SSH_CONNECTION` set — SSH X11 forwarding, over which Vulkan will most likely not negotiate a swapchain. **This does not touch AC8**: `vulkaninfo --summary` enumerates both `AMD Radeon RX 9070 XT (RADV GFX1201)` and `llvmpipe (LLVM 22.1.8, 256 bits)` here, and the golden renders **offscreen** — no display, no swapchain — so minting works fine from this shell. Only the **windowed** path needs a real desktop.
- **A recurring pattern worth naming:** this is the **second** time the design correctly identified something as human-only and then wrote an implementor-facing instruction contradicting it (the first was round 3's eyes-on golden step, since removed). When a step is human-only, the subtask cell must say so — otherwise an implementor dutifully attempts it, and here that means hanging until timeout.

**No font test** — removed by the spec's *Font-proof amendment*.

## Open questions

- **`render_frame`'s missing world→screen transform.** The AC5 signature reaches every §4 layer, but only by *deriving* a fit-to-view mapping from `painter.clip_rect()` + `TrackArtifact` — which pins the policy to auto-fit. If #17/#18 want pan/zoom, they will need an extra parameter or an `Overlays` field. Flagged for #17/#18; **not** a reason to reopen AC5 in this unit.

*(Resolved and closed: round 1's AC4(c) font question — dropped by the product owner at Step 7. Round 1's CI-system-library question — answered by finding 12. Round 2's tolerance plumbing — reversed to exact compare; tolerance deferred to the dx12/metal matrix, not an open question now.)*

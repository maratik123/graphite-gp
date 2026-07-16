# gp-render: choose + scaffold the native Rust GUI backend

**Source:** issue #11
**Date:** 2026-07-16
**Tracked in:** #11

Foundational decision unit for **Block 2 — gp-render** (`docs/design.md` §4 *Рендеринг + UX*, §6 *Архитектура*). The backend is **eframe/egui 0.35**; the window + event loop land in **`gp-game`**, not `gp-render`; and a **wgpu/Vulkan golden-image test harness** (`egui_kittest`) ships with this unit. Build-order position 8/40; the other twelve `block:render` issues (#12–#23) all sit downstream of this pick.

Two product-owner overrides govern this spec — both deliberate, both recorded below so a future reader can see the tradeoffs were made knowingly:

| Override | Round | Decision |
|---|---|---|
| Window/loop ownership: §6 over issue #11's text | 1 | `gp-game` owns the loop; `gp-render` is draw-only |
| Golden-image harness: adopt now, over the spec-writer's "render, no golden" recommendation | 3 | Full golden harness ships in #11 |
| Font proof: drop the capability from AC4 entirely, over the design's "amend AC4(c) to mean registration + family" recommendation | Step 7 | No face loaded in #11; fonts wholly deferred to #12 |
| Golden comparison: **strictest expressible** (bit-exact in flat regions), reversing the spec's own "never an exact compare" tolerance mandate | Step 7 | `threshold(0.0)` + `failed_pixel_count_threshold(0)`; AA-classified edge pixels exempt (library-hardcoded, accepted-for-now); tolerance + AA both deferred to the dx12/metal matrix |
| Golden trustworthiness: human eyeballing rejected; a **model image↔code check** adopted in its place, as a **file-based subagent** with a **durable calling contract** | Step 7 | `.claude/agents/image-check.md` (`model: sonnet`, `effort: medium`), called from a standing rule in `.claude/agents/code-writer.md` + a narrow carve-out to that file's "no other reviewer" invariant (`code-writer` only — `self-review` rejected for sync-group fan-out); mint/regen-time only, never a CI gate |

## ⚠ Ownership override — read before implementing

**Issue #11's text scopes "the window/canvas + main loop" into `gp-render`. The product owner overrode this (round-1 interview, 2026-07-16): the window/event loop lands in `gp-game` instead.**

Rationale: `docs/design.md` §6 assigns block **3b** (`gp-game`) the game loop — *"игровой цикл: ввод игрока, тайминги, оркестрация, UX"* (player input, timing, orchestration, UX). §4 is titled "Рендеринг + **UX**", so both blocks are named for UX and the canonical doc is genuinely ambiguous on the boundary. The override resolves it **in favour of §6**: `gp-render` stays a **draw-only library**, `gp-game` owns window, event loop, input, and timing.

Consequences, which the ACs below encode:

- `gp-render` **must not** depend on `eframe`, `winit`, or `wgpu`. It takes `egui` only.
- `render_frame` receives a **borrowed draw context** (`&egui::Painter`) — it does not own, create, or store one.
- Issue #11's AC "a window + canvas + main loop compile and open" is satisfied **in `gp-game`**. This is a deliberate deviation from the issue text, not an oversight; AC2 requires it be recorded in the written rationale.

## Scope

1. **Wire the backend.** `crates/render/Cargo.toml` takes `egui = "0.35"`, replacing its `TODO(2): pick a rendering backend ...` placeholder comment. `crates/game/Cargo.toml` takes `eframe = "0.35"`.
2. **Write the rationale** in `ai-docs/key-decisions.md` — the pick, the rejected alternatives, and the §6-over-#11 ownership override.
3. **Stand up window + event loop in `gp-game`** (an `eframe::App` impl + `eframe::run_native`) that compiles, opens, and draws the placeholder frame: cleared to the paper background, plus a rectangle and a hairline stroke.
4. **Re-signature `render_frame`** to take `&egui::Painter` as its draw context, retaining `(&TrackArtifact, &[CarState], Overlays)`.
5. **Headless tessellation smoke test in `gp-render`** driving an `egui::Context` through a full pass and asserting tessellated output — no window, no display server, no GPU adapter.
6. **wgpu/Vulkan golden-image harness in `gp-render`** — `egui_kittest = { version = "0.35", features = ["wgpu", "snapshot"] }` as a **dev-dependency**, **one** committed text-free golden PNG of the placeholder frame, an **exact bit-to-bit compare**, a **non-triviality guard** and a **CPU-adapter assertion** (so the golden cannot pass on a degenerate frame or the wrong rasteriser), a **mint-time model image↔code check** via a new `.claude/agents/image-check.md` subagent (so a wrong-from-birth golden cannot be pinned forever), and the `UPDATE_SNAPSHOTS=true` refresh workflow documented for contributors.
8. **Subagent + calling contract + inventory** — a new `.claude/agents/image-check.md` (`model: sonnet`, `effort: medium`); its **durable calling rule added to `.claude/agents/code-writer.md`** (mode-aware, standing, so #17/#18 inherit it) **plus the narrow carve-out to that file's "no other reviewer" invariant without which the rule is unimplementable**; plus the mandatory `ai-docs/claude-tools-hierarchy.md` inventory update in the same PR — **two rows** (new `image-check`, amended `code-writer`).
7. **CI failure diagnostics** — an `if: failure()` upload of golden diff artifacts in `ci.yml` (triggers the `actionlint` gate).

## Out of scope

Every other `block:render` issue owns its own unit; this one must not absorb them:

| Issue | Owns |
|---|---|
| #12 | design tokens → Rust consts (colors, spacing, type, effects) |
| #13 | core widgets — Button, IconButton, Badge, Tag, Card |
| #14 | forms widgets — Slider, Switch, SegmentedControl, Stepper |
| #15 | game HUD widgets — Telemetry, LapMeter, CarChip |
| #16 | MovePad |
| #17 | track canvas — regions/walls/S-F, cars, vectors, trails, move animation |
| #18 | analytics overlays — speed heatmap, fastest-lap line, graph-paper grid+dots |
| #19–#22 | Setup / Track lab / Race / Results screens |
| #23 | app shell — top bar + screen router |

Also out of scope:

- **Rendering a real `TrackArtifact`.** The block-1 generator (`gp-gen`, #24–#34) is `todo!()`, so nothing can produce one at runtime.
- **Real input handling, timing, orchestration.** `gp-game` gets the loop *shell* only; the §3b game loop proper is block 3b's own work.
- **The full type-token port** (#12) — and, per the *Font-proof amendment*, **font loading entirely**: no face is vendored or loaded here, and no code exercises the font path.
- **Cosmetic wall smoothing** (Chaikin, `docs/design.md` §4 + `[M6]`).
- **The §4 layer stack** (outfield/infield/asphalt → walls → S/F → grid → overlays → cars). `render_frame`'s signature must not preclude it; implementing it is #17/#18.

## Deferred

| What | Why | Separate issue needed? |
|---|---|---|
| **Font loading in full** — vendoring the real display/mono faces (Space Grotesk, JetBrains Mono — both flagged as *substitutions* by `docs/design-system/IMPORT.md`) **and the capability proof itself** | Product-owner call at Step 7 (*Font-proof amendment*): the repo has zero font files, so any proof here would either be a non-face stand-in or drag licensing + asset-pipeline into #11 against this table's own intent. **Accepted cost: no code exercises font loading until #12, so a surprise in the backend's font path is found later.** | No — folds into #12, which owns the type tokens |
| Xvfb (or equivalent) in the CI `test` job | Only needed to test `gp-game`'s **windowed** path. AC6's test lives in `gp-render` and needs no display server | No — revisit if a windowed `gp-game` test is ever demanded |
| ~~Exercising the CI Vulkan/llvmpipe env from an automated test~~ | **No longer deferred — ADOPTED into this unit** (round 3). The AC8 golden test renders through wgpu/Vulkan on lavapipe, so the pre-existing env-init is now exercised | No — in scope here |
| Replacing the placeholder golden with real track geometry | The golden's subject is a placeholder; #17 lands the real canvas | No — **#17** owns it (see *#17 follow-up*) |
| wgpu major-version alignment | `eframe 0.35` pins `wgpu ^29.0`; standalone latest is 30.0.0. Only bites if a second, independent wgpu-facing crate lands later | No |
| **Golden comparison strictness — two questions, one trigger:** (1) reconsidering a per-OS `OsThreshold<f32>` / `OsThreshold<usize>` instead of the exact `threshold(0.0)` + `failed_pixel_count_threshold(0)`; (2) **the AA-exemption hole** — `egui_kittest` hardcodes `detect_anti_aliased_pixels = true`, so AA-classified edge pixels differ silently with no diff artifact and no knob to disable it | **Trigger: when dx12 (Windows) and/or metal (macOS) lanes join the CI matrix** — product-owner directive: *"Let think about it when later renderers … is added to ci matrix with their platforms"*, and on the AA finding specifically: **"accept now, revisit with dx12/metal"**. Both are the *same* question (how strict can the compare be?) and both only bite once cross-platform variance exists: exactness is premised on the single Linux/lavapipe lane, and **the AA hole only matters if edge pixels actually drift — there is no evidence either way until a second platform exists**. `OsThreshold`'s `windows`/`macos`/`linux` variants exist precisely for this; quartzite runs exactly such a 3-OS matrix. **The AA exemption is accepted-for-now, NOT settled.** | **No issue yet** — the trigger is a CI-matrix change that does not exist and is not scheduled; filing now would sit stale. The unit that adds the second OS lane owns both questions, and this row is the standing record of why. |

## Key decisions

| Question | Decision |
|---|---|
| Backend | **eframe/egui 0.35** (round-1 interview). `egui` is the draw layer; `eframe` is the window/loop shell. eframe's **default** renderer is wgpu (not glow), matching the CI `WGPU_BACKEND=vulkan` env as-is. |
| Window + main loop owner | **`gp-game`** (round-1 interview) — a deliberate override of issue #11's text in favour of `docs/design.md` §6. See *Ownership override* above. |
| Crate → dependency split | `gp-render`: `egui = "0.35"` + `gp-core`. `gp-game`: `eframe = "0.35"` + existing `gp-core`/`gp-gen`/`gp-render`/`gp-ai`. **`gp-render` gets no `eframe`/`winit`/`wgpu`** — this is what keeps `cargo test -p gp-render` GUI-free *by construction*, not by convention. |
| `render_frame` draw context | A **borrowed** `&egui::Painter` parameter (obtainable from `Ui::painter()` or `Context::layer_painter(LayerId)`). Not an owned `Painter`, not a `Context`, not stored in a struct. |
| `render_frame` body | May stay `todo!()` — the §4 layer stack is #17/#18. |
| Placeholder drawing path | **Design's call** (e.g. a scaffold `draw_placeholder(&egui::Painter)` in `gp-render`, or a temporary `render_frame` body). Two binding constraints: (a) it must **not** require a generated `TrackArtifact` — `gp-gen` is `todo!()`, so `gp-game` cannot build one at runtime; (b) AC6's headless test must be able to drive it. If `render_frame`'s body stays `todo!()`, the placeholder cannot *be* `render_frame` — a `todo!()` body would panic the AC6 test. |
| Where the rationale lives | `ai-docs/key-decisions.md` (documented home for "key design-decision detail bodies"), cross-linked from the `gp-render` crate docs. `docs/design.md` is product-owner-authored canonical spec — a backend pick is an implementation decision and does not amend it. |
| Dependency pinning style | Per-crate `[dependencies]`, matching the house pattern (`enumflags2 = "0.7"` in `crates/core`). `[workspace.dependencies]` carries only the `gp-*` path deps. `0.x` for `0.x.y` ⇒ **`"0.35"`**; no `~`, no patch pin. |
| Font proof | **No face is loaded in this unit.** The backend's custom-font capability is evidenced by **AC2's rationale citing egui's `FontDefinitions` API** — the path AC4's own wording already permits ("*or* AC2's rationale cites the specific `egui` API that does"). Face choice, vendoring, licensing, and the asset pipeline are **wholly #12's** (it owns the type tokens). Product-owner call at Step 7 — see *Font-proof amendment*. |
| wgpu/Vulkan golden-image test harness (`egui_kittest`) | **ADOPTED (round 3, product-owner override of the "no golden yet" recommendation).** `egui_kittest = { version = "0.35", features = ["wgpu", "snapshot"] }` as a **dev-dependency of `gp-render`**. Accepted costs recorded in *Golden-harness override*. |
| Adapter selection / renderer pinning | **Rely on `egui_kittest`'s built-in CPU-first `native_adapter_selector`** — no env pinning, no `.cargo/config.toml`. Verified from source: the default selector sorts `DeviceType::Cpu` first, so lavapipe wins over this workstation's AMD RADV GPU automatically and matches CI. **Do NOT add a `WGPU_ADAPTER_NAME` env pin — `egui_kittest` reads no env vars, so it would be inert cargo-culting.** Tradeoff: adapter choice is the library's policy, not ours; if upstream ever changes that default, goldens shift — **AC10(b)'s CPU-adapter assertion turns that into a loud failure rather than a silent re-golden**, which matters more under exact compare than it would under a tolerance. |
| Golden comparison | **Exact, bit-to-bit** — `threshold(0.0)` + `failed_pixel_count_threshold(0)` (see *Exact-compare amendment*). Premise: llvmpipe is expected stable on the single Linux/lavapipe lane. **Both upstream fallbacks are overridden** — `threshold`'s `0.6` default is *not* bit-exact. A mean-style gate remains wrong *a fortiori* (quartzite's `report.mean > 0.05` on 64×64 cannot be tripped by a 1-px error); `egui_kittest` never used a mean anyway — its gate is a pixel **count**. **quartzite's `FLIP_TOLERANCE = 0.05` stays rejected** as unvalidated precedent. **Bound (verified in `dify` source):** `egui_kittest` hardcodes `detect_anti_aliased_pixels = true` with no `SnapshotOptions` knob, so **AA-classified edge pixels can differ silently, with no diff artifact** — the real guarantee is *bit-exact in flat regions*, not "any differing pixel fails". `threshold(0.0)` + `failed_pixel_count_threshold(0)` remains **the strictest the library can express**; the exemption is a library property, not our choice — and it makes the bet **safer**, since edge rasterisation is the likeliest mesa/LLVM variance source. **Accepted risk:** a mesa/LLVM upgrade could break flat-region exactness — acceptable because the frame is text-free flat fills + hairlines at `pixels_per_point = 1.0` with `RendererOptions::PREDICTABLE` (no MSAA, `dithering: false`, software texture filtering), and flat-region drift fails loudly. **Accepted for now, not settled** — revisited with the AA question at the dx12/metal trigger. **If it bites, revisit — do not pre-emptively loosen.** |
| Model image↔code check: file-based agent vs inline spawn | **File-based — `.claude/agents/image-check.md`, `model: sonnet` + `effort: medium`, `tools` omitted (inherit-all, so `Read` can render the PNG).** An inline `Agent(model="sonnet", …)` **cannot** enforce the tier: there is no per-invocation `effort` parameter, so frontmatter is the only lever — the rationale `.claude/agents/code-writer.md` already records, and it is the only other effort-pinned agent in the house set. Name avoids the `review` token deliberately (that token denotes the Review sync group here). Cost: a definition file reads as "installed", so the never-a-CI-gate boundary is restated in AC14 and in the *Addendum*. |
| Is `image-check` a "reviewer" under `code-writer`'s invariant? | **No — it is an artifact-validity check, and AC16(b) carves it out explicitly.** The invariant forbids spawning a reviewer because the orchestrator must review *the writer's work* before it is committed. `image-check` judges **the artifact against the code that generated it** — the same category as `cargo test`, which `code-writer` already runs freely — never the quality of the work. `self-review` of the whole diff stays with the orchestrator, so the invariant's stated rationale is untouched. **The boundary is written into the file, not left implied**, so a future subagent can be placed without re-deriving this: *artifact vs. work* is the test. |
| `image-check` calling contract: extend AC14 or its own AC? | **Its own — AC16.** AC14 is already dense (agent file + frontmatter + contract + invocation + never-CI-gate), and this is a **distinct deliverable file** (`code-writer.md`, not `image-check.md`) with a **distinct rationale** — surviving past this PR — which is easy to lose inside AC14's prose. Direct precedent in this spec: propagation became **AC15** rather than an AC14 clause for exactly that reason. The two are separable concerns: AC14 is *the agent exists*; AC16 is *something durably calls it*. |
| Model image↔code check: AC or Key-decision-only? | **An AC (AC14)**, deliberately. The directive says the model "can be used **for ACs**"; it is a real deliverable obligation, and un-AC'd process gets skipped. The counter-argument — *"every AC here is machine-checkable, and a model check isn't, so the AC table implies a CI gate"* — **does not hold for this spec**: **AC2** (a written rationale that "ties eframe/egui to the aesthetic") and **AC11** (contributor workflow "is documented") are already prose deliverables verified at **review** time, not by CI. AC14 joins that established class. The CI-gate risk is handled by wording instead: AC14 says so twice, and **AC13's gate list is explicitly exhaustive and excludes it**. |
| Non-triviality guard on the golden | **Required (AC9).** The golden is paired with cheap, explicit, non-golden pixel assertions: paper background is `#F5F1E6`, a hairline pixel is measurably darker, and the frame is **not uniform** (>1 distinct colour). Purpose, stated explicitly: quartzite's CI-enforced goldens are 5 byte-identical all-black squares that would pass with the renderer deleted — **a golden alone cannot tell a reviewer "correct" from "empty"**. This is a guard *on* the golden, additive; it does not narrow or replace it. |
| AC6 (tessellation) vs AC9 (pixel guard) — no redundancy | Distinct, non-overlapping jobs. **AC6** is the GPU-free fast check that the draw path *emits shapes at all* (runs on any machine, no ICD). **AC9** asserts the *rendered pixels* are non-degenerate on the very image the golden compares. They catch failure at different layers: quartzite's actual defect was widgets that existed but had zero-size geometry — shapes may still be emitted while pixels go uniform, so AC6 alone would not have caught it. |
| Assert the resolved adapter is a CPU/software device | **Yes — adopt (AC10).** Verified implementable: `egui_kittest::wgpu::create_render_state(default_wgpu_setup(), ..) -> egui_wgpu::RenderState`, whose **public `adapter` field** exposes `get_info().device_type`; feed the checked state back via `WgpuTestRenderer::from_render_state(..)` + `HarnessBuilder::renderer(..)`. Rationale: this converts our reliance on a library default from an *assumption* into an *asserted invariant*, and it is exactly the failure quartzite suffers — no adapter check anywhere, so a contributor silently goldens against a discrete GPU. It also makes my recorded residual risk (upstream changes the CPU-first sort) **fail loudly instead of silently**. Cost: may need `wgpu`/`egui-wgpu` as an explicit dev-dep to name `DeviceType` — dev-deps are already exempt under AC7. |
| Contributor `SKIP_*` escape hatch | **No.** `egui_kittest` honours exactly one env var — `UPDATE_SNAPSHOTS` (verified in source; no skip var exists) — so a hatch would be hand-rolled, and a hatch that *silently passes* is precisely how quartzite's suite rotted (`SKIP_RENDER_SNAPSHOT: "1"` in its generic lane under a now-false comment). A contributor without an ICD gets a loud failure plus the AC11 documentation telling them to install lavapipe. CI always has the ICD. |
| Golden canvas size | **192×128 logical px at `pixels_per_point = 1.0`** (`HarnessBuilder::with_size` + `with_pixels_per_point`), design may adjust with reasoning. Tradeoff named: quartzite's 64×64 keeps PNGs tiny (mean 787 B) but leaves too few pixels to place the paper / rect / hairline in **distinct, separately-probeable regions**. 192×128 gives AC9's probes and AC14's model check enough frame to distinguish those elements, while staying ~1–3 KB. **Pinning `pixels_per_point` is load-bearing** — an unpinned DPI changes the raster and churns the golden. |
| Does the golden contain rendered text? | **No — the golden is text-free** (paper + rect + hairline only). Text shaping is the #1 golden-churn source and is fully backend-agnostic: quartzite records `R7: Snapshot golden drift across parley minor versions` and absorbed real drift by regenerating (commit `99099ae`, "minor GPU sub-pixel rendering variance … ≤16 px difference"); egui's text stack will do the same to us. Upstream also says images should *"depict exactly what's tested and nothing else"*, and the golden's job here is the paper/lattice motif. **Now over-determined:** with the font capability out of AC4 entirely (see *Font-proof amendment*), this unit renders no text at all, so there is nothing for the golden to depict even if churn were free. Highest-churn, lowest-signal element stays out of the image. |
| Golden budget + helper placement | This unit commits **exactly one** golden. `gp-render` is the only golden consumer today, and #17/#18 will render from the **same crate** — so a shared helper crate is premature; any helper stays a private `mod` under `gp-render`'s tests. Only if a **second crate** needs it does AGENTS.md's ≥3-call-site rule fire, and that placement call is the `design` Subagent's (per spec-writer's contract). Recorded because quartzite grew 5 → 65 goldens in ~3 weeks with no budget rule and then copy-pasted a 272-line helper across two crates. |
| Image storage | **Plain git commit — no git LFS, no `.gitattributes`.** git-lfs is not installed and no `.gitattributes` exists here; upstream's LFS advice targets a large corpus, whereas this unit commits **one low-resolution PNG** (keep the harness canvas small per upstream's "low resolution" guidance; expect single-digit KB). Revisit only if the golden corpus grows past a handful of images. |
| Golden authority | **CI is authoritative.** If a local `UPDATE_SNAPSHOTS=true` run and CI disagree, the CI image wins — it is the only environment guaranteed to have lavapipe as the sole rasteriser. |
| Keep the CPU tessellation test alongside the golden? | **Yes — both.** The tessellation test (AC6) needs no GPU adapter and localises failures to the draw path; the golden (AC8) covers pixels. This is additive, not a dilution of the golden decision, and matches upstream's "prefer regular Rust tests where they suffice" guidance without displacing the golden. |

## Technical constraints

**Verified against the live crate registry + docs, 2026-07-16 (PROC-1):**

- Live `max_stable_version`: **eframe 0.35.0**, **egui 0.35.0**, macroquad 0.4.15, wgpu 30.0.0, winit 0.30.13, vello 0.9.0, lyon 1.0.19, tiny-skia 0.12.0, kurbo 0.13.1.
- MSRV fits: eframe/egui 0.35 → **1.92** ≤ workspace `rust-version = 1.97.0`.
- **`eframe 0.35.0` default features** = `accesskit, default_fonts, wayland, web_screen_reader, wgpu, winit/default, x11` → **wgpu is the default renderer** (`glow` is opt-in), and it pins **`wgpu ^29.0`** (not the standalone 30.0.0).
- **`egui 0.35.0` normal deps** = `accesskit, ahash, bitflags, emath, epaint, itertools, log, nohash-hasher, profiling, smallvec, unicode-segmentation` — **no `winit`, no `wgpu`, no `glow`, no windowing, no GPU**. Default features = `default_fonts` only. This is the mechanical basis for the GUI-free `gp-render` claim.
- **egui 0.35 `Context` API** (note: there is **no** `Context::run` in this version):
  - `pub fn run_ui(&self, new_input: RawInput, run_ui: impl FnMut(&mut Ui)) -> FullOutput`
  - `pub fn begin_pass(&self, new_input: RawInput)` / `pub fn end_pass(&self) -> FullOutput`
  - `pub fn layer_painter(&self, layer_id: LayerId) -> Painter`
  - `pub fn tessellate(&self, shapes: Vec<ClippedShape>, pixels_per_point: f32) -> Vec<ClippedPrimitive>`
- **egui 0.35 `Painter` primitives**: `rect_filled`, `rect_stroke`, `line_segment`, `hline`, `circle_filled` cover AC4's crisp-shape / hairline / graph-paper capabilities; `text` exists but is **not exercised by this unit** (see *Font-proof amendment*) and is noted for #17/#18.
- Dep-graph state (`grep -r --include=Cargo.toml` + `cargo tree --invert`): none of eframe/egui/macroquad/wgpu/winit/vello/lyon/tiny-skia resolve in the graph today. `crates/render` declares exactly one dependency — `gp-core` — plus the `TODO(2)` comment.

**CI (`.github/workflows/ci.yml`, `test` job):**

- Sets `WGPU_BACKEND: vulkan`, `WGPU_ADAPTER_NAME: llvmpipe`, `LIBGL_ALWAYS_SOFTWARE: "1"`; installs `mesa-vulkan-drivers` (lavapipe) + `vulkan-tools`; gates on `vulkaninfo --summary`.
- **There is no display server** — no Xvfb, no `DISPLAY`. Any test that opens a real window fails on the runner. AC6 is therefore satisfied **in `gp-render`**, CPU-only; any `gp-game` windowed test must be `#[ignore]`d with a justifying comment naming the display-server gap.
- Editing `ci.yml` triggers the **`actionlint` gate** (AGENTS.md AXIOM) before `git add`. **This unit DOES modify `ci.yml`** (AC12, the `if: failure()` golden-artifact upload) — so `actionlint` is a required gate here, not a hypothetical.

**Workspace:**

- Lint posture applies to the new code: `clippy::pedantic` + `clippy::nursery` = `deny`, `missing_docs` = `deny`, `broken_intra_doc_links` = `deny`, `clippy::arithmetic_side_effects` = `deny`. Screen-space coordinate math must satisfy `arithmetic_side_effects` (explicit-semantics ops, or a justified + test-covered `#[allow(..., reason = "...")]`).
- The integer-only rule (`docs/design.md` §3a) binds the physics crate only; `gp-render`'s screen-space mapping is not constrained by it.
- Files with ~50+ lines of substantial logic need a `#[cfg(test)] mod tests` block. File size: soft 500/800, hard 1000/1500.
- `gp-game` is the only binary (`[[bin]] name = "graphite-gp"`) and already depends on `gp-render`. `TrackArtifact`'s fields are all `pub`, so a hand-filled fixture is constructible from `gp-render` — but no *generated* artifact exists while `gp-gen` is `todo!()`.

**Design-system visual requirements (`docs/design-system/`, a spec to port — the web code is never compiled or run here):**

- Flat vector geometry on a lattice; **hairline** pencil borders; crisp radii; no photographic imagery.
- Palette source of truth `tokens/colors.css`: paper base `--paper-1 #F5F1E6`, brightest `--paper-0 #FBF8F0`, infield tint `--paper-2 #ECE6D6`; graphite ink `--graphite-900 #201E1A` → hairline `--graphite-300 #C4BBAA`; ruling `--grid-line #C3CEDD` / `--grid-line-major #A9B8CC` / `--grid-dot #93A2B8`; asphalt `--asphalt-1 #5E594F`; `--wall #201E1A`.
- Fonts are flagged **substitutions**: Space Grotesk (display/UI) + JetBrains Mono (telemetry), currently declared via a Google Fonts `@import` — a native GUI needs vendored or system faces instead.
- Downstream widget units (#13–#16) cover **13 components** across **4 screens** (#19–#22). The eframe/egui pick makes those *token-styling* tasks on an existing widget layer rather than from-scratch toolkit construction — the decisive cost argument, and AC2 must record it.

## ⚠ Golden-harness override — ADOPTED (rounds 2–3)

The product owner countered the round-2 *Deferred* finding (the CI `WGPU_BACKEND=vulkan` / llvmpipe env is exercised by nothing under a CPU-only smoke test) with: *"what about add to test harness wgpu/vulkan rendering, so tests can render and checks against golden images?"*

The spec-writer evaluated three options and recommended **B** ("render through wgpu, but no committed golden until #17 has real geometry"). **The product owner chose A — the full golden harness, now — overriding that recommendation (round 3).** That call is settled and is not reopened here. What follows records the mechanism and the **knowingly accepted costs**, so design and future readers see the tradeoff rather than rediscovering it.

### Accepted costs (decided with this evidence in hand, not in ignorance of it)

- **Upstream advises against this technique where avoidable.** Verbatim from `egui_kittest`'s own docs: *"Whenever possible prefer regular Rust tests or insta snapshot tests over image comparison tests because… they can be relatively slow to run… they are brittle since unrelated side effects (like a change in color) can cause the test to fail… images take up repo space."* **Accepted.**
- **The golden depicts a placeholder that #17 will delete.** `render_frame`'s body may stay `todo!()` and `gp-gen` is `todo!()`, so the golden pins a paper fill + one rect + one hairline — against upstream's *"images should depict exactly what's tested and nothing else."* **Accepted**; see *#17 follow-up* below, which plans the churn instead of being surprised by it.
- **Repo firsts.** This is the repo's **first committed binary asset** (zero PNG/font/image files today) and the workspace's **first dev-dependency** (zero today). **Accepted.**
- **`cargo test -p gp-render` stops being pure-CPU.** It now requires a working Vulkan ICD on the machine, unlike the existing integer-only, environment-independent suite. **Accepted.**
- **The round-1 "headless by construction" promise is partially downgraded.** `gp-render` still *ships* GUI-free **by construction** (dev-deps do not propagate to consumers or to `cargo build` — AC7 enforces this on *normal* edges). But *"`cargo test -p gp-render` needs no GPU"* is now **by convention**, held only by which tests we choose to write. **Accepted, and stated plainly rather than papered over.**

### Verified mechanism (live, 2026-07-16 — nothing from memory)

**The tooling exists and fits:**

**The tooling exists and fits — verified:**

| Fact | Verified value |
|---|---|
| Crate (canonical name) | **`egui_kittest` 0.35.0** — *"Testing library for egui based on kittest and AccessKit"*, owners `emilk` / `rerunio`. Not yanked. `egui-kittest` is the same crate (registry hyphen/underscore normalisation). |
| egui compatibility | Depends on **`egui ^0.35.0`** (non-optional) — an exact match for our pick. |
| MSRV | **1.92** ≤ workspace `rust-version = 1.97.0`. |
| Default features | **none** (no `default` key) — every capability is opt-in. |
| `wgpu` feature | `dep:egui-wgpu, dep:pollster, dep:image, dep:wgpu, eframe?/wgpu` → exposes `Harness::render(&mut self) -> Result<RgbaImage, String>` (offscreen render, no window). |
| `snapshot` feature | `dep:dify, dep:image, dep:open, dep:tempfile, image/png` → golden compare via **`dify`** (image-diff), plus `Harness::snapshot(name)` / `try_snapshot` / `snapshot_options`. |
| wgpu version alignment | egui_kittest pins **`wgpu ^29.0`** — the *same* major `eframe 0.35` pins. **No duplicate-wgpu hazard.** |
| Harness entry point | `Harness::new_ui(app: impl FnMut(&mut Ui))` — drives a `&egui::Painter` directly, so it targets `render_frame`'s AC5 shape with no adapter. |
| Golden update workflow | `UPDATE_SNAPSHOTS=true cargo test`; images land in `tests/snapshots/`; upstream tells you to gitignore `**/tests/snapshots/**/*.diff.png` + `*.new.png`. Config via `kittest.toml`. |

**Adapter selection — `egui_kittest` already solves the local-vs-CI divergence itself.** This is the round-3 hazard (this workstation resolves **both** `AMD Radeon RX 9070 XT (RADV GFX1201)` and `llvmpipe (LLVM 22.1.8, 256 bits)`; an unpinned run picking the AMD GPU would diff against CI's llvmpipe). Read from the **source** of `egui_kittest::wgpu::default_wgpu_setup()` (0.35.0):

```rust
pub fn default_wgpu_setup() -> egui_wgpu::WgpuSetup {
    // No display handle needed for headless testing — we don't present to a window.
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends.remove(wgpu::Backends::BROWSER_WEBGPU);
    // Prefer software rasterizers.
    setup.native_adapter_selector = Some(Arc::new(|adapters, _surface| {
        adapters.sort_by_key(|a| match a.get_info().backend {
            wgpu::Backend::Metal => 0, wgpu::Backend::Vulkan => 1, wgpu::Backend::Dx12 => 2,
            wgpu::Backend::Gl => 4, wgpu::Backend::BrowserWebGpu => 6, wgpu::Backend::Noop => 7,
        });
        // Prefer CPU adapters, otherwise if we can't, prefer discrete GPU over integrated GPU.
        adapters.sort_by_key(|a| match a.get_info().device_type {
            wgpu::DeviceType::Cpu => 0, // CPU is the best for our purposes!
            wgpu::DeviceType::DiscreteGpu => 1,
            wgpu::DeviceType::Other | wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::VirtualGpu => 2,
        });
        ...
```

Consequences — these **decide** the mechanism rather than merely noting the risk:

- The default selector sorts **`DeviceType::Cpu` first** (*"CPU is the best for our purposes!"*), with `Vulkan` preferred among backends. lavapipe reports `Cpu`; the AMD RADV GPU reports `DiscreteGpu`. **So `egui_kittest` picks lavapipe on this workstation automatically, matching CI — with no configuration.**
- **`egui_kittest::wgpu` contains no environment-variable handling whatsoever** (no `from_env`, no `WGPU_*`, no `var()`), and `egui_wgpu::WgpuSetupCreateNew` documents none either. Therefore **a `.cargo/config.toml` `[env]` pin of `WGPU_ADAPTER_NAME=llvmpipe` would be inert** — it must not be added, and the CI job's existing `WGPU_ADAPTER_NAME=llvmpipe` env var does **not** drive these tests. What actually matters in CI is the **`mesa-vulkan-drivers` ICD install**, which is already there.
- **Residual risk, mitigated not eliminated:** on a machine with **no** software ICD installed, the selector falls back to a real GPU, so a local `UPDATE_SNAPSHOTS=true` could regenerate the golden against the wrong rasteriser. Handled by **AC10(b)'s CPU-adapter assertion** (a hard failure, not a silent re-golden), the AC11 contributor documentation, and treating **CI as the authority** for golden content.
- **mesa/LLVM version drift** on `ubuntu-latest` remains the live risk under exact compare — see the *Exact-compare amendment* for why it is accepted, how flat-region drift surfaces loudly, and why AA-classified edge drift does not. Upstream's general caution, verbatim: *"especially when you're using custom rendering, you may observe images difference with different setups leading to unexpected test failures… Generally you should carefully enforcing the same set of features for all test runs."* — which is exactly what the single-lane, `PREDICTABLE`, CPU-adapter-asserted setup does.

**Comparison API — verified surface (`egui_kittest` 0.35.0):**

```rust
// egui_kittest::SnapshotOptions
pub fn new() -> Self
pub fn threshold(self, threshold: impl Into<OsThreshold<f32>>) -> Self
pub fn failed_pixel_count_threshold(self, failed_pixel_count_threshold: impl Into<OsThreshold<usize>>) -> Self
pub fn output_path(self, output_path: impl Into<PathBuf>) -> Self

// egui_kittest::OsThreshold<T> — pub windows: T, pub macos: T, pub linux: T, pub fallback: T
pub fn new(same: T) -> Self
pub fn windows(self, threshold: T) -> Self
pub fn macos(self, threshold: T) -> Self
pub fn linux(self, threshold: T) -> Self

// egui_kittest::Harness
pub fn new_ui(app: impl FnMut(&mut Ui)) -> Self
pub fn snapshot(&mut self, name: impl Into<String>)
pub fn try_snapshot(&mut self, name: impl Into<String>) -> SnapshotResult
pub fn snapshot_options(&mut self, name: impl Into<String>, options: &SnapshotOptions)
pub fn render(&mut self) -> Result<RgbaImage, String>
```

Both knobs are `OsThreshold`-typed: `threshold` (`f32`, per-pixel `dify` colour delta — **fallback `0.6`**) and `failed_pixel_count_threshold` (`usize`, how many pixels may exceed it — **fallback `0`**). `OsThreshold::default` reads `kittest.toml`. **This unit sets both to zero for an exact compare**; the `OsThreshold` per-OS variants (`windows`/`macos`/`linux`) are unused here and exist precisely for the deferred multi-OS matrix.

**`cargo tree` and the AC7 scoping:** verified empirically on a scratch crate — `cargo tree` **includes dev-deps by default** (they appear under a `[dev-dependencies]` header) and `--edges no-dev` suppresses them. The golden harness therefore *would* make a naive `cargo tree -p gp-render` show wgpu; AC7's `--edges no-dev` scoping is what keeps the "ships GUI-free" claim both true and checkable.

**Binary-asset policy:** upstream notes *"egui uses git LFS files for this purpose"*, but **git-lfs is not installed and no `.gitattributes` exists** in this repo. Decision in *Key decisions*: **plain git commit, no LFS** — see that table for size expectation and rationale.

**`ci.yml` — correction to the round-4 claim.** Round 4 stated "no `ci.yml` change is needed". **That was wrong and is retracted.** Two parts:

- *Still true:* the `test` job already installs `mesa-vulkan-drivers` + `vulkan-tools` and already runs `vulkaninfo --summary` (line 102), so **no Vulkan/diagnostic step is needed** — quartzite's `vulkaninfo --summary || true` step has no analogue to add. Offscreen wgpu on lavapipe needs no display server, so this does not reopen the DISPLAY problem. The pre-existing Vulkan env-init is now genuinely exercised — the product owner's original objection is resolved.
- *Newly required:* the repo has **no `upload-artifact` step at all** (verified: `grep -n 'upload-artifact\|if: failure' .github/workflows/ci.yml` → no match). Without one, a CI golden failure is unreadable — the reviewer sees "images differ" and has no pixels. **AC12 adds an `if: failure()` artifact upload**, so `ci.yml` **is** modified by this unit ⇒ the **AGENTS.md `actionlint` AXIOM fires: `actionlint .github/workflows/ci.yml` MUST pass before `git add`.**

## ⚠ Exact-compare amendment — bit-to-bit golden, tolerance DEFERRED (Step 7)

**This reverses the spec's earlier "a tolerance is mandatory / never an exact compare" position.** Product-owner directive:

> *"golden visual inspection is not driveable via llm. Just bit-to-bit image comparison. It is easy for CI. I'm expecting that llvmpipe outcome is stable. Let think about it when later renderers (like directx12 or metal) is added to ci matrix with their platforms (macos/win)"*

**Decision:** the golden is compared at **the strictest setting `egui_kittest` can express** — `threshold(0.0)` + `failed_pixel_count_threshold(0)`, i.e. **bit-exact in flat regions**. (It is *not* literal bit-to-bit: the library hardcodes an anti-aliasing exemption we cannot disable — see the AA bound below. The directive's intent — just compare the pixels, no tolerance judgement call — is honoured as far as the library permits.) The premise is that **llvmpipe output is stable** on the single Linux/lavapipe CI lane. The tolerance question — **and the AA exemption alongside it** — is **deferred, not deleted**: both reopen when dx12 (Windows) / metal (macOS) lanes join the CI matrix.

### Addendum — model-driven image↔code consistency check (same directive, refined)

> *"model (sonnet) can be used for ACs to see whether image consistent with code (for example, code draws red circle, but rendered image contains black triangle)"*

This **refines, and does not reverse, the "not driveable via llm" position.** The distinction that governs the spec: **human eyeballing as a workflow step is rejected and stays rejected**; **a model verifying that the image is semantically consistent with the drawing code is adopted**. The model check *replaces* the inspection a human would otherwise have done — same job, reproducible on demand, driveable by the `/task` flow.

**Three mechanisms now guard the golden, each catching a class the others cannot:**

| Mechanism | Catches | Where it runs |
|---|---|---|
| Exact bit-to-bit compare (AC10a) | **Regressions** — drift from the minted pixels **in flat regions**; AA-classified edge pixels are exempt (see the AA bound) | CI gate |
| AC9 pixel probes | **Degeneracy** — the quartzite all-black failure | CI gate |
| **Model image↔code check (AC14)** | **A golden that was wrong from birth** | **Mint/regen time — NOT a CI gate** |

The third closes a real gap neither of the others covers: **a black triangle minted as the golden compares bit-exact against itself forever**, and would pass AC9's three probes if the probe pixels happened to land plausibly. Exactness proves *"identical to what we minted"*, never *"what we minted was right"*. AC9 proves *"not degenerate"*, never *"the right shapes in the right places"*. Only a check that sees the **whole frame** and reads the **drawing code** catches the directive's example — code draws a red circle, image contains a black triangle.

**It MUST NOT become a CI gate.** It is non-deterministic and needs a model in the loop; CI has neither. Forcing it into CI would make it either flaky or a silent no-op — the exact anti-pattern this spec exists to prevent (quartzite's `SKIP_RENDER_SNAPSHOT` rotted precisely that way). Its non-determinism is *why* it sits at mint time, where a human would otherwise have eyeballed. **AC9 is not weakened by it**: the model check runs once at mint, AC9 runs on every CI push. Complements, not substitutes.

**Mechanism — a real file-based subagent, not an inline spawn** (product-owner directive, same decision refined):

> *"need to add file agent to .claude/agents for image testing (frotmatter with model is sonnet, effort is medium)."*

- **`.claude/agents/image-check.md`**, project-scoped, frontmatter `model: sonnet` + `effort: medium`, `tools` omitted → inherit-all.
- **Why a file and not `Agent(model="sonnet", …)`:** there is **no per-invocation `effort` parameter** on the Agent/Task tool, so an inline spawn cannot enforce the medium tier — **frontmatter is the only lever**. This is not a new finding; `.claude/agents/code-writer.md` already records exactly this rationale and is the house precedent (verified: it is the **only** one of the nine existing agents with a pinned `effort`; the other eight are `model: opus` with none).
- **Why `tools` is omitted:** the check must `Read` the PNG, and the `Read` tool renders images. Restricting `tools` risks breaking that; inherit-all matches `code-writer`'s pattern.
- **Why the name `image-check`:** role-descriptive kebab-case per the house set (`code-writer`, `spec-writer`, `triage-runner`, `design-review`), and it deliberately **avoids the `review` token** — in this repo `review` denotes the Review sync group (`project-review` / `review-findings` / `self-review`), so `image-review` would falsely imply sync-group membership and propagation obligations this agent does not have.
- **A definition file in `.claude/agents/` does NOT mean "runs in CI".** A file-based agent looks more *installed* than an inline spawn, so the boundary is restated here: `image-check` is spawned at **mint/regen time only**. CI has no model and never invokes it; **AC13's gate list is exhaustive and names AC14 as deliberately absent.**
- **Propagation is mandatory** — both agent-file edits change a Subagent contract, so `ai-docs/claude-tools-hierarchy.md` is updated in the same PR, **two rows** (AC15).

**Who calls it — the durability gap, and why `code-writer` owns it.** Product-owner question: *"who in project harness will call image-check? I think about code-writers and code-checkers"*. The honest answer was **nobody durable**. A design document is a **one-shot artifact**: an instruction there to spawn `image-check` at the mint subtask dies at merge. Specs are per-task too — so AC11's "a regen isn't complete until the check passes" would not reach **#17** when it re-points the golden at real track geometry. **The calling contract must live in an agent file or it evaporates**, and the check quietly stops happening — which is *exactly* how quartzite's suite rotted (a skip that silently passes, a comment that stops being true). Hence **AC16**: the rule goes in `.claude/agents/code-writer.md`.

**Decided: `code-writer` only.** Rationale, verified:

- **Zero propagation fan-out.** `code-writer` appears **nowhere in AGENTS.md** — not in any of the five sync groups (Review / Interview / Triage / Task-Design / Learning-Log). Wiring the rule into `self-review` instead ("code-checkers") was **rejected**: `self-review.md` *is* in the **Review sync group**, so editing it would drag `.claude/skills/project-review/SKILL.md` **and** `.claude/agents/review-findings.md` into this PR — re-introducing the very propagation burden the `image-check` **name** was chosen to avoid.
- **One edit, full coverage.** Both modes live in that one file, so `/bugfix` Step 5 and `/pr-commented` / `/pr-ci-failed` / `/main-ci-failed` Step 4 — all of which delegate code-writing to `code-writer` in Mode B — **inherit the rule for free**.
- **The writer is the only actor that knows a golden was minted.** A reviewer sees a PNG in a diff and cannot tell whether it is new, regenerated, or untouched; the writer just wrote it. Ownership sits with the actor that has the information.
- Leaving it design-only was **rejected** — that is the gap itself.

**The invariant carve-out — why it is principled, not a hole punched to unblock a task.** Design-review round 2 caught that AC16(a) was **unimplementable as first written**: `code-writer.md`'s `## Invariants (both modes)` — *"These hold in EVERY invocation, regardless of mode"* — says **"Do not spawn `self-review`; do not spawn any other reviewer."** `image-check` returns a PASS/FAIL verdict gating a commit, which matches that clause literally. Design-review named the class exactly: the design verified **tool capability** (`code-writer` inherits `Agent`) where **instruction permission** was required.

**The failure mode was silent, which is why this was a blocker and not a nit.** An implementor obeying its own contract **skips the spawn**; AC14's invocation half is dropped; the PNG is committed unchecked; and **every gate stays green** — the agent file exists, so an AC14 spot-check passes. That is the quartzite `SKIP_RENDER_SNAPSHOT` silent-no-op **reproduced inside this spec's own remedy**. The other branch is no better: obey the design, violate a written invariant.

**The principled basis — what is being judged.** `image-check` is **not a reviewer; it is an artifact-validity check.** It does not judge the *quality of the writer's work* — which is precisely what the invariant protects. It verifies that a **generated binary artifact matches the code that generated it**. That is a **test**, not a review; it is the same category as `cargo test`, which `code-writer` already runs freely. The invariant's own stated rationale — *"The orchestrator owns self-review — it must be able to review the work before it is committed/pushed"* — is **untouched**: the orchestrator still owns `self-review` of the whole diff. Nothing about who reviews the *work* changes.

**Rejected: keep the invariant absolute; have the orchestrator spawn `image-check` at the group boundary.** Two defects. (1) **Mode A commits per subtask**, so the PNG would **already be committed** by the group boundary — the check would gate a *fix-up*, not the commit, which is not the contract AC16 needs. (2) It would not survive to #17 without editing `/task` SKILL.md, firing the **Task/Design sync group** (`design.md` + `design-review.md` + `context-reset/SKILL.md`) — a **4-file fan-out** against the carve-out's **1 file**. Also rejected: dropping AC14/AC16 (that is the gap).

**Verified — the carve-out is surgical.** The sentence *"Never runs self-review, never pushes."* appears in **three** places (the `claude-tools-hierarchy.md` row, `code-writer.md`'s own frontmatter `description`, and Mode A step 4), and Mode B says *"the orchestrator owns self-review and the single commit/push"*. **All four remain true** — every one names `self-review` specifically, not "spawns nothing". Only the single `## Invariants` clause needs the carve-out; no other line in the file or the inventory becomes stale.

**Mechanism — verified from `egui_kittest` 0.35 source, not assumed:**

```rust
// egui_kittest::snapshot — the comparison, verbatim:
let threshold = if mode == Mode::UpdateAll { 0.0 /* Produce diff for any error, however small */ }
                else { *threshold };
let result = dify::diff::get_results(previous, new.clone(), threshold, true, None, &None, &None);
let Some((num_wrong_pixels, diff_image)) = result else { return Ok(()) }; // below threshold
let below_threshold = num_wrong_pixels as i64 <= *failed_pixel_count_threshold as i64;
```

- **`threshold(0.0)` + `failed_pixel_count_threshold(0)` is the strictest comparison the library can express.** `threshold` is a *per-pixel* `dify` colour-distance cut, not a mean; `failed_pixel_count_threshold` gates *how many* pixels may exceed it. Zero on both ⇒ **any single differing pixel in a flat region fails** — see the AA bound below for the exception.
- **`threshold(0.0)` is safe, for two independent structural reasons** (verified in `dify` 0.8 source, not inferred): (1) `dify` short-circuits `if left_pixel == right_pixel { DiffResult::Identical }` **before any threshold math**, and (2) the cut is `if delta.abs() > threshold` — a **strict `>`**, so a zero delta can never trip a zero threshold. The `>=`-vs-`>` failure mode that would make `0.0` reject every pixel **cannot structurally arise**. It is also upstream's own idiom: `egui_kittest` passes `0.0` under the comment *"Produce diff for any error, however small"* when regenerating.

**⚠ The one real bound — anti-aliased edge pixels are exempt, and we cannot turn it off.** Verified from source:

```rust
// dify::diff::get_results — classification:
if delta.abs() > threshold {
    if detect_anti_aliased_pixels && (antialiased(&left, ..) || antialiased(&right, ..)) {
        DiffResult::AntiAliased(x, y)      // → YELLOW_PIXEL, diffs NOT incremented
    } else { DiffResult::Different(x, y) } // → RED_PIXEL,    diffs += 1
}
// …and the return:
if diffs > 0 || blend_factor_of_unchanged_pixels.is_some() { Some((diffs, output_image)) } // else → None
```

- **`egui_kittest` hardcodes `detect_anti_aliased_pixels = true`** — it is the literal 4th positional argument of its `get_results(previous, new, threshold, true, None, &None, &None)` call, and **`SnapshotOptions` exposes no knob to disable it** (its only fields are `threshold`, `failed_pixel_count_threshold`, `output_path`).
- Consequence: when **only AA-classified pixels differ**, `diffs` stays `0` ⇒ `get_results` returns `None` ⇒ egui_kittest's `let Some(..) = result else { return Ok(()) }` ⇒ **the test passes and no diff image is written.**
- The heuristic is pixelmatch's (it bails once >2 neighbours share the centre's brightness), so **flat interiors are never exempt; feathered edges may be** — i.e. the exemption lands precisely on our **hairline, rect stroke, and grid lines**.
- **Therefore the honest guarantee is: bit-exact in flat regions; AA-classified edge pixels may differ silently, with no diff artifact.** Not "any differing pixel fails".
- **This makes the exactness bet safer, not weaker.** Edge rasterisation is the *likeliest* source of mesa/LLVM variance, so the exemption absorbs exactly the drift that would otherwise cause false failures — which is why it is accepted rather than worked around. The mechanism stays as specified: it is the strictest expressible, and the exemption is a library property we cannot configure away, not a choice we are making.
- **The upstream defaults are NOT bit-exact** and must both be overridden: `threshold` fallback is **`0.6`** (docs: *"enough for most egui tests to pass across different wgpu backends"*) with `failed_pixel_count_threshold` fallback **`0`**. Taking the default would silently permit a per-pixel colour distance up to 0.6.

**Why the exactness bet is defensible (determinism sources already eliminated):**

- `HarnessBuilder`'s render options **default to `egui_wgpu::RendererOptions::PREDICTABLE`** = `{ msaa_samples: 1, depth_stencil_format: None, dithering: false, predictable_texture_filtering: true }` — versus a plain `default()` of `dithering: true, predictable_texture_filtering: false`. Upstream's own words for `predictable_texture_filtering`: *"useful when you want predictable rendering across different hardware, e.g. for kittest snapshots."* No MSAA, no dithering, software texture filtering.
- The frame is **text-free flat fills + hairlines** at `pixels_per_point = 1.0` — text shaping, the dominant churn source, is absent by construction.
- **AC10(b) (CPU-adapter assertion) is now MORE load-bearing, not less:** a bit-exact golden minted against a discrete GPU would fail on every other machine. Under a tolerance that might have been absorbed; under exact compare it cannot be. The assertion is what keeps the premise honest.

**Accepted risk, recorded rather than dropped:** CI installs mesa/lavapipe from **apt on `ubuntu-latest`**, so a **mesa/LLVM version upgrade could break exactness on the same platform**. Judged acceptable because: the eliminated nondeterminism sources above make flat-fill/hairline rasterisation the only variable; quartzite's one observed real drift (commit `99099ae`) was **text sub-pixel variance across different backends** — neither condition applies here; and **flat-region drift fails loudly** (a golden diff plus the AC12 artifact upload). **Bounded honestly:** drift confined to AA-classified **edge** pixels is the case that passes silently with no diff artifact (see the AA bound above) — but that is also the drift most likely to be benign rasteriser noise rather than a real regression, which is the trade being accepted. **If it bites, revisit — do not pre-emptively loosen.**

## ⚠ Font-proof amendment — AC4's font capability DROPPED (Step 7)

**The font-loading requirement was removed from this unit by an explicit product-owner call at `/task` Step 7 design-review. It was not quietly skipped.**

The trigger: AC4 originally required proving *"a custom (non-default) font face loaded"*, but **the repo contains zero font files** and vendoring is deferred to #12 — so the design could only route the proof through `epaint_default_fonts`, which demonstrates font *registration + family selection* but **not** a non-default *face*. The design flagged that this "does not satisfy a literal reading" of AC4(c). Design-review agreed the instinct was right but ruled that leaving it open would have Step 8 implement code already known to fail an AC.

Design-review also caught a **contradiction inside this spec**: AC4 permitted proof *"or AC2's rationale cites the specific `egui` API that does"*, while the Key-decisions row demanded *"load exactly one non-default face"* — strictly more than AC4's own wording allowed. Both are now resolved in one direction.

**Decision:** drop the font capability from AC4. AC2's rationale cites egui's `FontDefinitions` API as the evidence — a path AC4's wording already permitted. #11 stays strictly a backend pick + scaffold; fonts belong wholly to #12, which owns the type tokens.

**Accepted cost, stated at decision time:** *the backend's font capability goes unexercised by any code until #12, so a surprise there is found later.*

**Rejected alternatives, for the record:** (a) amend AC4(c) to mean registration + family via `epaint_default_fonts` — the design's own recommendation; (b) vendor an OFL face now — rejected as dragging licensing + asset-pipeline into #11 against this spec's own Deferred row.

### Precedent — `../quartzite`'s golden suite, and why we deliberately diverge from it

`quartzite` is this project's convention-source (CI workflows, file-size ladder, doc-conventions were imported from it) and it **already runs a wgpu golden suite in production**. It is vello+wgpu+winit, not egui — so only the **methodology** transfers. The product owner directed this inspection after round 4; findings below were **re-verified first-hand**, not taken on report.

**The headline: quartzite's CI-enforced goldens test nothing, and its docs don't admit it.**

- The only golden suite its CI runs is `quartzite-widgets`: **5 PNGs, each 326 bytes, 64×64, all md5 `dae29011` — byte-identical.** Decoded with proper PNG unfiltering, every one is **`distinct_pixels = 1` → `(0,0,0,255)`: uniformly, purely black.** Five semantically different widgets (`button`, `label`, `line_edit`, `grid_layout`, `box_layout`) produce the **same black square** — the suite cannot distinguish a button from a grid layout from an empty frame, and would pass with the renderer deleted.
- Its ~60 real-pixel goldens live in `quartzite-style`, and **CI never runs them**: `grep -n 'quartzite-style' .github/workflows/ci.yml` → **no match**. The generic `test` lane sets `SKIP_RENDER_SNAPSHOT: "1"` (`ci.yml:114`) under a comment claiming a dedicated `gpu-tests` job covers them — which is no longer true.

**Binding lesson (drives AC9): a golden that still passes when the drawing code is deleted is worse than no test, because it reads as coverage on a green check.** Our placeholder frame — a paper fill, one rect, one hairline — is only a few pixels from "uniform fill" and is at direct risk of exactly this. AC9 exists to make that failure impossible.

**Point-by-point divergence:**

| quartzite does | Verified evidence | We do instead |
|---|---|---|
| Mean-based tolerance `if report.mean > FLIP_TOLERANCE` with `FLIP_TOLERANCE = 0.05` | `quartzite-style/tests/support/mod.rs:58,183`; duplicated at `quartzite-widgets/tests/support/mod.rs:66,191` | **Reject the 0.05 number as unvalidated precedent** — it was never checked against real pixels because the real-pixel suite never runs. A **mean across the frame hides localised regressions** (a 1-px error on 64×64 moves the mean ~0.02% and can never trip a 0.05 gate) — a defect now rejected ***a fortiori***, since we permit no flat-region difference at all. We compare at the library's strictest: `threshold(0.0)` + `failed_pixel_count_threshold(0)`. Note `egui_kittest` never used a mean regardless — its gate is a pixel **count** (`num_wrong_pixels <= failed_pixel_count_threshold`), so the mean pathology is structurally absent here. |
| No adapter pinning at all: `instance.request_adapter(&RequestAdapterOptions::default())` | `quartzite-renderer/src/render_harness.rs:150`; **zero** `get_info` / `force_fallback_adapter` / `DeviceType` matches repo-wide; **no `.cargo/config.toml`**; `CONTRIBUTING.md` tells contributors `WGPU_BACKEND=vulkan cargo test` "selects lavapipe" — it does not | Rely on `egui_kittest`'s CPU-first `default_wgpu_setup()` (**strictly better than this precedent**) **and** assert it (AC10) rather than trust it. |
| Regen → manual promote: regen writes to **gitignored** per-backend dirs; lookup is `if backend_path.exists() { backend } else { shared }` | `.gitignore` ignores `**/tests/snapshots/{auto,vulkan,dx12,metal}/` | **Rejected alternative.** Commit the golden **at the path the test reads**. No second promotion step, no shadowing override dir. The shadowing lookup means a developer who regenerates silently stops testing the committed golden with no warning. *(**This workflow has empirically rotted, not merely could.** Verified present on disk right now: **210 untracked PNGs** — 116 in `auto/`, 59 in `vulkan/`, plus **35 failure artifacts (18 `.actual.png` + 17 `.diff.png`) sitting inside the committed `shared/` dir** (60 tracked, 95 on disk). Those 35 are residue of snapshot runs that **actually failed** on a developer machine and were never cleaned up — corroborating the adapter-divergence hazard, and invisible in CI because CI never runs this suite. Counted via `ls` + `git check-ignore -v` (`.gitignore:11`); note a `git status`-based check reports **0** here, because it suppresses ignored paths — the debris is gitignored by construction.)* |
| `SKIP_RENDER_SNAPSHOT=1` escape hatch that returns and passes | `ci.yml:114` | **No skip hatch** — see Key decisions. A hatch that silently passes is precisely how this suite rotted. |
| 65 goldens, 5 → 65 in ~3 weeks, no budget rule; a 272-line `support/mod.rs` copy-pasted into two crates under a manual "sync group" | `git ls-files '*.png'` → 65 files / **51,215 B (50.0 KB)** / mean 787 B / **all 64×64**; its own `learnings.md:1267` flags the ≥3-call-site duplication anti-pattern it then committed | Storage precedent **confirms** plain-git at this scale (see Key decisions). Budget + helper placement: see Key decisions. |

**Worth stealing, adopted:** `.gitignore` entries for failure artifacts, and the `if: failure()` diff-artifact upload (AC12). **Considered, not applicable:** quartzite's helper self-tests (`support_internals.rs`: `regen_env_writes_golden`, `missing_golden_panics_with_helpful_message`, …) are the best idea in that repo, but they test a **hand-rolled** harness's skip/regen/mismatch logic. We use upstream's `egui_kittest` harness, so that surface is upstream's to test; our only local test-side logic is plain assertions (AC9/AC10), which are self-evidencing and need no meta-tests. **If** a future unit adds local skip/regen/wrapper logic, it inherits this obligation.

### #17 follow-up (planned churn, not a surprise)

When **#17** lands the real track canvas, the placeholder golden committed here is **expected to be replaced** — its subject (paper + rect + hairline) ceases to exist. #17 re-points the golden at real track geometry via `UPDATE_SNAPSHOTS=true cargo test`, and — because that is a regen — **the `image-check` subagent (AC14) gates it**, pointed at #17's drawing code. **#17 inherits that obligation automatically from AC16's standing rule in `code-writer.md`** — it needs to know nothing about #11, which is the entire point of putting the contract in an agent file rather than in this spec. The harness, the exact-compare policy, the AC9 guard, AC10(b), the `image-check` agent itself, and the workflow all carry forward **unchanged**; only the image does not. #17 should also re-aim AC9's probe coordinates at the new frame, since they are tied to where the code paints.

## Acceptance Criteria

| # | Criterion |
|---|---|
| AC1 | `crates/render/Cargo.toml` declares `egui = "0.35"` as a normal dep plus `egui_kittest = { version = "0.35", features = ["wgpu", "snapshot"] }` under `[dev-dependencies]`, and its `TODO(2)` placeholder comment is gone. `crates/game/Cargo.toml` declares `eframe = "0.35"`. All pinned `0.x`-style per AGENTS.md § *Dependency Versions*, against the version observed at implementation time. |
| AC2 | `ai-docs/key-decisions.md` carries a written rationale that (a) ties eframe/egui to the design system's flat-vector/lattice aesthetic, (b) names the rejected alternatives (macroquad, raw wgpu+winit+tessellator, vello), (c) covers each AC4 capability, (d) **cites egui's `FontDefinitions` API as the backend's custom-font evidence** — this rationale is the *sole* font proof in this unit; no face is loaded and no code exercises font loading here (see *Font-proof amendment*), and (e) **records the §6-over-#11 window/loop ownership override and why**. |
| AC3 | `cargo run -p gp-game` opens a window drawing the placeholder frame: cleared to the paper background (`--paper-1 #F5F1E6`), with at least one rectangle and one hairline stroke visible. The `eframe::App` impl + `run_native` call live in **`gp-game`**. |
| AC4 | The scaffold **demonstrates** — not merely asserts — (a) crisp shapes, (b) hairline strokes, (c) the graph-paper background motif. "Demonstrates" = the scaffold exercises it, or AC2's rationale cites the specific `egui` API that does. |
| AC5 | `render_frame` takes a **borrowed** `&egui::Painter` draw context as a parameter, retaining `(&TrackArtifact, &[CarState], Overlays)`. It does not own, construct, or store a `Painter`/`Context`. Body may remain `todo!()`. |
| AC6 | A tessellation smoke test **in `gp-render`** drives an `egui::Context` through a full pass (`RawInput` → `run_ui` or `begin_pass`/`end_pass` → `tessellate`) over the placeholder drawing path and asserts the tessellated output is non-empty. This individual test needs **no display server and no GPU adapter** (pure CPU tessellation) and does **not** require a generated `TrackArtifact`. *(Note: the crate's suite as a whole now requires a Vulkan ICD because of AC8 — the "no GPU" property is scoped to this test, not to `cargo test -p gp-render`.)* |
| AC7 | **`gp-render` has no `eframe`/`winit`/`wgpu` *normal* dependency** — verified by **`cargo tree -p gp-render --edges no-dev`** — so the crate *ships* GUI-free. Dev-dependencies are exempt by design (the AC8 harness lands wgpu 29.x on the dev edge): `cargo tree` includes dev-deps by default, which is why the `--edges no-dev` scoping is load-bearing rather than cosmetic. |
| AC8 | A **golden-image test in `gp-render`** renders the **text-free** placeholder frame through `egui_kittest`'s wgpu/Vulkan harness (`Harness::new_ui(..)` → `snapshot_options`) at a pinned canvas size and `pixels_per_point = 1.0`, and compares against a **committed PNG** at the path the test reads — `crates/render/tests/snapshots/` — with **no promotion step and no per-backend shadowing dir**. It passes in CI on lavapipe with no display server, and does **not** require a generated `TrackArtifact`. `.gitignore` gains `**/tests/snapshots/**/*.diff.png`, `**/tests/snapshots/**/*.new.png`, and `**/tests/snapshots/**/*.old.png` (all three artifact names verified in `egui_kittest`'s source). |
| AC9 | **Non-triviality guard — the automated stand-in for visual inspection.** The rendered frame is additionally asserted by cheap, explicit, **non-golden** checks that fail loudly if it degenerates: (a) the paper background pixel equals `#F5F1E6`, (b) a hairline pixel is measurably darker than the background, (c) the frame is **not uniform** — more than one distinct colour is present. **Exact compare (AC10a) does NOT subsume this**: a bit-exact golden of an all-black frame passes forever. Exact compare answers *"do the pixels still equal the committed ones?"*; AC9 answers *"are those pixels a real frame at all?"* — the only question a green check cannot otherwise answer, and the one nobody eyeballs the image to settle. **Second reason AC9 is load-bearing:** because AA-classified edge pixels can differ silently (see the AA bound), probe (b) — *a hairline pixel is measurably darker than the background* — is a flat-vs-edge check the golden may now miss. Claimed narrowly: AC9 samples **specific** pixels and is **not** a general edge-regression detector; it catches a hairline that vanished or inverted, not one that shifted a shade. A comment cites why: quartzite's five CI-enforced goldens are byte-identical, uniformly black (`(0,0,0,255)`) images that would pass with the renderer deleted. **A golden that survives deleting the drawing code is worse than no test.** |
| AC10 | **Exactness + adapter are asserted, not assumed.** (a) The golden comparison is set to **the strictest `egui_kittest` can express — bit-exact in flat regions**: `SnapshotOptions::new().threshold(0.0).failed_pixel_count_threshold(0)` via `Harness::snapshot_options`. **Both upstream fallbacks must be overridden** — `threshold` defaults to `0.6`, which would silently permit a per-pixel colour distance; `failed_pixel_count_threshold` already defaults to `0`. **It is not literal bit-to-bit**: the library hardcodes `detect_anti_aliased_pixels = true` with no knob to disable it, so **AA-classified edge pixels may differ silently with no diff artifact**. The guarantee is **bit-exact in flat regions**; the exemption is a library property we cannot configure away, and it lands on the hairline / rect stroke / grid lines. A comment states the premise (llvmpipe is expected stable on the single Linux/lavapipe lane), **the AA bound**, and the deferral trigger (revisit both when dx12/metal lanes join the matrix). (b) The test **asserts the resolved adapter is a CPU/software device** (`RenderState::adapter.get_info().device_type == DeviceType::Cpu`) and fails loudly otherwise — **load-bearing under exact compare**, since a bit-exact golden minted against a discrete GPU would fail on every other machine. |
| AC11 | Contributor workflow is documented (crate docs or `ai-docs/key-decisions.md`): where the golden lives; that an intended rendering change is refreshed with **`UPDATE_SNAPSHOTS=true cargo test`** (the only env var `egui_kittest` honours); that **a regen is not complete until AC14's `image-check` subagent confirms the image matches the drawing code** (spawn the agent — never an inline `Agent(model="sonnet", …)`, which cannot enforce the effort tier) — **enforced durably by AC16's rule in `code-writer.md`, not by this documentation alone**; that a **Vulkan software ICD (lavapipe / `mesa-vulkan-drivers`) must be installed** — there is deliberately **no skip hatch**; and that **CI is authoritative** if local and CI images ever disagree. Note the disagreement should be near-impossible in practice: AC10(b) guarantees both sides are on a CPU adapter, so under exact compare a local difference is a **hard failure pointing at a real environment divergence** (e.g. a mesa/LLVM delta) — not a tolerance judgement call to be talked away. |
| AC12 | `.github/workflows/ci.yml` gains an **`if: failure()` artifact upload** of the golden failure images (`*.diff.png`, `*.new.png`) so a CI golden failure is diagnosable instead of an opaque "images differ". **`actionlint .github/workflows/ci.yml` MUST pass before `git add`** (AGENTS.md AXIOM). No Vulkan/diagnostic step is added — `vulkaninfo --summary` already exists at line 102. |
| AC13 | Full gate green: `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`. **This list is exhaustive — AC14 is deliberately absent from it.** |
| AC14 | **A file-based subagent `.claude/agents/image-check.md` exists**, with frontmatter **`model: sonnet`** and **`effort: medium`**, and `tools` **omitted** (→ inherit-all, so its `Read` can render the PNG — restricting `tools` would break the check). Its `description` is written to be dispatch-accurate, since Claude routes delegation on that field. **Contract:** given the placeholder drawing code and the minted PNG, confirm the image is **semantically consistent with what the code draws** — paper background present; rectangle and hairline where the code puts them; graph-paper motif present; no unexplained shapes or colours. **The check is invoked by spawning this subagent**, never by an inline `Agent(model="sonnet", …)` call: there is no per-invocation `effort` parameter on the Agent/Task tool, so **frontmatter is the only lever that enforces the medium tier** (the rationale `.claude/agents/code-writer.md` already records — the house precedent and the only other effort-pinned agent). **A regen is not complete until this passes.** It catches the one class AC9 and AC10(a) cannot — a golden *wrong from birth* (the directive's "code draws a red circle, image contains a black triangle"), which exact compare would then pin forever, bit-identical to itself. **It is non-deterministic and model-dependent; it runs at mint/regen time only, NEVER in CI** — CI has no model (see *Addendum*). Verified at review time, exactly as AC2 and AC11 are. |
| AC15 | **Propagation + file-size obligations — for BOTH agent files (AGENTS.md AXIOMs).** (a) **`ai-docs/claude-tools-hierarchy.md` MUST be updated in this same PR** (AGENTS.md § *Propagation Rule*: *"Any edit that changes a Tool/Subagent/Skill/Hook contract → Update `ai-docs/claude-tools-hierarchy.md` in the same PR"*), and **two rows change, not one**: **(i)** add an `image-check` row to its `## Subagents (.claude/agents/)` table recording the frontmatter-pinned `model: sonnet` + `effort: medium` and the mint-time-only, never-CI scope; **(ii)** update the **existing `code-writer` row** — whose description column currently ends *"Never runs self-review, never pushes."* — to also carry its new AC16 obligation (spawns `image-check` on any golden mint/regen; Mode A must not commit the PNG, Mode B must not return, until it confirms). Do not let (ii) be missed because `code-writer` is already listed. **That row's closing sentence — *"Never runs self-review, never pushes."* — stays TRUE under AC16's carve-out and needs no correction** (verified: `image-check` is not `self-review`, and the sentence names `self-review` specifically rather than claiming the agent spawns nothing); the row must simply still read accurately once the golden obligation is appended. (b) **No sync-group sibling is implicated by either edit**: `image-check` is new and joins none of the five groups (Review / Interview / Triage / Task-Design / Learning-Log), and **`code-writer` appears nowhere in AGENTS.md at all** — verified — so editing it has **zero fan-out**. (c) The **40,000-char instruction-file AXIOM** covers *"every `.claude/agents/**.md`"* — it binds the new `image-check.md` (keep it small; one job) **and** `code-writer.md`, which this PR *adds* to: it is currently **6,935 chars**, i.e. **28,065 chars of headroom** below the 35,000 early-warning rung, so a tight insertion is comfortably safe — but the insertion must stay tight regardless; `code-writer.md` is an instruction file, not a tutorial. |
| AC16 | **The `image-check` calling contract lives in `.claude/agents/code-writer.md` — TWO edits to that one file, and (a) is unimplementable without (b). Do not land (a) alone.** **(a) The golden-spawn rule:** when a subtask mints or regenerates a golden image, spawn `image-check` and do not proceed until it confirms image↔code consistency. **Mode-aware**, matching that file's two contracts: **Mode A** (commits per subtask) must **not commit** the PNG until it passes; **Mode B** (returns without committing) must **not return** until it passes. Worded as a **standing rule for any golden** — #17/#18 inherit it — not as "the #11 placeholder golden". **(b) A narrow carve-out to the `## Invariants (both modes)` clause** that today reads *"Do not spawn `self-review`; **do not spawn any other reviewer**"* — which literally forbids (a), since `image-check` returns a PASS/FAIL verdict gating a commit. The carve-out **permits**: a **subtask-named artifact-validity check** that verifies a *generated artifact* against the code that generated it (`image-check` is the only instance today). It **still forbids, explicitly**: `self-review`, and **any approval-gate reviewer that judges the quality or correctness of the writer's own work**. The boundary must be **stated, not implied** — a future reader must be able to place a new subagent on one side or the other **without re-deriving this decision**. Both edits authored in that file's existing invariant voice (its bullets are cross-mode `NEVER` rules with mode-specific bite); no foreign-looking bolted-on block. **Scope is exactly this one file** — no `self-review` involvement (would fire the Review sync group), no `/task` SKILL.md edit (would fire the Task/Design group). **Still not a CI gate** — the writer spawns it at mint/regen time; CI has no model, and AC13's gate list stays exhaustive with AC14 deliberately absent. |

## Open questions

- **Which real faces ship, and vendored vs. system-resolved.** Wholly #12's (both flagged faces are substitutions). Per the *Font-proof amendment* this unit neither loads a face nor exercises the font path — AC2's `FontDefinitions` citation is the only font evidence here.
- **Whether the `wgpu ^29.0` pin ever needs alignment with a standalone wgpu 30.** Largely settled by this unit: `eframe 0.35` and `egui_kittest 0.35` both pin `wgpu ^29.0`, so the two wgpu-facing crates in the graph agree and no duplicate-major resolves. Only reopens if a *third* consumer needs wgpu 30 directly.
- **Theming (light/dark or paper-only).** `tokens/colors.css` is a single warm-paper palette; no dark variant was imported. Assumed paper-only until #12 says otherwise.
- **Whether `gp-game`'s loop shell should later move behind a feature or a separate `app` module** once block 3b's real input/timing lands. Not a concern for this unit.

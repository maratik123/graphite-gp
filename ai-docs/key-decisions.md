# Key design decisions

The finalized design and its full rationale live in [`docs/design.md`](../docs/design.md) (spec) and [`docs/design-review.md`](../docs/design-review.md) (the 4-round review that hardened it). This file holds implementation-level ADR bodies as they accrue — decisions made *while building* that aren't already captured in the design doc.

<!-- Append as: ### YYYY-MM-DD — <title>  →  **Context** / **Decision** / **Consequences** -->

### 2026-07-16 — Rendering backend: eframe/egui 0.35, with the window + loop in `gp-game`

**Context.** Issue #11 is the foundational decision unit for Block 2 — `gp-render` ([`docs/design.md`](../docs/design.md) §4 *Рендеринг + UX*, §6 *Архитектура*); the other twelve `block:render` issues (#12–#23) all sit downstream of the pick. The design system to port (`docs/design-system/`) specifies flat vector geometry on a lattice, hairline pencil borders, crisp radii, and no photographic imagery. The downstream units cover **13 components** (#13–#16) across **4 screens** (#19–#22).

**Decision.** **eframe/egui 0.35** — `egui` is the draw layer, `eframe` the window/loop shell. Verified live 2026-07-16: `egui` and `eframe` `max_stable_version` = **0.35.0**; MSRV 1.92 ≤ the workspace's `rust-version = 1.97.1`; eframe's **default** renderer is wgpu (glow is opt-in), which matches the CI `WGPU_BACKEND=vulkan` env as-is.

**(a) Why it fits the aesthetic.** egui's `Painter` draws the design system's vocabulary directly — flat fills, strokes, and lines on a lattice — with no photographic or raster-asset pipeline to fight. Verified against `egui-0.35.0/src/painter.rs`: `rect_filled` (:397) and `rect_stroke` (:406) give crisp shapes and radii; `line_segment` (:318) and `hline` (:332) give hairline strokes; `hline` + `circle_filled` (:356) give the graph-paper ruling + dot motif.

**The decisive argument is cost, not taste.** egui is the only candidate that **ships a widget layer** (crates.io, verified live: *"An easy-to-use immediate mode GUI that runs on both web and native"*). That makes #13–#16's 13 components across 4 screens **token-styling tasks on an existing widget layer** rather than from-scratch toolkit construction — and those 13 components are where the whole downstream block's cost sits.

**(b) Rejected alternatives.** All three lose on the same axis: each is a graphics/rendering layer, **not a GUI toolkit**, so all 13 components would be built from zero.

| Rejected | crates.io description (verified live 2026-07-16) | Why not |
|---|---|---|
| **macroquad** | *"Simple and easy to use graphics library"* | Game-oriented graphics; no widget layer. |
| **raw wgpu + winit + a tessellator** | — | Maximum control, but every widget, layout, event, and text pass becomes ours to build and maintain. The cost argument kills it outright. |
| **vello** | *"A GPU compute-centric 2D renderer."* | A renderer, not a toolkit — no widgets, no layout, no event loop. (It is `../quartzite`'s stack; we diverge deliberately — see the spec's precedent section.) |

**(c) AC4 capabilities** — crisp shapes, hairline strokes, and the graph-paper motif each name the specific `egui` 0.35 API that provides them under (a); the #11 scaffold exercises all three through `draw_placeholder`.

**(d) Custom fonts — cited, not exercised.** The backend's custom-font capability is **`egui::FontDefinitions`**, applied via **`Context::set_fonts(FontDefinitions)`** (`Context::add_font(FontInsert)` is the incremental variant). Verified against the 0.35.0 source: `set_fonts` at `context.rs:2038`, `add_font` at `context.rs:2061`, and `FontDefinitions` re-exported from `epaint::text` at `lib.rs:447` — so `egui::FontDefinitions` is a real path, not an approximation. **This citation is the *sole* font proof in #11: no face is loaded and no code here exercises the font path** (product-owner call — see the spec's *Font-proof amendment*). Fonts belong wholly to #12, which owns the type tokens. **Accepted cost, stated at decision time:** a surprise in the backend's font path is found later, at #12.

> **Resolved at #12 (2026-07-17) — the accepted cost was paid, and it paid off.** The deferred surprise was real and landed exactly where #11 predicted: **epaint 0.35 is on Google's `fontations` stack** (`skrifa` / `harfrust` / `read-fonts`, rasterising via `vello_cpu`), **not `ab_glyph`**. That made variable fonts fully supported, so #12 vendors **2 `[wght]` files serving all 7 weight instances** (`FontTweak::coords` overrides axes per registration; `FontData::from_static` borrows, so instances share one byte array) instead of 7 static faces — the surprise landed *in our favour*. Three further findings the citation could not have caught: **`VariationCoords` is not re-exported at `egui`'s top level** (only `egui::epaint::text::VariationCoords` compiles); **`set_fonts` overwrites wholesale**, so a builder must start from `FontDefinitions::default()`, never `::empty()`, or egui's emoji/fallback coverage is silently dropped — and note `default()` **is** `empty()` when the `default_fonts` feature is off, which changes that rationale with no compile error; and **Space Grotesk's `wght` axis defaults to 300**, so a bare registration renders Light. Each was found by *running* the path, not by reading it — which is precisely what the #11 citation deliberately did not do. `gp-render`'s draw-only constraint survived: it produces the `FontDefinitions`, `gp-game` calls `set_fonts`.

**(e) Window/loop ownership — §6 over issue #11's text.** Issue #11 scopes "the window/canvas + main loop" into `gp-render`. **The product owner overrode this** (round-1 interview, 2026-07-16): the window and event loop land in **`gp-game`** instead. §6 assigns block 3b (`gp-game`) *"игровой цикл: ввод игрока, тайминги, оркестрация, UX"* (player input, timing, orchestration, UX), while §4 is titled "Рендеринг + **UX**" — both blocks are named for UX, so the canonical doc is genuinely ambiguous on the boundary. The override resolves it in favour of §6. **A deliberate deviation from the issue text, not an oversight.**

**Consequences.**

- `gp-render` is a **draw-only library**: `egui` + `gp-core`, and **no `eframe`/`winit`/`wgpu` normal dependency** — which is what keeps it GUI-free *by construction* rather than by convention (checked with `cargo tree -p gp-render --edges no-dev`; dev-deps are exempt by design, since the golden harness lands wgpu on the dev edge).
- `render_frame` takes a **borrowed** `&egui::Painter` — it does not own, construct, or store a `Painter`/`Context`. `gp-game` owns window, event loop, input, and timing.
- `eframe 0.35` pins `wgpu ^29.0` (standalone latest is **30.0.0**, verified live) and `egui_kittest 0.35` pins the same major, so the two wgpu-facing crates agree and no duplicate-major resolves.
- **[`docs/design.md`](../docs/design.md) is NOT amended.** It is the product-owner-authored canonical spec; a backend pick is an *implementation* decision and does not amend it. That is precisely why this rationale lives here.

### 2026-07-16 — `gp-render`'s golden image: where it lives, and how to refresh it

**Context.** #11 ships a wgpu/Vulkan golden-image harness (`egui_kittest` 0.35 as a **dev**-dependency; verified live 2026-07-16: `max_stable_version` = 0.35.0) plus **one** committed text-free golden PNG of the placeholder frame. It was adopted on a product-owner override of the spec-writer's "render through wgpu, but no committed golden yet" recommendation; the knowingly-accepted costs are recorded in the spec's *Golden-harness override*. A golden nobody can refresh — or that a contributor refreshes against the wrong rasteriser — rots quietly, so the workflow is recorded here rather than left to lore.

**Decision — the contributor workflow.** When you intend to change what `gp-render` draws:

1. **The golden lives at `crates/render/tests/snapshots/placeholder.png`** — committed in plain git (no LFS, no `.gitattributes`), at **the path the test reads**. There is **no promotion step and no per-backend shadowing directory**: a regen updates the very file the test compares against.
2. **Install a Vulkan software ICD first** — lavapipe, from `mesa-vulkan-drivers`. `cargo test -p gp-render` requires it. **There is deliberately no skip hatch:** `egui_kittest` honours exactly one env var (`UPDATE_SNAPSHOTS`) and no skip variable exists, so a hatch would have to be hand-rolled — and a hatch that *silently passes* is exactly how `../quartzite`'s suite rotted. Without an ICD you get a loud failure, by design.
3. **Refresh with `UPDATE_SNAPSHOTS=true cargo test`** — the only env var the library honours.
4. **A regen is NOT complete until the `image-check` subagent confirms the image matches the drawing code.** Spawn it as `subagent_type="image-check"` — **never** as an inline `Agent(model="sonnet", …)`: there is no per-invocation `effort` parameter on the Agent tool, so **frontmatter is the only lever** that enforces its `medium` tier. This is **enforced durably by the standing rule in [`.claude/agents/code-writer.md`](../.claude/agents/code-writer.md) § Invariants (both modes)**, not by this documentation alone — Mode A must not commit the PNG, and Mode B must not return, until `image-check` PASSes. On FAIL: fix the drawing code and re-mint — never re-interpret the image.
5. **CI is authoritative** if a local image and CI ever disagree — CI is the only environment guaranteed to have lavapipe as its sole rasteriser.

**Consequences.**

- **Point 5 should be near-vacuous in practice, and that is the point.** The golden test asserts the resolved adapter is a CPU/software device, so *both* sides are guaranteed a software rasteriser. Under an exact compare, a local/CI difference is therefore a **hard failure pointing at a real environment divergence** (e.g. a mesa/LLVM delta) — **not a tolerance judgement call to be talked away**.
- The comparison is the strictest `egui_kittest` 0.35 can express (`threshold(0.0)` + `failed_pixel_count_threshold(0)`) — **bit-exact in flat regions**, with AA-classified edge pixels exempt via a library property we cannot configure away. The tolerance question and the AA exemption are **deferred together** on one trigger: a dx12 (Windows) / metal (macOS) lane joining the CI matrix. **If it bites, revisit — do not pre-emptively loosen.**
- **#17 inherits step 4 for free.** When it re-points the golden at real track geometry, that is a regen — so `code-writer`'s standing rule fires against #17's *own* drawing code, and #17 needs to know nothing about #11. That is the entire reason the calling contract lives in an agent file rather than in this document or in a one-shot design doc.

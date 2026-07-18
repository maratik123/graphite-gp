# gp-render: SVG → egui-texture icon pipeline (port from marshrutka)

**Source:** issue #88
**Date:** 2026-07-18
**Tracked in:** #88

## Scope

Give `gp-render` an **SVG → egui-texture bake pipeline** so downstream widgets (#13) can draw real Lucide icons, porting the *bake half* of the sibling `marshrutka` project's `src/emoji.rs`:

1. **Add deps** to `crates/render/Cargo.toml`: `resvg` and `tiny-skia` at verified-latest versions with `default-features = false` + minimal features (see Key decisions).
2. **Port the bake pipeline** (marshrutka `svg_to_texture`): SVG bytes → `resvg::usvg::Tree::from_data(bytes, &Options)` → `resvg::render(&tree, transform, &mut pixmap)` into a `tiny_skia::Pixmap` sized at target px × `ctx.pixels_per_point()` → `egui::ColorImage::from_rgba_premultiplied` → `ctx.load_texture(name, image, TextureOptions)` → cached `egui::TextureHandle`.
3. **Introduce an icon cache** (analogous to marshrutka's `EmojiMap`): pre-bakes a curated Lucide set at construction, keyed by (icon, size), constructed from an `&egui::Context`. This is a new module in `gp-render` (e.g. `icons`).
4. **Vendor the curated Lucide SVGs** into the crate and `include_bytes!` them (marshrutka's proven pattern), with the upstream license file, mirroring the vendored-fonts (`fonts/*/OFL.txt`) precedent.
5. **Provide a draw helper** wrapping `painter.image(handle.id(), rect, uv, tint)` that honors tint + alpha.
6. **Unit-test the CPU half** (SVG → pixmap → `ColorImage`): correct size, non-empty, alpha present — no GPU required; Miri-gate any test that aborts Miri (see Key decisions).

## Out of scope

- The `tl` (HTML) + `simplecss` (inline-CSS) ingestion half of marshrutka's render facility — tokens are already Rust consts (#12) and native widgets were chosen (#11).
- The core widgets themselves (#13) — this issue only delivers the pipeline + the icon assets the widgets will consume.
- Game marks (point / car / wall / S-F line) — those are drawn natively by later blocks, not through this SVG pipeline. Only UI-chrome Lucide icons flow through it.
- Any window / event-loop / `Context` *ownership* — `gp-render` stays draw-only + a Context-*borrowing* bake API; `gp-game` continues to own the window (eframe/wgpu).

## Deferred

- Broadening the vendored set to the Button game-screen icons (`shuffle`, `chevrons-right`, `rotate-ccw`, `arrow-right`) | those appear only in `ui_kits/game/Screens.jsx` (game screens, #14+), not in #13's core specimen | no separate issue — drop the SVG files in when game screens land.

## Key decisions

| Question | Decision |
|---|---|
| Which half of marshrutka's `emoji.rs` to port? | Bake only (`svg_to_texture` + the `init_emojis`-style pre-bake/cache). HTML/CSS ingestion excluded. |
| `resvg` version | **`0.47`** (verified latest stable on crates.io; marshrutka's `0.45` bumped), `default-features = false`. |
| `tiny-skia` version | **`0.12`** (verified latest stable; equals `resvg 0.47`'s internal `tiny-skia ^0.12.0` req → a single prod copy), `default-features = false`, `features = ["std", "simd"]`. This direct edge is effectively **feature-inert** — `resvg` declares its own `tiny-skia` edge with `default_features = true` and Cargo unification is additive, so our `default-features = false` cannot subtract `png-format`; the edge's purpose is to *pin the single `^0.12` copy*, not to trim features. |
| `usvg` as a direct dep? | No — use the `resvg::usvg` re-export (marshrutka does; `resvg` pulls `usvg ^0.47.0`). |
| Minimal features rationale | `resvg default-features = false` drops `text` / `system-fonts` / `memmap-fonts` / `raster-images`, which in turn drop `fontdb`, `rustybuzz`, `ttf-parser`, `unicode-*`, `gif`, `image-webp`, `zune-jpeg` — Lucide icons are text-free stroke/vector paths with no embedded rasters, so none are needed. **`png-format` (→ `dep:png`) cannot be dropped**, though: `resvg` pulls `tiny-skia` with its defaults (which include `png-format`) and Cargo unification is additive, so no feature flag on our edge removes it. `png` (with `fdeflate` / `simd-adler32`) is therefore accepted as a benign **link-only** transitive — we rasterize SVG → pixmap → `ColorImage` and never decode a PNG at runtime. |
| Icon source / vendoring mechanism | Vendor a curated set of Lucide SVGs under `crates/render/icons/` and `include_bytes!` them (marshrutka's proven `include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), …))` pattern). The `lucide-icons` crate was considered but vendoring a handful keeps the transitive graph minimal per the AC (avoids compiling the full icon corpus). |
| Initial icon set | The 5 icons #13's core specimen (`components/core/core.card.html`) actually references — the IconButton row: **`play`, `pause`, `grid-3x3`, `zoom-in`, `settings`**. Core Buttons in that card carry no icons. |
| Licensing | Lucide is **ISC** (verified upstream `LICENSE`; Feather-derived icons are **MIT**). Vendor Lucide's `LICENSE` next to the SVGs and extend the crate `license` SPDX (currently `(MIT OR Apache-2.0) AND OFL-1.1`) to add `ISC` — the same pattern as the vendored fonts' `OFL-1.1`. Verify per-vendored-icon whether any is Feather-derived (MIT already appears in the crate's own-code clause). |
| Cache lifetime / key | Pre-bake at construction (marshrutka style), keyed by (icon, size); the cache struct is constructed once from an `&egui::Context`. This is `gp-render`'s first Context-*touching*, stateful surface — still no window ownership. |
| Draw call | `painter.image(handle.id(), rect, uv, tint)`, honoring tint + alpha (the sole draw API — no new backend surface). |
| Determinism | Raster math is not integer-deterministic, but this is the **rendering** layer (`gp-render`) — the integer-only physics rule (`docs/design.md` §3a) does not reach this crate. |
| Miri | Gate any test that invokes `resvg` / `tiny-skia` with `#[cfg_attr(miri, ignore = "…")]` **in the same commit** if it aborts Miri (`tiny-skia`'s `Pixmap` performs checked `u8`↔wider casts that can abort under Miri's 1-byte allocator alignment, exactly as `tessellation_smoke` does). Any GPU / `egui_kittest` golden test is always Miri-gated per repo rules. |

## Technical constraints

- `gp-render` is on `egui = "0.35"` with `default-features = false`, and is **draw-only today** (`&egui::Painter`, holds no `Context`; fonts are installed by `gp-game` via `set_fonts`). The bake needs an `&egui::Context` (for `load_texture`) — this is the crate's first Context-consuming API. `ColorImage`, `Context::load_texture`, and `Painter::image` are all core egui (present under `default-features = false`) and backend-agnostic: identical on marshrutka's glow (0.31) and our wgpu (0.35) — a version bump on the port, not a rewrite.
- Existing modules to sit beside: `tokens` (#12), `fonts`, `placeholder`. Add the icon pipeline as a new sibling module.
- Vendored third-party (ISC) assets require a license file committed alongside them + an SPDX update, following the `crates/render/fonts/{onest,jetbrains-mono}/OFL.txt` precedent.
- After the dep edit: run `cargo update` then `cargo build`, and confirm `git diff --stat Cargo.lock` shows only the intended new edges before staging.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `gp-render` exposes a bake API taking SVG bytes + target size + DPI (via `&egui::Context`) and returning a cached `egui::TextureHandle`. |
| AC2 | The curated Lucide set (`play`, `pause`, `grid-3x3`, `zoom-in`, `settings`) is vendored into the crate and available as baked textures. |
| AC3 | A baked icon draws via `painter.image` honoring `tint` and alpha. |
| AC4 | `resvg = 0.47` and `tiny-skia = 0.12` are added; the direct `tiny-skia` edge uses `default-features = false`, `features = ["std", "simd"]` (pins the single `^0.12` prod copy — effectively feature-inert, since `resvg` forces `tiny-skia`'s defaults regardless). `cargo tree -p gp-render -e no-dev` shows a single prod `tiny-skia` and **no** `fontdb` / `rustybuzz` / `ttf-parser` / `gif` / `image-webp` / `zune-jpeg`. `png` (+ `fdeflate` / `simd-adler32`) IS pulled unavoidably by `resvg → tiny-skia`'s default `png-format` and is link-only (no runtime PNG decode) — it is **not** forbidden. (Scope the check with `-e no-dev`: a dev-only `tiny-skia 0.11.4` reaches the crate via `egui_kittest → egui-wgpu → winit → sctk-adwaita`, so a naive `cargo tree` would falsely show two copies.) |
| AC5 | Unit tests assert the SVG → pixmap → `ColorImage` step (correct pixel dimensions, non-empty buffer, alpha channel varies) without requiring a GPU. |
| AC6 | Any test invoking `resvg` / `tiny-skia` that aborts Miri is `#[cfg_attr(miri, ignore = "…")]`d in the same commit; the workspace Miri gate (`MIRIFLAGS=-Zmiri-tree-borrows cargo miri test --workspace`) stays green. |
| AC7 | Vendored Lucide icons carry their upstream ISC license file; the crate `license` SPDX is updated to include `ISC` (add `MIT` coverage too if any vendored icon is Feather-derived). |
| AC8 | `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` all pass; every public item has a `///`. |

## Open questions

- **Baked size variants.** The web design system sizes IconButton glyphs at ~18px and Button icons at 16–20px, with IconButton dims 30/38/46 (sm/md/lg). Design picks the concrete set; sensible default: a single logical size (~18px) scaled by `pixels_per_point`, expanded only if #13 needs per-size crispness.
- **Set breadth.** Whether to vendor the Button game-screen icons now or defer to #14+. Default: defer (see Deferred) — adding an SVG file later is trivial and changes no design.
- **Direct `tiny-skia` dep vs. `resvg::tiny_skia` re-export.** Default: add the direct dep (the issue lists it; pinning `0.12` guarantees the single-copy alignment) unless design finds the re-export strictly cleaner.
- **Icon-name representation & bake timing.** Enum of vendored icon names (compile-time safety) vs. string key; eager pre-bake (marshrutka) vs. lazy-on-first-request. Pure architecture — design's call; default: enum keys + eager pre-bake.

# Design: gp-render SVG → egui-texture icon pipeline (port from marshrutka)

**Issue:** #88 (prerequisite of #13 — core widgets, deferred/blocked on #88)
**Date:** 2026-07-18

## Approach

Port the **bake half** of marshrutka's `src/emoji.rs` (`svg_to_texture` + the
`init_emojis`-style eager pre-bake/cache) into a new `gp-render` module,
`crates/render/src/icons.rs`, bumping the raster stack from marshrutka's
glow-0.31 / resvg-0.45 / tiny-skia-0.11 to our wgpu-0.35 / resvg-0.47 /
tiny-skia-0.12. The three egui APIs the bake rides on — `ColorImage::from_rgba_premultiplied`,
`Context::load_texture`, `Painter::image` — all exist in egui 0.35 and are
backend-agnostic, so this is a **version bump, not a rewrite**
`[measured: grep ~/.cargo/registry/.../egui-0.35.0 → load_texture context.rs:2322, pixels_per_point context.rs:2220, Painter::image painter.rs:447; epaint-0.35.0 → from_rgba_premultiplied image.rs:128, TextureHandle::id texture_handle.rs:64, TextureOptions textures.rs:153]`.

### Module shape (`crates/render/src/icons.rs`, new sibling of `tokens`/`fonts`/`placeholder`)

Three layers, deliberately split so the Context-free CPU half is unit-testable
without a GPU (AC5) and the Context-touching half is a thin wrapper:

1. **`Icon` enum** — the five vendored names as compile-time-safe variants
   (`Play`, `Pause`, `Grid3x3`, `ZoomIn`, `Settings`), with `const ALL: [Icon; 5]`,
   `const fn svg_bytes(self) -> &'static [u8]` (`include_bytes!` per variant,
   mirroring `fonts.rs`'s `include_bytes!("../fonts/…")` `[measured: fonts.rs:23,27]`),
   and `const fn name(self) -> &'static str` (the `load_texture` cache key /
   `Debug` label).
2. **CPU bake step** — `svg_to_color_image(svg: &[u8], logical_px: f32, ppp: f32)
   -> Result<ColorImage, IconError>`: `usvg::Tree::from_data` → compute physical
   size (`logical_px * ppp`, scaled to the SVG's aspect) → `tiny_skia::Pixmap` →
   `resvg::render` → `ColorImage::from_rgba_premultiplied`. **No `&Context`** — DPI
   is passed as a plain `f32`, so the AC5 unit test drives it Context-free and
   GPU-free.
3. **Context-touching surface** — `pub fn bake_texture(ctx, name, svg, logical_px)
   -> Result<TextureHandle, IconError>` (AC1: SVG bytes + target size + DPI via
   `ctx.pixels_per_point()` → cached handle via `ctx.load_texture`), and
   `IconSet(HashMap<Icon, TextureHandle>)` with `IconSet::new(ctx) ->
   Result<Self, IconError>` eager-pre-baking all `Icon::ALL` at one logical size,
   plus `get(icon) -> Option<&TextureHandle>` (marshrutka's `get_texture` shape
   `[measured: ../marshrutka/src/emoji.rs EmojiMap::get_texture]`).
4. **Draw helper** — `pub fn draw_icon(painter, handle, rect, tint) -> ShapeIdx`
   wrapping `painter.image(handle.id(), rect, FULL_UV, tint)` (AC3), where
   `const FULL_UV: Rect` is a struct literal `Rect { min: Pos2::ZERO, max:
   Pos2::new(1.0, 1.0) }` (`Pos2::new` is `const`-usable — `placeholder.rs`'s
   `const CANVAS_RECT` already does this `[measured: placeholder.rs:42-44]`).

### Fallible bake, NOT marshrutka's `.unwrap()` chain — the load-bearing deviation

marshrutka's `svg_to_texture`/`init_emojis` panics in four places
(`Tree::from_data(..).unwrap()`, `Pixmap::new(..).unwrap()`,
`IntSize::from_wh(..).unwrap()`, `scale_*(..).unwrap_or_else(..)`)
`[measured: ../marshrutka/src/emoji.rs]`. This project's **`panic-gate` hook
fires on every production `.unwrap()`/`.expect(`/`panic!(` outside `#[cfg(test)]`
in any `.rs` file** `[measured: .claude/settings.json:73]`, `self-review` REJECTs
a new production panic site lacking an `ai-docs/panic-index.md` row, and the
panic-index is intentionally EMPTY `[measured: cat ai-docs/panic-index.md → header only, zero rows]`.
So the port returns `Result` with a `thiserror` enum instead of panicking
(AGENTS.md § Error types: `thiserror` for new error enums):

```
#[derive(thiserror::Error, Debug)]
pub enum IconError {
    Parse(..),          // usvg parse failure (reachable for arbitrary SVG bytes via pub bake_texture)
    PixmapAlloc { width, height },  // tiny_skia::Pixmap::new returned None
}
```

This keeps `gp-render` at **zero production panics** (no panic-index churn, no
`# Panics` doc sections, no self-review friction), and the error is genuinely
reachable because `bake_texture` is `pub` and accepts arbitrary bytes — a
`Result` there is honest, not defensive boilerplate. Rejected alternative:
port the `.unwrap()` chain and add four panic-index rows — costs the empty-index
invariant, adds self-review REJECT surface (`.expect` on `Option` is an
auto-REJECT `[measured: .claude/agents/self-review.md:76]`), and buys nothing.

Note on egui's own panic: `ColorImage::from_rgba_premultiplied` panics if
`w*h*4 != rgba.len()` `[measured: epaint-0.35.0/src/image.rs — panic doc on the sibling ctor]`,
but the length comes from the very `Pixmap` we pass (`[pixmap.width(),
pixmap.height()]` vs `pixmap.data()`), so it is unreachable by construction and,
being inside epaint, is outside our panic-gate scope regardless.

### Resolution of the spec's 4 Open questions

| Open question | Decision | Rationale |
|---|---|---|
| **Baked size variants** | **Single logical size**, `ICON_LOGICAL_SIZE_PX: f32 = 18.0` (a new module const, NOT the coincidental `typography::FS_TITLE = 18.0` — that is a *font-size* token, wrong semantics `[measured: grep tokens → typography.rs:38]`); DPI applied via `pixels_per_point` inside the bake. Cache keyed by `Icon` **alone**. | Supersedes the spec Key-decisions table's provisional "(icon, size)" toward the Open-question default. The only initial consumer is #13's IconButton row (~18px glyphs); Button icons carry none. `bake_texture` stays size-parametric, so a future `(Icon, size)` cache is additive, not a rewrite (YAGNI). |
| **Direct `tiny-skia` dep vs. re-export** | **Direct dep**, `tiny-skia = "0.12"`, `default-features = false`, `features = ["std", "simd"]`. | AC4 requires it added; the `^0.12` range unifies with resvg-0.47's internal `tiny-skia ^0.12` to a single prod copy. The edge is **feature-inert** (see next subsection) — its sole purpose is to pin that single copy, not to trim features; a direct `use tiny_skia::Pixmap` is cleaner than `resvg::tiny_skia::…`. |
| **Icon-name representation** | **Enum** (`Icon`), not string keys. | Compile-time safety; `Icon::ALL` drives the eager bake loop and keeps the vendored set and the code in lockstep. |
| **Bake timing** | **Eager** pre-bake at `IconSet::new` (marshrutka style). | Simpler than lazy; the `Result` covers the (impossible-for-vendored) parse/alloc failure once, at construction. |

### Dependency features & the unavoidable `png` transitive (amended AC4)

The initial design's rationale that our `tiny-skia` `default-features = false`
"drops `png-format`" was **wrong** and is corrected here to match the amended
spec (Key decisions / AC4):

- **Our `tiny-skia` edge is feature-inert.** `resvg 0.47.0` declares its own
  `tiny-skia ^0.12.0` edge with `default_features = true`, and tiny-skia's
  default set is `["std", "simd", "png-format"]` where `png-format → dep:png`.
  Cargo feature unification is **additive**, so our crate's
  `default-features = false, features = ["std", "simd"]` **cannot subtract**
  `png-format` that resvg turned on. The direct edge therefore only pins the
  single `^0.12` copy; it changes no features. `[derived → AC4 cargo tree gate]`
- **`png` (+ `fdeflate` / `simd-adler32`) IS in the prod tree, unavoidably, and
  is ACCEPTED.** It is a **link-only** transitive — the pipeline rasterizes
  SVG → pixmap → `ColorImage` and never decodes a PNG at runtime. AC4 does
  **not** forbid `png`.
- **`resvg default-features = false` still drops the font/raster-decode codecs**
  (`fontdb` / `rustybuzz` / `ttf-parser` / `gif` / `image-webp` / `zune-jpeg`) —
  Lucide icons are text-free stroke/vector paths with no embedded rasters. AC4's
  absence check covers exactly this **droppable** set, plus a single prod
  `tiny-skia`. `[derived → AC4 cargo tree gate]`

### Licensing + vendoring (AC7, plus the coordinator's `.gitattributes` add-on)

- Vendor the 5 Lucide SVGs + Lucide's `LICENSE` (ISC) under
  `crates/render/icons/`, mirroring the `fonts/{onest,jetbrains-mono}/OFL.txt`
  precedent (license file beside the assets).
- Extend the crate `license` SPDX from `(MIT OR Apache-2.0) AND OFL-1.1`
  `[measured: grep license --include=Cargo.toml → crates/render/Cargo.toml:8]`
  to `(MIT OR Apache-2.0) AND OFL-1.1 AND ISC`. **MIT is already present** in
  the `(MIT OR Apache-2.0)` own-code clause, so any Feather-derived (MIT) icon
  among the five needs **no** further SPDX change — only `ISC` is added.
- **`.gitattributes` MUST gain `crates/render/icons/** -text`** (sibling of the
  existing `crates/render/fonts/** -text` `[measured: cat .gitattributes]`),
  and the rule MUST be on disk **before** the SVGs/LICENSE are `git add`-ed —
  otherwise a contributor's `core.autocrlf` silently rewrites EOLs at stage
  time, changing bytes and breaking both the SHA-256 pin and the exact-text ISC
  redistribution. Use `-text` (store byte-for-byte, matching upstream's SHA),
  **not** `text eol=lf`, exactly as the fonts rule does.
- **SHA-256 pin decision: YES, pin the vendored SVGs + LICENSE** (record byte
  size + SHA-256 per file, verify before `git add`), mirroring the fonts
  precedent `[measured: ai-docs/plans/done/2026-07-17-render-onest-font-swap.spec.md:243 — Onest .ttf/.OFL.txt pinned by size+SHA-256, verified before staging]`.
  The design cannot pre-compute the hashes (the implementer downloads them at a
  pinned Lucide release tag); it mandates the discipline and records the pins in
  the vendoring commit message. With the pin in place, `.gitattributes -text`
  is load-bearing **for** the pin; even without it, keep `-text` for
  byte-reproducibility and exact ISC text. Recommendation: keep `-text`
  unconditionally.

## Decomposition

One **orchestrator in-thread pre-step (P)** — the vendored-asset **network
fetch** — plus five **code** subtasks (1–5). The pre-step is carved out because
it FETCHES 5 SVGs + a LICENSE from a pinned upstream tag and computes SHA-256:
network + arbitrary-URL work a background `code-writer` cannot be assumed to
have (AGENTS.md § Workflow — verify the delegate's *environment* fit, not just
its charter). Subtasks 1–5 are all **code** change-type (Rust `*.rs`,
`Cargo.toml`) run against the already-placed assets — no `*.md` / `.claude/**` /
`ai-docs/**` edits (the fallible-bake decision removes all panic-index churn).

| # | Task | Owner | Files | Depends on |
|---|------|-------|-------|------------|
| **P** | **[Orchestrator, in-thread, network] Vendor assets.** Edit `.gitattributes` to add `crates/render/icons/** -text` (rationale comment mirroring the fonts block) **before** any `git add`. Fetch the 5 Lucide SVGs (`play`, `pause`, `grid-3x3`, `zoom-in`, `settings`) + Lucide `LICENSE` from a pinned Lucide release tag into `crates/render/icons/`; record each file's byte size + SHA-256 and **verify before `git add`**. Assets + `.gitattributes` are placed on disk (and may be committed) before Group A is spawned. | **orchestrator** (Step 8, in-thread) | `.gitattributes`, `crates/render/icons/{play,pause,grid-3x3,zoom-in,settings}.svg`, `crates/render/icons/LICENSE` | — |
| 1 | **Add deps + SPDX.** In `crates/render/Cargo.toml`: `resvg = "0.47"` (`default-features = false`) and `tiny-skia = "0.12"` (`default-features = false`, `features = ["std", "simd"]`); extend `license` to `(MIT OR Apache-2.0) AND OFL-1.1 AND ISC`. Run `cargo build`; verify AC4 via `cargo tree -p gp-render -e no-dev` (single prod `tiny-skia 0.12`; droppable set `fontdb`/`rustybuzz`/`ttf-parser`/`gif`/`image-webp`/`zune-jpeg` absent; `png` present + accepted, **not** checked-absent); confirm `git diff --stat Cargo.lock` is only the intended new edges before staging. | code-writer | `crates/render/Cargo.toml`, `Cargo.lock` | P |
| 2 | **`Icon` enum + module wiring + asset-pin guard.** New `crates/render/src/icons.rs`: `Icon` enum (5 variants; derive `Clone, Copy, Debug, PartialEq, Eq, Hash`), `const ALL`, `const fn svg_bytes`/`name` (`include_bytes!` the P-placed vendored SVGs), `const ICON_LOGICAL_SIZE_PX`. Register `pub mod icons;` in `lib.rs`. Miri-clean enum + asset-pin-guard tests (non-empty, `<svg` marker, byte size matches P's recorded pins). | code-writer | `crates/render/src/icons.rs`, `crates/render/src/lib.rs` | P, 1 |
| 3 | **CPU bake step + error type + public bake.** `IconError` (`thiserror`), `svg_to_color_image(svg, logical_px, ppp) -> Result<ColorImage, IconError>`, `pub fn bake_texture(ctx, name, svg, logical_px) -> Result<TextureHandle, IconError>`. Raster tests (AC5), empirically Miri-clean → ungated. | code-writer | `crates/render/src/icons.rs` | 1, 2 |
| 4 | **`IconSet` cache.** `IconSet(HashMap<Icon, TextureHandle>)`, `IconSet::new(ctx) -> Result<Self, IconError>` (eager pre-bake over `Icon::ALL` at `ICON_LOGICAL_SIZE_PX`), `get(icon) -> Option<&TextureHandle>`. Miri-gated construction test (AC1, AC2). | code-writer | `crates/render/src/icons.rs` | 3 |
| 5 | **`draw_icon` helper.** `const FULL_UV: Rect` + `pub fn draw_icon(painter, handle, rect, tint) -> ShapeIdx`. Miri-clean structural test using a hand-built 1×1 texture (AC3). | code-writer | `crates/render/src/icons.rs` | 2 |

(AC7's SPDX + license-file placement is split: the **license file + `.gitattributes`**
land in pre-step P; the **crate `license` SPDX** edit lands in code subtask 1.
AC8's gate list — `clippy -D warnings`, `fmt --check`, `RUSTDOCFLAGS doc`, every
public item `///`d — and AC6's `cargo miri test --workspace` green are the
`/task` Step-9 verify gates, not separate subtasks; each subtask writes its own
`///` docs and its own Miri gate as it lands.)

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping (a)–(h) and `/task`
Step 8's every-group `/context-reset` contract.

- **Orchestrator pre-step P (in-thread, network — NOT a group, NOT delegated).**
  At Step 8, **before** spawning any `code-writer`, the orchestrator (which has
  network) does pre-step P: edit `.gitattributes` (`crates/render/icons/** -text`)
  **first**, then FETCH the 5 Lucide SVGs + `LICENSE` from a pinned upstream tag
  into `crates/render/icons/`, record + verify each file's byte size + SHA-256
  **before `git add`**. The `.gitattributes` rule must be on disk before any
  `git add` under `crates/render/icons/` (the EOL rule is read from the
  working-tree `.gitattributes` at stage time). This is carved out of the code
  group because a background `code-writer` cannot be assumed to reach the network
  (AGENTS.md § Workflow — delegate *environment* fit). Group A then runs against
  the already-placed assets.
- **(a) Grouping required** — 5 code subtasks (M ≥ 1), so a `## Handoff plan` is
  mandatory (this section).
- **Group A** — **code** change-type, implemented via the **`code-writer`**
  subagent (`model: sonnet`, effort **`medium` (pinned in frontmatter)**,
  1M-token window) — **subtasks 1–5**. All are Rust `*.rs` + `Cargo.toml`
  against P's already-placed assets; there are no instructions/harness (`*.md` /
  `.claude/**` / `ai-docs/**`) subtasks, so no second group and no model split.
  **Terminal group** (5 subtasks; within `1..=10`).
  - **Ordering inside Group A** is dependency-respecting: 1 first (needs P's
    assets present for the `cargo build`/`cargo tree` verify and licence SPDX);
    2 needs P + 1; 3 needs 1 + 2; 4 needs 3; 5 needs 2. A valid linear order is
    `1, 2, 3, 4, 5`.
- **(c) Entry handoff** — the every-group handoff fires even for the first
  group: Group A is entered by spawning **`/context-reset`** per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry),
  **after** pre-step P completes. The single group completes `/task` Step 8 in
  its own `/context-reset` subagent.
- **(e) Homogeneity** — Group A is homogeneous (code only). **(f)
  Minimization** — one code group is the fewest possible; no reordering across
  groups is needed since there is only one. **(h) Max-groups** — 1 code group,
  well under the default cap of 4; no user gate needed.
- No inter-group handoff exists (single code group; pre-step P is orchestrator
  work, not a group).

## Risks

- **resvg 0.47 / usvg 0.47 API drift from marshrutka's 0.45.** `usvg::Tree::from_data(&[u8], &Options)`,
  `resvg::render(&Tree, Transform, &mut PixmapMut)`, and the `resvg::usvg`
  re-export are used at 0.45 by marshrutka `[measured: ../marshrutka/src/emoji.rs]`;
  the 0.47 signatures are asserted equivalent but not built here. Mitigation:
  `[derived → cargo build gate (Step 9)]` — a signature change surfaces as a
  compile error the implementer resolves against the 0.47 docs.
- **AC4 "single tiny-skia" false-fail from a pre-existing dev-only copy.** A
  `tiny-skia 0.11.4` is already in the graph via `winit → sctk-adwaita`
  `[measured: cargo tree --invert tiny-skia → tiny-skia v0.11.4 → sctk-adwaita → winit …]`,
  but it is **dev-only** for `gp-render` and absent from its production tree
  `[measured: cargo tree -p gp-render -e no-dev | grep -i tiny-skia → (none)]`.
  A naive all-edges `cargo tree | grep tiny-skia | wc -l` would show two copies
  (0.11.4 dev + 0.12 prod) and falsely fail AC4. Mitigation: verify AC4 with
  `cargo tree -p gp-render -e no-dev` (production edges) and assert **(i)** a
  single prod `tiny-skia 0.12` and **(ii)** the confirmed-droppable set
  (`fontdb`/`rustybuzz`/`ttf-parser`/`gif`/`image-webp`/`zune-jpeg`) absent —
  **NOT** that `png` is absent (`png` is the accepted feature-inert transitive,
  see § Dependency features). The dev-only 0.11.4 is orthogonal to the
  resvg/tiny-skia addition. `[derived → AC4 gate scoped to -e no-dev]`.
- **Denied-lint casts in the size/transform math.** `logical_px * ppp` is raw
  `f32` arithmetic (does NOT trip `arithmetic_side_effects` — `placeholder.rs`
  finding 8 `[measured: placeholder.rs:121-124 comment]`), but the subsequent
  `f32 → u32` (physical px) trips `cast_possible_truncation` + `cast_sign_loss`,
  and `u32 → f32` (transform scale) trips `cast_precision_loss` — all `pedantic`,
  `deny` `[measured: Cargo.toml [workspace.lints.clippy] pedantic = deny]`.
  Mitigation: a documented `#[allow(clippy::cast_possible_truncation,
  clippy::cast_sign_loss, clippy::cast_precision_loss, reason = "…")]` scoped to
  the bake fn, mirroring `placeholder.rs`'s `grid_lines`/`pixel_at` precedent
  `[measured: placeholder.rs:234-239,340-342]`. `[derived → clippy -D warnings gate]`.
- **`missing_const_for_fn` (nursery, deny) FORCES `const fn`** on `Icon::svg_bytes`
  and `Icon::name` — both are pure `match`es returning `&'static` consts, so they
  are const-eligible and the lint fires. They MUST be `const fn` (not a YAGNI
  choice) `[measured: Cargo.toml → nursery = deny]`. `bake_texture` /
  `svg_to_color_image` / `IconSet::*` / `draw_icon` call non-const APIs
  (`load_texture`, `resvg::render`, `HashMap::get`, `painter.image`), so the
  lint correctly does NOT fire on them. `[derived → clippy -D warnings gate]`.
- **Miri abort in the raster tests — resolved empirically (verify-before-gate).**
  This bullet originally *predicted* that any `resvg::render` / `tiny_skia::Pixmap`
  test would abort Miri via a checked-cast, and pre-emptively gated all of them.
  The shipped + verified outcome corrects that: subtask 3's two raster tests
  (`svg_to_color_image_produces_square_rgba` @ `Play/36px`,
  `svg_to_color_image_rejects_garbage`) are **empirically Miri-clean** and left
  **ungated** — this CPU-bake call site does NOT hit the vello_cpu checked-cast
  path. Only subtask 4's `icon_set_bakes_all_five` aborts, bisected to
  `Icon::Settings`: baking `settings.svg@18` panics at
  `tiny-skia-0.12.0/src/pipeline/mod.rs:205` (*"range end index 330 out of range
  for slice of length 324"* — a `"simd"` scanline slice over-read, Miri-interpreter
  only, native passes). Only that one test is `#[cfg_attr(miri, ignore = "…")]`d,
  with a mechanism-specific reason explicitly distinguished from the vello_cpu
  class. The discipline that produced this: **gate only tests that actually abort,
  after running them under Miri — never a copied/fabricated reason string**
  (AGENTS.md § Rust Test Conventions). Net: strictly better coverage than the
  original prediction (fewer ignores), AC6 met by the letter. `[derived → AC6 Miri gate: measured workspace exit 0, gp-render 34 passed / 0 failed / 3 ignored]`.
- **`#[from] usvg::Error` viability.** `IconError::Parse` intends `#[from]` on the
  usvg error, which requires `usvg::Error: std::error::Error`. If the upstream
  type does not satisfy `#[from]`, wrap it via `#[error("…: {0}")]` with a
  `String`/display instead. `[derived → cargo build gate]`.

## Test Design

All tests live in `crates/render/src/icons.rs`'s `#[cfg(test)] mod tests`
(unit tests, same-file convention; AGENTS.md § Rust Test Conventions). Tests use
`.expect("reason")`, never bare `.unwrap()` (self-review §61) — and `.expect`
inside `#[cfg(test)]` is invisible to the panic-gate hook `[measured: .claude/settings.json:73 awk skips inside #[cfg(test)] mod]`.

**Miri split (AC6), as shipped + verified:** exactly **one** test is gated —
subtask 4's `icon_set_bakes_all_five` (the only one that actually aborts; it
bakes `settings.svg@18`). Every other icon test, **including subtask 3's two
resvg/tiny-skia raster tests**, runs **ungated + Miri-clean** — empirically
verified, not assumed (AC6 gates only tests that actually abort).
`[measured: gp-render under Miri = 34 passed, 0 failed, 3 ignored]`

### Subtask 2 — `Icon` enum + asset-pin guard (Miri-clean)
- Entry points: `Icon::ALL`, `Icon::svg_bytes`, `Icon::name`.
- `all_icons_have_nonempty_svg_bytes`: for each `Icon::ALL`, `svg_bytes()` is
  non-empty and contains `b"<svg"`. (Byte-slice ops only — no resvg/tiny-skia,
  Miri-clean.)
- `vendored_svg_byte_sizes_match_recorded_pins` (the asset-pin guard): each
  `Icon::svg_bytes().len()` equals the byte size pre-step P recorded for that
  file (a dep-free identity guard against accidental truncation/corruption of
  the `include_bytes!`'d asset). The authoritative **SHA-256** pin is verified by
  P before `git add`; this test deliberately does **not** duplicate the hash as a
  brittle runtime check — mirroring the fonts design's explicit rejection of a
  second identity check `[measured: ai-docs/plans/done/2026-07-17-render-onest-font-swap.design.md:111 — "Duplicates AC1's SHA-256 pin … brittle second identity check. Assert the requirement"]`.
- `icon_all_and_names_are_the_five_distinct_variants`: `ALL.len() == 5`; the five
  `name()`s are pairwise distinct.

### Subtask 3 — CPU bake step (Miri-**CLEAN, ungated** — empirically verified as shipped)
- Entry point: `svg_to_color_image`.
- Both tests below shipped **ungated**: this resvg-0.47/tiny-skia-0.12 CPU-bake
  call site was **empirically confirmed Miri-clean** (ran + PASSED under
  `MIRIFLAGS=-Zmiri-tree-borrows … cargo miri test -p gp-render …`), i.e. it does
  NOT hit the checked-cast abort `tessellation_smoke`'s vello_cpu path hits.
  Leaving them ungated is correct per AC6 (gate only tests that *actually* abort)
  and § Risks' no-fabricated-reason rule; the verification command is recorded in
  each test's doc comment. `[measured: workspace MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --workspace → exit 0; gp-render 34 passed, 0 failed, 3 ignored — these two run clean]`
- `svg_to_color_image_produces_square_rgba` (ungated): bake `Icon::Play` at
  `logical_px = ICON_LOGICAL_SIZE_PX`, `ppp = 2.0`; assert the returned
  `ColorImage::size()` equals the expected physical dimensions (exact — a square
  Lucide viewBox scaled to `18 * 2 = 36` → `[36, 36]`), the pixel buffer is
  non-empty, and the **alpha channel varies**
  (`pixels.iter().map(Color32::a).collect::<HashSet<_>>().len() > 1`
  — opaque strokes over a transparent field) (AC5).
- `svg_to_color_image_rejects_garbage` (ungated — reaches `usvg::Tree::from_data`):
  `svg_to_color_image(b"not an svg", 18.0, 1.0)` is `Err(IconError::Parse(..))`
  (`assert_matches!`; `IconError: Debug` via the derive).
- Fixtures: none beyond the vendored bytes.

### Subtask 4 — `IconSet` cache (Miri-**gated** — the one genuinely-aborting test)
- Entry points: `IconSet::new`, `IconSet::get`.
- `icon_set_bakes_all_five` (**gated**): `let ctx = egui::Context::default(); let
  set = IconSet::new(&ctx).expect("vendored icons bake"); for icon in Icon::ALL {
  assert!(set.get(icon).is_some()); }`, and the five `TextureHandle::id()`s are
  pairwise distinct (AC1, AC2). `Context::default()` needs no fonts/window — the
  marshrutka `init_emojis` test proves a bare Context bakes textures
  `[measured: ../marshrutka/src/emoji.rs mod tests::init_emojis_get uses Context::default()]`.
- **Miri gate reason (bisected, mechanism-specific — NOT the vello_cpu class):**
  baking **`settings.svg` at width 18** (unlike subtask 3's `Play@36`) panics
  inside `tiny-skia-0.12.0/src/pipeline/mod.rs:205` — *"range end index 330 out
  of range for slice of length 324"*, an actual **slice-bounds over-read** (a
  tiny-skia `"simd"` scanline-width edge case for that icon's stroke geometry)
  that reproduces **only under Miri's interpreter** (native `cargo test` passes).
  Isolated to `Icon::Settings` by per-variant bisection. The `#[cfg_attr(miri,
  ignore = "…")]` reason names this slice-over-read site and is **explicitly
  distinguished** from the `tessellation_smoke`/vello_cpu checked-cast abort
  (AGENTS.md § Rust Test Conventions — write this test's own reason, never a
  sibling's). `[measured: bisected in progress Decisions log Step-8 subtask-4; workspace Miri exit 0 with this one test ignored]`

### Subtask 5 — `draw_icon` helper (Miri-**clean**)
- Entry point: `draw_icon`.
- `draw_icon_emits_tinted_textured_mesh`: build a real `TextureHandle` from a
  hand-built **1×1** `ColorImage::filled([1, 1], Color32::WHITE)` via
  `ctx.load_texture` — this bypasses resvg/tiny-skia entirely
  `[measured: epaint-0.35.0/src/image.rs:75 ColorImage::filled]`, so no raster
  path and no Miri abort. Run `ctx.run_ui(input, |ui| { draw_icon(ui.painter(),
  &handle, rect, tint); })` with a semi-transparent `tint` (alpha < 255), then
  inspect `output.shapes` (NO tessellation) for the emitted shape. `Painter::image`
  → `Shape::image` builds a **`Shape::Mesh`** (`Mesh::with_texture` +
  `add_rect_with_uv`) `[measured: epaint-0.35.0/src/shapes/shape.rs:373 body]`,
  so assert: exactly one `Shape::Mesh`; `mesh.texture_id == handle.id()`; every
  vertex `color == tint` (tint + alpha honored — AC3); the four vertex `uv`
  corners equal `FULL_UV`'s corners. Drawing an image (not text) invokes no
  glyph raster and `run_ui` does not tessellate, so the whole test is Miri-clean.
- Fixtures: the 1×1 texture; no fonts (`set_fonts` unnecessary — no text drawn).

## Open questions

None. The spec's four Open questions are resolved in § Approach → "Resolution of
the spec's 4 Open questions"; the coordinator's `.gitattributes` add-on and the
SHA-256-pin decision are resolved in § Approach → "Licensing + vendoring" and
executed by orchestrator pre-step **P** (network fetch + `.gitattributes` +
SHA-256), with the crate `license` SPDX edit + the dep verify in code subtask 1
and the dep/`png` reconciliation in § Approach → "Dependency features". The
amended AC4 (`png` accepted, feature-inert `tiny-skia` edge, `-e no-dev` scope)
is reconciled throughout § Decomposition / § Risks / § Approach.

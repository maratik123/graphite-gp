# gp-render: core widgets — Button, IconButton, Badge, Tag, Card

**Source:** issue #13
**Date:** 2026-07-18
**Tracked in:** #13
**Prerequisite (must land first):** #88 (open) — *gp-render: SVG icon pipeline —
resvg→egui texture bake (port from marshrutka)*. This spec's icon slots consume
#88's API; #13 cannot be implemented until #88 has landed.
**Depends on:** #11 (closed) — native Rust GUI backend scaffold · #12 (closed) —
design tokens → Rust consts.

Port the five design-system **core** components to native `gp-render` (crate
`gp-render`, at `crates/render`) widgets, styled entirely from the design-token
consts (`crate::tokens`) that already exist in `crates/render/src/tokens`. Port
the *spec* (`.d.ts` prop contract + `.jsx` style tables), not the web code.

## Scope

1. **Button** — variants `primary` / `secondary` / `ghost` / `danger` (the
   `.d.ts` adds `danger` beyond the issue body's three; AC6 "mirror the `.d.ts`"
   governs), sizes `sm` / `md` / `lg`, `iconLeft` + `iconRight` icon slots
   (consume the #88 SVG-icon pipeline — see Key decisions), `fullWidth`,
   `disabled`, and hover/press visual states (hover-darken; press →
   inset shadow + 1-pt downward nudge). UI face (Onest), semibold, radius-2.
2. **IconButton** — square (dim = 30 / 38 / 46 for sm/md/lg), variants
   `secondary` / `ghost`, `active` toggle (graphite fill), `disabled`,
   hover/press; single glyph icon slot (consume the #88 SVG-icon pipeline);
   `label` as accessible/hover text.
3. **Badge** — tones `neutral` / `accent` / `ok` / `warn` / `danger`, `solid`
   (filled) vs tinted, pill radius, mono face, uppercase-ish label.
4. **Tag** — square chip (radius-0), optional leading color dot, `selected`
   state (2-pt graphite border), optional remove (×) affordance, mono face.
5. **Card** — paper face (`surface-card`), hairline border (`selected` → 2-pt
   graphite), radius-2, `elevation` 0–3 → shadow tokens, `eyebrow` + `title`
   header, optional `right` header slot, optional faint grid watermark, body
   content, `padding`.
6. **Style-resolution layer** — a pure `variant/size/tone/state → resolved
   colors + metrics` mapping, unit-testable independent of live pointer input,
   so the Test-notes' "assert state→style mapping" is achievable Miri-clean.
7. **Specimen / gallery** — renders all five widgets across their
   variant/size/state matrix for by-eye comparison against
   `docs/design-system/guidelines/*.card.html` (exact form left to design).

### Per-component style mapping (ported from the `.jsx`; grounds the port)

**Button** — `background: active ? bgActive : (hover ? bgHover : bgRest)`;
`border: bw-1 solid <border>`; radius-2; press adds `SHADOW_INSET` + 1-pt nudge;
`disabled` → opacity 0.45.

| variant | rest bg | hover bg | press bg | fg | border |
|---|---|---|---|---|---|
| primary | `accent` | `accent-hover` | `accent-press` | `text-on-accent` | transparent |
| secondary | `paper-0` | `paper-2` | `paper-3` | `text-ink` | `border-strong` |
| ghost | transparent | graphite@6% | graphite@12% | `text-body` | transparent |
| danger | `danger-tint` | `danger` | `accent-press` | `danger` → `text-on-accent` on hover | `danger` |

Sizes: sm `control-h-sm`/pad-x 12/`fs-sm`; md `control-h-md`/16/`fs-body`; lg
`control-h-lg`/22/`fs-title`.

**IconButton** — `bg: active ? graphite-900 : (variant ghost ? graphite@6%/12% :
paper-0/2/3)`; `fg: active ? paper-0 : text-ink`; `border: active ? graphite-900
: (ghost ? transparent : border-strong)`; radius-2; press → `SHADOW_INSET`.

**Badge** — height 20, pad-x 8, `fs-xs`, `fw-medium`, `ls-mono`, radius-pill.
`solid` → fg `paper-0`, bg `solidBg`, transparent border; tinted → fg/bg/border
per tone:

| tone | tint bg | tint fg | border | solid bg |
|---|---|---|---|---|
| neutral | `paper-2` | `text-body` | `border-hairline` | `graphite-900` |
| accent | `accent-tint` | `accent-press` | `accent` | `accent` |
| ok | `ok-tint` | `#1E6B3C` | `ok` | `ok` |
| warn | `warn-tint` | `#8A6410` | `warn` | `warn` |
| danger | `danger-tint` | `danger` | `danger` | `danger` |

**Tag** — height 26, pad-x 10, `fs-sm`, mono, `text-ink`, radius-0; rest
`paper-0` + hairline `border-hairline`; `selected` `paper-2` + `bw-1`
`border-strong`. Color dot: 10×10 circle, `bw-1` `graphite-900` ring. Remove
button: 16×16, hover bg `paper-3`, `text-muted`, radius-1.

**Card** — `surface-card` fill; border `bw-hair` `border-hairline`, `selected` →
`bw-2` `border-strong`; radius-2; `elevation` 0/1/2/3 → `SHADOW_0/1/2/3`
(default 1). Eyebrow: mono `fs-xs` uppercase `ls-label` `text-muted`. Title:
display (Onest) `fs-title` semibold `text-ink` `lh-snug`. Grid watermark: the
`effects` `BG_GRID_*` / `BG_DOTS_*` decomposition at pitch `spacing::CELL`, ~0.5
opacity.

## Out of scope

- `render_frame` — the track / asphalt / wall / S-F / car frame drawing
  (`docs/design.md` §4); it stays `todo!()` and is a separate task.
- The SVG icon pipeline **itself** — resvg→egui texture baking is #88's job
  (this spec's prerequisite); #13 only *consumes* #88's API, it does not build
  the bake path.
- marshrutka's `tl` + `simplecss` HTML/CSS-ingestion path — explicitly excluded:
  #11 chose native widgets over running the web code and #12 already ported the
  tokens to Rust consts, so no HTML/CSS ingestion belongs in this crate.
- Window / event-loop ownership — remains in `gp-game`; this crate stays
  draw-facing (widgets receive an egui context, they do not own a window).
- Dark-mode / theming — the tokens are a single light theme.
- The web-only `style?: CSSProperties` escape hatch (dropped).

## Deferred

- SVG icon pipeline (resvg→egui texture bake) | Lucide is web SVG and needs a
  real bake path — too large to inline here | **carved out to prerequisite #88**
  (already created, open; must land before #13).

## Key decisions

| Question | Decision |
|---|---|
| Interaction model | **Default:** interactive egui widgets returning `egui::Response` (hover/press/active/selected derive from egui input via a `Ui`), NOT pure `Painter` draw functions — the ACs require live hover/press, which egui can only supply through `Ui`/`Response`. Compatible with `gp-game`'s window ownership (`gp-game` passes the `Ui` down). Design may revisit (Open questions). |
| React-only props | `.d.ts` prop surface is the contract (AC6). Callback props (`onClick`, `onRemove`) → `Response.clicked()` (no stored closures); `type` / `aria-label` / `title` → dropped or mapped (`label` → egui accessible/hover text); `style` → dropped. |
| Toggle / selected state | Caller-owned bool passed in; widget renders it and returns a `Response` for click detection (egui `selectable_label` pattern). Applies to Tag `selected`, IconButton `active`, Card `selected`. |
| Non-token source colors | The ghost hover/press overlays (`rgba(32,30,26, 0.06/0.12)`) appear in **Button + IconButton (2 sites)**, and Badge's `ok`/`warn` tinted foregrounds (`#1E6B3C` / `#8A6410`) are raw hex — none are in `crate::tokens`. Design chooses crate-const-vs-inline placement (flag the 2-site count per the shared-const rule). |
| Icon slots | Consume the **#88** SVG-icon pipeline (prerequisite). A slot (Button `iconLeft`/`iconRight`, IconButton glyph) takes an icon identifier/handle that #88 resolves to a baked `egui::TextureHandle`, drawn via `painter.image(id, rect, uv, tint)` honoring tint/alpha. The concrete handle/identifier type is **owned by #88** — reference it abstractly here; #13 pins the exact type via a Design Amendment when implemented against #88's landed API. |

## Technical constraints

- Style **only** from `crate::tokens` (`color`, `spacing`, `typography`,
  `effects`); any semantic literal not already a token becomes a module
  `const` (code-style magic-number rule). Two non-token source colors exist
  (ghost overlays; badge ok/warn fg) — see Key decisions.
- Token metrics are `f32` logical points and are used directly; radii saturate
  via `From<f32> for CornerRadius` at the use site (see `spacing.rs`). No
  integer-only constraint applies to this crate.
- Fonts: use the registered families — `fonts::ONEST_{MEDIUM,SEMIBOLD,BOLD}`,
  `fonts::JETBRAINS_MONO_{REGULAR,MEDIUM}`. Button/IconButton = UI (Onest
  semibold); Badge/Tag = mono; Card title = display (Onest); Card eyebrow = mono
  uppercase. Any path that lays out text resolves a `FontFamily::Name(..)`, so
  the caller must have installed `fonts::definitions()` first (same precondition
  `draw_placeholder` documents).
- Prerequisites already present in `crates/render/src`: the token consts
  (`tokens::{color,spacing,typography,effects}`) and the egui backend scaffold
  (`fonts`, `placeholder`, the `egui` / `egui_kittest` deps).
- **Miri:** any wgpu / `egui_kittest` golden-image test MUST be
  `#[cfg_attr(miri, ignore = "...")]` (a red workspace Miri blocks merge).
  Prefer Miri-clean unit tests on the pure style-resolution layer for
  state→token assertions; reserve wgpu goldens for by-eye/pixel checks only.
- File size: five widgets — design decides the module split (a `widgets/`
  submodule per widget vs. grouped), honoring the soft 500/800 limits and the
  counter-rule against one-struct-per-file over-splitting.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | Button renders `primary` / `secondary` / `ghost` / `danger` variants, `sm` / `md` / `lg` sizes, `iconLeft` (+`iconRight`), `fullWidth`, and `disabled`, with hover-darken and press → inset-shadow states, every color/metric sourced from `crate::tokens` per the Button mapping table. The `iconLeft`/`iconRight` slots draw a baked texture from #88's SVG-icon API (this AC's icon portion is gated on #88 landing). |
| AC2 | IconButton renders as a square (30/38/46), supports `secondary` / `ghost` variants, the `active` toggle (graphite fill), `disabled`, and hover/press, per the IconButton mapping. The glyph slot draws a baked texture from #88's SVG-icon API (gated on #88 landing). |
| AC3 | Badge renders the five tones in both `solid` and tinted forms, pill radius, mono face, per the Badge mapping. |
| AC4 | Tag renders a square chip with optional leading color dot, the `selected` state (2-pt graphite border), and an optional remove affordance, per the Tag mapping. |
| AC5 | Card renders the paper face, hairline border (`selected` → 2-pt graphite), radius-2, `elevation` 0–3 → shadow token, `eyebrow` + `title` header, optional `right` slot, and the optional grid watermark, per the Card mapping. |
| AC6 | Each widget's public prop surface mirrors its `.d.ts` contract (variants / sizes / tones / flags / slots), minus the web-only props mapped or dropped per Key decisions; icon-slot props type against #88's icon handle/identifier. |
| AC7 | The pure style-resolution layer has unit tests (Miri-clean) asserting the state→style mapping: variant → color token, size → height, tone → token, and pressed → `SHADOW_INSET`. |
| AC8 | A specimen/gallery renders all five widgets across their variant/size/state matrix for by-eye comparison against `docs/design-system/guidelines/*.card.html`. |
| AC9 | `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, the doc gate, and the workspace Miri job stay green (any wgpu golden is Miri-ignored). |

## Open questions

- **Interaction model** — the spec defaults to interactive `Response`-returning
  widgets (Key decisions). If a static caller-supplies-state form is preferred,
  design can adopt it via a Design Amendment.
- **#88 icon-handle type** — resolved to *consume #88's SVG-icon pipeline* (Key
  decisions). The concrete handle/identifier type is owned by #88; #13 references
  it abstractly and pins the exact type via a Design Amendment once #88 lands.
- **Specimen form** — whether the AC8 gallery is a shippable `examples/`
  binary, a test-only harness, or a golden-tested render is design's
  test-coverage call, not a spec constraint.

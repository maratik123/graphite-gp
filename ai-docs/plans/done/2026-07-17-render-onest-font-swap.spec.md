# gp-render: swap Space Grotesk → Onest and drop egui's bundled default fonts

**Source:** issue #73
**Date:** 2026-07-17
**Tracked in:** #73

Replace the vendored display/UI face (Space Grotesk → **Onest**) and turn off
egui's `default_fonts` feature. The two are one task: Space Grotesk has **zero
Cyrillic**, which is the only reason egui's Ubuntu-Light is load-bearing today.
Fix the face and the bundled faces become droppable.

The candidate decision (**Onest**) was made by the product owner on 2026-07-17
and is **not re-opened here**. Golos Text, Unbounded, and Cygre were evaluated
and rejected on the record in #73's comments.

> **This spec deviates from #73's text in two places, both deliberate and both
> decided by the product owner — see Key decisions 8 and 9.** A reader diffing
> spec against issue should not read either as drift:
> 1. #73's *"Glyph coverage — no gaps … all present in both faces"* is
>    **factually wrong**: U+2713 is absent from all six faces (measured).
> 2. #73's prescribed AC10 replacement — *"Onest-only Proportional, JBM-only
>    Monospace"* — is **superseded**: `Monospace` becomes
>    `[JetBrainsMono-Regular, Onest-Regular]`.

## Scope

1. **Vendor Onest**, remove Space Grotesk. `Onest[wght].ttf` + its `OFL.txt`
   from `google/fonts` at the pin already used by #12
   (`389b770410cc0b7c21c85673bfa2077420fe7f65`, path `ofl/onest/`), into
   `crates/render/fonts/onest/`. Delete `crates/render/fonts/space-grotesk/`
   (both files). SHA-256 verified before staging.
2. **`crates/render/Cargo.toml`** → `egui = { version = "0.35", default-features = false }`.
3. **`crates/game/Cargo.toml`** → `eframe = { version = "0.35", default-features = false, features = ["accesskit", "wayland", "web_screen_reader", "wgpu", "x11"] }`, **plus a direct `winit = { version = "0.30", default-features = true }`**. eframe's `default` also carries `"winit/default"`, which a dependent **cannot** express — the direct `winit` dep restores it by feature unification instead (Key decision 12).
4. **`crates/render/src/fonts.rs`** — `SPACE_GROTESK` byte const + the four
   `SPACE_GROTESK_*` key consts → Onest equivalents; build explicitly from
   `FontDefinitions::empty()`; keep the explicit `coords` override on every
   instance; add Onest behind JetBrains Mono in the mono families (Key
   decision 8 + constraint 3); correct the module/`definitions()` doc
   comments, which currently assert the opposite of the new behaviour.
5. **`crates/render/src/tokens/typography.rs`** — `FONT_DISPLAY` / `FONT_UI`
   → `"Onest"`.
6. **`docs/design-system/tokens/fonts.css`** — swap the family + the Google
   Fonts `@import`; its own header already sanctions the substitution.
7. **Replace #12's AC10 assertion** in `crates/render/src/fonts.rs`'s test
   module with an exact-family-list assertion (see constraint 2 — the current
   assertion does not fail, it goes **vacuous**).
8. **Spec-amend #12** (`ai-docs/plans/done/2026-07-17-render-design-tokens.spec.md`)
   — AC9 and AC10.
9. **Text sample in `draw_placeholder`** + regenerate the golden, so the font
   path is exercised by a picture rather than only structurally.
10. **Measure** the release binary before/after.

## Out of scope

- Re-litigating the face choice (Onest is decided; alternatives rejected on
  the record in #73).
- `render_frame` — stays `todo!()`; nothing drives it until `gp-gen` lands.
- The #19–#22 UI port of `Screens.jsx` / `MovePad.jsx`. **In particular this
  task does not port, edit, or re-tone `Screens.jsx`'s phase table** — it only
  makes the font stack able to draw what that table already contains.
- #17's wholesale replacement of `placeholder.rs` (this task's text sample is
  deliberately temporary and dies with it).
- Any `--fs-*` / `--lh-*` / `--ls-*` retuning to suit Onest's larger x-height
  (+8.4% x/em). Tokens are #12's contract and stay as they are.

## Deferred

| What | Why | Separate issue? |
|---|---|---|
| `--fs-*` / `--ls-*` retune for Onest's +8.4% x/em | Needs a rendered specimen + the product owner's eye; the design system's type scale was drawn against Space Grotesk | Yes, if the regenerated golden looks off |
| Golos Text as a fallback pick if Cyrillic *typography quality* later leads over geometric fit | #73 comment 2 records it as runner-up (+48 KB) | No — recorded in #73 |
| The `Ф6 'warn'` row renders the literal word `repair` while the six `'ok'` rows render a glyph — a glyph-vs-word asymmetry inside one pill | Cosmetic mockup-data question, noticed while tracing U+2713; belongs to whoever ports the phase table | Yes — #19–#22's call, not this task's |

## Key decisions

| Question | Decision |
|---|---|
| 1. Which face? | **Onest**, decided by the product owner. Verified at the pin: 124,376 B (−12,300 vs Space Grotesk's 136,676), `wght` 100/**400**/900, Cyrillic 64/64, `OS/2` x-height 527 / cap 707. Metrically the closest candidate (x/cap +7.4%, x/em +8.4%). |
| 2. Licence | **Unchanged.** Verified at the pin: `METADATA.pb` → `license: "OFL"`; `OFL.txt` copyright line carries **no** Reserved Font Name clause. AC15 (`license = "(MIT OR Apache-2.0) AND OFL-1.1"`) needs no edit; per-face `OFL.txt` continues to satisfy OFL bundling. |
| 3. Vendoring architecture | **Unchanged.** Same repo, same pin, same `.gitattributes` rule (`crates/render/fonts/** -text`, already covers a new subdirectory), same one-byte-array-per-family + 7-instances-via-`coords` shape. A file swap, not a design change. |
| 4. Keep explicit `coords`? | **Yes, mandatory.** Onest's `wght` default is 400 and Space Grotesk's was 300, but the requirement stands on its own (#12 finding 3). Do **not** drop it on the reasoning that a 400 default makes it redundant. |
| 5. How the bundled faces are dropped | Via the **feature flag only**. `epaint_default_fonts` is an *optional* crate dep gated by `epaint/default_fonts`; turning the feature off removes the crate — and its 1,414,020 B — from the graph. Removing entries from the map at runtime would **not** shrink the binary. |
| 6. Which declarations turn the feature on | **Exactly two, both ours** — verified via `cargo tree -e features`: `crates/render/Cargo.toml`'s `egui = "0.35"` and `crates/game/Cargo.toml`'s `eframe = "0.35"`. No upstream crate forces it: `eframe`, `egui-wgpu`, and `egui_kittest` all declare `egui` with `default-features = false`. Nothing else in the graph mentions the feature. |
| 7. Does any UI surface need emoji? | **No — verified, #73 checklist item closed.** Every non-ASCII codepoint in `docs/design-system` (31 total) was enumerated and tested against both faces' `cmap`s. None are emoji (nothing at U+1F300+). `NotoEmoji` + `emoji-icon-font` are not load-bearing for any glyph the design system uses. |
| 8. `✓` U+2713 — **Onest as the mono fallback** | **Decided by the product owner**, with the size of the motivating case already on the table. `Monospace` becomes `[JetBrainsMono-Regular, Onest-Regular]`, and the same fallback reaches the per-weight mono families (constraint 3). **This is cheap robustness, not a defect repair** — see the framing note below. |
| 9. Glyph coverage after the swap — **#73's "no gaps" line is wrong** | **Corrected, not repeated.** U+2713 is absent from **all six** faces (JetBrains Mono, Space Grotesk, Ubuntu-Light, Hack, NotoEmoji, emoji-icon-font) — measured from their `cmap`s. So `✓` is tofu *today*, before this change; dropping egui's faces does not cause it. Decision 8 makes Onest reachable from mono, which fixes it. The three other codepoints absent from Onest are all non-rendered: `∈` (JSDoc comment, `MovePad.jsx:6`), `∝` (`readme.md` prose), `⊆` (`Screens.jsx:132`, inside `fontFamily: 'var(--font-mono)'` → JetBrains Mono has it). |
| 10. Binary-size figure | **Not asserted — measured.** #73's ~1.36 MiB is an *arithmetic prediction* (1,414,020 + 12,300 = 1,426,320 B = 1.360 MiB) from verified byte counts, **not** an end-to-end measurement. AC15 requires the real number; the prediction is only the comparison baseline. |
| 11. Golden text sample is temporary | Accepted. It buys font verification for the window between #73 and #19–#22, which is otherwise unverified, and dies with `placeholder.rs` at #17. #12's AC12 (golden byte-identical) was #12's constraint and does **not** bind here — this is a regen. |
| 12. eframe's `"winit/default"` is **not copyable** into `gp-game` — direct `winit` dep instead | **`gp-game` declares `winit` directly, purely to carry features.** AC3's list is eframe's `default` minus `default_fonts`, but `"winit/default"` cannot survive the copy: the `pkg/feature` slash syntax is legal **only inside a crate's own `[features]` table**. eframe's `default` may write it (that sits in *eframe's* table); `gp-game` cannot reach through eframe the same way. Cargo rejects the manifest at **parse time**, before resolution — error reproduced verbatim in the framing note below. The earlier derivation ("eframe's `default` minus `default_fonts`") was sound as a *derivation* but produced an **unbuildable line**; both the old AC3 and the design's decomposition subtask 4 asserted it compiles, and that assertion was false. **Only two features were ever at stake** — see the framing note below. **The product owner chose bit-for-bit preservation** of today's Wayland behaviour: `winit = { version = "0.30", default-features = true }` in `gp-game` restores them through Cargo's feature unification. **Rejected alternative:** simply drop `"winit/default"` — builds fine, costs only those two Wayland-only features; rejected in favour of keeping today's behaviour exactly. |

### Framing of decision 12 — what the direct `winit` dep actually buys

The rejected manifest line and its verbatim error (reproduced against
`eframe 0.35`, `cargo metadata`):

```
eframe = { version = "0.35", default-features = false, features = ["accesskit", "wayland", "web_screen_reader", "wgpu", "winit/default", "x11"] }
```

```
error: failed to parse manifest at `.../Cargo.toml`

Caused by:
  feature `winit/default` in dependency `eframe` is not allowed to contain slashes
  If you want to enable features of a transitive dependency, the direct dependency needs to re-export those features from the `[features]` table.
```

Recorded so a future reader neither over-reads the dep nor "cleans it up":

- **Only two of the five were ever at stake.** `winit/default` = `["rwh_06", "x11", "wayland", "wayland-dlopen", "wayland-csd-adwaita"]` (verified, `winit-0.30.13/Cargo.toml:73-79`). Three reach winit regardless of the slash line: `rwh_06` is forced unconditionally by eframe's own manifest (`features = ["rwh_06"], default-features = false`, `eframe-0.35.0/Cargo.toml:272-275`), and `x11` / `wayland` arrive via eframe's own `x11` / `wayland` features → `egui-winit/{x11,wayland}` → `winit/{x11,wayland}` (verified, `egui-winit-0.35.0/Cargo.toml:79-86`) — both of which AC3 keeps. Only **`wayland-dlopen`** and **`wayland-csd-adwaita`** depended on `"winit/default"`.
- **What those two do.** `wayland-dlopen = ["wayland-backend/dlopen"]` makes winit `dlopen` libwayland at runtime rather than link it. `wayland-csd-adwaita = ["sctk-adwaita", "sctk-adwaita/ab_glyph"]` selects `sctk_adwaita::AdwaitaFrame` for client-side decorations; **without it winit falls back to `sctk::shell::xdg::fallback_frame::FallbackFrame`** (`winit-0.30.13/src/platform_impl/linux/wayland/window/state.rs:47-50`) — a **plainer title bar, not a missing one**. Both are **Wayland-only** paths. This is the exact size of what the amendment preserves; it is not a correctness fix.
- **Why `"0.30"`, and why unification applies.** winit resolves to **0.30.13** and `grep -c 'name = "winit"' Cargo.lock` → **1** — a **single** winit package, which is the precondition that makes unification work at all. eframe requires `version = "0.30.13"`, so the `"0.30"` pin is **dictated by eframe's own requirement**, not a free choice: a semver-incompatible pin would silently resolve **two** winits and fix nothing. `"0.30"` also satisfies AGENTS.md § *Dependency Versions* (`0.x` for `0.x.y`, never pin the patch).
- **The dep is never imported and no gate protects it.** `gp-game` never `use`s winit — the dep exists **solely** to carry features. `cargo clippy -p gp-game --all-targets` exits **0**, and `unused_crate_dependencies` is not configured anywhere in the workspace (verified). So nothing mechanical stops a future reader from deleting an apparently-unused dependency; **AC18 is the only thing that makes that fail loudly.**
- **AC4's intent is untouched.** `epaint_default_fonts` is absent from `Cargo.lock` entirely with the direct winit dep in place. The task's actual goal is unaffected by this amendment.

### Framing of decision 8 — proportion, so nobody over-reads it later

The product owner chose this **after** being shown exactly how small the
motivating case is. Recorded so a future reader mistakes it for neither an
urgent bug fix nor an accident:

- **One site, repo-wide.** `grep -rn '✓' docs/design-system/` returns exactly
  one hit: `ui_kits/game/Screens.jsx:194`.
- **It is demo data.** The pill is driven by the hardcoded array at
  `Screens.jsx:153-159` — six rows tone `'ok'` → `✓`; one row (`Ф6`,
  `'Local repair'`) tone `'warn'` → the literal word `repair`.
- **Nothing renders it.** `Screens.jsx` / `Badge.jsx` are an un-ported React
  mockup — `grep -rln 'Badge\|Screens' crates/` returns nothing. They are
  #19–#22's source material, not running code. (Both files are tracked.)
- **User-visible impact of the `✓` itself today: none.** It cannot be seen —
  `render_frame` is `todo!()`.

The justification is **cost**, not urgency: the fallback costs **no new font
bytes** (Onest's array is already linked to serve `Proportional`; this adds
family-list entries, not a file), and it clears a latent tofu out of the face
stack before #19–#22 has to reason about it. Any effect on the shipped binary
is captured by AC15 — which **measures**; no byte claim is made here.

**Accepted side effect, stated plainly:** `Monospace` now carries a
**proportional fallback for *any* glyph JetBrains Mono lacks** — not only
`✓`. A future mono string containing some other JBM-missing codepoint will
silently render it in Onest's proportional metrics inside a monospace run,
rather than showing tofu. That is a real behavioural widening and the
deliberate price of decision 8: it trades a loud failure (tofu) for a quiet one
(a metrically-wrong glyph). AC7's ordering pin is what keeps it confined to the
fallback position.

## Technical constraints

**1. `default()` silently becomes `empty()` — no compile error.**
`epaint-0.35.0/src/text/fonts.rs:499-501` (verified):

```rust
#[cfg(not(feature = "default_fonts"))]
fn default() -> Self { Self::empty() }
```

#12's builder starts from `default()` *specifically* so egui's faces survive
`set_fonts`'s overwrite semantics. Turning the feature off inverts that
rationale silently. The new builder must call `FontDefinitions::empty()`
**explicitly** and say why, not inherit the behaviour from a feature flag's
side effect.

**2. `builtin_font_names()` returns `&[]` — the existing AC10 test goes vacuous, not red.**
`epaint-0.35.0/src/text/fonts.rs:590-593` (verified) returns `&[]` under
`not(default_fonts)`. So `fonts.rs`'s current assertion:

```rust
for builtin in FontDefinitions::builtin_font_names() {
    assert!(fonts.font_data.contains_key(*builtin), "...");
}
```

iterates **zero times** and **still passes**, asserting nothing. This is the
same silent-inversion trap as constraint 1, one layer down: the test keeps
compiling *and* keeps going green while its guarantee has evaporated. It must
be **replaced** with an exact-family-list assertion, not deleted. The
neighbouring `assert_eq!(fonts.font_data.len(), 11)` *will* fail loudly (11 →
7), which is the only reason this gets noticed at all. **#12's AC10 amendment
(AC11 below) exists because of this constraint.**

**3. `empty()` zeroes the fallback the per-weight `Name` families inherit — decision 8 must follow it there.**
`FontDefinitions::empty()` (`fonts.rs:567-576`, verified) inserts
`Monospace => vec![]` and `Proportional => vec![]`: the keys exist, the vectors
are empty. Today's builder snapshots those vectors *before* prepending our
faces and reuses each snapshot as the tail of every per-weight
`FontFamily::Name(key)` family. Under `default()` that tail was egui's fallback
chain; under `empty()` it is **`[]`**, so each `Name` family collapses to
`[key]` with no fallback at all.

This is load-bearing for decision 8, because **the Badge is not
`FontFamily::Monospace`**: `Badge.jsx:21` is
`fontFamily: 'var(--font-mono)', fontWeight: 'var(--fw-medium)'` — mono at
**wght 500**, which resolves through `FontFamily::Name("JetBrainsMono-Medium")`,
not `Monospace` (JBM-Regular @ 400). Adding Onest to `FontFamily::Monospace`
**alone would leave the motivating Badge exactly as tofu as it is today** — a
fix that misses its own use case. The Onest fallback must reach the per-weight
mono `Name` families too. Which Onest weight backs each JBM weight
(weight-matched vs always-Regular) is design's call: Onest carries
400/500/600/700 and JBM 400/500/700, so weight-matching is available for all
three.

**4. The golden harness must install the fonts itself.**
`placeholder.rs`'s `golden_guard` builds an `egui_kittest::Harness` that never
calls `set_fonts`. That is harmless today (no text is drawn). Once the sample
lands, the harness's `Context` starts from `FontDefinitions::default()` — which
is now `empty()` — so it has **no fonts at all** unless the test installs
`fonts::definitions()`. Without that the sample cannot render, and the test
would be exercising a font stack that does not match `gp-game`'s.

**5. `egui_kittest`'s `default_fonts` does not reach us.**
`egui_kittest-0.35.0/Cargo.toml:170` carries `features = ["default_fonts"]`,
but under **`[dev-dependencies.egui]`** — a dependency's dev-deps are not built
downstream. Its regular `[dependencies.egui]` (line 119) is
`default-features = false`. So the test build and the shipped binary agree on
the font set; no divergence to design around.

**6. The 192×128 golden canvas is small.**
`CANVAS_RECT` is 192×128 at `pixels_per_point = 1.0`, already carrying a card,
a grid, and a hairline. `GRAPHITE GP` at `FS_DISPLAY` (56.0) alone overruns it.
Design chooses: smaller sample sizes, or a larger `CANVAS_RECT`. Both are open
— the golden regenerates regardless, and every probe in `geometry()` is already
derived from `rect`, so growing the canvas hardcodes nothing.

**7. The golden is a structural guard, not a typographic one.**
`egui_kittest` hardcodes dify's `detect_anti_aliased_pixels`, so AA-classified
edge pixels are exempt even under `threshold(0.0)` +
`failed_pixel_count_threshold(0)`; #11 recorded this as bit-exact **in flat
regions only**, and glyph edges are almost entirely AA. **Caught:** wrong face,
tofu, mixed-typeface labels, missing text, a weight silently rendering Light —
all change glyph shape and mass. **Not caught:** sub-pixel rasterisation drift
from a `skrifa`/`harfrust` bump — which is a *feature*, keeping the golden from
reddening on every dependency bump. The test's doc comment must say this, or a
later reader will over-trust it.

**8. The golden changes → `image-check` is mandatory.**
This is a regen, so `code-writer` must spawn `image-check` per its contract.

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `crates/render/fonts/onest/Onest[wght].ttf` is vendored from `google/fonts` @ `389b770410cc0b7c21c85673bfa2077420fe7f65`, path `ofl/onest/`, byte-identical to upstream: **124,376 B**, SHA-256 `3faa4b905661849b2332e394b42f91b5bf5575e553c516caa81811e868a4d589`. `crates/render/fonts/onest/OFL.txt` sits beside it (4,384 B, SHA-256 `071195d8806e226faeee60259c28ca67b458227af5195a73f5cfcab06e3003bc`). |
| AC2 | `crates/render/fonts/space-grotesk/` is gone — **both** `SpaceGrotesk[wght].ttf` and its `OFL.txt`; `git ls-files crates/render/fonts/` lists only the `onest/` and `jetbrains-mono/` pairs. |
| AC3 | `crates/render/Cargo.toml` declares `egui = { version = "0.35", default-features = false }`. `crates/game/Cargo.toml` declares **two** deps — `eframe = { version = "0.35", default-features = false, features = ["accesskit", "wayland", "web_screen_reader", "wgpu", "x11"] }` (eframe's `default` minus `default_fonts`, and minus `"winit/default"`, which a dependent cannot express — Key decision 12) **and** `winit = { version = "0.30", default-features = true }`. **`gp-game` never `use`s winit**: the dep exists **solely to carry features** to the one shared winit package via unification, restoring the `wayland-dlopen` + `wayland-csd-adwaita` that `"winit/default"` would have granted. A comment in `crates/game/Cargo.toml` states this, because **nothing mechanical protects the dep** — deleting it as an "unused dependency" still compiles, still passes clippy, and **silently** regresses Wayland CSD to `FallbackFrame` while linking libwayland instead of `dlopen`ing it. AC18 is the guard that makes such a deletion fail loudly. |
| AC4 | `cargo tree -e features -p gp-game` and `-p gp-render` show **no** `egui feature "default_fonts"` node, and `cargo tree --invert epaint_default_fonts` reports the crate is absent from the graph. |
| AC5 | `fonts.rs` builds from `FontDefinitions::empty()` **explicitly**, with a doc comment stating why (constraint 1); no code path relies on `default()` collapsing to `empty()`. The stale module/`definitions()` docs asserting the opposite are corrected. |
| AC6 | 7 weight instances are registered (Onest 400/500/600/700; JetBrains Mono 400/500/700), **each** carrying an explicit `VariationCoords` `wght` override. A test asserts every instance's `coords != VariationCoords::default()`. |
| AC7 | The AC10-replacement test asserts the **exact** family lists by **full-vector equality** — `FontFamily::Proportional == ["Onest-Regular"]` and `FontFamily::Monospace == ["JetBrainsMono-Regular", "Onest-Regular"]` — not `first()` / non-empty / `contains` checks. `font_data.len() == 7`. **Ordering is load-bearing and deliberately pinned:** JetBrains Mono must stay first, or *all* mono text silently renders in Onest's proportional metrics; full-vector equality catches a reorder, `contains` would not. |
| AC8 | The Onest fallback reaches the family the motivating use site actually resolves through (constraint 3): `Badge.jsx:21` is mono at `--fw-medium` (wght 500) → `FontFamily::Name("JetBrainsMono-Medium")`, **not** `FontFamily::Monospace`. A test asserts each per-weight mono `Name` family carries an Onest instance behind its JBM instance, so `✓` renders at the Badge's own weight. |
| AC9 | A test asserts Onest's `wght` axis range covers every registered weight (400–700 within 100–900) and that its `cmap` contains **U+0424 `Ф`** (the glyph the swap exists for) and **U+2713 `✓`** (the glyph decision 8 exists for). |
| AC10 | `crates/render/src/tokens/typography.rs` — `FONT_DISPLAY == FONT_UI == "Onest"`; `docs/design-system/tokens/fonts.css` swapped (family + `@import`); the existing `family_names_match_css` test passes against both. |
| AC11 | #12's spec (`ai-docs/plans/done/2026-07-17-render-design-tokens.spec.md`) is amended per the `/task` Spec Amendment recipe: **AC9** (line 289 — names `SpaceGrotesk[wght].ttf` explicitly, *and* its axis-range parenthetical `SG 400–700 within 300–700` → Onest's `100–900`) and **AC10** (line 290 — `built on FontDefinitions::default()` + "egui's default fallback entries are still present" → AC7's exact-family-list assertion, **including the two-entry `Monospace` list**). The amendment is recorded, not silently rewritten. |
| AC12 | `draw_placeholder` draws a text sample covering `Ф1 – Ф7` (Cyrillic + en-dash + digits, display face), `GRAPHITE GP` (wordmark), and a mono telemetry row (JetBrains Mono + middot + arrow), at ≥2 `--fw-*` weights. The golden is regenerated and `image-check` is spawned at regen. |
| AC13 | The golden test installs `fonts::definitions()` into its harness `Context` (constraint 4), so it exercises the same font stack `gp-game` ships. |
| AC14 | The golden test's doc comment states it is a **structural** guard (AA edges exempt → catches wrong face / tofu / mixed typefaces, not rasterisation drift) and that it dies with the placeholder at #17. |
| AC15 | The release binary is measured **before and after** with the same toolchain + profile. Both figures and the delta are recorded in the design doc / PR body. The delta is a **shrink**; it is compared against the predicted 1,426,320 B and any deviation >5% is explained rather than waved through. |
| AC16 | Confirmed by eye from the regenerated golden that `Ф` renders **in Onest** — not a fallback, not tofu, and not a mixed-typeface `Ф1`. This is the point of the swap and the one thing no structural assertion can prove. |
| AC17 | `cargo build`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` all pass. |
| AC18 | **The winit unification is asserted, not assumed** — same fail-loud principle as AC7's ordering pin. `cargo tree -e features -p gp-game -i winit` shows **all five** members of `winit/default` live on winit's own feature edges: `rwh_06`, `x11`, `wayland`, `wayland-dlopen`, `wayland-csd-adwaita`. `grep -c 'name = "winit"' Cargo.lock` returns **1** — exactly one winit package resolves, the precondition without which unification cannot apply (a semver-incompatible pin would resolve **two** winits and restore nothing, silently). Both results are recorded in the design doc / PR body. A future dependency edit that breaks unification — or a deletion of AC3's `winit` dep — must fail **here, loudly**, rather than silently dropping Wayland decorations. |

## Open questions

**1. Sample sizes vs canvas.** Whether `CANVAS_RECT` grows past 192×128 to fit
the text sample is design's call (constraint 6) — flagged so it is chosen
deliberately rather than discovered at regen time.

**2. Whether the text sample should also render `✓`.** Decision 8 makes it
renderable, and the golden is the only place it could be seen before #19–#22.
Design's call: it is one more glyph in an already-tight canvas, and AC8/AC9
already pin the behaviour structurally. Not blocking.

**3. Onest's letterform character.** x-height ratios do not capture the
technical feel Space Grotesk's `G`/`R`/`a` carry. Onest is the closest match
*defensible by measurement*; the aesthetic verdict needs the regenerated golden
and the product owner's eye (AC16). If it reads wrong, the Deferred retune row
and #73's Golos Text runner-up are the escape hatches.

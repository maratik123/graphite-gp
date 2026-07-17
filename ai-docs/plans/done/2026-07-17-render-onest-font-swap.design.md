# Design: gp-render — swap Space Grotesk → Onest and drop egui's bundled default fonts

**Issue:** #73
**Spec:** `ai-docs/plans/2026-07-17-render-onest-font-swap.spec.md`
**Date:** 2026-07-17

## Approach

The spec's shape is already right: a file swap plus a feature flip. The design's
job is to (a) make the two silent-inversion traps fail **loudly**, (b) close the
two open questions the spec left, (c) pick the Onest↔JBM weight pairing, and
(d) route AC11 through the subagent the `/task` AXIOM names.

Investigation changed the plan in **four** places beyond the spec's text. Each is
recorded as a Key Decision below; two are load-bearing enough to state up front:

1. **`docs/design-system/tokens/typography.css` is missing from the spec's Scope
   list and is MANDATORY.** `crates/render/src/tokens/typography.rs:95`
   `include_str!`s **`typography.css`** — not `fonts.css` — and
   `family_names_match_css` (`:152`) asserts
   `primary_family(CSS, "--font-display") == FONT_DISPLAY`. **No crate reads
   `fonts.css` at all** (verified: `rg 'fonts\.css' crates/` → no hits;
   `rg 'include_str!' crates/render/src/` → four token CSS files, `fonts.css`
   absent). `typography.css:9-10` still say `'Space Grotesk'`. So swapping only
   `fonts.css` (Scope 6 / AC10) leaves `family_names_match_css` **RED**. This is
   not a spec contradiction — AC10 already requires that test to pass and AC17
   requires `cargo test` green — it is an incomplete file list, which is
   design's to fill (spec-writer § *What to leave to the design phase*: file
   layout).
2. **`Fonts::has_glyph` is NOT a sound oracle — rejected.** See § *Rejected
   alternatives*. AC9's cmap assertion goes through **`skrifa`** (already in the
   graph) instead.
3. **The empty-family vs missing-family distinction** (below) is the premise D11
   and § Test Design § `golden_guard` each derive from — **separately, to
   opposite conclusions**. Design round 1 conflated the two and shipped a
   provably-panicking harness sequence; the premise is now stated once, in one
   place, so the two derivations cannot drift back together.

### Load-bearing premise — empty family ≠ missing family

`FontsImpl::font` (`epaint fonts.rs:1027-1031`, verified) does **not** degrade on
a family it cannot find:

```rust
let fonts = &self.definitions.families.get(family);
let fonts = fonts.unwrap_or_else(|| panic!("FontFamily::{family:?} is not bound to any fonts"));
```

Two different paths, routinely mistaken for one:

| Case | Map state | Behaviour | Where it bites |
|---|---|---|---|
| **Empty** family | key **present**, `vec![]` | **Silent.** `CachedFamily::new` returns `replacement_face_key: FontFaceKey::INVALID` (`fonts.rs:646-653`); `glyph_info` then yields `GlyphInfo::INVISIBLE` (`font.rs:767-770`). Text lays out invisibly, no panic. | `Proportional` / `Monospace` only — the **two** keys `empty()` binds (`fonts.rs:567-576`). |
| **Missing** family | key **absent** | **Loud panic**, at layout time. | **Every `FontFamily::Name(..)`** — `empty()` binds none of them. |

**Every row of this design's sample is a `FontFamily::Name(..)`** (D1/D3 — that
is the whole point of the per-weight families), so the sample takes the
**missing** path exclusively. It never touches the silent one. This is
independent of the feature flip: `Name` families are unbound in a default
`Context` whether `default_fonts` is on or off.

That is a **robustness property, not a hazard**: a forgotten `set_fonts` is a
panic naming the exact unbound family, not an invisible frame. It is also why the
harness needs a real mechanism rather than a tweak (§ Test Design § `golden_guard`).

### Chosen solution

**Builder shape — explicit vectors, no snapshot.** Today's builder snapshots
`families[Proportional]` / `families[Monospace]` *before* prepending, then reuses
each snapshot as the tail of every per-weight `Name` family. Under `empty()`
both snapshots are **statically `[]`** (`epaint fonts.rs:567-576`), so the
snapshot machinery silently degrades to a no-op that produces exactly
constraint 3's collapse. The builder is rewritten to `insert` **fully-written
family vectors**, so the code reads as a mirror of AC7's full-vector assertions
and the ordering pin is visible at the call site rather than emergent.

Resulting map — 7 `font_data` entries, 9 `families` entries:

| Family | List |
|---|---|
| `Proportional` | `["Onest-Regular"]` |
| `Monospace` | `["JetBrainsMono-Regular", "Onest-Regular"]` |
| `Name(k)` for each Onest key `k` (Regular/Medium/SemiBold/Bold) | `[k]` — single-entry |
| `Name("JetBrainsMono-Regular")` | `["JetBrainsMono-Regular", "Onest-Regular"]` |
| `Name("JetBrainsMono-Medium")` | `["JetBrainsMono-Medium", "Onest-Medium"]` |
| `Name("JetBrainsMono-Bold")` | `["JetBrainsMono-Bold", "Onest-Bold"]` |

The proportional `Name` families stay **single-entry** — AC7 pins
`Proportional == ["Onest-Regular"]` (a one-entry list), so a JBM tail on the
proportional side would contradict the spec's own posture. No known proportional
use site needs JBM: Key decision 9's one JBM-only glyph (`⊆`) is at
`Screens.jsx:132` *inside* `fontFamily: 'var(--font-mono)'`.

**Feature-independence is a property, not an accident.** Because the builder
calls `empty()` **explicitly** (AC5), its output is identical whether
`default_fonts` is on or off. That is what makes the subtask ordering safe: the
`fonts.rs` rewrite (subtask 3) lands and goes green *before* the Cargo.toml flip
(subtask 4), and the flip then changes **no font-stack behaviour at all** — no
test flips on it. Constraint 1's requirement buys the ordering for free.

Scope that claim precisely: it is about the **font stack**. The flip is not a
behaviour-free dependency edit in general — dropping eframe's defaults also drops
two Wayland-only winit features, which is what the amended AC3's direct `winit`
dep exists to restore (spec Key decision 12, D12, § Risks).

### Rejected alternatives

| Alternative | Why rejected |
|---|---|
| **`Fonts::has_glyph` / `has_glyphs` as the AC8/AC9 oracle** (the obvious choice — `Fonts::new(TextOptions::default(), definitions())` needs no `Context`, and it would test fallback resolution end-to-end) | **Unsound.** `epaint font.rs:720`: `has_glyph(c) = resolve_face(c) != cached_family.replacement_face_key`. `replacement_face_key` is the **first face in the chain carrying `◻` U+25FB, else `?`** (`fonts.rs:664-676`). Measured: JBM's charset **lacks `◻`** (`fc-query` → 975 codepoints; U+25FB absent). So for `Name("JetBrainsMono-Medium") = [JBM-Medium, Onest-Medium]` the answer **inverts on an unmeasured property of a face not yet vendored**: if Onest carries `◻`, `replacement_face_key == Onest-Medium`, `resolve_face('✓') == Onest-Medium`, and `has_glyph('✓')` returns **false** — a false negative on the one assertion decision 8 exists for. If Onest lacks `◻`, it returns true — but then `has_glyph('L')` returns false. The API is wrong in **both** configurations, just for different chars (upstream's own `TODO(emilk)` understates it: for a *single-face* family it returns false for **every** char). A headline assertion may not be contingent on an unmeasured property of the new face. |
| Assert Onest's exact `wght` range `100–900` | Duplicates AC1's SHA-256 pin (the face cannot change without a deliberate revendor) and adds a brittle second identity check. Assert the **requirement** — `min <= 400 && max >= 700` — mirroring the existing test's shape. |
| Keep the snapshot-and-extend builder | It is the mechanism of constraint 3's collapse. Under `empty()` it silently yields `[]` tails. Writing the vectors literally is the fix. |
| Hand-roll a `cmap` parser over the TTF bytes | `skrifa` 0.42.1 is **already in the graph** as epaint's own direct dep (`cargo tree -p gp-render --invert skrifa` → `skrifa v0.42.1 └── epaint v0.35.0`). As a **dev-dep** it costs zero new compilation and zero shipped bytes, and mirrors exactly how epaint implements `variation_axes` (`use skrifa::MetadataProvider as _`). |
| Symmetric JBM fallback on `Proportional` | Contradicts AC7's one-entry `Proportional` pin. No use site needs it. |

## Key decisions

| # | Question | Decision |
|---|---|---|
| **D1** | **Which Onest weight backs each JBM weight?** (spec constraint 3 — explicitly design's call) | **Weight-matched, 1:1.** `JBM-Regular→Onest-Regular` (400), `JBM-Medium→Onest-Medium` (500), `JBM-Bold→Onest-Bold` (700). **Why:** the per-weight `Name` family exists *precisely* to express "this exact weight". Falling back to Onest-Regular from a 500 run would render `✓` at wght 400 inside a wght-500 Badge — a **visible weight mismatch stacked on top of** decision 8's already-accepted metric mismatch, i.e. paying the fallback's price twice. Weight-matching costs **nothing**: Onest 400/500/700 are already registered `font_data` entries serving the proportional families, so this adds family-list entries only — no new bytes, no new instances. Onest-SemiBold (600) has no JBM counterpart (JBM 600 is unregistered) and stays proportional-only. Made **testable** by AC8's full-vector equality on all three mono `Name` families — a pairing slip is caught by an exact list compare, not by a `contains`. Expressed in code as a literal `[(JBM_REGULAR, ONEST_REGULAR), (JBM_MEDIUM, ONEST_MEDIUM), (JBM_BOLD, ONEST_BOLD)]` pairing table, so the decision is legible at the call site. |
| **D2** | **Open question 1 — does `CANVAS_RECT` grow past 192×128?** | **Yes → 320×192.** (1) AC16 asks a **human** to judge whether `Ф` is Onest, not a fallback, not a mixed-typeface `Ф1` — that needs legible glyphs, and 192×128 already carries a card, a grid and a hairline. (2) egui **clips silently** at the painter's clip rect: a too-small canvas fails quietly, which is the exact failure class this task exists to remove. (3) `geometry()` derives every position from `rect`, and card/hairline are fixed `rect.min`-relative offsets, so growing hardcodes nothing and leaves both existing probes valid (paper `(8.5, 8.5)`, hairline `(88.5, 100.5)`). (4) The golden regenerates anyway (AC12) — #12's AC12 byte-identity does **not** bind here (spec Key decision 11), so growth is free. Cost is 2.5× golden pixels (61,440 px), which is nothing. |
| **D3** | **Open question 2 — does the sample render `✓`?** | **Yes — in the mono row, at the Badge's own weight** (`Name("JetBrainsMono-Medium")`, wght 500). This makes the golden reproduce the motivating use case *exactly* (mono @ `--fw-medium` containing U+2713, per `Badge.jsx:21`). It is also the **only** end-to-end evidence that epaint actually walks the family list: AC7/AC8 assert the family-list **data**, AC9 asserts Onest's **cmap** — neither proves epaint's resolver joins them. The golden + `image-check` + AC16 are that proof. Open question 2's stated cost ("one more glyph in an already-tight canvas") is dissolved by D2. |
| **D4** | **AC9's cmap oracle** | **`skrifa` as a `gp-render` dev-dep**, `skrifa = "0.42"`. Version reasoning: observed **in-graph** at `0.42.1` (epaint 0.35.0's own direct dep, `epaint/Cargo.toml:149-150`); AGENTS.md § Dependency Versions → `0.x.y` pins as `0.x` → `"0.42"`, which `^`-unifies with epaint's resolution. A different minor would **duplicate** the crate in the graph for no gain. API: `skrifa::FontRef::from_index(bytes, 0)` + `skrifa::MetadataProvider::charmap()` + `Charmap::map(ch) -> Option<GlyphId>` (all verified in `skrifa-0.42.1/src/{lib,provider}.rs`). |
| **D5** | **`weighted_instance`'s doc rationale must be *replaced*, not deleted** | Its current doc says *"finding 3: Space Grotesk defaults to 300, not 400"*. Onest defaults to **400**, so that sentence becomes false — and deleting it without replacement invites exactly the "400 default makes `coords` redundant" reasoning spec Key decision 4 forbids. The **new, face-independent** rationale: the builder registers **four distinct Onest weights from ONE byte array**; without an explicit per-instance `wght`, all four render at the face's default and Medium/SemiBold/Bold are silently identical to Regular. This is *stronger* than the SG-300 rationale (it does not depend on the default being wrong) and is what AC6's `coords != VariationCoords::default()` test guards. |
| **D6** | **AC15's figures do NOT go in the design doc** | AC15 says "recorded in the design doc **/** PR body". They land in **`.progress.md`'s `## Decisions log`** (written by the implementor, durable across the group handoff) and the **PR body** (Step 12). They must **not** go in `*.design.md`: `/task` SKILL.md's AXIOM makes `*.design.md` writes subagent-owned (`design` only) — a `code-writer` editing it is FORBIDDEN, and a Step-11 finding touching it is a Design-Amendment trigger. AC15's own "/" permits the PR-body route. |
| **D7** | **Scope line for `docs/design-system/`** | **`docs/design-system/tokens/**` is IN** (it is the Rust token layer's source of truth and is `include_str!`-coupled to tests); **the rest of `docs/design-system/` is OUT** — `readme.md:91,190`, `IMPORT.md:34`, `SKILL.md:13,21`, `guidelines/type-display.card.html:1,12`, `guidelines/type-scale.card.html:1` are the un-ported mockup + its documentation, the same category the spec puts out of scope for `Screens.jsx`/`MovePad.jsx` ("#19–#22's source material, not running code"). The guidelines cards are rendered **specimens** ("Display — Space Grotesk", "letter-spacing −0.02em") whose re-toning needs the product owner's eye — i.e. spec Deferred row 1's territory. Surfaced in § Open questions so Step 12 propagates it. |
| **D8** | **`ai-docs/deferred/_inbox.jsonl:97` — DO NOT TOUCH** | It carries a stale `Space Grotesk` mention. AGENTS.md AXIOM: `_inbox.jsonl` is written **ONLY** by `/task` Step 12 and `/triage`; hand-edits defeat the propagation contract and one malformed line breaks the whole `jq` read. Named here so no implementor "helpfully" fixes it. |
| **D9** | **AC11 is a deliverable, NOT a Step-7 Spec-Amendment trigger** | The `/task` § *Spec Amendment recipe* fires when a **design-review note** implies a change to **this task's own spec**, and it then re-loops Step 6 → Step 7. AC11 is neither: it is spec-approved scope (Scope 8 / AC11) amending **#12's `done/` spec**, which is not this task's implementation contract. Re-entering Step 6/7 would be a no-op loop. **Only the recipe's step-4 mechanism applies** — *"amend the spec via the `spec-writer` Subagent; orchestrator-side direct `Edit`/`Write` are FORBIDDEN"* — and it applies because the AXIOM's rule (*"All such writes go through the responsible Subagent: `spec-writer` for `*.spec.md`"*, **explicitly including `done/` siblings**) is unqualified. The recipe's step-3 "surface to user for approval" is already satisfied: the product owner approved the spec carrying AC11. |
| **D10** | **`cargo update` must NOT be run bare** | Both dep edits here (skrifa dev-dep; `default-features = false` ×2) are dep-graph edge changes that **`cargo build` resolves minimally**. skrifa 0.42.1 is *already* in `Cargo.lock` (transitive via epaint), so its dev-dep edge adds no package. A bare `cargo update` is the exact misfire recorded at `ai-docs/learnings.md:133-135` (2026-07-17 — pulled ~9 unrelated transitive bumps). Gate: `cargo build`, then `git diff --stat Cargo.lock` to confirm the delta is only the intended edges. |
| **D11** | **`tessellation_smoke` must install fonts too — constraint 4 under-scopes this** | AC13 names only the golden test, but `tessellation_smoke` drives the **same** `draw_placeholder`, which now draws text, from a bare `egui::Context::default()`. Derived from § *Load-bearing premise*: every sample row is a `FontFamily::Name(..)`, so this is the **missing**-family path — `tessellation_smoke` **panics loudly** (`FontFamily::Name("Onest-Bold") is not bound to any fonts`). It does **not** silently pass, and this is **not** a silent-inversion trap in the sense of constraints 1–2; the empty-family INVISIBLE path exists but the sample never takes it. **Fix (unchanged): `ctx.set_fonts(crate::fonts::definitions())` before `run_ui`.** One pass suffices here, and the reason is the asymmetry with `golden_guard`: this test **owns its `Context` and calls `set_fonts` before the first pass ever runs**, so `run_ui`'s internal `begin_pass` consumes `new_font_definitions` and the fonts are live for that same pass. `golden_guard` owns no such window (§ Test Design). Not a spec amendment — AC13 does not forbid it and AC17 requires green tests. |
| **D12** | **Retraction + the measured/derived rule this design is now held to** | Rounds 1–2 wrote *"verified against eframe's `default`"* for a `gp-game` dep line carrying `"winit/default"`. The **derivation was sound** — that list genuinely *is* eframe's `default` minus `default_fonts` — and the line is nevertheless **unbuildable**: cargo rejects it at manifest-parse time, *before* resolution and before any of the reasoning gets to apply. `code-writer` hit it at subtask 4 and stopped rather than deviating. **Three errors in this design now share one shape** — D11's original *"no panic"*, this slash line, and the round-3 Miri bullet (§ Risks) — *a correct-looking inference never executed against the thing it describes*. Reading the binding file is necessary and **not sufficient**: in D11 and here, the evidence had been read and was mis-applied at the point of assertion; in the Miri case the binding fact was in **neither** file I read (it is an allocator property, observable only by running Miri). **Rule:** a claim about what a manifest, API, or toolchain *does* is "verified" only if it was **run**. A claim obtained by reading a table is a **derivation**, discharged by a **gate**, never by prose — which is why AC18's probes live in subtask 4's gate rather than in a design-doc certification. **The gate is the verification.** |
| **D12a** | **Why the Miri bullet outlived two review rounds — negatives have no gate** | The mechanism, not a restatement of D12. A **positive** claim (*"X does Y"*) is eventually executed by something: the code runs, the test runs, the manifest parses — the world pushes back. A **negative** (*"X is not a problem"*, *"not applicable"*, *"no CI step exists"*) is executed by **nothing**; no gate ever tries it, so it survives every round unchallenged and ages into a false premise. Round 3's Miri bullet was this design's **only** "Not applicable" bullet and also its longest-lived error — that is not a coincidence. **Rule: every negative claim is either executed on the spot — naming the command and its output — or explicitly marked `[derived]`.** Applied this round: § Risks' *"AC18 makes deletion fail loudly"* was a negative in disguise (*"a future deletion cannot slip through"*); executing it (`rg 'cargo tree\|winit\|default_fonts' .github/workflows/` → **no hits**; only `ci.yml` exists) showed it **false**, and the bullet is corrected below. **Two grades of negative, and the second is worse:** a *describing* negative (*"not applicable"*) merely misinforms; a **prescribing** negative (*"no precedent exists, **so this sets the shape**"* — § Subtask 8, rounds 1–4) converts an unverified absence into an **instruction** the next reader obeys. Prescribing negatives are the highest-priority claim in any artifact to execute. **Tag scope — every section that asserts a fact about the world, not just § Risks.** Round 4 scoped per-bullet tagging to § Risks because that is where round 3's error happened; the § Subtask 8 precedent claim then survived rounds 1–4 **in the untagged § Test Design**. That is the meta-lesson and the family's sixth instance: **round 4 fixed the *location* of the previous error instead of its *class*** — the same move as reading the right file and mis-applying it (D12). An untagged factual claim is a defect **wherever it lives**. |

## Decomposition

`M = 8`. Subtasks **1–3 are committed and gated** (baseline `25,488,456 B` @ `ec0a471`;
`44bdefd`; `707e1f7`; **workspace 131/131** green — 97 `gp-core` + 2 `gp-gen` + 30
`gp-render` + 2; 129 at `ec0a471`. Rounds 3–4 wrote *"97/97 workspace"*, which
mislabels **`gp-core`'s** figure as the workspace's) — they are **not** re-planned by this
amendment and the numbering is **unchanged**. `code-writer` resumes at **subtask 4**.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | **Measure the AC15 baseline — BEFORE any edit.** `cargo build --release -p gp-game`; record `stat -c %s target/release/graphite-gp` + `rustc -V` + profile (workspace declares **no** `[profile.*]` — verified — so it is cargo's default release profile) into `.progress.md` § Decisions log. **Nothing else in this subtask.** If it runs after subtask 2+, the baseline is lost (D6). | *(none — measurement only; `.progress.md` is gitignored)* | — |
| 2 | **Vendor Onest — add only, no deletion.** Fetch `Onest[wght].ttf` + `OFL.txt` from `google/fonts` @ `389b770410cc0b7c21c85673bfa2077420fe7f65`, path `ofl/onest/` → `crates/render/fonts/onest/`. **Verify SHA-256 against AC1's two pins BEFORE `git add`** (124,376 B / `3faa4b90…`; 4,384 B / `071195d8…`). `.gitattributes`'s `crates/render/fonts/** -text` already covers the new subdir (Key decision 3) — no rule change; the rule must be in effect *at stage time*, and it is. Deletion is deferred to subtask 3 so the tree never fails to build. | `crates/render/fonts/onest/Onest[wght].ttf`, `crates/render/fonts/onest/OFL.txt` | 1 |
| 3 | **`fonts.rs` — consts, builder, docs, tests — plus the space-grotesk deletion and the `.gitattributes` comment.** Byte const + 4 key consts → Onest (clean break, **no** `SPACE_GROTESK` aliases — AGENTS.md § API Stability). Builder → explicit `FontDefinitions::empty()` + explicit family vectors per § Approach (D1). Module doc + `definitions()` doc + `weighted_instance` doc corrected (AC5, D5). Add `skrifa = "0.42"` to `[dev-dependencies]` (D4). Replace #12's AC10 test (AC7) and add the AC6/AC8/AC9 tests per § Test Design. Delete **both** space-grotesk files. Update `.gitattributes`'s comment, which cites the now-deleted `space-grotesk/OFL.txt` as its worked example — keep the face-independent rationale, drop the dead referent. Feature-independent by construction, so this lands green **before** the flip. | `crates/render/src/fonts.rs`, `crates/render/Cargo.toml` (`[dev-dependencies]` only), **delete** `crates/render/fonts/space-grotesk/{SpaceGrotesk[wght].ttf,OFL.txt}`, `.gitattributes` | 2 |
| 4 | **Feature flip ×2 + the direct `winit` dep + the AC4/AC18 probes.** *(Amended — rounds 1–2 of this design certified an unbuildable line here; see **D12**. The retraction is the point of this row, not a footnote.)* `crates/render/Cargo.toml` → `egui = { version = "0.35", default-features = false }` (egui's `default = ["default_fonts"]`, so dropping defaults drops **exactly** that — read from `egui-0.35.0/Cargo.toml`). `crates/game/Cargo.toml` → **two** deps per the amended AC3 + spec Key decision 12: `eframe = { version = "0.35", default-features = false, features = ["accesskit", "wayland", "web_screen_reader", "wgpu", "x11"] }` **and** `winit = { version = "0.30", default-features = true }`. **What was wrong:** the old line put `"winit/default"` in `gp-game`'s own `features` list; `pkg/feature` slash syntax is legal **only inside a crate's own `[features]` table**, so cargo rejects the manifest at **parse time** — *"feature winit/default in dependency eframe is not allowed to contain slashes"*. eframe may write it (that is eframe's table); a dependent cannot reach through eframe the same way. The direct `winit` dep restores the two lost features (`wayland-dlopen`, `wayland-csd-adwaita`) by unification instead. **AC3 also mandates a `crates/game/Cargo.toml` comment** recording that `gp-game` never `use`s winit and the dep exists solely to carry features — see § Risks; nothing mechanical protects it. `"0.30"` is **not a free choice**: eframe requires `winit = "0.30.13"` (verified this round, `eframe-0.35.0/Cargo.toml:272-275`), so `"0.30"` ^-matches and unifies, and AGENTS.md § *Dependency Versions* independently forbids pinning the patch. **Gate (both probes run here, where the edit is):** AC4 — `cargo tree -e features -p gp-game` / `-p gp-render` show no `egui feature "default_fonts"` node, `cargo tree --invert epaint_default_fonts` reports absence; **AC18** — `cargo tree -e features -p gp-game -i winit` shows all **five** of `rwh_06` / `x11` / `wayland` / `wayland-dlopen` / `wayland-csd-adwaita` live on winit's own feature edges, **and** `grep -c 'name = "winit"' Cargo.lock` returns **1** (the unification precondition — two winits would restore nothing, silently). Record both AC18 results in `.progress.md` § Decisions log (D6 — **not** the design doc). Per D10: `cargo build`, then `git diff --stat Cargo.lock`; **no bare `cargo update`** — this row adds a dep and drops another, exactly the shape that tempts one. Font-stack behaviour is unchanged by the flip (§ Approach); Wayland behaviour is preserved by the winit dep. | `crates/render/Cargo.toml`, `crates/game/Cargo.toml` | 3 |
| 5 | **Token layer + the two stale doc sites.** `typography.rs`: `FONT_DISPLAY = FONT_UI = "Onest"`. **`docs/design-system/tokens/typography.css`** (lines 3, 9, 10) — the file `family_names_match_css` actually reads (§ Approach finding 1); **mandatory**. `docs/design-system/tokens/fonts.css` — family + Google Fonts `@import`. `crates/render/src/tokens/mod.rs:35` — the AC1 disposition table restates `"Space Grotesk" ×2` as the value of `--font-display`/`--font-ui`; it is prose that **no gate catches** (`cargo doc` sees no broken link) and it would directly contradict the const it documents. | `crates/render/src/tokens/typography.rs`, `docs/design-system/tokens/typography.css`, `docs/design-system/tokens/fonts.css`, `crates/render/src/tokens/mod.rs` | 4 |
| 6 | **The picture: sample + canvas + harness fonts + doc + regen + `image-check`.** `CANVAS_RECT` → 320×192 (D2) — **and the four stale doc-prose sites no gate catches** (same class as `tokens/mod.rs:35`): `:25` (the const's own doc), `:242` (`pixel_at`'s doc), `:249` (the `#[allow(clippy::cast_possible_truncation, …)]` **`reason` string — it justifies a cast's domain bound**, so a stale figure there is a false justification, not a typo), and **`:265-270`** — `golden_guard`'s own `cfg_attr` rationale, which argues the gate exists so the Miri job does not abort *"losing this crate's whole binary — `tessellation_smoke` included, **though it is pure CPU and passes under Miri**"*. That clause is **false at D11**: `tessellation_smoke` now rasterizes, is no longer pure CPU, and is itself miri-ignored by this subtask. Correct the **argument**, not just the adjective — the comment assumes **one** abort site and there are now **two**. **Add `#[cfg_attr(miri, ignore = "…")]` to `tessellation_smoke`** (§ Risks — measured panic), with a reason naming **vello_cpu's checked u8→u32 pixmap cast under Miri's 1-byte allocator alignment**; **do NOT copy `golden_guard`'s FFI/`dlopen` reason** — a wrong-but-plausible justification is the D12 class again. Add the three text anchors to `PlaceholderGeometry` as `rect.min`-relative derivations (keeps the file's "never write a probe coordinate twice" property). Draw the three rows per § Test Design. Font install: `tessellation_smoke` per D11 (own ctx → `set_fonts` before `run_ui`); `golden_guard` per § Test Design's gated-closure mechanism (AC13) — **the two differ and must not be copied onto each other**. `golden_guard` doc comment → AC14. Then regen (`UPDATE_SNAPSHOTS=true`) and **spawn `image-check`** — mandatory per constraint 8 and `code-writer.md` § Invariants; **do not commit the PNG until PASS**. Regen and sample must be **one** subtask: split across two, `golden_guard` is red at the boundary and cannot gate/commit. | `crates/render/src/placeholder.rs`, `crates/render/tests/snapshots/placeholder.png` | 5 |
| 7 | **Measure the AC15 delta.** Same toolchain + profile as subtask 1. Record before / after / delta into `.progress.md` § Decisions log (**not** the design doc — D6). Compare against the predicted **1,426,320 B** shrink (1,414,020 `epaint_default_fonts` + 12,300 face delta); explain any deviation > 5% rather than waving it through. Confirm the delta is a **shrink**. | *(none — measurement only)* | 6 |
| 8 | **AC11 — amend #12's `done/` spec via `spec-writer`.** Spawn `spec-writer` per D9 + § Test Design's spawn contract; verify the diff is surgical; stage + commit. **Never** a direct `Edit`/`Write` of the `*.spec.md`. | `ai-docs/plans/done/2026-07-17-render-design-tokens.spec.md` | 7 |

**Not decomposition subtasks — owned by `/task`'s own steps** (named here so they are not missed):

- **Step 9.5 (docs):** `README.md:54` — *"faces — Space Grotesk + JetBrains Mono, vendored as OFL-1.1 variable `[wght]` files"* becomes **factually false** at AC2. `ai-docs/context.md:38` — #12's "Design tokens + fonts" entry names the vendored faces. Both are Step 9.5's content-file scope.
- **Step 12:** `ai-docs/plans/INDEX.md` row status + the `done/` move. INDEX.md:10 (#12's row) is an accurate **historical** record of #12 — leave it.
- **Never:** `ai-docs/deferred/_inbox.jsonl:97` (D8), `ai-docs/key-decisions.md:29` (history), `ai-docs/plans/done/**` other than AC11's deliberate amendment.

## Handoff plan

Grouping per `.claude/agents/design.md` § Rules → handoff-grouping (a)–(h). `M = 8`;
**2 groups** — the minimum, since change-type homogeneity (e) forbids merging a
`*.spec.md` write into a code group, and no dependency forces any further split.
2 ≤ the default max of 4 (h), so no user approval is needed.

**Change-type classification.** Subtasks 1–7 are **code**: `*.rs`, `Cargo.toml`,
`*.css` (`include_str!`-coupled test data — `tokens/typography.rs:95`),
`.gitattributes`, `*.ttf`, `*.png`. All are mechanical implementation, which is
what the code tier routes on. Subtask 8 is **instructions/harness**:
`ai-docs/plans/done/*.spec.md` matches `ai-docs/**`.

- **Entry into Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). The
  handoff is binding at the **first** group too (`/task` SKILL.md Step 8 —
  *Every-group handoff*).
- **Group A** — model `sonnet`, effort **`medium` (pinned)**, via the
  `code-writer` subagent, 1M-token window — subtasks **1–7** (code change-type).
  7 subtasks, ≤ 10 ✓. Routing per (g): `subagent_type="code-writer"` with **no**
  inline `model=`/effort override — its `model: sonnet` + `effort: medium` are
  frontmatter-pinned, which is the only lever (there is no per-invocation
  `effort` parameter). Within this group, subtask 6 spawns `image-check`
  (`subagent_type="image-check"`, no inline override) — permitted and in fact
  **required** by `code-writer.md` § Invariants' golden-image bullet, which
  carves it out of the "no other reviewer" rule on the artifact-vs-work
  discriminator; `image-check.md` § Scope confirms mint/regen time is its only
  sanctioned trigger.
- **Handoff after Group A:** spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
  Parent `/task` re-validates state (branch matches `**Branch:**`, `base_commit`
  unchanged, `git diff --quiet` clean) and resumes in Group B with fresh context.
- **Group B** — model `opus`, effort **inherited from the orchestrator (typically
  xHigh) — NOT pinned**, via `subagent_type="general-purpose"` with inline
  `model="opus"`, 1M-token window — subtask **8** (instructions/harness
  change-type). **Terminal group** (1 subtask; within the `1..=10` range ✓).

**Why Group B is `general-purpose` and not `code-writer`.** Three independent
reasons agree: (g) mandates `general-purpose`+`opus` for an instructions/harness
group; `code-writer.md`'s **NEVER re-delegate** invariant sits awkwardly with a
required `spec-writer` spawn; and homogeneity (e) forbids folding a `*.spec.md`
write into the code group regardless.

**Permission check for the Group-B → `spec-writer` spawn (CAN vs MAY).**
*CAN:* `general-purpose` is `tools: *`. *MAY:* nothing in its definition forbids
sub-spawns, and `spec-writer.md` carries no "only `/interview` may invoke me"
invariant — `/task` `reference.md` § Spec Amendment recipe step 4 independently
sanctions *"Spawn `spec-writer` with the user's approved amendment as a synthetic
round"*. The `/task` AXIOM does not merely permit this route, it **requires** it.
`spec-writer` stays on Opus per (g) regardless of any group marker.

## Risks

- **`spec-writer` reformats the whole `done/` spec instead of amending it.** Its
  contract says *"Write the spec at `spec_path` using the format from
  `/interview` § Spec format"* — an interview framing, not a surgical one, and
  AC11 demands the amendment be *"recorded, not silently rewritten"*.
  **Mitigation:** the spawn prompt states *"surgical amendment: AC9 (line 289)
  and AC10 (line 290) rows and one amendment record ONLY; every other byte
  unchanged"*, and Group B's implementor **verifies with `git diff`** that only
  those lines moved before staging. On over-write → re-spawn with corrective
  `extra_context`; do **not** hand-repair (the AXIOM forbids it).
  `[derived → discharged by Group B's `git diff --stat` + `git diff` check before staging]`
- **`too_long_first_doc_paragraph` (nursery, **deny**) fires on the new docs.**
  AC5 requires a doc comment *stating why* `empty()` is explicit; the natural
  prose produces a long opening paragraph and `cargo clippy -D warnings` fails.
  **Mitigation:** first paragraph = one short line, blank line, then the
  rationale. Companion trap: `doc_markdown` (pedantic, **deny**) flags camelCase
  in prose — `` `SemiBold` ``, `` `JetBrains` ``, `` `FontDefinitions` `` need
  backticks (`Onest`, a single capitalised word, does not). Neither is caught by
  `cargo doc`; neither build nor `cargo doc` subsumes clippy here.
  `[derived → discharged by `cargo clippy --workspace --all-targets -- -D warnings`]`
- **`gp-game`'s `winit` dep is never imported and looks exactly like dead weight.**
  It is load-bearing: it exists **solely** to carry `wayland-dlopen` +
  `wayland-csd-adwaita` to the single shared winit package by feature
  unification (spec Key decision 12). **Nothing mechanical protects it** —
  deleting it as an "unused dependency" still **compiles**, still passes
  `cargo clippy -p gp-game --all-targets`, and *silently* regresses Wayland CSD
  to `FallbackFrame` (a plainer title bar, not a missing one) while linking
  libwayland instead of `dlopen`ing it; `unused_crate_dependencies` is
  configured nowhere in the workspace. **This is the same hazard class as
  `golden_guard`'s early `return`** — code whose removal looks like a cleanup
  and whose absence is invisible at every gate. **Mitigation, stated without
  over-claiming:** **AC3's `Cargo.toml` comment is the only *durable* guard.**
  AC18 verifies the unification **once, at subtask-4 time** — it is a
  subtask-scoped probe, not a workspace or CI gate (§ Gates), so a deletion six
  months from now passes `build` + `test` + `clippy` + `fmt` + `doc` with
  everything green and AC18 never re-runs. The comment is what a future deleter
  actually meets. **Rejected — a `cargo tree` grep step in CI** as a durable
  guard: it would make this real, but it buys one Wayland-only cosmetic
  (`FallbackFrame` vs `AdwaitaFrame`) plus `dlopen`-vs-link at the cost of a
  permanent CI step whose failure mode is opaque to anyone who has not read
  spec Key decision 12; out of proportion, and out of this task's scope.
  Recorded so the gap is a **choice**, not an oversight.
  `[measured: `rg 'cargo tree|winit|default_fonts' .github/workflows/` → no hits,
  only `ci.yml` exists; `cargo clippy -p gp-game --all-targets` exits 0 with the
  dep deleted (orchestrator); `unused_crate_dependencies` configured nowhere]`
- **The two font-install sites look interchangeable and are not.** `tessellation_smoke`
  owns its `Context` (`set_fonts` before the first `run_ui` — one pass suffices);
  `golden_guard` does not (the closure already ran at build time → gated-closure +
  `run_steps(1)`). Copying either onto the other reintroduces the round-1 panic or
  adds a pointless frame dance. **Mitigation:** both are spelled out in § Test
  Design with their frame ordering, and subtask 6 names the asymmetry explicitly.
  `[measured: reviewer reproduced the round-1 sequence — `PANIC: FontFamily::Name("Onest-Bold")
  is not bound to any fonts`; `from_builder`'s `None` ctx re-read at source this round]`
- **A red commit if the subtask order slips.** Deleting space-grotesk before
  `fonts.rs` stops referencing it breaks `cargo build`; flipping the feature
  before the test rewrite breaks `cargo test` (11 → 7) while `cargo build`
  stays green — so a `build`-only gate would **pass a red commit**. **Mitigation:**
  the order in § Decomposition is load-bearing, not cosmetic. Deletion rides with
  subtask 3; the flip follows it.
  `[derived → discharged by `code-writer`'s per-subtask `cargo build` + `cargo test` gate]`
- **The mono row's `·`/`→` might not be in JBM**, which would silently demote
  AC12's "JetBrains Mono + middot + arrow" row to Onest's proportional metrics.
  **Retired, not mitigated — measured:** `fc-query` over the vendored
  `JetBrainsMono[wght].ttf` (975 codepoints) → `U+00B7 ·` **present**, `U+2192 →`
  **present**, `U+2013 –` present, `U+0424 Ф` present, `U+2713 ✓` **absent**
  (independently confirming spec Key decision 9). The § Test Design control
  assertion pins all of this in-tree.
  `[measured: `fc-query` charset dump over the vendored face, six probes, round 1]`
- **`tessellation_smoke` becomes a second Miri abort site once it draws text —
  it MUST be `#[cfg_attr(miri, ignore)]`d.** Round 3 asserted the opposite
  (*"Not applicable … no `Fonts` construction"*); that bullet was wrong on the
  axis as well as the verdict — the risk is not Miri **slowness**, it is a hard
  **panic**, and it is D12a's canonical case. **Measured chain:** drawing any
  glyph rasterizes it — `epaint::text::font::FontCell::allocate_glyph_uncached`
  (`epaint-0.35.0/src/text/font.rs:280`) → `vello_cpu::RenderContext::render_to_pixmap`
  → `vello_cpu::fine::lowp::U8Kernel::copy_solid` (`vello_cpu-0.0.9/src/fine/lowp/mod.rs:115`),
  whose body is a **checked** `bytemuck::cast_slice_mut::<u8, u32>(dest)`. Native
  malloc over-aligns the pixmap so the cast succeeds (0.01s); Miri's allocator
  grants only `u8`'s alignment of 1, so the checked cast **correctly refuses** →
  `TargetAlignmentGreaterAndInputNotAligned`. **Not UB, not a soundness finding,
  not ours to fix** — a checked cast doing its job. But without the gate,
  `tessellation_smoke` aborts the advisory Miri job and, via cargo's fail-fast,
  takes gp-render's whole test binary and every phase behind it — **precisely the
  regression `golden_guard`'s own gate was added to prevent.** **Accepted coverage
  loss, stated plainly:** under Miri gp-render then exercises only the
  `fonts.rs` + tokens tests, not the placeholder path at all. (Side note, not the
  reason: the text-free `tessellation_smoke` currently passes Miri in 34.29s.)
  **The `ignore` reason must name vello_cpu's u8→u32 pixmap cast under Miri's
  allocator — copying `golden_guard`'s FFI/`dlopen` reason would be a false
  justification, i.e. the D12 class again.** `[measured: reviewer ran Miri on a
  D11-exact probe — panic; all four committed `fonts.rs` tests pass Miri in
  1.88s; chain re-read at source this round]`
- **AC15's delta may miss the 1,426,320 B prediction.** The default release
  profile does not strip, and `include_bytes!` arrays land in `.rodata`
  alongside symbol tables. **Mitigation:** AC15 asks for the *measurement*, not
  the prediction (spec Key decision 10); >5% deviation is explained in the PR
  body, not waved through.
  `[measured: indicative orchestrator probe 24,036,800 B vs 25,488,456 B baseline =
  1,451,656 B shrink, beating the prediction by ~1.8%; subtask 7 owns the real one]`
- **`Cargo.lock` churn.** See D10 — bare `cargo update` is a known live misfire
  on this exact shape.
  `[measured: `ai-docs/learnings.md:133-135` records the incident; derived → discharged
  by `git diff --stat Cargo.lock` in subtasks 3 and 4]`

## Test Design

### `crates/render/src/fonts.rs` — `#[cfg(test)] mod tests`

| Test | AC | Entry point | Scenarios |
|---|---|---|---|
| `wght_axis_covers_registered_weights` *(retarget)* | AC9 | `FontData::variation_axes()` | Onest reports a `wght` axis with `min <= 400.0 && max >= 700.0`; JBM likewise. Assert the **requirement**, not the exact `100–900` (§ Rejected alternatives). |
| `definitions_registers_seven_instances_with_exact_families` *(**replaces** `definitions_preserve_builtin_fonts_and_add_seven_instances`)* | AC6, AC7 | `definitions()` | `font_data.len() == 7`; all 7 keys present; **full-vector equality** `families[&Proportional] == [ONEST_REGULAR]` and `families[&Monospace] == [JETBRAINS_MONO_REGULAR, ONEST_REGULAR]` (`Vec<String> == [&str; N]` works via `String: PartialEq<&str>`); every instance's `tweak.coords != VariationCoords::default()`. The `builtin_font_names()` loop is **replaced by these assertions, not deleted** (constraint 2) — and the old test **name** must go with it: it asserts the opposite of the new behaviour. |
| `mono_name_families_fall_back_to_weight_matched_onest` *(new)* | AC8, D1 | `definitions()` | Full-vector equality on all three mono `Name` families: `["JetBrainsMono-Medium", "Onest-Medium"]` etc. Ordering is load-bearing — JBM **first**. Plus: the four proportional `Name` families are **single-entry**. |
| `vendored_faces_cover_the_glyphs_the_swap_exists_for` *(new)* | AC9, AC12 | `skrifa` `Charmap::map` | Onest's charmap **has** `U+0424 Ф` (the glyph the swap exists for) and `U+2713 ✓` (the glyph decision 8 exists for), plus every codepoint of the three sample strings. **Control:** JBM's charmap **lacks** `U+2713` — which is what makes D1's Onest tail load-bearing rather than decorative — while **having** `U+00B7 ·`, `U+2192 →`, `U+2013 –`. This lifts spec Key decision 9's measured facts out of prose and into a live assertion; a revendor that breaks them goes red. |

### `crates/render/src/placeholder.rs`

- **`tessellation_smoke`** — add `ctx.set_fonts(crate::fonts::definitions())` before
  `run_ui` (D11). Existing vertex/index assertions unchanged; the golden owns text
  verification, this test owns "the path tessellates". **Also gains
  `#[cfg_attr(miri, ignore = "…")]`** — drawing text rasterizes glyphs, which
  panics under Miri (§ Risks: `epaint font.rs:280` → `vello_cpu` `copy_solid`'s
  checked `bytemuck::cast_slice_mut::<u8, u32>`, unsatisfiable at Miri's 1-byte
  allocator alignment). Its `reason` must name **that** cause; `golden_guard`'s
  `dlopen`/FFI reason is a *different* one and copying it would ship a false
  justification. The two ignores are **not** duplicates — they are two distinct
  abort sites with two distinct causes, which is exactly what `:265-270`'s
  corrected comment must now say.
- **`golden_guard` — install from *inside* the UI closure, gated; never post-build.**

  **Why post-build `set_fonts` cannot work** (design round 1 got this wrong):
  `Harness::from_builder` runs the app closure **at build time** — `ctx.run_ui(input.clone(), …)`
  (`egui_kittest lib.rs:145-147`) followed by `harness.run_ok()` (`:182`) — against
  `ctx.unwrap_or_default()`, and **both** `build_ui` (`builder.rs:239`) and
  `build_ui_state` (`:189`) pass **`None`** for that ctx. So by the time
  `harness.ctx` is reachable, the closure has already laid out text against a
  default `Context` in which every `Name` family is **missing** → panic (§ *Load-bearing
  premise*). `HarnessBuilder` has **no** font/context hook (verified — `with_size`,
  `with_pixels_per_point`, `with_theme`, `with_options`, `with_os`, `with_max_steps`,
  `with_step_dt`, `with_wait_for_pending_images`, `with_render_options`, `renderer`;
  no `with_fonts`/`with_ctx`), and `Harness::from_builder` + `AppKind` are both
  `pub(crate)` — so the ctx **cannot** be pre-seeded from outside.

  **Rejected — `build_eframe`** (`builder.rs:196-218`): it *does* pass `Some(ctx)`
  with the creation closure running pre-first-frame, but it requires
  `egui_kittest`'s `eframe` feature (pulling eframe/winit into gp-render's dev
  graph) and `State: eframe::App`. It would not breach #12's AC13 (`--edges no-dev`
  excludes dev-deps), but it is a large dependency and a whole App type to serve
  one frame-ordering problem, and it sits against gp-render's draw-only posture.
  `[measured: `cargo tree --help` lists `no-dev` as an `--edges` value, and
  `cargo tree -p gp-render --edges no-dev` → **0** eframe/winit/wgpu hits, so
  AC13 holds today — executed this round per AGENTS.md's never-assert-a-tool-flag
  rule; rounds 1–4 asserted the flag's semantics from memory]`

  **Chosen mechanism — install on frame 1, draw from frame 2:**

  ```
  let mut fonts_installed = false;
  let mut harness = Harness::builder()
      … .renderer(renderer)
      .build_ui(move |ui| {
          if !fonts_installed {
              ui.ctx().set_fonts(crate::fonts::definitions());
              fonts_installed = true;
              return;                      // frame 1 draws NOTHING — see below
          }
          let painter = ui.ctx().layer_painter(egui::LayerId::background());
          draw_placeholder(&painter, CANVAS_RECT);
      });
  harness.run_steps(1);                    // unconditional ≥1 pass with fonts live
  let image = harness.render().expect("offscreen wgpu render failed");
  ```

  **Why the first frame cannot panic:** it returns **before** `draw_placeholder`
  is ever called, so no text is laid out and no family is resolved. The panic is a
  *layout-time* event (`FontsImpl::font`), so drawing nothing is a complete
  defence — this is the only reason the early `return` is load-bearing rather
  than cosmetic. It must not be "optimised away" into an unconditional draw.

  **Why frame 2+ cannot panic:** `Context::set_fonts` is **deferred** — it stores
  `mem.new_font_definitions` (`egui context.rs:2038-2051`) and the sibling
  `add_font` doc states the contract: *"The new font will become active at the
  start of the next pass."* Frame 2's `begin_pass` consumes it, binding all **9**
  families — including the four Onest and three JBM `Name` families the sample
  needs. So the draw is only ever reached with every family bound.

  **Why `run_steps(1)` and not `run()`:** `run`/`run_ok`/`try_run` loop on
  *repaint requests* (`_try_run`, `lib.rs:344-356`), so relying on them would
  make the test depend on whether `set_fonts` requests a repaint — a property I
  deliberately do not want to rest on. `run_steps` is **unconditional**
  (`lib.rs:431-435`: *"Run a number of steps. Equivalent to calling `Harness::step`
  x times."*), so it guarantees a text-drawing pass regardless. `render()` renders
  the **last** frame's output, so the final pass is the one snapshotted. Note
  `from_builder`'s own `run_ok()` may already have run frame 2 — `run_steps(1)`
  is then simply one more identical frame, which is harmless and keeps the
  guarantee independent of that behaviour.

  Existing guards (paper probe, modal colour, hairline darkening, `colour_counts.len() > 1`)
  are unchanged and stay **before** the golden compare. Doc comment → AC14:
  structural guard (AA edges exempt → catches wrong face / tofu / mixed
  typefaces / missing text / a weight silently rendering Light; **not**
  rasterisation drift from a `skrifa`/`harfrust` bump), and it dies with the
  placeholder at #17.
- **The sample** (fixtures — exact strings, pinned so `image-check` has a
  derivable expectation):

  | Row | Text | Family | Size | Weight | Colour | Anchor (`Align2::LEFT_TOP`, `rect.min`-relative) |
  |---|---|---|---|---|---|---|
  | 1 | `GRAPHITE GP` | `Name("Onest-Bold")` | `FS_H2` 30.0 | 700 `FW_BOLD` | `TEXT_INK` | `(16, 106)` |
  | 2 | `Ф1 – Ф7` | `Name("Onest-Medium")` | `FS_H3` 22.0 | 500 `FW_MEDIUM` | `TEXT_BODY` | `(16, 140)` |
  | 3 | `L3 · v4→6 ✓` | `Name("JetBrainsMono-Medium")` | `FS_SM` 13.0 | 500 `FW_MEDIUM` | `TEXT_MUTED` | `(16, 168)` |

  Satisfies AC12: Cyrillic + en-dash + digits in the display face (row 2);
  wordmark (row 1); mono telemetry with middot + arrow (row 3); **two** `--fw-*`
  weights (700, 500). Row 3 carries `✓` at the Badge's own weight per D3. All
  three rows sit **below** the hairline (`y > 100.5`) and clear both probes —
  `(8.5, 8.5)` and `(88.5, 100.5)` — so every existing guard keeps its meaning.
  Estimated extents at 320×192: row 1 ≈ 205 pt wide, rows 2/3 ≈ 90 pt; deepest
  ink ≈ y 185 < 192. **No letter-spacing — a scope decision, not an API limit.**
  Rounds 1–4 said *"`painter.text` has no tracking knob"*, a true narrow negative
  used to imply a false broad one. `painter.text` indeed takes no tracking
  argument (`pos, anchor, text, font_id, text_color` — `egui painter.rs:469-476`),
  but the capability **exists**: `TextFormat::extra_letter_spacing` (*"Extra
  spacing between letters, in points"* — `epaint text_layout_types.rs:474-477`)
  via `Painter::layout_job` (`egui painter.rs:517`). We decline it because the
  spec puts `--ls-*` retuning **out of scope**, **not** because it is
  unavailable — recorded so an implementor who finds `extra_letter_spacing` reads
  a decision rather than an oversight, and does not "fix" the design by wiring
  `LS_DISPLAY` in. Do not improvise tracking.
  `[measured: both signatures + the field read at source this round]`

### Subtask 8 — the `spec-writer` spawn contract (AC11)

`Agent(subagent_type="spec-writer", …)` — stays on **Opus** per (g). Input
fields per `spec-writer.md` § Input contract:

| Field | Value |
|---|---|
| `issue_ref` | `#12` — the spec under `spec_path` carries `**Tracked in:** #12` |
| `issue_body` | #12's body (via `gh issue view 12`), for context only |
| `round` / `round_cap` | `1` / `1` — **equal on purpose**: Hard rule 3 forbids `ask` when `round == round_cap`, which is correct here (the amendment is pre-approved and mechanical; there is nothing to ask) |
| `questions_per_round_cap` | `3` (default; unreachable given the above) |
| `prior_qa` | `[]` |
| `spec_path` | `ai-docs/plans/done/2026-07-17-render-design-tokens.spec.md` |
| `extra_context` | The approved amendment: the exact new AC9/AC10 text below, the amendment-record requirement, and **"surgical — change ONLY those two rows and add the record; every other byte identical"** |

Expected return: `status: ready`.

**AC9 (line 289)** — `SpaceGrotesk[wght].ttf` → `Onest[wght].ttf`; axis
parenthetical `SG 400–700 within 300–700` → `Onest 400–700 within 100–900` (JBM's
`400–700 within 100–800` unchanged).

**AC10 (line 290)** — `SG 400/500/600/700` → `Onest 400/500/600/700`; *"built on
`FontDefinitions::default()`, so egui's bundled fallback faces survive"* → built
on `FontDefinitions::empty()`, egui's bundled faces no longer in the graph
(`default_fonts` off); *"resolve to a non-empty list … egui's default fallback
entries are still present"* → AC7's **exact family lists by full-vector
equality**, **including the two-entry `Monospace` list**
`["JetBrainsMono-Regular", "Onest-Regular"]`.

**Amendment record** (AC11's *"recorded, not silently rewritten"*).

> **Correction — rounds 1–4 of this design asserted the opposite here, and it was
> false.** The text read *"no in-tree precedent exists — none of the `done/` specs
> carries an amendment block, so this sets the shape."* **Executed**
> (`grep -lE '^\*\*Amendment [0-9]' ai-docs/plans/done/*.spec.md`):
> `done/2026-07-16-core-collisions.spec.md` carries **two**, at `:7` and `:9`.
> **Follow the precedent; do not invent a shape.** See D12a — this is the family's
> worst variant: a **prescribing** negative, which converts an unverified absence
> into an instruction (*"so this sets the shape"*) that the next reader obeys.

**In-tree precedent — label from core-collisions, placement from this design.**
The shape is `**Amendment N (YYYY-MM-DD, provenance).**` — e.g.
*"Amendment 2 (2026-07-16, PR #68 review, product-owner directed) — RNG handle,
not a per-call seed."* Adopt that label verbatim.

**Placement differs from the precedent, deliberately.** core-collisions' two
amendments sit at **file top** (under the `**Tracked in:**` header) because they
are **scope-wide** — they restate the collision predicate and the RNG contract,
facts the whole spec depends on. This design's amendment is **AC-local** (AC9 +
AC10 only), so it belongs as a blockquote **directly above the
`## Acceptance Criteria` table**, where the rows it corrects live and where a
reader meets it before them. That reasoning never depended on the false premise
and stands unchanged.

The record names the amending issue (#73) and date, quotes the **original**
AC9/AC10 clauses verbatim, and states why they became false (the Onest swap +
`default_fonts` off), citing #73's spec § Technical constraints 1–3.

**Already shipped, correctly.** Group B hit the false premise at subtask 8,
verified it, and deviated — adopting core-collisions' label at this design's
placement. The result is live at
`done/2026-07-17-render-design-tokens.spec.md:277`:
*"**Amendment 1 (2026-07-17, #73 `render-onest-font-swap`) — AC9 and AC10 only.**"*
This section is now written back to match the shipped artifact, so what ships to
`done/` at Step 12 records what was actually done.
`[measured: `grep -lE` above, this round; shipped block re-read at `:277`]`

**Verification (Group B implementor, before staging):** `git diff --stat` shows
exactly one file; `git diff` shows only the two AC rows plus the added record.
Anything wider → re-spawn `spec-writer`, do not hand-repair.

### Gates

`cargo build`; `cargo test`; `cargo fmt --check`;
`cargo clippy --workspace --all-targets -- -D warnings`;
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` (AC17). No workflow
files change → no `actionlint`. Note the gates do **not** subsume one another:
the doc-prose lints above are clippy's, not `cargo doc`'s.

**AC4 + AC18 are subtask-4-scoped probes, not workspace gates** — they run where
the manifest edit happens (subtask 4's Gate), and their results go to
`.progress.md` § Decisions log + the PR body (D6). Per D12 they are *the*
verification of the dep line; no prose in this document substitutes for them.
Note what **no** gate covers: `cargo clippy -p gp-game --all-targets` exits 0
with the `winit` dep deleted (verified — `unused_crate_dependencies` is
configured nowhere in the workspace), which is precisely why AC18 exists.

## Open questions

- **Spec Open question 3 (Onest's letterform character)** stays open by design —
  correctly routed to the product owner's eye via AC16 on the regenerated golden.
  D2's larger canvas exists partly to make that judgement possible. If it reads
  wrong, the spec's Deferred retune row and #73's Golos Text runner-up are the
  escape hatches.
- **`docs/design-system/`'s non-token layer still says "Space Grotesk"** —
  `readme.md:91,190`, `IMPORT.md:34`, `SKILL.md:13,21`,
  `guidelines/type-display.card.html:1,12`, `guidelines/type-scale.card.html:1`.
  Held out of scope per D7 (un-ported mockup + rendered specimens needing the
  product owner's eye — the same category as `Screens.jsx`/`MovePad.jsx`).
  `readme.md:190`'s substitution flag is the one `fonts.css`'s header points at
  ("SUBSTITUTION (flagged in README)"), so after this task the two disagree.
  **Recommend a follow-up issue**; recorded here so `/task` Step 12 propagates it
  to `ai-docs/deferred/_inbox.jsonl`.

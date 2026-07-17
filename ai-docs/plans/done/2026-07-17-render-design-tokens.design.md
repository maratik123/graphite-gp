# Design: gp-render — design tokens → Rust consts (colors, spacing, type, effects)

**Issue:** #12
**Date:** 2026-07-17
**Spec:** [`2026-07-17-render-design-tokens.spec.md`](2026-07-17-render-design-tokens.spec.md)
**Branch:** `feat/2026-07-17-render-design-tokens`

## Approach

Two new modules in `gp-render` — `tokens` (the 127 CSS tokens as module-level
consts) and `fonts` (the vendored faces + a `FontDefinitions` builder) — plus a
pixel-neutral repoint of `placeholder.rs` and a one-line `set_fonts` call in
`gp-game`'s existing creation closure.

Everything the spec marked "verified" was **re-derived here against live sources**
rather than carried across on the spec's authority. That re-derivation confirmed
the spec on every font/egui fact and on AC12's pixel-neutrality, and produced
**seven findings the spec did not anticipate** — five of which would have shipped a
defect or failed the build. They are called out inline below and summarised in
*Risks*.

### Round 2 — what changed in this revision

The spec was amended after round 1's GO. Re-verified against the amended spec
directly (not on the intermediary's report):

- **Finding 2 ratified.** `--radius-pill` is a normal token; the exclusion row is
  gone (spec § *Technical constraints*: *"is NOT in this table — it ports
  exactly"*, plus a do-not-revert note). **Subtask 2 stands as designed.**
- **Ten, not nine.** The spec now reads *"the ten tokens that cannot round-trip
  faithfully"* and itemises them. Removing `--radius-pill` took 10 → 9, but the
  same amendment **added `--text-eyebrow-transform`** as disposition (b) → back to
  **10**. Re-counted from the spec's own itemisation and from the CSS: they agree.
  The *AC1 disposition table* below is reconciled to exactly these ten.
- **AC1's denominator is unchanged at 127** — re-derived independently here
  (`grep -cE '^\s*--[a-z0-9-]+:'` → 56/30/26/15). Reconciliation: 10 branch-(b) +
  117 branch-(a) = 127.
- **AC15 is new** and mandates the `license` field → **subtask 11 is no longer
  droppable** (its only change this round).
- **Both spec Open questions are closed**, so this design's Open questions 1/3/4
  close with them; 2 was an FYI, not a question. That section is now **None**.

Three design-internal review notes are folded in: the `.gitattributes` body is
pinned **verbatim** (§ *Finding 1*), `core.autocrlf`'s scope is corrected to
**global-only**, and **unit semantics are now stated per token group**
(§ *Unit semantics*).

**Chasing note 4 surfaced two new blocking findings**, both in Group A's
subtasks 2–4 and neither previously visible:

- **Finding 6** — round 1's line-reading `px`-strip parser breaks twice over
  (counts `grep`-verified): **54 of 127** tokens carry a trailing `/* … */` comment
  — **28 in `colors.css`**, which pulls **subtask 1** into the contract round 1
  thought was numeric-only — and **15 of the 53** numeric tokens are not `px`
  (`--space-0: 0` is bare and is declaration #1 in `spacing.css`; `--ls-*` are
  `em`; `--dur-*` are `ms`; `--fw-*`/`--lh-*` are bare).
- **Finding 7** — round 1's inventory assertion (`parse CSS → assert_eq!` against
  an `f32` const) **does not compile**: `clippy::float_cmp` is pedantic, pedantic
  is `deny`, and AC14 lints `--all-targets`. It would have failed at the first
  numeric token in subtask 2.

Both share one remedy (a single `assert_token` helper), verified clippy-clean.
The decomposition and the handoff plan are otherwise **unchanged**; the amendment
forced no re-cut.

### Round 3 — what changed in this revision

Design-review round 2 returned ITERATE on **finding 7 alone**. Scope of this round
is finding 7's table and its remedy; **everything else is untouched** — same
approach, same 12 subtasks, same dependencies, **same handoff plan (M = 12, two
groups of 6 + 6; no re-cut, and none was needed)**. Nothing here is spec-amending.

- **Finding 7's mechanism table was FALSE and is replaced.** Round 2 claimed clippy
  is silent on `assert_eq!(CELL, 24.0)` and errors only once the value comes from
  `include_str!` — and marked it "verified". It was never run. **Every row has now
  been executed** in an isolated crate compile; **every row fires**. The *Risks*
  bullet built on that false premise is rewritten. Corollary: the lint is **less**
  treacherous than round 2 described — it fires immediately and unconditionally.
- **The remedy was under-scoped and is re-layered.** Round 2's `assert_token(name,
  want)` is a CSS-parsing **scalar** comparator, so `RADIUS_PILL`, the 2 `--role-*`
  aliases, the 3 `[f32; 4]` eases, and `--bg-*`'s numbers had **no home** and each
  hard-failed AC14 (all verified firing). Inverted: **the comparator is now the
  primitive and the parsers delegate to it**. Every residual float site has a
  stated home; three of them got *stronger* assertions (`--ease-*` and `--role-*`
  are now value-checked from the CSS, moving AC8's coverage **112 → 117 of 127**).
- **A booby trap was found and defused.** `clippy::float_cmp` silently skips any fn
  whose name ends in `_eq` — so round 2's natural `assert_f32_eq` would have been
  suppressed **by its name**, leaving the `#[allow]` inert and the whole guarantee
  resting on an undeclared coincidence. The comparator is named `assert_f32`, and
  the `#[allow]` is now **verified load-bearing by negative control**.
- **Note (issue 2) accepted: the helper set is hoisted to `tokens/mod.rs`** —
  4 parser copies → 1, 3 `#[allow]` sites → **1**. Call-site count cited in
  *Key decisions*. It lands in subtask 1, which subtasks 2–4 already depend on, so
  the Group A/B boundary is undisturbed.
- **Contract unchanged: `==` stays exact.** No epsilon, no `to_bits`, no weakening.

Method note: this round's conclusions come from **running** every claim — the
round-2 defect was a table asserted from reasoning while the probe beside it only
exercised the remedy.

### Round 4 — the review-note fold-in (design is FINAL after this)

Design-review round 3 returned **GO** with four design-internal notes. All four are
folded in below; **none is spec-amending** (no AC wording, constraint, or count
changes — AC8 coverage stays **117 of 127**). **The handoff plan is untouched and no
re-cut was forced**: M = 12, two groups of 6 + 6.

Every note was **re-verified by running it here**, not accepted on review's
authority — which is the whole point, and it paid for itself:

- **Note 1 — `clippy::arithmetic_side_effects` recorded, `value_of`'s body pinned.**
  The workspace sets it as an **explicit `deny`** (`Cargo.toml:55`, its own line —
  not merely inherited from a group), and it **fires** on both natural spellings of
  a cut-at-`;` reader (verified: `map_or(0, |p| p + 1)` and `rest[colon + 1..semi]`
  both error). It joins `map_unwrap_or` and `option_if_let_else` as the third
  implementor-blind lint. `value_of` was the one helper whose body was **not**
  pinned — it is now (§ *Remedy*).
- **Note 1, continued — review's proposed `value_of` body is BROKEN, and running it
  is how we know.** Review supplied a body it had verified clippy-clean and asked
  that it be checked before pinning. Transcribed literally and run against the real
  CSS, it **fails on `--bg-grid`**: `split_once(name)` returns only the **first**
  occurrence, and `--bg-grid`'s first occurrence is inside **comment prose at
  `effects.css:27`** (`background-image: var(--bg-grid);`), three lines above the
  real declaration at line 29. The starts-a-line check then rejects that match and
  the parser dies, instead of searching on. **126 of 127 tokens pass; `--bg-grid`
  fails.** The predicates were right; the *anchor* was wrong. The pinned body keeps
  review's two predicates verbatim and applies them as a **search over occurrences**
  (`match_indices` + `split_at` + `find_map`) rather than an assert on the first —
  still arithmetic-free, now **127/127**. Review's own maxim held against review's
  own code: unrun code is candidate-truth.
- **Note 2 — prefix collisions; rule 1 gains the `:` clause.** Re-derived
  independently: **14** (short, long) pairs across **9** short tokens, including
  **both AC6 exemplars** (`--cell` ⊂ `--cell-sm`/`-lg`, `--accent` ⊂
  `--accent-hover`/`-press`/`-tint`). **All 14 are short-first in source order**
  (verified by indexing every declaration), so a first-match parser survives on
  ordering luck alone. Rule 1 now also requires the next non-space char to be `:`.
- **Note 3 — the `--role-*` check augments, it does not replace. Review proved this;
  it did not argue it, and the probe reproduces it exactly.** Round 3's
  "tautology / compile-checked" reasoning was **wrong**. Re-ran review's
  counterexample: with `ROLE_DISPLAY_SIZE` mis-pointed at `FS_H2`, the CSS-side
  `var_target` check **PASSES** (it never touches the Rust const) while the Rust-side
  `assert_f32` **FAILS** — `left: 30.0, right: 56.0`, review's "56 vs 30" to the
  digit. **Both** checks are kept, mirroring what subtask 1 already does for the 18
  colour aliases.
- **Note 4 — the funnel inference was over-claimed; restated.** Both halves verified:
  a float `==` inside a `*_eq`-named fn with **no** `#[allow]` is reported **zero**
  times (invisible), while **3** lint-visible sites are reported as **3** (clippy
  does not abort at the first within a crate, so the count *is* informative). The
  negative control therefore proves the `#[allow]` is load-bearing and that no other
  **lint-visible** comparison exists — **not** that every float comparison funnels
  through it. The funnel holds **by construction** (the helper table). Subtask 12's
  `-D warnings` wording is tightened by the same measurement.

### Module layout

```
crates/render/
  fonts/                              # vendored faces (NEW; mirrors upstream `ofl/<family>/`)
    space-grotesk/{SpaceGrotesk[wght].ttf, OFL.txt}
    jetbrains-mono/{JetBrainsMono[wght].ttf, OFL.txt}
  src/
    lib.rs                            # + `pub mod tokens;` `pub mod fonts;`
    tokens/
      mod.rs                          # module doc + deviations table + re-exports + AC8 helper
                                      #   + #[cfg(test)] mod css: the shared CSS parser +
                                      #     the crate's ONE float comparator (finding 7)
      color.rs                        # colors.css      — 56
      spacing.rs                      # spacing.css     — 30
      typography.rs                   # typography.css  — 26
      effects.rs                      # effects.css     — 15
    fonts.rs                          # face bytes, 7 registration keys, `definitions()`
    placeholder.rs                    # repointed (AC12: golden byte-identical)
.gitattributes                        # NEW, repo root — body pinned verbatim in finding 1
```

**Why one submodule per CSS file** (Scope A.1 delegates this): each submodule maps
1:1 onto a source-of-truth file, so AC1's per-file denominator (56/30/26/15) is
auditable *per module*, and the AC8 inventory test naturally lives beside the
consts it guards. Estimated sizes are ~225/~100/~90/~105 lines excl. tests — all
far under the 500/800 soft ladder (`ai-docs/code-style.md` § File size).

*Rejected — a single `tokens.rs`.* Estimated ~455 lines excl. tests against a
500-line soft limit: no headroom, and it would put four unrelated inventories in
one test module. The code-style counter-rule ("don't over-split") targets
one-struct-per-file fragmentation; a split along the source-of-truth boundary is
a responsibility split, not a mechanical one.

### Key decisions

| Question | Decision |
|---|---|
| Module path (Scope A.1) | `gp_render::tokens` (+ `gp_render::fonts`), mirroring `docs/design-system/tokens/`. |
| Radius token type | **`f32`** — finding 2, **ratified by the product owner** (spec amendment). `--radius-pill` is a normal token, not an exclusion. |
| **Unit semantics** | **Documented per group, not crate-wide** — the spec's "1 CSS px = 1 point" shorthand is true only for px-suffixed tokens. See § *Unit semantics* (finding 6). |
| `--shadow-inset` (delegated) | **Port under a distinct `InsetShadow` type** — see *`--shadow-inset` disposition*. |
| Car-ramp accessor (delegated) | `pub fn car_color(index: usize) -> Option<Color32>`, **not** `const fn` — see finding 4. |
| Durations | `std::time::Duration` — `Duration::from_millis(120)` is `const` (verified). Exact; `0.12_f32` is not representable. Consumers call `.as_secs_f32()`. |
| Ease curves | `[f32; 4]` control points — the four numbers *are* the token's value. |
| Semantic aliases (AC5) | `pub const SURFACE_PAGE: Color32 = PAPER_1;` — a Rust const reference. Applies to typography's 2 `--role-*` aliases too. Their test asserts **both** sides: the CSS-side `var()` target name (`var_target`) **and** the Rust-side identity (`assert_f32`). The Rust-side check is **not** a tautology — the const reference compiles fine when pointed at the *wrong* base, and only the identity assertion catches that (**proven**: `ROLE_DISPLAY_SIZE = FS_H2` passes the CSS-side check and fails the Rust-side one, 30 vs 56 — round 4 note 3). Same two-sided logic as the 18 colour aliases in subtask 1. |
| **Test CSS parser + comparator location** | **Hoisted to `tokens/mod.rs` `#[cfg(test)] mod css` — one copy, not four.** **Call-site count: 4** (`color`, `spacing`, `typography`, `effects`), in **1** lib-test binary. This does **not** trip `.claude/agents/design.md`'s ≥3-site duplication rule — that threshold counts **crates or test binaries**, and these are 4 modules inside one binary — so hoisting is a judgement call, and it is taken. It collapses **4 parser copies → 1** and **3 `#[allow]` sites → 1**, and it is what gives finding 7's residual float sites (`--ease-*`, `--role-*`, `--bg-*`) a home: the comparator becomes the primitive and the parsers delegate to it. `tokens/mod.rs` already hosts the `#[cfg(test)] token_names` helper, so the location is established, not invented. Lands in subtask 1; subtasks 2–4 already depend on it. |
| Vendoring layout | Per-family subdirectory. Two files both named `OFL.txt` cannot share a directory, and AC9 says "each beside its `OFL.txt`" — subdirs satisfy it literally and mirror upstream's `ofl/spacegrotesk/`. |
| Font byte consts | `pub const SPACE_GROTESK: &[u8] = include_bytes!(...)` — exactly the `epaint_default_fonts` precedent; **verified to compile clean** under our stricter lints (probe D), including the `[wght]` brackets in the filename. |

### Unit semantics — the module rustdoc states the unit **per group**

The spec's Key-decisions shorthand (*"Spacing / typography unit: `f32` logical
points, 1 CSS px = 1 point"*) is a statement about the **numeric type** (`f32`, not
an integer — `--bw-1: 1.5px` forces it). Read as a statement about **units** it is
**false for 11 of the 26 typography tokens**, verified by reading the CSS:

| Group | CSS | Rust | Unit — what the number MEANS |
|---|---|---|---|
| `--space-*`, `--cell*`, `--radius-*`, `--bw-*`, `--control-h-*`, `--tap-min`, `--content-max`, `--panel-max`, `--fs-*` | `16px` | `f32` | **logical points** (1 CSS px = 1 point) |
| `--lh-*` (3) | `1.05` *(unitless)* | `f32` | **ratio of font size** — a line-height multiplier. `LH_TIGHT` at `FS_DISPLAY` ⇒ 56 × 1.05 = 58.8 pt |
| `--ls-*` (4) | `-0.02em` | `f32` | **ratio of font size** — an em is the font size, so `LS_DISPLAY` at `FS_DISPLAY` ⇒ 56 × −0.02 = −1.12 pt. **Not points.** |
| `--fw-*` (4) | `400` *(unitless)* | `f32` | **OpenType `wght` axis value**, 400–700. **Not a length at all** — fed straight to `VariationCoords::new([(b"wght", FW_BOLD)])`, whose value parameter is `f32` (verified `text_layout_types.rs:425`). |
| `--dur-*` (3) | `120ms` | `Duration` | **milliseconds**, carried in the type — no bare `f32` to misread. |

**Why this is load-bearing, not a doc nicety.** `LS_DISPLAY: f32 = -0.02` and
`FW_BOLD: f32 = 700.0` are both bare `f32` sitting in the same module as
`FS_DISPLAY: f32 = 56.0`. Under a blanket "logical points" banner the first reads
as a −0.02 pt nudge (it is −1.12 pt) and the second as a **700 pt** length. That is
exactly the silent-misuse class `InsetShadow` exists to prevent — except here the
type system cannot help, because these genuinely are `f32`. **The doc is the only
barrier, so it is a deliverable, not a comment.**

Each of `spacing.rs` / `typography.rs` / `effects.rs` opens with a `//!` banner
naming its groups' units; `tokens/mod.rs` carries the table above. Consumers
(#13–#16) multiply `--ls-*`/`--lh-*` by a font size; they never add them to a
point value.

*Rejected — newtypes (`Em(f32)`, `Weight(f32)`).* They would make the misuse a
compile error, but every consumer immediately unwraps to `f32` to hand to egui
(`FontId::new(size, family)` and `VariationCoords` both take bare `f32`), so the
newtype is pure friction at every call site with no API to protect. `InsetShadow`
earns its type because egui has a **colliding** type (`Shadow`) that would silently
do the wrong thing; there is no colliding type here, only a colliding *reading* —
which a doc fixes. Revisit if #13–#16 grow a text-layout helper worth typing.

### The `FontDefinitions` builder

`pub fn definitions() -> egui::FontDefinitions`, starting from
`FontDefinitions::default()` (never `::empty()` — verified: `empty()` yields
`font_data.len() == 0`).

Seven registration keys, each an opaque `font_data` key carrying its own
`FontTweak::coords`, all borrowing from **two** `&'static [u8]` arrays
(`FontData::from_static` → `Cow::Borrowed`, verified `fonts.rs:131–137`):

| Key | Face | `wght` |
|---|---|---|
| `SpaceGrotesk-Regular` / `-Medium` / `-SemiBold` / `-Bold` | `SpaceGrotesk[wght].ttf` | 400 / 500 / 600 / 700 |
| `JetBrainsMono-Regular` / `-Medium` / `-Bold` | `JetBrainsMono[wght].ttf` | 400 / 500 / 700 |

Family wiring — **prepend, never replace** (verified live, see *Test Design*):

- `FontFamily::Proportional` → `["SpaceGrotesk-Regular", "Ubuntu-Light", "NotoEmoji-Regular", "emoji-icon-font"]`
- `FontFamily::Monospace` → `["JetBrainsMono-Regular", "Hack", "Ubuntu-Light", "NotoEmoji-Regular", "emoji-icon-font"]`
- `FontFamily::Name(k)` for each of the 7 keys → `[k, <egui's fallbacks>]`, so emoji still resolve inside a bold heading.

Resulting `font_data.len()` = 4 (egui's) + 7 (ours) = **11**.

`gp-render` produces the value; **`gp-game` applies it** —
`cc.egui_ctx.set_fonts(gp_render::fonts::definitions())` inside the existing
`eframe::run_native` closure (`CreationContext::egui_ctx`, `eframe/src/epi.rs:58`,
whose own rustdoc names `set_fonts` as its intended use). `gp-render` constructs
no `Context`; AC13 re-asserts the draw-only edge.

### `--shadow-inset` disposition (delegated by the spec)

**Port the numeric parameters under a distinct `InsetShadow` type** in
`tokens::effects`, mirroring `epaint::Shadow`'s field names/types
(`offset: [i8; 2]`, `blur: u8`, `spread: u8`, `color: Color32`).

Rationale — the *values* are fully portable (`inset 0 1px 2px rgba(32,30,26,0.14)`
→ offset `[0, 1]`, blur `2`, spread `0`); only the **inset semantics** lack an
egui primitive. Excluding would discard real data that #13–#16 need for pressed
states, and they would have to re-derive it from CSS. A distinct type is the
*minimum* vehicle for a value egui cannot represent, and it makes the actual
failure mode impossible by construction: an `InsetShadow` handed to a
`Shadow`-taking API **does not compile**, whereas a `Shadow`-typed `SHADOW_INSET`
would silently render an *outer* drop shadow — visually plausible and wrong.

This is not speculative abstraction: it is ~6 lines of struct for a token whose
alternative disposition is data loss.

*The other five shadow tokens are `epaint::Shadow` and need no new type* —
`--shadow-0` → `Shadow::NONE`; `--shadow-1/2/3/pop` → offsets `[0,1]/[0,2]/[0,8]/[0,12]`,
blurs `2/6/24/40`; `--focus-shadow: 0 0 0 3px` → `Shadow { offset: [0,0], blur: 0, spread: 3, .. }`.
All fit `i8`/`u8`.

### Findings — re-derivation results

Numbered for reference from *Risks* and *Decomposition*.

#### Finding 1 (blocking) — EOL normalisation silently corrupts a vendored licence file

This repo has **no `.gitattributes`** (verified: `find . -name '.gitattributes'`
→ nothing). Space Grotesk's upstream `OFL.txt` contains **93 CR bytes** (CRLF
endings); JetBrains Mono's does not.

**`core.autocrlf = input` is set GLOBALLY ONLY** — corrected this round; round 1
said "both locally and globally", which was wrong:

```
git config --show-origin --get-all core.autocrlf
  file:/home/syt/.gitconfig   input        # ← the only scope that sets it
git config --local  --get core.autocrlf → exit 1 (unset)
git config --system --get core.autocrlf → exit 1 (unset)
```

**The correction strengthens the case rather than weakening it.** The setting lives
in **one developer's personal `~/.gitconfig`**, not in the repo. So the bytes that
land in a commit depend on **who runs `git add`**: `autocrlf=input` (this machine)
strips the CRs; git's default `false` does not; a typical Windows `true` strips
them too. A vendored file pinned by SHA-256 would therefore hash correctly for some
contributors and not others, and **nothing in the repo controls which**.
`.gitattributes` is the only mechanism that is *committed* — i.e. the only one that
binds every contributor. That portability gap, not this laptop's config, is the
real argument for the file.

Empirically reproduced in a scratch repo (round 1, re-run this round):

```
FILE                       DISK_BYTES  COMMITTED  VERDICT
SG_OFL.txt                       4495       4402  *** MANGLED: 93 bytes stripped ***
JBM_OFL.txt                      4399       4399  byte-exact
SpaceGrotesk[wght].ttf         136676     136676  byte-exact   (binary auto-detect saves it)
```

So `git add` would silently commit a 4,402-byte `OFL.txt`; the SHA-256 pin below
would never verify, and the OFL's redistribution requirement would be met with a
file that is not the licence text upstream shipped. Git emits only a soft
`warning: … CRLF will be replaced by LF` on stderr — a usable tripwire, but not a
failure.

##### The `.gitattributes` body — **exactly this, verbatim**

The whole point of this file is byte-exactness, so its content is pinned here
rather than left to the implementor:

```gitattributes
# Vendored font assets are byte-pinned to their upstream hashes (see
# ai-docs/plans/2026-07-17-render-design-tokens.design.md § Vendoring pin).
# `-text` disables EOL normalisation: without it, a `core.autocrlf` set in any
# contributor's personal ~/.gitconfig rewrites the CRLF endings in Space
# Grotesk's OFL.txt at `git add` time (4,495 -> 4,402 B, 93 CR stripped),
# silently breaking the SHA-256 pin and the OFL's redistribution requirement.
crates/render/fonts/** -text
```

**One rule, not two.** Verified this round in a scratch repo — with this exact body
(comments included), all four assets stage with `blob == git hash-object
--no-filters`, `git check-attr text` reports `unset` for each, and no CRLF warning
is emitted. The `**` does match nested paths (`fonts/space-grotesk/OFL.txt`).

**`*.png binary` is deliberately OMITTED.** It was implied by round 1's *Risks*
section and is **inert**: the golden already stages raw today (disk 3,895 = blob
3,895; `git hash-object --no-filters` == `HEAD:…placeholder.png` ==
`590b692e…`), and adding the line changes the golden's `text` attribute
`unspecified` → `unset` **without changing a single staged byte** (verified both
ways). It is inert on *every* platform, not just this one: `core.autocrlf` only
touches files git classifies as text, and a PNG's IHDR length field puts NUL bytes
at offset 8, so git's binary auto-detection cannot fail for it. A rule that does
nothing on any platform, in a file whose only job is byte-exactness, is worse than
absent — it implies the golden was at risk and invites a future reader to trust it.
The golden's protection is auto-detection; that fact belongs in *Risks*, which is
where it now lives. (PNG's signature does embed a literal CRLF — precisely as a
canary against text-mode mangling — so the concern is well-founded in general; it
is simply already handled.)

##### Ordering: the file must **exist on disk** before `git add`, not be committed first

Verified as an A/B pair this round — this is the detail subtask 7 turns on:

| | `.gitattributes` state at `git add` | staged blob |
|---|---|---|
| **A** | on disk, **unstaged** | **== raw** (byte-exact) |
| **B** | absent (control) | **≠ raw** — mangled, + the CRLF warning |

Git reads attributes from the **working tree**, so writing the file is sufficient;
it need not be staged or committed first, and it may be staged in the same
`git add` as the assets. The `.ttf` files survive today via binary auto-detection,
but the rule covers them anyway — they are under the same path and the pin asserts
all four.

#### Finding 2 (RATIFIED — spec amended) — `--radius-pill` is *not* unportable; it round-trips exactly as `f32`

> **Status: accepted by the product owner and folded into the spec.** The exclusion
> row is gone; the spec now carries its own do-not-revert note. Retained here as the
> derivation. **Do not re-derive the `u8` objection and revert this** — the
> saturation is what makes the exact port safe. Subtask 2 stands as designed.

Round 1's spec exclusion table said `CornerRadius`'s `u8` fields make `999px`
unrepresentable, so it must ship as `255` with a documented deviation. That is
true **only if radii are typed `u8`** — but Scope A.3 explicitly lists `--radius-*`
under "Spacing / sizing → **f32** logical points". The two statements conflicted.

Resolved in favour of Scope A.3, because `epaint` accepts the f32 directly:

```rust
impl From<f32> for CornerRadius {
    fn from(radius: f32) -> Self { Self::same(radius.round() as u8) }   // corner_radius.rs:41
}
```

`radius.round() as u8` is a **saturating** float→int cast. Verified by running it:
`CornerRadius::from(999.0f32)` → `CornerRadius { nw: 255, ne: 255, sw: 255, se: 255 }`.

So `pub const RADIUS_PILL: f32 = 999.0;` **matches the CSS exactly** (AC1 branch
(a)), and the 255 clamp happens inside epaint at the use site — its documented
behaviour, not our lossy re-typing. Semantically the clamp is correct anyway: 999
and 255 both mean "fully rounded" for any control height in this system
(`--control-h-lg` = 46px). `painter.rect_filled(rect, RADIUS_2, fill)` compiles
unchanged via `impl Into<CornerRadius>`.

**Consequence: `--radius-pill` left the exclusion table**, and radii are `f32`
like every other length token — one mental model for all of `spacing.css`. This
edited a spec table, so round 1 raised it as an Open question rather than assuming
it; the product owner ratified it and `spec-writer` applied it. The exclusion count
stayed at **ten** because the same amendment added `--text-eyebrow-transform` as
disposition (b) — see *Round 2* above.

#### Finding 3 (blocking) — Space Grotesk's `wght` **default is 300**, not 400

Read from the font's own `fvar` table and confirmed through
`FontData::variation_axes()` at runtime:

| Face | `wght` min | **default** | max |
|---|---|---|---|
| Space Grotesk | 300 | **300** | 700 |
| JetBrains Mono | 100 | 400 | 800 |

A `FontData::from_static(SPACE_GROTESK)` registered **without** a `coords` tweak
therefore renders **Light (300)**, not Regular — silently, with no error. There is
no "register the file bare for the 400 case" shortcut. **All seven instances carry
an explicit `coords` override**, including both Regular 400s (JBM's 400 happens to
match its default; it is still written explicitly, so an upstream default change
cannot silently re-weight our UI).

This also confirms the spec's axis-coverage claim: SG 400–700 ⊂ 300–700, JBM
400–700 ⊂ 100–800.

#### Finding 4 — `car_color` cannot be `const fn`, and `missing_const_for_fn` correctly declines

Probed against the real workspace lint set (`pedantic` + `nursery` = deny):

- **A.** `pub fn car_color(i: usize) -> Option<Color32> { CAR_COLORS.get(i).copied() }` → **clippy clean**; `missing_const_for_fn` does **not** fire.
- **B.** the same body as `pub const fn` → **hard error**:
  `error[E0658]: cannot call conditionally-const method `core::slice::<impl [Color32]>::get::<usize>` in constant functions`
  / `` error: `core::slice::<impl [T]>::get` is not yet stable as a const fn ``
- **C.** `const fn` via an explicit bounds check + raw index → compiles clean.

This is exactly the `Rect::index` counter-example from `.claude/agents/design.md`:
const-*ineligibility* comes from the **callee's const-stability**, so the lint
correctly declines and non-`const` is permitted, not merely tolerated.

**Chosen: form A.** Both A and C are lint-clean, so this is a genuine design
choice, not a lint-forced one — and A is the idiomatic combinator form AGENTS.md
§ Rust idioms prefers, with no raw indexing. C's only gain is const-context use,
which no consumer has: #13–#16 index by a runtime car index.

The const-ineligibility **must be recorded in the fn's doc**, or a future reviewer
"tidies up" a missing `const` and hits E0658.

#### Finding 5 — the new font tests are Miri-safe (no `cfg_attr(miri, ignore)` needed)

`variation_axes()` parses through skrifa's zero-copy readers, and the crate's
`golden_guard` is already Miri-gated, so this was worth checking rather than
assuming. Ran the probe under the CI job's exact flags:

```
MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test
→ test result: ok. 1 passed; finished in 1.61s
```

`variation_axes()`, `FontDefinitions::default()`, and the builder are all clean
under Tree Borrows and fast. **Do not add a `miri` gate** to the AC9/AC10 tests —
unlike `golden_guard`, they drive no FFI/GPU.

#### Finding 6 (blocking) — the value parser: **54 of 127** tokens carry a trailing comment, and **15 of 53** numerics are not `px`

Surfaced while documenting unit semantics. The test-side "value-checked inventory"
parses each CSS value and asserts it against the const. Round 1 described that as a
*"px-strip"* reading a line. Read against the actual CSS, both halves of that break.
All counts below are **verified by `grep -cE` against the four files**, not
estimated:

**(A) Trailing `/* … */` comments — 54 of 127 tokens, in all four files.**

| File | Comment-bearing | Example |
|---|---|---|
| `colors.css` | **28** | `--paper-1:  #F5F1E6;   /* base graph-paper cream */` |
| `spacing.css` | 6 | `--cell:  24px;   /* one graph-paper square */` |
| `typography.css` | 14 | `--fw-regular:  400;  /* @kind other */` |
| `effects.css` | 6 | `--dur-fast:  120ms;  /* @kind other */` |
| **total** | **54** | |

A parser that reads **to end-of-line** ingests `#F5F1E6;   /* base graph-paper
cream */`. **This pulls subtask 1 (colours) into the contract** — round 1 assumed
only the numeric modules parsed values. **Cut at the `;`, never at `\n`.** Aliases
need it too: `--surface-ink: var(--graphite-900);   /* inverse / dark panels */`.

> **The insidious part:** `--accent: #E24A2B;` — **AC6's own exemplar** — has *no*
> trailing comment, while `--paper-1` does. A writer who validates the parser on
> the exemplar the spec names sees it work, then fails on the bulk of the file.

**(B) Units — 15 of the 53 numeric-parsed tokens are not `px`.**

| Unit | Count | Tokens | A `px`-strip does… |
|---|---|---|---|
| `px` | **38** | 29 spacing + 9 `--fs-*` | works |
| **bare** `0` | 2 | `--space-0`, `--ls-normal` | `strip_suffix("px")` → `None` → **panic**. Note `--radius-0: 0px` *does* carry the suffix — the source is inconsistent, so "0 is always bare" is equally wrong. |
| **bare** ratio/weight | 7 | 3 `--lh-*`, 4 `--fw-*` | **panic** |
| `em` (one negative) | 3 | `--ls-display` (`-0.02em`), `--ls-label`, `--ls-mono` | **panic**; `-` must survive the parse |
| `ms` | 3 | 3 `--dur-*` | **panic** — these are `Duration`, not points |
| | **53** | | 38 + 15 non-`px` |

`--space-0: 0` is **declaration #1 in `spacing.css`**, so a naive
`v.trim().strip_suffix("px").unwrap().parse::<f32>()` panics on the very first
token it meets.

**(C) Multi-line values** — `--bg-grid` and `--bg-dots` span continuation lines,
so any read must run to the `;`, not the newline. (They are branch-(b) decomposed,
not numerically parsed, but the same cut rule applies.)

**Consequence.** Every inventory parser — **subtasks 1–4** — takes the value as
**the text between `:` and the terminating `;`**, trims it, then dispatches: hex →
`Color32` · `var(--x)` → alias target · `px` → points · `em` → ratio · `ms` →
`Duration` · bare numeric → ratio/weight per group. An unhandled case is a **test
failure naming the token**, never an `unwrap` panic — a panicking test parser
reports `called Option::unwrap on a None value` and names neither the token nor the
file, which is the worst possible message for a test whose entire job is to say
*which* token drifted.

The **AC8 name-only helper is unaffected** — it reads names up to the `:`, so
comments and units never reach it, and round 1's validation that it reproduces
56/30/26/15 = 127 still holds (re-confirmed this round by the same `grep -cE`).

#### Finding 7 (blocking) — the round-1 inventory test **does not compile**: `clippy::float_cmp` is denied

Surfaced while validating finding 6's parser. `float_cmp` is a **pedantic** lint, and
`Cargo.toml` sets `pedantic = { level = "deny", priority = -1 }`; `gp-render` inherits
it (`[lints] workspace = true`). AC14 runs
`cargo clippy --workspace --all-targets -- -D warnings`, which **covers test code**.
Round 1's Test Design says *"parse `--cell: 24px` → assert `CELL == 24.0`"* — that
assertion is a **hard build failure**.

> **Round 3 correction — the round-2 table was wrong and is replaced.** Round 2
> claimed clippy is *silent* on `assert_eq!(CELL, 24.0)` and errors only once the
> value comes from `include_str!`, and built a "probing in isolation misleads you"
> narrative on it. **That row is false.** It was never run — round 2's probe only
> exercised the remedy. Every row below has now been **executed**, each in its own
> isolated crate compile (so a `-D warnings` abort cannot mask a later row), on
> rustc 1.97.1 (8bab26f4f) / clippy 0.1.97 with the workspace lint set copied
> verbatim. **The real situation is *less* treacherous, not more: the lint fires
> immediately and unconditionally. It does not lie in wait for `include_str!`.**

Every row **RUN**; every row **FIRES**. Nothing here is inferred:

| Assertion | `float_cmp` | Where it would appear |
|---|---|---|
| `assert_eq!(CELL, 24.0)` — both const, **equal** | **error** | — (round 2 claimed *silent*) |
| `assert_eq!(CELL, 25.0)` — both const, **unequal** | **error** | — |
| `assert_eq!(parsed_from_css, CELL)` | **error** | the inventory strategy itself |
| `assert_eq!(got, want, "token {name}")` in a loop | **error** | the inventory strategy itself |
| `assert_eq!(RADIUS_PILL, 999.0)` | **error** | subtask 2 |
| `assert_eq!(ROLE_DISPLAY_SIZE, FS_DISPLAY)` ×2 | **error** | subtask 3 |
| `assert_eq!(EASE_STANDARD, [0.2, 0.0, 0.1, 1.0])` | **error** | subtask 4 |
| `assert_eq!(BG_DOTS_RADIUS, 1.2)` ×3 | **error** | subtask 4 |

The `[f32; 4]` row reports a *different message* — `strict comparison of `f32` or
`f64` **arrays**` — but it is the **same lint** (`-D clippy::float-cmp` implied by
`-D clippy::pedantic`), so one `#[allow(clippy::float_cmp)]` covers scalars and
arrays alike. Verified by reading the full diagnostic, not the message text.

**Why it is unconditional — and why no dodge works.** Also run:

| Shape | fires? |
|---|---|
| `a == 0.0` (param vs zero literal) | silent |
| `a == f32::INFINITY` | silent |
| `a == CELL` (param vs named const) | silent |
| **`assert_eq!(SPACE_0, 0.0)` — both const, *zero*** | **error** |
| **`assert_eq!(parsed, SPACE_0)` — runtime vs zero** | **error** |

`assert_eq!` expands to `match (&$left, &$right) { (left_val, right_val) => … *left_val == *right_val … }`,
so by the time the lint sees the comparison both operands are **opaque bindings**,
not the literals written at the call site. Every exemption that rescues a bare `==`
(zero, infinity, a named const) is therefore **unreachable from `assert_eq!`** —
and every site in this design is an `assert_eq!`. Do not try to dodge the lint by
comparing against `0.0` or a named const; it fires anyway.

**`==` is nonetheless the correct contract, and the lint is a false positive here.**
Verified: `"−0.02"` and `"1.05"` are *not* exactly representable in `f32`, yet
`"-0.02".parse::<f32>() == -0.02_f32` and `"1.05".parse::<f32>() == 1.05_f32` both
hold — Rust's float parsing and float literals are **both correctly rounded**, so
the two spellings land on bit-identical `f32`. An epsilon would be actively wrong:
it would mask exactly the token drift AC8 exists to catch.

##### The `_eq` booby trap — read this before naming anything

Round 2's remedy was `assert_token(name: &str, want: f32)` carrying the `#[allow]`.
While verifying it this round, a **negative control** (strip the `#[allow]`, expect
a hard error) came back **clean** — i.e. the attribute was doing nothing. Probed to
ground truth, one fn-name at a time, body byte-identical in every case:

| helper name | `float_cmp` |
|---|---|
| `assert_f32_eq` | **silent** |
| `assert_token_eq` | **silent** |
| `eq_f32` | **silent** |
| `assert_f32_equal` | **error** |
| `assert_token` | **error** |
| `check_f32` / `compare_f32` | **error** |

**`clippy::float_cmp` skips any fn whose name is `eq`/`ne`/`is_nan`, starts with
`eq_`, or ends with `_eq`.** So a helper called `assert_f32_eq` is silenced *by its
name*, and its `#[allow]` is **inert** — the suppression is accidental, undeclared,
and evaporates the moment someone renames the helper to `assert_f32_equal`. This
design therefore **names the comparator outside the heuristic** so the `#[allow]` is
load-bearing and its removal is a build failure. That property is **verified by
negative control, not asserted** (see below). The rustdoc on the fn records the
naming constraint so a future "tidy-up" rename cannot silently re-arm the trap.

**Remedy — ONE comparator for the whole crate, hoisted into `tokens/mod.rs`.**
Round 2 scoped the `#[allow]` to `assert_token`, a CSS-parsing **scalar**
comparator — which left every non-scalar and non-CSS float site with **no home**
(all verified firing above): `RADIUS_PILL == 999.0`, the two `--role-*` aliases,
the `[f32; 4]` eases, and `--bg-*`'s `1.0`/`1.2`/`1.4`. The fix is to invert the
layering: **the comparator is the primitive; the parsers delegate to it.**

`tokens/mod.rs` gains a `#[cfg(test)] pub(crate) mod css` — beside the existing
`token_names` helper — containing:

| Helper | Float `==`? | `#[allow]`? |
|---|---|---|
| `value_of(css, name) -> &str` — finding 6's cut-at-`;` reader; **body pinned below** | no (string) | no |
| **`assert_f32(label, got, want)`** — **the ONE comparison site** | **yes** | **yes — the crate's only one** |
| `assert_f32_slice(label, got, want)` — element-wise, names the index | no (delegates) | no |
| `assert_token(css, name, want)` — strip `px`/`em`, parse, delegate | no (delegates) | no |
| `assert_cubic_bezier(css, name, want)` — 4 control points, delegate | no (delegates) | no |
| `var_target(css, name) -> &str` — `var(--x)` → `--x` | no (string) | no |

Only `assert_f32` compares floats; everything else routes through it. Note the
names deliberately avoid the `_eq` suffix (`assert_f32`, not `assert_f32_eq`).

```rust
/// The ONLY float-comparison site in the crate.
///
/// NOTE: must NOT be named `*_eq` / `eq_*` — `clippy::float_cmp` silently skips
/// such fns, which would make the `#[allow]` below inert and the suppression
/// accidental rather than declared.
#[allow(
    clippy::float_cmp,
    reason = "CSS text and the const are two spellings of one decimal; Rust's \
              float parsing and float literals are both correctly rounded, so \
              they yield bit-identical f32 even for values like 1.05 that are \
              inexact in binary. Exact equality is the intended contract - an \
              epsilon would mask the token drift AC8 exists to catch."
)]
pub(crate) fn assert_f32(label: &str, got: f32, want: f32) {
    assert_eq!(got, want, "{label}: CSS value != const");
}
```

##### `value_of` — body pinned, and **why review's proposal is not the one pinned**

`value_of` was the one helper this design left unpinned while pinning `assert_f32`
in full — so *"the implementor copies; it does not invent"* held for the comparator
and failed for the parser. It is pinned now. **A third denied lint governs it:**
`arithmetic_side_effects` (`Cargo.toml:55`, an explicit `deny` on its own line)
**fires on the natural index arithmetic of a cut-at-`;` reader** — verified, both
spellings error: `s.find(':').map_or(0, |p| p + 1)` and `&rest[colon + 1..semi]`.
The body below has **no index arithmetic**, so it needs **no `#[allow]`**.

Review proposed a body and asked that it be run before pinning. **It was, and it is
broken** — `126 of 127`:

```
token --bg-grid: match does not start a line     *** FAIL effects.css --bg-grid
value_of_review: 126 ok, 1 FAILED
```

`split_once(name)` yields only the **first** occurrence, and `--bg-grid`'s first
occurrence is **comment prose at `effects.css:27`** — `background-image:
var(--bg-grid);` — three lines above the real declaration at line 29. The
starts-a-line predicate correctly rejects that match, but `split_once` has already
committed to it, so the parser dies instead of searching on. Review's **predicates
are right and are kept verbatim**; only the **anchor** changes — from *assert on the
first occurrence* to *search the occurrences*. Note the `:` clause (note 2) is what
actually discriminates line 27 (`var(--bg-grid)` → next char is `)`, not `:`), but
it can only do so if the parser is allowed to keep looking.

```rust
/// The value text between `:` and the terminating `;`, for the declaration of
/// `name` that starts a line (finding 6 + the prefix-collision clause).
///
/// Anchored by SEARCHING the occurrences, not by taking the first: `--bg-grid`
/// occurs in comment prose at `effects.css:27` before its real declaration at
/// line 29, so a `split_once` anchor binds to the comment and dies. Deliberately
/// free of index arithmetic — `clippy::arithmetic_side_effects` is a workspace
/// `deny` and fires on the `colon + 1` spelling of this same function.
pub(crate) fn value_of<'a>(css: &'a str, name: &str) -> &'a str {
    let rest = css
        .match_indices(name)
        .find_map(|(idx, _)| {
            let (before, at) = css.split_at(idx);
            let after = at.strip_prefix(name)?;
            // Rule 1b: the next non-space char must be `:` — `--cell` vs `--cell-sm`.
            let value = after.trim_start().strip_prefix(':')?;
            // Rule 1a: the declaration must start a line.
            before
                .lines()
                .next_back()
                .is_none_or(|l| l.trim().is_empty())
                .then_some(value)
        })
        .unwrap_or_else(|| panic!("token {name}: no declaration starts a line"));
    rest.split_once(';')
        .unwrap_or_else(|| panic!("token {name}: value has no terminating ';'"))
        .0
        .trim()
}
```

**Verified as pinned**, against the real four CSS files: `cargo clippy
--all-targets -- -D warnings` **clean** (no `arithmetic_side_effects`, no
`map_unwrap_or`, no `option_if_let_else`), and **127 of 127** tokens resolve —
including `--bg-grid`/`--bg-dots` past the comment prose, and both AC6 exemplars
against their longer siblings (`--cell`→`24px` vs `--cell-sm`→`16px`,
`--space-1`→`4px` vs `--space-10`→`40px`).

> **`find_map`, not `filter_map(..).next()`** — the latter trips
> `clippy::filter_map_next` (pedantic → deny). Likewise `var_target` must use a
> `let … else`, **not** `.map(..).unwrap_or_else(panic!)`, which trips
> `clippy::map_unwrap_or` (found by running the probe, not by reading).

**Verified end-to-end** against a probe mirroring the real module layout
(`tokens/mod.rs` + `spacing.rs`/`typography.rs`/`effects.rs`), fed the **real**
`spacing.css` / `typography.css` / `effects.css`:

1. **With** the `#[allow]`: `cargo clippy --all-targets -- -D warnings` → **clean**.
2. `cargo test` → **6 passed**, covering bare `0` (`--space-0`, `--ls-normal`),
   suffixed `0px` (`--radius-0`), `px`, `em` + negative + trailing comment
   (`--ls-display`), the binary-inexact `1.05` (`--lh-tight`), the bare weight
   `700` (`--fw-bold`), `999px` (`--radius-pill`), both `--role-*` aliases, both
   `cubic-bezier` eases, `--dur-fast`, and the multi-line `--bg-*` recipes.
3. **Negative control** (`cargo clean` + strip the `#[allow]`): **`error: strict
   comparison of f32 or f64` — exactly ONE**, then `could not compile`.

**What the negative control does and does not prove** (round 4, note 4 — the
inference was over-claimed and is restated; both halves re-measured here):

- **It DOES prove the `#[allow]` is load-bearing.** Removing it breaks the build.
- **It DOES prove no other *lint-visible* float comparison exists** — and the count
  is informative, not just a first-hit abort: with **3** bare `==` sites added,
  clippy reports **3**, not 1. Within a crate every lint error is reported.
- **It does NOT prove that *every* float comparison funnels through that single
  site.** A comparison inside a fn named `*_eq` / `eq_*` is **invisible** to the
  control: verified — a bare `a == b` in `fn silent_f32_eq` with **no** `#[allow]`
  is reported **zero** times, leaving the count at exactly ONE regardless. **This
  design's own `_eq` finding (below) is the counterexample to that inference** — the
  same heuristic that would have silenced `assert_f32_eq` also hides any stray
  comparator a future contributor names that way.
- **The funnel is guaranteed BY CONSTRUCTION, not by the error count** — the helper
  table above is the closed set of comparison sites, and only `assert_f32` compares
  floats. AC14 rests on the **positive** control (the `#[allow]` is required for the
  build to pass), which is sound. A future reviewer must not read "exactly one
  error" as an exhaustiveness proof.

**Every residual float site now has a stated home** (issue 1(b)) — and three of them
get *stronger* assertions than round 2 proposed, not weaker:

| Site | Round-2 status | Round-3 home |
|---|---|---|
| `RADIUS_PILL == 999.0` (subtask 2) | no home, **fires** | **Deleted as redundant.** The inventory already does `assert_token(CSS, "--radius-pill", RADIUS_PILL)`, which value-checks it *against the CSS* — strictly stronger than restating `999.0` in the test. The `CornerRadius::from(RADIUS_PILL) == CornerRadius::same(255)` saturation assertion **stays** and needs no allow: `CornerRadius`'s fields are `u8`, so it is an integer comparison (**verified: clippy-clean and passing** against real `egui` 0.35). |
| `ROLE_DISPLAY_SIZE == FS_DISPLAY`, `ROLE_VALUE_SIZE == FS_H2` (subtask 3) | no home, **fires** | **AUGMENTED, not replaced (round 4, note 3 — round 3 had this WRONG).** Round 3 dropped the Rust-side identity as a "tautology, the const reference is compile-checked" and kept only `var_target(CSS, "--role-display-size") == "--fs-display"`. **That reasoning is unsound and the probe disproves it:** the identity is a tautology only *given correct code* — which is true of every test ever written. It is precisely the guard against the const being pointed at the **wrong base**, and a const reference to the wrong base **compiles fine**. Re-ran review's counterexample (`ROLE_DISPLAY_SIZE = FS_H2`): the CSS-side check **PASSES** — it never touches the Rust const — while the Rust-side check **FAILS**, `left: 30.0, right: 56.0`. So: **keep both.** `var_target(…)` for the CSS side **and** `assert_f32("ROLE_DISPLAY_SIZE", ROLE_DISPLAY_SIZE, FS_DISPLAY)` for the Rust side — one line each, routed through the existing comparator, **zero new `#[allow]`s**. This is what hoisting the comparator bought, and it restores consistency with subtask 1, which keeps **both** checks for the 18 colour aliases on identical logic. |
| 3 × `--ease-*` `[f32; 4]` (subtask 4) | no home, **fires** | **Upgraded to value-checked from CSS.** `assert_cubic_bezier(CSS, "--ease-standard", EASE_STANDARD)` parses `cubic-bezier(0.2, 0, 0.1, 1)` → `[0.2, 0.0, 0.1, 1.0]` and delegates element-wise. Was a hand-written expectation; is now real drift detection. (The CSS writes `0` and `1`, not `0.0`/`1.0` — the parser handles both; verified.) |
| `--bg-*` `1.0` / `1.2` / `1.4` (subtask 4) | no home, **fires** | `assert_f32("BG_DOTS_RADIUS", BG_DOTS_RADIUS, 1.2)` — a hand-written expectation (the numbers live inside a multi-line gradient *recipe*, not a `name: value;` declaration), but routed through the one comparator. Additionally pinned against the CSS text: `value_of(CSS, "--bg-dots").contains("1.2px")`. |

*Rejected — `#[expect(clippy::float_cmp, …)]`.* House style is `allow`: **17**
`#[allow(…, reason = …)]` sites and **zero** `#[expect]` in-tree. Introducing a
second attribute form for one test helper is a gratuitous divergence; adopting
`expect` workspace-wide is an `/improve` question, not #12's. (It is also a live
foot-gun *here specifically*: an `#[expect]` on a helper that the `_eq` heuristic
already silences would fire `unfulfilled_lint_expectation` → `-D warnings` → build
failure. `allow` degrades silently where `expect` would break the build.)

*Rejected — `assert_eq!(got.to_bits(), want.to_bits())`.* Lint-clean and bit-exact,
but it reports `left: 1067869798` on failure, which is unreadable for a test whose
job is to name the drifted token.

*Rejected — an epsilon.* Wrong contract, masks drift — and unnecessary: verified
that `"-0.02".parse::<f32>() == -0.02_f32` and `"1.05".parse::<f32>() == 1.05_f32`
both hold, since Rust's float parsing and float literals are **both correctly
rounded** onto bit-identical `f32`. (For the record, `f32::EPSILON` would not even
work: `FS_DISPLAY * LH_TIGHT` differs from `58.8` by `3.8e-06` ≈ **32 ×**
`f32::EPSILON`.)

**Scope: all four modules 1–4 share the one helper set; subtask 1 lands it.** The
`#[allow]` count for the whole task is **1** (round 2's plan: 3). Subtask 1's own
colour assertions still need no `float_cmp` allow — `Color32` is four `u8`s — but
subtask 1 **owns** `tokens/mod.rs`, so the shared `css` module lands there, and
subtasks 2–4 (which already depend on 1) consume it. **This does not exempt subtask
1 from finding 6**, whose cut-at-`;` rule binds it hardest (28 of its 56 tokens
carry a trailing comment).

**Three lints the implementor will hit while writing the parser** (all found by
running, all fixed in the probe; recorded so they are not rediscovered):

1. **`clippy::arithmetic_side_effects`** (**an explicit workspace `deny`**,
   `Cargo.toml:55` — not merely inherited from `pedantic`/`nursery`, and the one
   this parser hits hardest). It **fires on the natural index arithmetic of a
   cut-at-`;` reader** — verified, both spellings error:
   `s.find(':').map_or(0, |p| p + 1)` and `&rest[colon + 1..semi]`. **The pinned
   `value_of` body above sidesteps it entirely** (`match_indices` / `split_at` /
   `strip_prefix` / `split_once` — no arithmetic operator anywhere), which is why it
   carries no `#[allow]`. Do **not** reintroduce index math to "simplify" it.
2. **`clippy::map_unwrap_or`** (**pedantic**) on
   `…find(…).map(…).unwrap_or_else(panic!)` — use a `let … else`. (Hit for real in
   `var_target` while verifying this round.)
3. **`clippy::option_if_let_else`** (**nursery**) on the natural `if let Some(n) =
   raw.strip_suffix("px") … else if let … else …` suffix chain. It dissolves once
   you notice `px`/`em`/bare all parse **identically** — the unit is semantics, not
   syntax — so the whole dispatch collapses to
   `raw.strip_suffix("px").or_else(|| raw.strip_suffix("em")).unwrap_or(raw).trim().parse()`,
   with a non-numeric leftover (e.g. `120ms`) falling out as finding 6's named panic.

### Vendoring pin (spec Open question 2)

Established against live upstream on **2026-07-17**. The spec's byte sizes
re-confirmed **exactly** (136,676 / 187,208 / 4,495 / 4,399).

**Source:** `google/fonts`, tree pinned at commit
**`389b770410cc0b7c21c85673bfa2077420fe7f65`** (`main`, 2026-07-16T17:54:03Z).

| Upstream path | Bytes | SHA-256 | git blob |
|---|---|---|---|
| `ofl/spacegrotesk/SpaceGrotesk[wght].ttf` | 136,676 | `acad6de1fc93436f5c0f1f4137751ef04f1aea3063e7036535970ffcfbd79f72` | `a1b2e6c26093066510a31147e7aec9abdc8d6c5e` |
| `ofl/spacegrotesk/OFL.txt` | 4,495 | `564ce565c371c5e5bbf286006565a7c9aa55a9f56e7ca58d56e05d649dd61a72` | `cb512b9af44ff61e75e1aad387b7424cdfab36a3` |
| `ofl/jetbrainsmono/JetBrainsMono[wght].ttf` | 187,208 | `48715a42ec242c21e9f02692891e147d022299a52e48d5e413e1a942193ffeda` | `aa310be8b717fe3774f9444dd89d5f4101cc6d10` |
| `ofl/jetbrainsmono/OFL.txt` | 4,399 | `b2fe5e8987594e9ffd1d2ca52a2f5d73eb8335243893c5d6254b5ad69269591d` | `821a3dac22aff15a1f1c9689a1d79c45bb58ca39` |

Fetch (note the `[`/`]` must be percent-encoded for `raw.githubusercontent.com`):

```bash
SHA=389b770410cc0b7c21c85673bfa2077420fe7f65
curl -sSL -o 'SpaceGrotesk[wght].ttf' \
  "https://raw.githubusercontent.com/google/fonts/$SHA/ofl/spacegrotesk/SpaceGrotesk%5Bwght%5D.ttf"
```

Licence facts re-verified from the files themselves (not from `METADATA.pb` prose):
both are OFL-1.1; copyright lines are `Copyright 2020 The Space Grotesk Project
Authors (https://github.com/floriankarsten/space-grotesk)` and `Copyright 2020 The
JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono)`;
**neither declares a Reserved Font Name** (the single "Reserved Font Name" hit in
each file is the OFL's own generic definitions section), so the faces may be
vendored unrenamed. `JetBrainsMono-Italic[wght].ttf` (191,556 B) exists upstream
and is deliberately **not** vendored (spec § Out of scope).

`git blob` is recorded alongside SHA-256 because it is what finding 1's
byte-exactness check compares against directly (`git hash-object --no-filters`).

### AC1 disposition table

The module's rustdoc carries this table. **Every row is an AC1 branch-(b)
disposition and there are exactly TEN** — matching the amended spec's itemisation
token-for-token: `--shadow-inset` (1) + `--font-*` (3) + `--ease-*` (3) +
`--bg-grid`/`--bg-dots` (2) + `--text-eyebrow-transform` (1) = **10**.

Every token **not** in this table is a plain 1:1 const (branch (a)). The
reconciliation is the spec's: 10 branch-(b) + **117** branch-(a) = **127**.

| # | Token(s) — all branch (b) | Disposition |
|---|---|---|
| 1–3 | `--font-display`, `--font-ui`, `--font-mono` | Primary family name only (`"Space Grotesk"` ×2, `"JetBrains Mono"`). The CSS stack's fallbacks (`ui-sans-serif`, `system-ui`, `SFMono-Regular`, `Menlo`) are browser concepts; egui supplies the fallback role structurally via `FontDefinitions`' per-family list. |
| 4–6 | `--ease-standard`, `--ease-out`, `--ease-in` | `[f32; 4]` control points — values exact, shape differs (no egui easing type). |
| 7 | `--bg-grid` | Decomposed: `BG_GRID_RULING_WIDTH: f32 = 1.0` + `BG_GRID_COLOR = color::GRID_LINE` + pitch = `spacing::CELL`. A CSS gradient recipe, not a value. |
| 8 | `--bg-dots` | Decomposed: `BG_DOTS_RADIUS: f32 = 1.2`, `BG_DOTS_TRANSPARENT_STOP: f32 = 1.4`, `BG_DOTS_COLOR = color::GRID_DOT`. |
| 9 | `--shadow-inset` | `InsetShadow` — distinct type; egui has no inner-shadow primitive (see above). |
| 10 | `--text-eyebrow-transform` | **Excluded.** A text-transform behaviour, not a value token; belongs to whichever component draws an eyebrow (spec § Deferred → #13–#16). Listed so AC1's "no token is silently absent" is literally true. |

**Listed for completeness — branch (a), NOT among the ten.** Mirrors the spec's own
convention of tabling `--shadow-0` and marking it "not one of the ten":

| Token(s) | Why it is (a), not (b) |
|---|---|
| `--shadow-0: none` | `Shadow::NONE` (`shadow.rs:40`) — an exact port. |
| `--radius-pill: 999px` | `RADIUS_PILL: f32 = 999.0` matches the CSS exactly; `From<f32>` saturates to 255 at the use site (finding 2, **ratified**). |
| `--shadow-1/2/3/pop`, `--focus-shadow` | Written with the **exact** CSS numbers; premultiplied *storage* is epaint's documented internal representation. See the carve-out below. |

**The alpha row moved out of the branch-(b) table this round** (round 1 listed it
there, which made the table read as 15 tokens and disagree with the spec's ten).
Alpha storage is an **AC6 test-methodology** matter, **not** an AC1 disposition:
`SHADOW_1`'s colour is `from_rgba_unmultiplied_const(32, 30, 26, 20)` — the CSS's
own numbers, unaltered — so the const's *value* matches the CSS and AC1 is
satisfied by branch (a). This is the **same principle the product owner just
ratified for `--radius-pill`**: a lossless const plus a documented conversion
inside epaint is an exact port, not a deviation. Only a round-trip *back* through
`to_srgba_unmultiplied()` is lossy, and that affects what a **test** may assert —
never whether the token ported.

**Alpha round-trip (AC6 carve-out).** `from_rgba_unmultiplied_const` premultiplies.
The spec cited epaint's "might be slightly different (rounding errors)" rustdoc;
measured, it is worse than "slightly" for the dark, low-alpha shadow colour, and
the design should not pretend otherwise:

| Token | CSS | stored (premultiplied) | `to_srgba_unmultiplied()` | exact? |
|---|---|---|---|---|
| `--shadow-1` | `rgba(32,30,26,0.08)` → a=20 | `[3,2,2,20]` | `[38,26,26,20]` | no |
| `--shadow-2` | a=26 | `[3,3,3,26]` | `[29,29,29,26]` | no |
| `--shadow-3` | a=36 | `[5,4,4,36]` | `[35,28,28,36]` | no |
| `--shadow-pop` | a=51 | `[6,6,5,51]` | `[30,30,25,51]` | no |
| `--shadow-inset` | a=36 | `[5,4,4,36]` | `[35,28,28,36]` | no |
| `--focus-shadow` | `rgba(226,74,43,0.35)` → a=89 | `[79,26,15,89]` | `[226,74,43,89]` | **yes** |

The stored value is what epaint actually renders, so it is the faithful thing to
assert. AC6's exact-round-trip assertion covers **opaque colour and numeric tokens**;
alpha'd tokens assert their stored representation — except `--focus-shadow`, which
round-trips exactly and gets the stronger assertion. CSS alpha → u8 is
`round(a × 255)`: 0.08→20, 0.10→26, 0.14→36, 0.20→51, 0.35→89 (verified).

### `placeholder.rs` migration (AC12)

The spec's pixel-neutrality claim is **confirmed** — all six colours and both
geometry consts are exact matches, so the golden stays byte-identical, **no regen
and no `image-check` spawn** (`.claude/agents/code-writer.md` § Invariants ties
that spawn to *minting or regenerating* a golden; this subtask does neither, and
must not run `UPDATE_SNAPSHOTS=true`).

The **binding** mapping — migrate on *semantic identity*, never numeric coincidence:

| `placeholder.rs` | → token | Note |
|---|---|---|
| `PAPER` | `color::PAPER_1` | |
| `CARD_FILL` | `color::PAPER_2` | **NOT `SURFACE_CARD`** — see below |
| `CARD_STROKE` | `color::GRAPHITE_900` | also equals `--wall`; ink, not a track wall |
| `HAIRLINE` | `color::GRAPHITE_300` | |
| `GRID_LINE` | `color::GRID_LINE` | |
| `GRID_DOT` | `color::GRID_DOT` | |
| `GRID_SPACING` | `spacing::CELL_SM` | also equals `--space-4` (16px); it is a graph-paper pitch |
| `HAIRLINE_STROKE_WIDTH` | `spacing::BW_HAIR` | |
| `CARD_CORNER_RADIUS` | **stays local** | 4 matches no radius token (ramp 0/3/6/10) |
| `GRID_DOT_RADIUS` | **stays local** | 1.0 ≠ `--bg-dots`'s 1.2px dot radius |

Two traps here, both found by scanning every const against every token:

1. **`CARD_FILL` must map to `PAPER_2`, not `SURFACE_CARD`.** The names invite the
   error, and `--surface-card: var(--paper-0)` = `#FBF8F0` ≠ `CARD_FILL`'s
   `#ECE6D6` = `--paper-2`. Taking the semantic alias **changes the pixel and
   breaks AC12**. The scaffold's "card" is not the design system's "card surface".
2. **A value-based migration over-migrates.** `GRID_DOT_RADIUS` (1.0) numerically
   equals `--bw-hair`, and `CARD_CORNER_RADIUS` (4) numerically equals `--space-1`
   — both pixel-neutral but semantic lies. The spec's C.12/C.13 enumeration is
   correct; it is *exactly two* geometry consts.

The local names are **repointed, not deleted** (`const CARD_FILL: Color32 =
tokens::color::PAPER_2;`): the spec says "repoint", it is the smallest diff and so
the safest for AC12, and the scaffold's semantic names carry information the raw
token names do not. The `Stroke::new(1.0, GRID_LINE)` literals inside `draw_grid`
are deliberately untouched — outside C.11/C.12's enumeration.

## Decomposition

> **Each subtask declares its own module.** A `.rs` file that nothing `mod`-declares
> is not compiled — `cargo build` stays green while the code is dead **and its
> `#[cfg(test)]` tests never run**. So the `tokens/mod.rs` skeleton + `lib.rs`
> wiring land in subtask 1, and subtasks 2–4 each append their own `pub mod` line
> with the file they add. Every subtask is independently buildable *and* actually
> exercised by `cargo test` at its own commit.

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | `tokens/mod.rs` skeleton (`pub mod color;`) + `pub mod tokens;` in `lib.rs`; **the shared `#[cfg(test)] mod css` helper set — `value_of` / `assert_f32` / `assert_f32_slice` / `assert_token` / `assert_cubic_bezier` / `var_target` (finding 7 § *Remedy*; the crate's ONE `#[allow(clippy::float_cmp)]` lives on `assert_f32`, which MUST NOT be renamed to `*_eq`)**. **`value_of`'s body is PINNED VERBATIM in finding 7 § *`value_of` — body pinned* — COPY IT, do not re-derive**: it must anchor by **searching** the occurrences (`match_indices`), never `split_once`, or `--bg-grid` binds to comment prose (verified: `split_once` → 126/127); it must stay **arithmetic-free** (`clippy::arithmetic_side_effects` is an explicit workspace `deny` and fires on the `colon + 1` spelling); and rule 1 requires **both** "starts a line" **and** "next non-space char is `:`" (**14** prefix-collision pairs, incl. both AC6 exemplars — see *Test Design* § *The shared value-parser contract*). `colors.css` → 38 base + 18 alias consts; `CAR_COLORS` / `HEAT_RAMP`; `car_color` accessor (finding 4) + tests (AC3–AC7) — parser **cuts at `;`** (finding 6: **28 of 56** colour tokens carry a trailing comment); colour assertions themselves need no `float_cmp` allow | `crates/render/src/tokens/{mod.rs,color.rs}`, `crates/render/src/lib.rs` | — |
| 2 | `spacing.css` → 30 consts (radii as `f32`, finding 2) + `//!` unit banner (px→points; `--space-0` is a bare `0`) + tests | `crates/render/src/tokens/{spacing.rs,mod.rs}` | 1 |
| 3 | `typography.css` → 26 consts (families = primary name; 2 `--role-*` as const refs) + `//!` unit banner (**px→points / `em`→ratio / unitless→ratio / `--fw-*`→`wght` axis**, finding 6) + tests | `crates/render/src/tokens/{typography.rs,mod.rs}` | 1 |
| 4 | `effects.css` → 15 consts: `Shadow` ×5, `InsetShadow` type + const, `[f32;4]` eases, `Duration` ×3 (`ms`), decomposed `--bg-*` + `//!` unit banner + tests | `crates/render/src/tokens/{effects.rs,mod.rs}` | 1 |
| 5 | Complete `tokens/mod.rs`: module doc + **unit-semantics table** (§ *Unit semantics*) + AC1 disposition table (**the ten**) + re-exports + `#[cfg(test)]` CSS **name** helper + AC1/AC2/AC8 count test | `crates/render/src/tokens/mod.rs` | 1,2,3,4 |
| 6 | Repoint `placeholder.rs` per the binding table; **no regen, no `image-check`**; verify golden byte-identical | `crates/render/src/placeholder.rs` | 1,2 |
| 7 | Write `.gitattributes` **verbatim per finding 1 § *The `.gitattributes` body*** (one rule; **no `*.png binary`**) → **then** fetch/place 2 faces + 2 `OFL.txt` at the pinned SHA → **then** `git add` → verify all 4 SHA-256 **and** git-blob hashes **after** staging | `.gitattributes`, `crates/render/fonts/**` | — |
| 8 | `fonts.rs`: 2 byte consts, 7 key consts, `definitions()` from `default()` with explicit `coords` on all 7 (finding 3); wire `pub mod fonts;` in `lib.rs` | `crates/render/src/fonts.rs`, `crates/render/src/lib.rs` | 5,7 |
| 9 | Font tests (AC9/AC10) — **no `miri` gate** (finding 5) | `crates/render/src/fonts.rs` | 8 |
| 10 | `gp-game`: `cc.egui_ctx.set_fonts(gp_render::fonts::definitions())` in the `run_native` closure (AC11) | `crates/game/src/main.rs` | 8 |
| 11 | `gp-render` `license = "(MIT OR Apache-2.0) AND OFL-1.1"`; **no** top-level `NOTICE`/`THIRD-PARTY-LICENSES` (**AC15** — now mandated, no longer droppable) | `crates/render/Cargo.toml` | 7 |
| 12 | Full gate: AC13 `cargo tree`, AC14 five gates, AC12 golden byte-identity re-verify | — | 1–11 |

12 subtasks, all **code** change-type. No instructions/harness edits: `docs/design-system/**` is read-only (spec § Out of scope) and this design doc is the decision record, so no `ai-docs/key-decisions.md` entry is invented.

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping. `M = 12`, all one
change-type (**code**), so the size cap alone forces the boundary: 12 > 10 ⇒
**2 groups is the minimum** achievable under (b)+(f), and 2 ≤ 4 (h). The cut is
placed at the real dependency seam (tokens land green before anything consumes
them) rather than at the arbitrary 10/2 split.

> **Round 2: the plan is unchanged, and the amendment did not force a re-cut.**
> `M` is still 12 — the amendment added no subtask, removed none, changed no
> subtask's change-type, and moved no dependency. AC15 only made subtask 11
> *mandatory* (it was already in Group B); the three review notes land **inside**
> subtasks 3, 5 and 7, all already grouped. Both groups stay 6 + 6.

**Change-type homogeneity (e).** Both groups are **code**. `.gitattributes`,
`Cargo.toml`, and the vendored `.ttf`/`OFL.txt` assets are none of `*.md`,
`.claude/**`, `AGENTS.md`, or `ai-docs/**` — the taxonomy is binary, so they sit on
the code side with the crate they belong to, implemented by the same `code-writer`.
No group mixes change-types.

- **Handoff into Group A:** spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). The orchestrator never implements a group in its own context.
- **Group A** — model `sonnet`, effort `medium` (pinned) via the `code-writer` subagent, 1M-token window — subtasks **1–6** (code change-type: `*.rs`). The complete token module + the pixel-neutral `placeholder.rs` repoint. 6 subtasks (≤ 10). Ends with tokens green and the golden verified byte-identical.
- **Handoff after Group A:** spawn `/context-reset` per `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry). Parent `/task` resumes in Group B with fresh context.
- **Group B** — model `sonnet`, effort `medium` (pinned) via the `code-writer` subagent, 1M-token window — subtasks **7–12** (code change-type: `*.rs`, plus the vendored assets, `.gitattributes`, and `Cargo.toml` that belong to the same code change-type). Fonts, `gp-game` wiring, and the final gate. Terminal group (6 subtasks; within the `1..=10` range).

Both groups route to `subagent_type="code-writer"` with **no inline `model=`/effort
override** — its `model: sonnet` + `effort: medium` are frontmatter-pinned
(verified in `.claude/agents/code-writer.md`), and there is no per-invocation
`effort` parameter. The `design` / `design-review` / `self-review` subagents stay
on Opus regardless.

## Risks

- **Vendored `OFL.txt` silently corrupted by EOL normalisation** (finding 1, *verified to occur*): `core.autocrlf=input` — set **only** in the developer's global `~/.gitconfig`, so the committed bytes depend on **who runs `git add`** — strips 93 bytes from Space Grotesk's `OFL.txt`. Git warns on stderr but does not fail. → Subtask 7 writes `.gitattributes` (body pinned verbatim) so the file **exists on disk** before staging any asset (verified sufficient — it need not be committed first), and verifies all four SHA-256 **and** blob hashes **after** `git add` (not before — before is the trivially-true check that hides the bug).
- **The one subtle file in the task is the one nothing tests.** `.gitattributes` has no compiler, no lint, and no test; a wrong-but-plausible body (`*` , `-text` on the wrong path, `binary` instead of `-text`) passes every gate in AC14 while corrupting the very bytes AC9 pins. → The body is pinned **verbatim** in finding 1 and was verified as written (all four assets `blob == hash-object --no-filters`, `check-attr text: unset`). Subtask 7 copies it; it does not compose it.
- **`.gitattributes` renormalisation churn on existing tracked files** — *disproven, not mitigated.* Round 1 flagged `*.png binary` as possibly touching the committed golden. Verified: **that line is omitted** (finding 1), so the file's only rule is scoped to `crates/render/fonts/**`, which is **currently untracked and non-existent** — it can touch no tracked file, and there is nothing to renormalise. → Subtask 7 still confirms `git status` is clean after writing the file; a non-empty diff on any pre-existing path means the pattern was mistyped.
- **A line-reading `px`-strip test parser breaks on the bulk of the CSS** (finding 6, counts `grep`-verified): **54 of 127** tokens carry a trailing `/* … */` comment (**28 in `colors.css`** — so **subtask 1 is affected too**, which round 1 missed), and **15 of the 53** numerics are bare / `em` / `ms`. `--space-0: 0` is the 1st declaration in `spacing.css`, so a naive strip panics immediately. → Subtasks **1–4** cut the value at the `;` and dispatch on suffix; an unhandled case is a **named test failure**, never an `unwrap`. **Validating on `--accent` proves nothing** — AC6's exemplar is one of the few colour tokens with no trailing comment.
- **The parser binds to the wrong occurrence, and both failure modes are silent-ish** (round 4, notes 1 + 2; both verified by running). Two distinct traps: **(i) a `split_once(name)` anchor scores 126/127** — `--bg-grid`'s first occurrence is comment prose at `effects.css:27`, three lines above its declaration, so the parser dies on a token it should read (this one at least fails loudly, and it is what review's own proposed body did). **(ii) a first-match parser with no `:` clause is wrong-but-PASSING** — **14** prefix-collision pairs across **9** short tokens (incl. **both AC6 exemplars**, `--cell` and `--accent`) are today safe **only because all 14 are short-first in source order**; a CSS reorder or a new shorter sibling silently binds `--cell` to `--cell-sm`'s value. → `value_of`'s body is **pinned verbatim** (finding 7 § *`value_of` — body pinned*): it **searches** the occurrences and requires **both** rule-1 clauses. It is copied, not composed. Verified 127/127 against the real CSS.
- **The parser reintroduces index arithmetic and hard-fails the build** (round 4, note 1): `clippy::arithmetic_side_effects` is an **explicit workspace `deny`** (`Cargo.toml:55`, its own line — easy to miss when scanning for `pedantic`/`nursery` group denies) and **fires** on both natural spellings of a cut-at-`;` reader (`map_or(0, |p| p + 1)`, `&rest[colon + 1..semi]` — both verified erroring). The `sonnet`/`medium` implementor's most likely instinct here is an `#[allow]`. → The pinned body is arithmetic-free by construction and therefore needs **no** `#[allow]`; the lint is recorded beside `map_unwrap_or` and `option_if_let_else` in finding 7 so it is not rediscovered mid-subtask.
- **The inventory test does not compile** (finding 7, *every row re-run this round*): `clippy::float_cmp` is pedantic → `deny`, and AC14 lints `--all-targets`, so round 1's `assert_eq!(parsed, CELL)` is a hard error. The lint fires **immediately and unconditionally** on every `assert_eq!` between floats — including `assert_eq!(CELL, 24.0)`, and including comparisons against `0.0`, because `assert_eq!` binds its operands opaquely and the bare-`==` exemptions never reach it. *(Round 2 claimed the opposite — that clippy const-folds equal operands and stays silent until the value comes from `include_str!` — and marked it "verified". It was **false and never run**; the round-2 probe only ever exercised the remedy. The lint is **less** treacherous than round 2 described, not more: you cannot fail to notice it.)* → **One** `#[allow(clippy::float_cmp, reason = …)]` on a single `assert_f32` comparator in `tokens/mod.rs`, which every parser delegates to **by construction**; **verified load-bearing by negative control** — stripping it yields exactly one error. *(Round 4, note 4: that single error proves the attribute is load-bearing and that no other **lint-visible** comparison exists — it does **not** prove exhaustive funnelling, since a `*_eq`-named comparator would be invisible to it. The funnel rests on the helper table, not the error count.)* **Do not "fix" this with an epsilon** — `==` is the intended exact contract (both spellings are correctly rounded to bit-identical `f32`), and a tolerance would mask the drift AC8 exists to catch.
- **The `#[allow]` silently goes inert if the comparator is renamed** (finding 7, *verified*): `clippy::float_cmp` skips any fn named `eq`/`ne`/`is_nan`, or starting `eq_`, or **ending `_eq`** — so the obvious name `assert_f32_eq` suppresses the lint *by name*, making the attribute dead and the suppression accidental. A later rename to `assert_f32_equal` re-arms the hard error with no visible cause. → The comparator is named **`assert_f32`** (outside the heuristic) so the `#[allow]` is load-bearing; its rustdoc records the constraint; the negative control in finding 7 is the executable proof.
- **A blanket module-level `#[allow(clippy::float_cmp)]` improvised mid-subtask.** The `sonnet`/`medium` implementor meets this lint as a hard `error` in the middle of subtask 2, and the cheapest escape is a module blanket — which AGENTS.md § Code Style forbids without justification, and which would blind the module to *real* float bugs. → The remedy is **fully resolved in this document**, not delegated: the helper set, its exact placement (`tokens/mod.rs` `#[cfg(test)] mod css`), the attribute body, the naming constraint, **and — as of round 4 — `value_of`'s body** are all specified and verified. *(Round 3 left `value_of` as the one unpinned helper, so "copies, does not invent" held for the comparator and quietly failed for the parser — the exact gap where the `sonnet`/`medium` implementor would have met `arithmetic_side_effects` alone. It is closed.)* The implementor copies; it does not invent.
- **A bare `f32` token read in the wrong unit** (finding 6): `FW_BOLD = 700.0` under a "logical points" banner reads as a 700 pt length; `LS_DISPLAY = -0.02` is −1.12 pt at display size, not −0.02 pt. The type system cannot catch this — both genuinely are `f32`. → Per-group `//!` unit banners (subtasks 2–4) + the unit table in `tokens/mod.rs` (subtask 5) are **deliverables**, and #13–#16 are the consumers that would misread them.
- **AC12 broken by a name-driven mis-map** (`CARD_FILL` → `SURFACE_CARD` = a real `#ECE6D6`→`#FBF8F0` pixel change). → The binding mapping table is normative; subtask 6 verifies `git diff --stat` shows the PNG untouched and `golden_guard` green.
- **`image-check` spawned / golden re-minted** despite AC12. → Subtask 6 states no-regen explicitly; `code-writer`'s invariant only fires on mint/regen, which this task never does.
- **Space Grotesk registered at Light** (finding 3): omitting `coords` on the 400 instance yields wght 300, silently. → All 7 instances carry explicit `coords`; AC9's axis test plus an AC10 per-instance `coords` assertion cover it.
- **`VariationCoords` import guessed** → `E0433`. It is **not** at `egui`'s top level, nor in the curated `egui::text` module — only `egui::epaint::text::VariationCoords` resolves (verified by reading all three re-export sites *and* compiling it in probe D).
- **`::empty()` instead of `::default()`** drops emoji/fallback coverage silently. → Verified `empty()` → `font_data.len() == 0`; AC10 asserts every `builtin_font_names()` entry survives.
- **A reviewer "fixes" `car_color` into a `const fn`** → E0658 (finding 4). → The fn's doc records the const-ineligibility and its cause.
- **`--radius-pill` gets "corrected" back to a hardcoded `255`** (finding 2). No longer an open decision — the product owner **ratified** `f32 = 999.0` and the spec now carries a do-not-revert note. The risk is now the *reverse* of round 1's: a future reader re-derives the `u8` objection from `CornerRadius`'s field types and "fixes" an exact port into a lossy one. → The const's doc records that `From<f32>` saturates at the use site, and subtask 2's test pins `CornerRadius::from(RADIUS_PILL) == CornerRadius::same(255)` so the reasoning is executable, not just prose.
- **An undeclared module compiles green but never runs its tests.** A `tokens/*.rs` file with no `pub mod` line is silently excluded from the crate — `cargo build` and `cargo test` both pass while the consts are dead and their AC3–AC7 assertions never execute. → Each subtask adds its own `pub mod` line with its file (see the Decomposition callout); subtask 5's AC8 count test is the backstop, since a missing module makes its inventory unreachable at compile time.
- **AC8's inventory is hand-maintained on the Rust side** — the test catches CSS-side additions (its stated purpose) but cannot prove a listed token has a real const. → Value-checking against the parsed CSS closes the gap for **117 of 127**: 56 colours (38 hex + 18 `var()` targets), 30 spacing, **25** typography (20 numerics + 3 family names + **2 `--role-*` `var()` targets**), 3 `--dur-*`, **3 `--ease-*`**. The residual **10** are structured values asserted against hand-written expectations — 5 `--shadow-*`, `--shadow-inset`, `--focus-shadow`, 2 `--bg-*`, `--text-eyebrow-transform` — and rest on review. *(Round 1 said "94 of 127"; finding 6 made subtask 3 value-check all 20 typography numerics → 112; round 3's hoisted comparator then pulled the 3 `--ease-*` in via `assert_cubic_bezier` and the 2 `--role-*` via `var_target` → **117**. 117 + 10 = 127. **Round 4 does not move this count**: note 3 *adds* a Rust-side `assert_f32` to each `--role-*`, but those 2 tokens are already counted here via their `var_target` CSS check — the second assertion deepens the check, it does not widen coverage.)*
- **No performance/panic/unsafe surface.** The whole change is `const` data plus one `BTreeMap`-building function called once at startup. No `unsafe`, no arithmetic, no new panic path — `car_color` is total by construction and `definitions()` cannot fail.

## Test Design

**Subtask 1 — `tokens/color.rs`** (`#[cfg(test)] mod tests`)
- Entry points: the 56 consts, `CAR_COLORS`, `HEAT_RAMP`, `car_color`.
- `const CSS: &str = include_str!("../../../../docs/design-system/tokens/colors.css");` — path arithmetic verified (`crates/render/src/tokens/` + `../../../..` = repo root).
- **Value-checked inventory** (AC1/AC6/AC8): `const BASE: [(&str, Color32); 38]`. Parse every `#RRGGBB` token from `CSS`; assert the name set equals `BASE`'s **and** each parsed hex equals the const. Drift in *either* direction fails. Gives AC6's `accent = #E24A2B` for free.
- **Parser: cut at the `;`** (finding 6). **28 of the 56** colour declarations carry a trailing `/* … */`; reading to end-of-line yields `#F5F1E6;   /* base graph-paper cream */`. Note `--accent` — AC6's exemplar — is *not* one of them, so a parser validated only against AC6's named exemplar passes while broken. No `float_cmp` allow here (integer comparison).
- AC5: `const ALIASES: [(&str, Color32, Color32); 18]` — assert each alias const == its base const, and that the parsed `var(--x)` target name matches. The alias parse cuts at `;` too (`--surface-ink: var(--graphite-900);   /* inverse / dark panels */`).
- AC3: `CAR_COLORS.len() == 6`; `CAR_COLORS[0] == ACCENT`; `car_color(0) == Some(ACCENT)`; **`car_color(6) == None`** and `car_color(usize::MAX) == None` (totality — the edge case that matters).
- AC4: `HEAT_RAMP == [HEAT_0, HEAT_1, HEAT_2, HEAT_3]`, len 4, ordered slow→fast `#2E6FB5, #17999B, #E8B23A, #E24A2B`.
- AC6 cross-file identity: `CAR_COLORS[0] == ACCENT && ACCENT == HEAT_RAMP[3]`; alias identity `SURFACE_PAGE == PAPER_1`.

**The shared value-parser contract — ONE copy, in `tokens/mod.rs` (subtasks 1–4)**
— findings 6 + 7. A `#[cfg(test)] pub(crate) mod css` beside the existing
`token_names` helper holds the whole set (`value_of`, `assert_f32`,
`assert_f32_slice`, `assert_token`, `assert_cubic_bezier`, `var_target` — see
finding 7 § *Remedy*). It **lands in subtask 1** with the `tokens/mod.rs` skeleton;
subtasks 2–4 already depend on 1 and consume it. **`assert_f32` is the crate's only
float-comparison site and carries its only `#[allow]`**; every other helper
delegates — **by construction**, which is what makes `assert_f32` the sole
comparison site; the negative control's single error does **not** by itself prove
that (finding 7 § *What the negative control does and does not prove*). Subtask 1's
colour assertions need no `allow` (`Color32` is four `u8`s ⇒ integer comparison) but
**do** obey rules 1 and 5. **Verified against the real CSS: clippy `--all-targets --
-D warnings` clean, `value_of` resolves 127/127 tokens (both AC6 exemplars against
their longer siblings, and `--bg-grid`/`--bg-dots` past the comment prose), and
stripping the `#[allow]` yields exactly one error.**

**`value_of`'s body is pinned verbatim** in finding 7 § *`value_of` — body pinned*.
**Copy it.** Two properties of it are load-bearing and are *not* obvious from the
rules below, which is why it is pinned rather than described:

- **It anchors by SEARCHING the occurrences (`match_indices`), never `split_once`.**
  A `split_once` anchor scores **126/127** — it binds `--bg-grid` to the comment
  prose at `effects.css:27` and dies (verified by running).
- **It is arithmetic-free.** `clippy::arithmetic_side_effects` is an **explicit
  workspace `deny`** (`Cargo.toml:55`) and **fires** on both natural spellings of
  this parser (`map_or(0, |p| p + 1)`, `&rest[colon + 1..semi]` — both verified).
  Arithmetic-free ⇒ **no `#[allow]`**. Do not reintroduce index math.

1. Take the text **between `:` and the terminating `;`** — *never* to end-of-line.
   **54 of 127 tokens carry a trailing comment, 28 of them in `colors.css`**;
   `--bg-grid`/`--bg-dots` span multiple lines. **Applies to subtask 1 as well.**
   The declaration matches only where **both** hold — a name that satisfies one but
   not the other must be **skipped and the search continued**, not rejected:
   - **(a) it starts a line** — `--cell` also occurs as `var(--cell)` inside
     `effects.css`'s comment prose, and that comment contains `;` characters that
     would otherwise terminate the wrong value.
   - **(b) the next non-space char is `:`** (`after.trim_start().starts_with(':')`).
     **Without this clause the parser is wrong-but-passing.** There are **14**
     (short, long) token pairs across **9** short tokens where one token is a strict
     prefix of a longer token that *also* starts a line — **including both AC6
     exemplars**: `--cell` ⊂ `--cell-sm`/`--cell-lg` and `--accent` ⊂
     `--accent-hover`/`-press`/`-tint`; also `--space-1` ⊂ `--space-10`/`-12`/`-16`,
     `--space-2` ⊂ `--space-20`, `--grid-line` ⊂ `--grid-line-major`, `--text-link`
     ⊂ `--text-link-hover`, and `--ok`/`--warn`/`--danger` ⊂ their `-tint`.
     **All 14 happen to be short-first in source order** (verified by indexing every
     declaration), so a first-match parser without this clause survives **by
     source-ordering luck alone** — the same "guarantee resting on an undeclared
     coincidence" shape as the `_eq` trap, and it breaks the day someone reorders a
     CSS file or adds a shorter sibling. Verified with the clause, against the real
     CSS: `--cell`→`24px` vs `--cell-sm`→`16px`, `--space-1`→`4px` vs
     `--space-10`→`40px`.
2. Trim, then **dispatch on suffix**: `px` → `f32` points · `em` → `f32` ratio ·
   `ms` → `Duration::from_millis` · bare numeric → `f32` ratio/weight per group.
   For the `f32` helper the three float cases parse **identically** (the unit is
   semantics, not syntax), so the dispatch is
   `strip_suffix("px").or_else(|| strip_suffix("em")).unwrap_or(raw)` — which also
   sidesteps the `clippy::option_if_let_else` **nursery** error the natural
   `if let`/`else if let` chain trips (verified).
3. A **bare `0`** is legal and is **not** an error (`--space-0: 0`,
   `--ls-normal: 0`) — while `--radius-0: 0px` *does* carry the suffix. The source
   is inconsistent; the parser must accept both.
4. Negative values parse (`--ls-display: -0.02em`).
5. An unhandled case ⇒ `panic!("token {name}: unhandled unit in {value:?}")`
   *from the assertion*, naming the token — not `Option::unwrap` on a `None`.
   `ms` reaching the `f32` helper falls out here for free: it strips no suffix and
   fails `parse()`, producing the named panic.

**Subtask 2 — `tokens/spacing.rs`**
- Value-checked inventory over `spacing.css`'s 30 tokens per the contract above: parse `--cell: 24px` → assert `CELL == 24.0`. Covers AC6's `--cell` = 24 and `--bw-heavy` = 3.
- **`--space-0: 0` is the parser's first and hardest case** — it is declaration #1 in the file and the only bare `0` among 30 otherwise-`px` tokens. Assert `SPACE_0 == 0.0` parsed *from the CSS*, so a regression to a naive `strip_suffix("px")` fails here loudly.
- `CornerRadius::from(RADIUS_PILL) == CornerRadius::same(255)` — pins finding 2's (ratified) saturation as a tested contract, not a comment. Needs **no** `float_cmp` allow: `CornerRadius`'s fields are `u8` (**verified clippy-clean and passing** against real `egui` 0.35). **No separate `assert_eq!(RADIUS_PILL, 999.0)`** — round 2 proposed it, but it both *fires* `float_cmp` and is redundant: `assert_token(CSS, "--radius-pill", RADIUS_PILL)` in the inventory above already value-checks it against the CSS, which is strictly stronger than restating the literal.

**Subtask 3 — `tokens/typography.rs`** — inventory over the 9 `--fs-*` (px), 4 `--fw-*` (bare), 3 `--lh-*` (bare), 4 `--ls-*` (`em`, incl. the bare `0` of `--ls-normal` and the negative `-0.02em` of `--ls-display`); family consts assert the primary name. This file exercises **every** branch of the parser contract, so it is where a unit regression surfaces first.
- **AC5 `--role-*` aliases — BOTH a name check AND the Rust-side identity. Two lines each, not one.** These are `var()` aliases with **no numeric to parse**, so `assert_token` cannot serve them. Assert both sides:
  - **CSS side:** `assert_eq!(var_target(CSS, "--role-display-size"), "--fs-display")`, likewise `--role-value-size` → `--fs-h2`.
  - **Rust side:** `assert_f32("ROLE_DISPLAY_SIZE", ROLE_DISPLAY_SIZE, FS_DISPLAY)`, likewise `assert_f32("ROLE_VALUE_SIZE", ROLE_VALUE_SIZE, FS_H2)`. Routed through the existing comparator ⇒ **no new `#[allow]`** (this is exactly what hoisting the comparator bought).
  - **Why both — round 3 dropped the Rust side and was WRONG (round 4, note 3).** Round 3's argument was that `pub const ROLE_DISPLAY_SIZE: f32 = FS_DISPLAY;` is a const *reference*, so the identity is compile-checked and asserting it is a tautology. **It is a tautology only *given correct code*** — which is true of every test ever written. The identity is precisely the guard against the const being pointed at the **wrong base**, and a const reference to the wrong base **compiles fine**. **Proven, not argued:** with `ROLE_DISPLAY_SIZE` mis-pointed at `FS_H2`, the CSS-side check **PASSES** (it never touches the Rust const) while the Rust-side check **FAILS** — `left: 30.0, right: 56.0`. Dropping it would also contradict subtask 1, which keeps **both** checks for the 18 colour aliases on identical logic. (Both sides verified against the real `typography.css`: `--fs-display: 56px`, `--fs-h2: 30px`.)
- **No separate "unit-semantics" arithmetic assertion.** An earlier draft of this
  revision proposed pinning `FS_DISPLAY * LH_TIGHT == 58.8`. **Dropped — it is
  wrong twice over**, and is recorded here so it is not re-proposed: (1) it
  **fails** — the product is `58.799995`, off by `3.8e-06` ≈ 32 × `f32::EPSILON`
  (measured); (2) even repaired with a tolerance it asserts `56 × 1.05 ≈ 58.8`,
  which tests the FPU, not this crate. The **inventory test already catches the
  regression it was aiming at**: if someone "converts `--ls-*` to points",
  `LS_DISPLAY` becomes `-1.12` while the CSS still says `-0.02em`, and
  `assert_token("--ls-display", LS_DISPLAY)` fails by construction. The unit
  *doc* protects consumers (#13–#16); the unit *value* is protected by the CSS
  comparison. Nothing else is needed.

**Subtask 4 — `tokens/effects.rs`**
- `SHADOW_0 == Shadow::NONE`; `SHADOW_1 == Shadow { offset: [0,1], blur: 2, spread: 0, .. }`; `FOCUS_SHADOW.spread == 3`.
- Alpha (per the table above): assert **stored** bytes, e.g. `SHADOW_1.color.to_array() == [3,2,2,20]`; and the one exact case `FOCUS_SHADOW.color.to_srgba_unmultiplied() == [226,74,43,89]`.
- `DUR_FAST == Duration::from_millis(120)` (`Duration` is integer-typed — no `float_cmp`); `value_of(CSS, "--dur-fast") == "120ms"` pins the CSS side.
- **Eases — value-checked from the CSS, not hand-written.** `assert_cubic_bezier(CSS, "--ease-standard", EASE_STANDARD)` parses `cubic-bezier(0.2, 0, 0.1, 1)` → `[0.2, 0.0, 0.1, 1.0]` and delegates element-wise to `assert_f32` (naming the index on failure). Round 2's bare `EASE_STANDARD == [0.2, 0.0, 0.1, 1.0]` had no home and *fires* `float_cmp` — as an **array** comparison, a different message but the same lint (verified). Note the CSS writes `0`/`1`, not `0.0`/`1.0`; the parser handles both (verified against the real `effects.css`).
- **`--bg-*` decomposition** — `assert_f32("BG_DOTS_RADIUS", BG_DOTS_RADIUS, 1.2)`, likewise `1.4` and `--bg-grid`'s `1.0`. Hand-written expectations (the numbers live inside a multi-line gradient *recipe*, not a `name: value;` declaration) but routed through the one comparator; additionally pinned against the CSS text via `value_of(CSS, "--bg-dots").contains("1.2px")`.
- `BG_GRID_COLOR == color::GRID_LINE`, `BG_DOTS_COLOR == color::GRID_DOT` (the `var()` relationship, compile-checked).

**Subtask 5 — `tokens/mod.rs`**
- Helper: `#[cfg(test)] fn token_names(css: &str) -> Vec<&str>` — trim line; `starts_with("--") && contains(':')` → name up to `':'`. **Validated to reproduce the AC1 denominator exactly**: 56/30/26/15 = 127, all unique, and no continuation line of the multi-line `--bg-grid`/`--bg-dots` values starts with `--` (so they cannot inflate the count).
- AC8: assert per-file counts 56/30/26/15 and total **127**; assert `PORTED` ∪ `DEVIATIONS` == parsed names, and the two are disjoint. A token added to any CSS file later fails this test.

**Subtask 6 — `placeholder.rs`** — existing `tessellation_smoke` + `golden_guard` must pass **unchanged**; `git diff --stat crates/render/tests/snapshots/placeholder.png` must be empty. No new test; the golden *is* the test.

**Subtasks 8/9 — `fonts.rs`**
- AC9: for each face, `FontData::from_static(BYTES).variation_axes()` reports a `wght` axis; assert `axis.range` covers every registered weight (SG 400–700 ⊂ 300–700; JBM 400–700 ⊂ 100–800). Non-parsing bytes would return an empty vec — so a non-empty axis list *is* the parse assertion.
- AC10: `definitions().font_data.len() == 11`; all 7 keys present; **every `FontDefinitions::builtin_font_names()` entry still in `font_data`** (the exact, self-updating assertion for "egui's defaults survived"); `Proportional` and `Monospace` non-empty with our face first; each of the 7 carries a non-default `coords` (finding 3).
- No fixtures needed — the faces are `include_bytes!` consts. **No `miri` gate** (finding 5).

**Subtask 12 — gates** — `cargo build`; `cargo test`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --check`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`; `cargo tree -p gp-render --edges no-dev | grep -iE "eframe|winit|wgpu"` → empty (AC13; verified empty at baseline).
- **What `-D warnings` does and does not mask** (round 4, note 4 — round 3's "aborts on first failure" was loose). **Within a crate, every lint error is reported** — verified: 3 bare float `==` sites yield **3** errors, not 1, so a clippy run's error list for `gp-render` is complete for that pass and its *count* is informative. What **does** abort is the **cross-crate** build: once `gp-render` fails to compile, its dependents (`gp-game`) are never linted, so their errors surface only after `gp-render` is clean. Expect a second clippy pass to reveal `gp-game`-side findings — not `gp-render`-side ones.

## Open questions

**None.** All four round-1 questions are closed, and the spec's own Open questions
section is now empty:

1. ~~Radius token type — `f32` vs `u8`~~ — **ratified** by the product owner
   (Path A). `--radius-pill` is a normal `f32` token; the exclusion row is gone and
   the spec carries a do-not-revert note. Subtask 2 stands as designed (finding 2).
2. ~~`--shadow-inset` → `InsetShadow`~~ — was an **FYI, not a question** (the spec
   delegated the call). Design-review confirmed the type **earns its keep**:
   `epaint::Shadow` has no inset flag, so a `Shadow`-typed const would silently
   render an *outer* drop shadow. Closed.
3. ~~`gp-render`'s `license` field~~ — **closed by AC15**, which mandates
   `license = "(MIT OR Apache-2.0) AND OFL-1.1"`. Subtask 11 is no longer droppable.
4. ~~Top-level `NOTICE` / `THIRD-PARTY-LICENSES`~~ — **closed by AC15**: the owner
   chose "follow the `epaint_default_fonts` precedent fully" — per-face `OFL.txt` +
   the `license` field + **no** top-level aggregation.

Nothing in this revision requires a product-owner decision. The two new findings
(6, 7) are internal test-design corrections with verified remedies, and they change
no AC, no subtask boundary, and no group.

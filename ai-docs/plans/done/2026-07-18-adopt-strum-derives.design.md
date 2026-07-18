# Design: Adopt strum derives and enum_map for enum boilerplate

**Issue:** none — user opted out of a tracking issue (free-text de-boilerplate task).
**Spec:** `ai-docs/plans/2026-07-18-adopt-strum-derives.spec.md`
**Date:** 2026-07-18

## Approach

A mechanical, behaviour-preserving de-boilerplate. Three hand-written `const ALL`
arrays and one hand-written kebab-string `match` are replaced by strum derives,
and the one enum-keyed `HashMap` becomes an array-backed `EnumMap`. The observable
variant set, iteration order, and icon cache keys are byte-for-byte unchanged.
A fourth, **documentation-only** scope item (spec § C, added by the 2026-07-18
amendment) codifies the enum-keyed-collection preference as a **preventive** house
rule in `ai-docs/code-style.md` — an instructions/harness prose edit, not code
(see Key decision 8 + subtask 3).

All design forks the spec left open were resolved empirically against strum 0.28.0 /
enum-map 2.7.3 in a throwaway crate that mirrors the real enum attribute stacks and
call-site shapes under the workspace's pedantic+nursery `-D warnings` posture
(`/tmp/.../scratchpad/strumcheck`). The measured results drive the decisions below.

### Key decisions

1. **Variant-enumeration derive → `strum::EnumIter` on all three enums (`Side`,
   `Action`, `Icon`).** Every current `::ALL` site is an *iteration* — three
   by-value `for x in ::ALL` loops and one `Action::ALL.into_iter().filter().collect()`.
   `EnumIter` gives `Self::iter()` yielding owned values in **declaration order**, so
   each site swaps `::ALL` → `::iter()` with no `&`/`.copied()` adapter.
   `iter()` is a method on the `strum::IntoEnumIterator` trait, so every module that
   calls it needs `use strum::IntoEnumIterator;` in scope
   `[measured: build without the import → E0599 "no ... function ... named iter ... trait IntoEnumIterator ... is implemented but not in scope"]`.
   A `#[cfg(test)] mod tests { use super::* }` **inherits** the parent module's
   import, so one module-level `use` covers production + tests when production also
   iterates `[measured: probe tests call Action::iter()/Side::iter()/Icon::iter() with only a parent-scope import + `use super::*` → 6 passed]`.
   - *Rejected: `VariantArray` (`Self::VARIANTS: &'static [Self]`).* Yields `&[Self]`,
     forcing `for &x in ...` / `.iter().copied()` at every by-value site and a
     `.copied()` in the `Action` filter chain — strictly more adapter noise for no
     gain. Both derives are declaration-order-deterministic, so AC4 is satisfied by
     either; ergonomics decide.

2. **`Icon` cardinality → `strum::EnumCount` (`Icon::COUNT`).** Of the two
   `Icon::ALL.len()` sites, the `HashMap::with_capacity` one **disappears** (EnumMap
   is fixed-size, no capacity hint), leaving only the test assertion
   `Icon::ALL.len() == 5` → `Icon::COUNT == 5` (AC9). `Icon::COUNT` is a compile-time
   `usize` const and reads as the honest cardinality invariant.
   `COUNT` is an associated const of the `strum::EnumCount` trait, so accessing
   `Icon::COUNT` needs `use strum::EnumCount;` in scope
   `[measured: probe `Icon::COUNT == 5` compiles+passes with the import]`.
   - *Weighed alternative (YAGNI-minimal): drop `EnumCount`, use `Icon::iter().count() == 5`*
     (no extra derive, `EnumIter` already present). Chosen `EnumCount` anyway: it is a
     spec-named in-scope candidate, is verified zero-cost + lint-clean, and expresses a
     const cardinality a runtime `.count()` cannot. Recorded so the trade-off is auditable.

3. **`Icon::name()` string → `strum::IntoStaticStr`, and `name()` is REMOVED (clean
   break).** `IntoStaticStr` generates `impl From<Icon> for &'static str` returning
   string *literals* — `&'static str`, **no heap allocation** (contrast `Display`/
   `ToString`, which allocate a `String`). Keys stay byte-identical via
   `#[strum(serialize_all = "kebab-case")]` on the enum plus explicit
   `#[strum(serialize = "grid-3x3")]` / `#[strum(serialize = "zoom-in")]` on the two
   digit/compound variants (kebab-case alone does not split a digit boundary); Play/
   Pause/Settings fall out of kebab-case correctly
   `[measured: probe asserts play,pause,grid-3x3,zoom-in,settings all byte-exact → passed]`.
   Both call sites migrate to `<&'static str>::from(icon)` (`From` is in the prelude —
   **no** runtime strum import needed for the conversion). `name()` is deleted per the
   AGENTS.md API-Stability AXIOM (no compat shims); it also *could not* remain a `const
   fn` wrapper because strum's generated `From` impl is not `const`. No intra-doc link
   targets `Icon::name` `[measured: rg -Un '\[`Icon::name`\]' → no match]`, so removal
   breaks no doc link.

4. **`IconSet(HashMap<Icon, TextureHandle>)` → `IconSet(EnumMap<Icon, TextureHandle>)`,
   built with the **`enum_map!` explicit-key macro** (compile-time exhaustiveness +
   named keys), NOT a positional `from_array` and NOT `FromIterator`.** This is the
   load-bearing correction to the spec's suggested "bake into an intermediate then
   materialize":
   - enum_map's `impl FromIterator<(K,V)> for EnumMap<K,V>` is bounded `where Self: Default`
     (it starts from an all-`V::default()` map and `extend`s)
     `[measured: enum-map-2.7.3/src/enum_map_impls.rs:38-48]`. `Self: Default` requires
     **`V: Default`**.
   - `egui::TextureHandle` has **no `Default`** — it holds `Arc<RwLock<TextureManager>>`
     + `TextureId` with a custom `Clone`(retain)/`Drop`(free)
     `[measured: epaint-0.35.0/src/texture_handle.rs:20-40, no `impl Default`]`.
   - Therefore `.collect::<Result<EnumMap<Icon, TextureHandle>, IconError>>()` **does not
     compile** `[measured: probe with a non-Default `Tex` stand-in → E0277 "the trait bound `Tex: Default` is not satisfied ... required for `EnumMap<Icon, Tex>` to implement `Default`"]`.
     (An earlier `String`-valued probe falsely greened because `String: Default` — the
     stand-in must be non-`Default` to be truthful.)
   - **`enum_map! { Icon::Play => bake(Icon::Play)?, … }`** — the explicit-key macro
     expands to a per-slot **exhaustiveness-checked `match`** on the key type (`enum-map-2.7.3/src/lib.rs:195`
     — `value = match (&eq.guard).get_key() { $($t)* }`, **no closure wrapping the arms**),
     then `EnumMap::from_array` internally (`:203`), imposing **no `Default` bound**. Build with a **single DRY `bake` closure** (bake/`svg_bytes`/name/size
     written ONCE, capturing `ctx`) called inside each arm; `?` propagates the first
     `IconError` straight out of the arm (arms are not closure-wrapped):
     ```text
     let bake = |icon: Icon|
         bake_texture(ctx, <&'static str>::from(icon), icon.svg_bytes(), ICON_LOGICAL_SIZE_PX);
     enum_map! {
         Icon::Play     => bake(Icon::Play)?,
         Icon::Pause    => bake(Icon::Pause)?,
         Icon::Grid3x3  => bake(Icon::Grid3x3)?,
         Icon::ZoomIn   => bake(Icon::ZoomIn)?,
         Icon::Settings => bake(Icon::Settings)?,
     }
     ```
     Compiles, **strict-clippy clean** (pedantic+nursery `-D warnings`, all-targets, **and
     `undocumented_unsafe_blocks = "deny"`** — the macro's internal `unsafe`/`MaybeUninit` is
     external-macro-generated, so that lint does not fire), fallible via `?`, **panic-free**
     (no `.expect`/`unwrap`), and each non-`Default`/non-`Copy` value is created in-arm and
     moved exactly once — preserving icons.rs's "zero production panics" invariant
     `[measured: enum_map! ?-inside-arms with non-Default `Tex` → clippy -D warnings clean incl. undocumented_unsafe_blocks; per-icon-mapping test passed]`.
     **Values MUST be computed in-arm, not hoisted:** a `let play = bake(..)?; … enum_map!{ Icon::Play => play, … }`
     form **fails to compile** for non-`Copy` `TextureHandle` because the macro evaluates arms
     in a per-slot loop `[measured: hoisted-let probe → E0382 "use of moved value: `play` … value moved here, in previous iteration of loop"]`.
   - **Rejected: `EnumMap::from_fn` (the product owner's DRY suggestion) and any from_fn
     transpose.** `from_fn<F: FnMut(K) -> V>` is **infallible** (`enum-map-2.7.3/src/lib.rs:298`,
     body `enum_map!{k => cb(k)}`) — the closure must return `V`, so `?` cannot live inside
     it — and enum-map 2.7.3 ships **no** fallible constructor (`try_from_fn`/`try_*` — grep
     empty) `[measured: grep 'try_from_fn\|try_from_array\|try_new\|fn try_' enum-map-2.7.3/src → empty]`.
     The only fallibility-recovering route, `from_fn(|i| bake(i))` → `EnumMap<Icon, Result<V, E>>`
     then transpose to `Result<EnumMap<Icon, V>, E>`, routes its final
     `collect::<Result<EnumMap<…>,_>>()` back through `EnumMap: FromIterator` → `Self: Default`
     → `V: Default` and **fails to compile on the very same wall**
     `[measured: from_fn+transpose with non-Default `Tex` → E0277 "the trait bound `Tex: Default` is not satisfied … required for `EnumMap<Icon, Tex>` to implement `FromIterator<(Icon, Tex)>` … required for `Result<EnumMap<Icon, Tex>, BakeErr>` to implement `FromIterator<Result<(Icon, Tex), BakeErr>>`"]`.
     The only Default-avoiding from_fn variant — bake into `EnumMap<Icon, Option<V>>` then
     `.map(|_, o| o.unwrap())` — reintroduces a `.unwrap()` **panic** and an Option-valued
     backing store, disqualified by icons.rs's zero-panic invariant **and** AC6's non-Option
     `EnumMap<Icon, TextureHandle>` value-type requirement. So from_fn buys **no** DRY the
     single `bake` closure does not already deliver, at the cost of a panic + AC6 violation.
   - **Compile-time exhaustiveness + named keys (the positional footgun is gone).** Unlike a
     positional `from_array([…])` — whose `[V; LENGTH]` gives only a **count** check, so a
     duplicated-one/omitted-another array still compiles — the `enum_map!` match makes
     **omitting a variant a compile error** `[measured: dropped one arm → E0004 "non-exhaustive patterns: `Icon::Settings` not covered"]`,
     and keys are **named**, so a positional swap is structurally impossible. **Do NOT add a
     `_ =>` wildcard arm:** it compiles but silently **defeats** exhaustiveness
     `[measured: `_ =>` arm → builds clean]` — every variant must be listed by name. The one
     residual class the compiler does *not* catch is a mistyped arm *argument*
     (`Icon::Play => bake(Icon::Pause)?`); the AC6 per-icon `.name()` test still guards that
     (`TextureHandle::name()` exists, `epaint-0.35.0/src/texture_handle.rs:118`) and is the
     AC6 acceptance artifact — **kept as defense-in-depth**, its role narrowed from
     positional-swap (now compile-prevented) to arm-argument correctness.

5. **`IconSet::get` → `&TextureHandle` (total).** `EnumMap` indexing is infallible, so
   `get` drops the `Option`: `pub fn get(&self, icon: Icon) -> &TextureHandle { &self.0[icon] }`.
   The sole call site is its own module test (`set.get(icon).expect(...)` → `set.get(icon)`);
   no external consumer exists `[measured: rg -Un '\bIconSet\b' → only crates/render/src/icons.rs]`.

6. **Dep wiring.** `strum = { version = "0.28", features = ["derive"] }` in
   `[workspace.dependencies]`, referenced `strum = { workspace = true }` by **both**
   gp-core and gp-render (2 consumers → workspace single-sourcing, mirroring the
   existing `thiserror` precedent). `enum-map = "2"` likewise added to
   `[workspace.dependencies]` and referenced `enum-map = { workspace = true }` by
   gp-render (single consumer today; kept in the workspace table for one-source version
   truth, consistent with `thiserror`). The `derive` feature bundles the macros through
   the `strum` facade — everything imports from `strum::` and derives are used as
   `strum::EnumIter` etc.
   - *Rejected: a separate `strum_macros` dep.* The `features = ["derive"]` single-crate
     form is the idiomatic 0.28 surface (traits + re-exported derives from one crate,
     one version pin); `strum_macros` is the lower-level split requiring two synced pins
     and split imports `[measured: probe uses only `strum` w/ `derive` feature → build+clippy+test green]`.
   - Version constraints per AGENTS.md `0.x`→`0.28` (no patch pin), `x.y.z`→`2` (major
     only). Live max-stable **verified**: strum `0.28.0`, enum-map `2.7.3`
     `[measured: crates.io API w/ UA → strum 0.28.0, enum-map 2.7.3, strum_macros 0.28.0]`.

7. **Derive-path style: fully-qualified in `#[derive(...)]`** (`strum::EnumIter`,
   `strum::EnumCount`, `strum::IntoStaticStr`, `enum_map::Enum`), mirroring the repo's
   existing `#[derive(thiserror::Error, Debug)]`. Runtime *trait* imports
   (`IntoEnumIterator` for `.iter()`, `EnumCount` for `COUNT`) are added only where
   consumed; **test-only** trait imports go **inside** the `#[cfg(test)] mod tests`
   block, never at module scope, because a module-scope import consumed only by tests is
   `unused_imports` in the non-test lib target and fails `-D warnings`
   `[derived → cargo clippy --workspace --all-targets -- -D warnings]`.

8. **§ C house rule → `ai-docs/code-style.md` § "Deterministic collections"
   (instructions/harness, authored IN-THREAD — NOT `code-writer`).** The existing section
   picks among `BTreeMap`/`BTreeSet` (sorted) and `IndexMap`/`IndexSet` (insertion-order)
   for **arbitrary-key** collections. Add a **distinct-axis** note: when the key/element
   is itself an enum (a closed variant set), prefer **`enum_map::EnumMap<K,V>`** (enum
   map) / **`enumflags2::BitFlags<K>`** (enum set) over `HashMap<Enum,_>`/`HashSet<Enum>`
   **and even over** `IndexMap`/`BTreeMap`/`BTreeSet`; rationale — array-/bit-backed,
   total (every variant present/representable), allocation- and hasher-free, deterministic
   by enum **declaration** order; also the idiomatic replacement for a hand-written
   `[V; N]` indexed by an enum's discriminant surrogate (`as usize`). Requirements to note:
   `EnumMap` needs `K: enum_map::Enum` (a derive); `BitFlags` needs `#[bitflags] #[repr(uN)]`
   — **cross-linked** to § "Enum repr" as `[Enum repr](#enum-repr)` (anchor `## Enum repr`
   present `[measured: code-style.md:57]`). Framed **preventive**, consistent with the
   crates this task adopts (`enum_map` new to gp-render; `enumflags2` already in gp-core).
   - **Insertion point:** immediately **after** the `indexmap::IndexMap / IndexSet` bullet
     (`code-style.md:44`) and **before** the fixed-hasher-is-not-an-escape-hatch paragraph
     (`:46`) — this groups all "what collection to use" guidance (BTree, IndexMap,
     enum_map/BitFlags) together, ahead of the footgun + test-only-exemption paragraphs.
   - **Authoring:** per AGENTS.md § Workflow ("a predominantly-prose diff … has no code to
     delegate — author it in-thread") + spec C2, this edit is authored **in-thread by the
     orchestrator** (opus), **not** delegated to `code-writer`; its acceptance is
     **doc-review + the Propagation grep**, not a cargo gate.
   - **Propagation finding (measured):** the "Deterministic collections" rule is **not**
     mirrored as an enforcement checklist item in `.claude/agents/self-review.md` or
     `.claude/agents/review-findings.md` — both mirror only *Error types / File size /
     Magic numbers* from code-style.md
     `[measured: grep -rn -iE 'collection|hashmap|hashset|deterministic|indexmap|enum[_ -]?map|bitflags' both files → 2 benign matches (review-findings.md:92 + self-review.md:116, both the identical `# Panics` guidance "… indexes / slices a collection …"), NEITHER a deterministic-collections enforcement reference]`.
     So the Propagation grep surfaces **no un-updated enforcement reference** and **no
     review-file edit is required**. (Adding a *new* deterministic-collections review-checklist
     item would be **out-of-amendment-scope** — C1/C2 say "align *if found*", and none is
     found — so it is NOT auto-included; flag to the product owner if separately desired,
     per AGENTS.md § Communication "deviating from approved scope requires an ask".) The
     remaining grep matches (`ai-docs/library-survey.md` survey record, `ai-docs/context.md`
     architecture prose, `ai-docs/deferred/_inbox.jsonl` triage rows) are point-in-time
     records, not enforcement surfaces — left unchanged
     `[measured: full propagation grep across .claude/ AGENTS.md ai-docs/, excl. plans/ + learnings.md]`.
   - **Cap:** `ai-docs/code-style.md` is 11,861 chars `[measured: wc -c]`; the ~400-char
     note keeps it far under the 40,000 instruction-file cap (AC10).

### Verified facts (measured)

| Fact | Evidence |
|---|---|
| strum 0.28.0 / enum-map 2.7.3 / strum_macros 0.28.0 are live max-stable | `[measured: crates.io API + UA]` |
| No prior strum/enum-map presence (direct or transitive) | `[measured: grep Cargo.toml empty; cargo tree --invert strum / enum-map → "did not match any packages"]` |
| `strum::EnumIter`+`EnumCount`+`IntoStaticStr` and `enum_map::Enum` coexist with `#[bitflags]`+`#[repr(u8)]`+`#[allow(clippy::use_self)]` | `[measured: probe mirrors the exact Action stack → build+clippy+test green]` |
| Generated code passes pedantic+nursery `-D warnings` with **no** new enum-level `#[allow]` (only the pre-existing `use_self` carve-out on Action) | `[measured: probe `cargo clippy --all-targets -- -D warnings` clean; the only diagnostics were probe-prose doc_markdown/unnecessary_wraps, all fixed]` |
| `EnumIter` iterates in declaration order (AC4) | `[measured: probe asserts Action → Coast,East,West,North,South; Side → East,West,North,South → passed]` |
| Icon keys byte-exact & no-alloc (`&'static str`) | `[measured: probe passed]` |
| `FromIterator` build non-viable (TextureHandle not Default) | `[measured: E0277 for non-Default V]` |
| `EnumMap::from_fn` is **infallible** (`FnMut(K)->V`); **no** `try_*` in 2.7.3; from_fn→transpose hits the same `V: Default` wall | `[measured: lib.rs:298; try_* grep empty; transpose → E0277]` |
| **`enum_map!` explicit-key = per-slot exhaustiveness-checked `match`** (macro src `lib.rs:195` match / `:203` internal `from_array`, no closure wrapping arms); `?` propagates inside arms; fallible, panic-free, strict-clippy clean incl. `undocumented_unsafe_blocks` | `[measured: enum_map!{…?…} clippy -D warnings clean + per-icon test passed]` |
| `enum_map!` omitting a variant = **compile error**; `_ =>` wildcard **defeats** exhaustiveness (must be banned); hoisted-`let` of non-`Copy` = move error | `[measured: E0004 (missing arm); `_ =>` builds; E0382 (hoisted-let)]` |
| Doc gate clean | `[measured: RUSTDOCFLAGS="-D warnings" cargo doc --no-deps → Finished]` |

### Verified per-file migration sites (`rg -U`, binding contract)

| Enum / method | File | Sites (verified) |
|---|---|---|
| `Side::ALL` | `crates/core/src/geom/graph.rs` | 1 prod loop (`:315`). No `[`Side::ALL`]` doc link. |
| `Action::ALL` | `crates/core/src/sim.rs` | 1 doc link (`:118`), 1 prod filter/collect (`:121`), 3 test loops (`:462`,`:478`,`:645`), 1 prose comment (`:633`). |
| `Icon::ALL` | `crates/render/src/icons.rs` | 1 doc link (`:181`), 2 prod (`:189` with_capacity → removed, `:190` loop → removed), 4 test (`:244`,`:267` len,`:268` iter,`:337`), 1 prose in miri-reason (`:324`). |
| `Icon::name()` | `crates/render/src/icons.rs` | 1 prod bake-arg (`:191`), 1 test (`:268`), 1 test message string (`:272` prose). |

`[measured: rg -Un 'Side::ALL|Action::ALL|Icon::ALL|\.name\(\)' per file — output matches the spec's enumeration exactly]`

## Decomposition

| # | Task | Files | Depends on |
|---|------|-------|------------|
| 1 | Wire `strum` into `[workspace.dependencies]` + gp-core; add `strum::EnumIter` to `Side` and `Action` (coexisting with `#[bitflags]`); delete both `const ALL`; migrate `Side::iter()`/`Action::iter()` at the 1+4 call sites; add `use strum::IntoEnumIterator;` to each module; repoint the `[`Action::ALL`]` doc link (`:118`) → `[`Action`]` declaration-order wording and the `:633` prose; add the AC4 order test. | `Cargo.toml`, `crates/core/Cargo.toml`, `crates/core/src/geom/graph.rs`, `crates/core/src/sim.rs` | — |
| 2 | Full gp-render migration (strum **and** enum-map together — same file, tightly coupled): add `strum = { workspace = true }` + wire `enum-map` into `[workspace.dependencies]` and `enum-map = { workspace = true }` to gp-render. On `Icon`: add `strum::EnumIter`+`strum::EnumCount`+`strum::IntoStaticStr`+`enum_map::Enum` derives, `serialize_all="kebab-case"` + explicit `Grid3x3`/`ZoomIn` tags; delete `const ALL`; **remove `name()`**. Convert `IconSet(HashMap<…>)` → `IconSet(EnumMap<Icon, TextureHandle>)` (swap `use std::collections::HashMap` → `use enum_map::EnumMap`); rewrite `IconSet::new` with a **single DRY `bake` closure** (bake/`svg_bytes`/name/size written once, capturing `ctx`) fed to the **`enum_map! { Icon::X => bake(Icon::X)? … }`** explicit-key macro (compile-time exhaustiveness + named keys; values in-arm not hoisted; **NO `_ =>` wildcard**; from_array/from_fn rejected — Key decision 4) (production uses `<&'static str>::from`, **not** `iter()`/`COUNT`); make `get` total (`&TextureHandle`). Migrate the test `Icon::ALL`/`name()` sites (`iter()`, `Icon::COUNT`, `<&'static str>::from`), drop `.expect` on `get`; put test-only `use strum::{EnumCount, IntoEnumIterator};` inside `mod tests`. Repoint the `[`Icon::ALL`]` doc link (`:181`) and the `:324`/`:272` prose; preserve `icon_set_bakes_all_five`'s `#[cfg_attr(miri, ignore)]`. Add the AC5 byte-exact-keys test + the AC6 per-icon `.name()` mapping assert. | `Cargo.toml`, `crates/render/Cargo.toml`, `crates/render/src/icons.rs` | 1 |
| 3 | **Instructions/harness — authored IN-THREAD by the orchestrator, NOT `code-writer`.** Extend `ai-docs/code-style.md` § "Deterministic collections" with the enum-keyed-collection distinct-axis note (Key decision 8): insert **after** the `IndexMap`/`IndexSet` bullet (`:44`) and **before** the fixed-hasher paragraph (`:46`) — prefer `enum_map::EnumMap<K,V>` (enum map) / `enumflags2::BitFlags<K>` (enum set) over `HashMap`/`HashSet`/`IndexMap`/`BTree*`; the `[V; N]`-`as usize`-surrogate replacement; rationale array-/bit-backed, total, hasher-free, declaration-order-deterministic; requirements `K: enum_map::Enum`, `#[bitflags] #[repr(uN)]`; **cross-link** `[Enum repr](#enum-repr)`; framed preventive. Then run the Propagation grep (`grep -rn "<changed-keyword>"` across `.claude/` `AGENTS.md` `ai-docs/`) and confirm no un-updated enforcement reference (self-review.md / review-findings.md verified clean — Key decision 8; **no** review-file edit needed). No cargo gate — acceptance is doc-review + grep + `wc -c < 40000`. | `ai-docs/code-style.md` | — (no code dependency; recommended after Group A) |

Subtasks 1–2 leave the whole workspace green (`cargo build` + `cargo clippy --workspace --all-targets -- -D warnings` + `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` + `cargo test --workspace`). Dependency edges land with their first use: strum→gp-core in 1, strum+enum-map→gp-render in 2. Subtask 3 is a prose edit with **no** cargo gate — its acceptance is doc-review + the Propagation grep + the `< 40,000`-char cap (AC10).

**Why gp-render is one subtask, not two (strum then enum-map).** Removing `Icon::const ALL` forces production `IconSet::new` to change in the same step; its cleanest non-`ALL` form is the `enum_map!` build, which needs enum-map wired. A strum-only intermediate would have to give `IconSet::new` a throwaway `HashMap`+`Icon::iter()` body (deleted one step later) and place `iter()`/`COUNT` imports at module scope only to move them into `mod tests` when the production loop disappears — wasted code + avoidable import churn. Merging goes straight to the final form: production never calls `iter()`/`COUNT`, so those imports are test-only from the outset.

## Handoff plan

Per `.claude/agents/design.md` § Rules → handoff-grouping. M = 3 subtasks across **two
change-types** → **two groups** (change-type homogeneity (e) forces a boundary;
group-minimization (f) keeps each change-type to exactly one group — the minimum).

- **Group A (code)** — implementor model `sonnet` (sonnet-5), effort **`medium` (pinned)**
  via the `code-writer` subagent (frontmatter-pinned model+effort; no inline override),
  1M-token window — subtasks **1, 2** (code change-type: Rust `*.rs` + `Cargo.toml` build
  manifests). Size 2, within `1..=10`; homogeneous.
- **Handoff into Group A:** at group entry, spawn `/context-reset` per
  `.claude/skills/context-reset/SKILL.md` § Compaction recovery (re-entry).
- **Handoff into Group B:** at the Group A → B boundary, spawn `/context-reset` again
  (re-entry) so the doc edit is authored with fresh context.
- **Group B (instructions/harness)** — authored **IN-THREAD by the orchestrator**
  (opus, effort **inherited from the orchestrator**, typically xHigh — NOT pinned),
  1M-token window — subtask **3** (instructions/harness change-type: `ai-docs/code-style.md`).
  **Not** delegated to `code-writer` and **not** routed to `general-purpose`: per AGENTS.md
  § Workflow the predominantly-prose diff is authored in-thread by the orchestrator itself
  (an opus agent), satisfying the instructions/harness opus marking without a spawn. Gate =
  **doc-review + Propagation grep + `wc -c < 40000`** (no cargo). **Terminal group** (size 1,
  within `1..=10`; homogeneous); completes /task Step 8 in its own `/context-reset` context.

**Ordering — Group A first, then Group B** (recommended, not correctness-forced): subtask 3
has **no** code dependency (`Depends on: —`), so order is free. A-then-B is preferred
because (a) the house rule *generalizes* the exact pattern Group A concretely demonstrates
— the live `IconSet → EnumMap` adoption becomes an in-tree precedent the rule can cite —
and (b) front-loading the gate-heavy code work surfaces any code surprise before the
low-risk doc edit. Group B could equally run first or standalone; nothing breaks either way.

Group count 2 ≤ 4 (h), no user gate needed. The `design` / `design-review` / `self-review`
quality gates stay on Opus regardless of the per-group implementor markers — only the
Group-A implementor (code-writer/sonnet) differs from the in-thread opus authoring of Group B.

## Risks

- **`FromIterator`/`Default`-based EnumMap build is a trap** (TextureHandle not
  `Default`): both a naive `.collect::<Result<EnumMap<…>,_>>()` **and** a
  `from_fn`→transpose route compile against a `Default` stand-in but not the real type
  (they share the `Self: Default` → `V: Default` requirement). Mitigation: the chosen
  **`enum_map!` explicit-key macro** sidesteps the `Default` bound; both `FromIterator`
  and `from_fn` rejected (Key decision 4). — `[measured: E0277 for non-Default V on both FromIterator + from_fn-transpose; enum_map! green]`
- **Positional-swap footgun — eliminated by `enum_map!`**: a positional `from_array([…])`
  can silently map `get(icon)` to the wrong texture (count-only check, no per-variant
  correspondence). The chosen `enum_map!` explicit-key form removes this at **compile
  time** — omitting a variant is E0004, keys are named. Residual (a mistyped arm
  *argument*) is caught by the AC6 per-icon `.name()` test; a `_ =>` wildcard is
  **forbidden** (it defeats exhaustiveness). — `[measured: E0004 on missing arm; `_ =>` builds (so banned); per-icon test green]`
- **Panic-free invariant** (`icons.rs` "zero production panics"): the build must not
  introduce `.expect`/`unwrap`. `enum_map!` with `?` in each arm propagates `IconError`
  cleanly; no Option-materialization `.expect` is used. — `[measured: probe enum_map!(?) clippy-clean incl. undocumented_unsafe_blocks, no panic path]`
- **Test-only trait import → `unused_imports` in the lib target**: placing
  `use strum::{EnumCount, IntoEnumIterator};` at module scope in icons.rs (where only
  tests iterate) fails `-D warnings`. Mitigation: those imports live inside `mod tests`.
  — `[derived → cargo clippy --workspace --all-targets -- -D warnings]`
- **`#[bitflags]` coexistence on `Action`**: strum EnumIter is discriminant-agnostic
  (constructs by variant name), so it expands cleanly alongside enumflags2's power-of-two
  discriminants. — `[measured: probe replicates the exact Action stack → green]`
- **Cargo.lock churn beyond intent**: new edges expected are `strum`, `strum_macros`,
  `enum-map`, `enum-map-derive` (+ `heck` via strum_macros); `proc-macro2`/`quote`/`syn`/
  `unicode-ident` already present. Mitigation: after each dep edit run `cargo update` then
  `cargo build` and confirm `git diff --stat Cargo.lock` shows only those edges (AGENTS.md
  dep-version rule). — `[derived → git diff --stat Cargo.lock]`
- **Generated code lint surface**: no new enum-level `#[allow]` is required; if a future
  clippy version flags generated code, resolve with a justified `#[allow(..., reason=…)]`
  scoped to the enum (mirroring the `use_self` carve-out), never a blanket allow.
  — `[measured: current clippy pedantic+nursery -D warnings clean on the probe]`
- **Miri gate on `icon_set_bakes_all_five`**: `enum_map!` still bakes `settings.svg`,
  which over-reads under Miri, so the existing `#[cfg_attr(miri, ignore = …)]` MUST be
  kept. — `[derived → MIRIFLAGS=-Zmiri-tree-borrows cargo miri test -p gp-render]`

## Test Design

**Subtask 1 — gp-core (`Side`, `Action`)**
- Location: `crates/core/src/sim.rs` `#[cfg(test)] mod tests`, `crates/core/src/geom/graph.rs` `#[cfg(test)] mod tests`.
- Existing tests recompile against `iter()` (sim.rs `:462`/`:478`/`:645` loops; graph.rs wall tests exercise `walls_from_boundary`'s `Side::iter()`) — AC3/AC9, assert the same facts.
- **New (AC4):** `action_iter_is_declaration_order` — `assert_eq!(Action::iter().collect::<Vec<_>>(), vec![Action::Coast, Action::East, Action::West, Action::North, Action::South])`. This is the explicit order assertion "over the strum surface" AC4 requires. `[measured: probe form passes]`
- Gate: build/clippy/doc/test green; `rg 'const ALL' crates/core/` empty (AC2); doc link repointed (AC8).

**Subtask 2 — gp-render `Icon` strum + `IconSet` EnumMap (one file)**
- Location: `crates/render/src/icons.rs` `#[cfg(test)] mod tests`.
- **New/updated (AC5):** `icon_names_are_byte_exact_static_str` — assert
  `<&'static str>::from(Icon::Play) == "play"`, `…Pause=="pause"`, `…Grid3x3=="grid-3x3"`,
  `…ZoomIn=="zoom-in"`, `…Settings=="settings"`. The `&'static str` return type is the
  compile-time no-alloc proof. `[measured: probe passes]`
- Update `icon_all_and_names_are_the_five_distinct_variants`: `Icon::ALL.len()` →
  `Icon::COUNT`; `Icon::ALL.iter().map(|i| i.name())` → `Icon::iter().map(<&'static str>::from)`;
  keep the distinctness assertion (AC9).
- Update `all_icons_have_nonempty_svg_bytes` loop → `Icon::iter()`; `svg_bytes()` unchanged (AC5).
- **Updated (AC6):** `icon_set_bakes_all_five` — drop `.expect(...)` on `get` (now
  total); for each `icon in Icon::iter()` assert `set.get(icon).name() == <&'static str>::from(icon)`
  and that ids are pairwise distinct. **Disposition — KEEP (defense-in-depth):** under
  `enum_map!` the positional-swap class this test used to guard is now **compile-prevented**
  (exhaustiveness E0004 + named keys), so the test is no longer the sole guarantee; but it
  is retained because (a) it is the **AC6 acceptance artifact** ("lookup returns the correct
  baked texture per icon"), and (b) it still catches the one residual class the compiler
  cannot — a mistyped arm *argument* (`Icon::Play => bake(Icon::Pause)?`). **Preserve** the
  `#[cfg_attr(miri, ignore = …)]` attribute verbatim.
- Fixtures: `egui::Context::default()` (bare Context suffices to bake — existing precedent).
- Scrutinee `Debug` note: no new `assert_matches!` introduced here; existing `matches!` assertions untouched.
- Gate: `rg 'fn name' crates/render/src/icons.rs` shows no `Icon::name`, `rg 'HashMap' crates/render/src/icons.rs` empty (AC2/AC5/AC6); doc link `:181` repointed (AC8); `git diff --stat Cargo.lock` shows only strum/enum-map edges (AC1).

**Subtask 3 — `ai-docs/code-style.md` house rule (instructions/harness; no cargo gate, no unit test)**
- This is a prose edit — acceptance is **doc-review + Propagation grep + char cap**, per AC10; there is no compile/test artifact.
- **Rule-text present:** `grep -n 'enum_map::EnumMap\|enumflags2::BitFlags' ai-docs/code-style.md` inside the § "Deterministic collections" span shows the enum-keyed preference + the `[V; N]`-`as usize`-surrogate replacement clause.
- **Cross-link resolves:** the inserted `[Enum repr](#enum-repr)` targets the `## Enum repr` heading (`:57`, anchor `#enum-repr`) — verify per the AGENTS.md "trace one link before committing" habit (in-file anchor). `[measured: anchor present, code-style.md:57]`
- **Propagation grep clean:** `grep -rn "<changed-keyword>"` across `.claude/` `AGENTS.md` `ai-docs/` surfaces no un-updated **enforcement** reference; `.claude/agents/self-review.md` + `.claude/agents/review-findings.md` carry no deterministic-collections checklist item, so nothing to align. `[measured: the collection-keyword grep over both review files returns 2 benign matches (review-findings.md:92 + self-review.md:116, both the identical `# Panics` "… indexes / slices a collection …" guidance), NEITHER a deterministic-collections enforcement reference — Key decision 8]`
- **Char cap:** `wc -c ai-docs/code-style.md` < 40,000 (11,861 pre-edit + ~400 ≪ 40,000). `[measured: wc -c → 11861]`

**AC → subtask/test map**

| AC | Subtask | Discharging gate / test |
|---|---|---|
| AC1 (deps @ constraints; lock only-intended; build clean) | 1 (strum), 2 (strum-render + enum-map) | `cargo build` + `git diff --stat Cargo.lock` |
| AC2 (no `const ALL`, no alias) | 1 (Side,Action), 2 (Icon) | `rg 'const ALL' crates/` empty |
| AC3 (every `::ALL` site compiles on strum API) | 1,2 | recompiled existing tests |
| AC4 (Action order preserved, asserted) | 1 | `action_iter_is_declaration_order` |
| AC5 (`name()` gone, strum string, exact no-alloc keys, sites migrated, svg_bytes intact) | 2 | `icon_names_are_byte_exact_static_str` |
| AC6 (EnumMap backing, `Enum` derive, correct per-icon lookup) | 2 | `icon_set_bakes_all_five` (per-icon `.name()` assert) |
| AC7 (clippy pedantic+nursery `-D warnings` clean) | 1,2 | `cargo clippy --workspace --all-targets -- -D warnings` |
| AC8 (doc gate; no broken `::ALL` link) | 1,2 | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` |
| AC9 (workspace tests green; updated `::ALL`/count/name/IconSet tests assert same facts) | 1,2 | `cargo test --workspace` |
| AC10 (code-style.md enum-keyed rule + `[Enum repr]` cross-link + propagation clean + < 40k) | 3 (in-thread) | `grep` rule text + trace `#enum-repr` anchor + Propagation grep across `.claude/` `AGENTS.md` `ai-docs/` + `wc -c < 40000` (doc-review, **no** cargo gate) |

## Open questions

None. The spec's round-1 fork (`Icon::name()` in scope) is resolved, and every
implementation fork the spec deferred to design (which strum derive, keep-vs-remove
`name()`, `IconSet::get` signature, fallible `EnumMap` construction, crate/feature
wiring) is decided above against measured evidence.

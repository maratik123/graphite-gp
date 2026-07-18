# Adopt strum derives and enum_map for enum boilerplate

**Source:** user description (free-text task)
**Date:** 2026-07-18
**Tracked in:** none — user opted out (free-text de-boilerplate task; no tracking issue requested)

One combined task with two libraries: **strum/strum_macros** (replace hand-written
variant-enumeration and variant→string boilerplate) and **enum_map** (replace an
enum-keyed map with an array-backed total map).

## Scope

### A. strum — variant-enumeration + variant→string boilerplate

- **A1.** Add `strum` (with derive support) to **gp-core** and **gp-render**, constraint `0.28` (latest stable 0.28.0, verified live on crates.io; 0.x rule → `0.28`, no patch pin). No existing presence in the workspace (verified: `grep -r strum --include=Cargo.toml .` empty + `cargo tree --invert strum` no match).
- **A2.** Replace the three hand-written `const ALL: [Self; N]` arrays with a strum variant-enumeration derive, preserving declaration order in every case:
  - **`Side`** — `crates/core/src/geom/graph.rs` (4 variants: East, West, North, South; `const ALL` at line 32).
  - **`Action`** — `crates/core/src/sim.rs` (5 variants: Coast, East, West, North, South; `const ALL` at line 64). Declaration order **matches the policy's logit order** (`legal_mask`) and must not change. The derive must coexist with the existing `#[bitflags]` (enumflags2) + `#[repr(u8)]` + `#[allow(clippy::use_self)]` attributes on the enum.
  - **`Icon`** — `crates/render/src/icons.rs` (5 variants: Play, Pause, Grid3x3, ZoomIn, Settings; `const ALL` at line 45).
- **A3.** Replace `Icon::name()`'s hand-written kebab-string `match` (icons.rs:69) with a strum **string** derive (`#[strum(serialize = "grid-3x3")]`-style per-variant tags) that yields `&'static str` **with no heap allocation** — i.e. an `IntoStaticStr`-family derive, **not** `Display`/`ToString` (those allocate). The five keys must stay byte-identical (`play`, `pause`, `grid-3x3`, `zoom-in`, `settings`) — they are the `ctx.load_texture` cache labels. Migrate `name()`'s two call sites: icons.rs:191 (production, `IconSet::new`) and icons.rs:268 (test). **`Icon::svg_bytes()` stays hand-written** — its `include_bytes!` mapping is semantic asset selection, not boilerplate.
- **A4.** Migrate every `::ALL` call site to the strum-provided API as a **clean break** — remove `const ALL` outright, no `pub const ALL` alias kept (per API-Stability AXIOM and the issue's stated house style). Enumerated call sites:
  - `Side::ALL` — graph.rs:315 (production loop).
  - `Action::ALL` — sim.rs:121 (production, `.into_iter().filter().collect()`); sim.rs:462 / :478 / :645 (test loops).
  - `Icon::ALL` — icons.rs:189 (production, `HashMap::with_capacity(Icon::ALL.len())`), :190 (production loop); icons.rs:244 / :337 (test loops), :267 (test `Icon::ALL.len() == 5`), :268 (test `.iter().map(...)`).
- **A5.** Repoint the intra-doc links that reference the removed const so the doc gate stays green: `` [`Action::ALL`] `` (sim.rs:118) and `` [`Icon::ALL`] `` (icons.rs:181). Update the plain-comment mentions (sim.rs:633, icons.rs:324) for accuracy.

### B. enum_map — enum-keyed map

- **B1.** Add `enum-map` to **gp-render**, constraint `2` (latest stable 2.7.3, verified live; x.y.z rule → major only). No existing presence in the workspace (verified: `grep` empty + `cargo tree --invert enum-map` no match).
- **B2.** Convert `IconSet(HashMap<Icon, TextureHandle>)` (icons.rs:178) to an array-backed `EnumMap<Icon, TextureHandle>` — the one collection literally keyed by an enum in the tree today (total, no hashing). `Icon` derives `enum_map::Enum`. `IconSet` is today constructed and read (`get`) only inside its own module's tests (icons.rs:334 / :338), so the change is fully contained — no external consumer to migrate.

### C. Documentation — enum-keyed collection house rule

- **C1.** Extend `ai-docs/code-style.md` § "Deterministic collections" (line 38) with a **distinct-axis** note. The existing guidance chooses among `BTreeMap`/`BTreeSet` (sorted) and `indexmap::IndexMap`/`IndexSet` (insertion-order) for **arbitrary-key** collections; add that when the **key/element is itself an enum** (a closed variant set), prefer `enum_map::EnumMap<K, V>` for an enum-keyed **map** and `enumflags2::BitFlags<K>` for an enum **set**, over `HashMap<Enum, _>` / `HashSet<Enum>` — and even over `IndexMap` / `BTreeMap` / `BTreeSet`. Rationale to state: array-/bit-backed, total (every variant present / representable), allocation- and hasher-free, and deterministic by construction (iteration follows enum **declaration** order). Also note it is the idiomatic replacement for a hand-written `[V; N]` array indexed by an enum's discriminant surrogate (`as usize`). Requirements to note: `EnumMap` needs `K: enum_map::Enum` (a derive); `BitFlags` needs `#[bitflags] #[repr(uN)]` on the enum — cross-link § "Enum repr" (line 57). Frame it as a **preventive** house rule consistent with the crates this task adopts (`enum_map` new to gp-render; `enumflags2` already in gp-core).
- **C2. (in-thread, NOT code-writer).** This is a prose / instruction-file edit to `ai-docs/code-style.md`. Per AGENTS.md § Workflow ("a predominantly-prose diff … has no code to delegate — author it in-thread"), the design MUST mark this an **instructions/harness in-thread subtask authored by the orchestrator**, not a `code-writer` implementation group. The Propagation Rule fires on a `code-style.md` edit: run the changed-keyword grep and check `.claude/agents/self-review.md` / `.claude/agents/review-findings.md` for any enforcement reference to the collection rule, aligning them in the same PR.

## Out of scope

- **`Icon::svg_bytes()`** — a semantic `include_bytes!` mapping (per-variant asset selection), not boilerplate; stays hand-written.
- **`RaceDir` / `Orient`** — no `ALL` array and no other enumerate/name boilerplate; a speculative derive with no consumer would be dead code. Not touched.
- **`CAR_COLORS` / `HEAT_RAMP`** (color.rs:154/156) — `[Color32; N]` palettes indexed by a plain `usize` (`CAR_COLORS.get(index)`), **not keyed by an enum**; not an enum_map candidate.
- **collision's `HashMap<Point, Vec<usize>>`** (collision.rs:61) — keyed by the `Point` struct, not an enum; not an enum_map candidate.
- **gp-ai policy/logit arrays keyed by `Action` order** — do not exist in the tree yet (block 4); enum_map/`Action: Enum` is deferred to when they land, not pre-emptively derived now.
- Any behavior change to the physics core, the icon bake pipeline, or `legal_mask` semantics — this is a mechanical de-boilerplate; the observable variant set, iteration order, and icon keys are unchanged.

## Deferred

- `Action: enum_map::Enum` + `EnumMap<Action, _>` logit storage | no consumer until gp-ai (block 4) exists | separate issue needed? — arises with the block-4 policy work, not here.

## Key decisions

| Question | Decision |
|---|---|
| strum version constraint | `0.28` (latest stable 0.28.0, verified live; 0.x → no patch pin). |
| enum-map version constraint | `2` (latest stable 2.7.3, verified live; x.y.z → major only). |
| Keep a `const ALL` alias for compatibility? | No — clean break. `const ALL` is removed and all call sites migrate. gp-core/gp-render are a never-published game app; no downstream clients. |
| Which strum derive replaces `ALL`? | **Left to design.** Candidates: `EnumIter` (`Self::iter()`) vs `VariantArray` (`Self::VARIANTS: &'static [Self]`), optionally `EnumCount` (`Self::COUNT`) for the two `Icon::ALL.len()` sites. The choice must preserve: (a) declaration-order iteration determinism, (b) by-value iteration (`for x in ...`), (c) a length/count usable where `Icon::ALL.len()` is used today. |
| `Icon::name()` — how replaced? | **In scope (round-1 answer).** A strum string derive with per-variant `#[strum(serialize = "…")]` tags, yielding `&'static str` with **no allocation** (`IntoStaticStr`-family — not `Display`/`ToString`). Exact kebab keys preserved. Whether a thin `name()` method is kept as a wrapper over the derive or removed with call sites using `<&'static str>::from(icon)` is **left to design** (clean break allowed). |
| strum / enum-map crate + feature wiring | **Left to design** — strum `features = ["derive"]` vs a separate `strum_macros` dep; enum-map's derive is a default feature; workspace-dep (`x.workspace = true`, gp-render already does this for `thiserror`) vs per-crate entry. |
| `IconSet` under `EnumMap`: `get()` signature + fallible build | **Left to design.** `EnumMap` is total, so indexing is infallible — `get()` may become `&TextureHandle` (clean break) or keep `Option`. Construction stays fallible (`bake_texture -> Result`), so the total map must be built fallibly (e.g. bake into an intermediate then materialize). Its sole call site is one test. |

## Technical constraints

- **Crate placement:** `Side` + `Action` live in gp-core; `Icon` lives in gp-render — so **strum** lands in both. **enum-map** lands in **gp-render only** (`Icon`/`IconSet`).
- **Exact icon keys:** strum `serialize_all = "kebab-case"` alone does **not** reliably yield `Grid3x3 → "grid-3x3"` (a digit boundary is not separated), so at least `Grid3x3` (and `ZoomIn → "zoom-in"`) need explicit per-variant `#[strum(serialize = "…")]`. All five keys must be byte-identical to today's — they are `ctx.load_texture` cache labels; a drifted key silently breaks texture reuse.
- **Production call site `bake_texture(ctx, icon.name(), …)`** passes into `name: impl Into<String>`. If `name()` is removed, the replacement must produce `&'static str` explicitly (e.g. `<&'static str>::from(icon)`) to avoid `Into<String>` inference ambiguity at that site.
- **Strict clippy, pedantic + nursery denied** (Cargo.toml:47-48, `-D warnings`). strum-`derive`- and enum_map-`Enum`-generated code must pass this gate; any generated construct that trips a pedantic/nursery lint is resolved with a **justified** `#[allow(..., reason = "…")]` scoped to the enum (mirroring the existing `#[allow(clippy::use_self, reason …)]` carve-out on `Action`), never a blanket allow.
- **`Action` `#[bitflags]` coexistence:** the strum derives must expand cleanly alongside enumflags2's `#[bitflags]` attribute macro, `#[repr(u8)]`, and the existing derive/allow stack. strum's variant-enumeration derives construct variants by name (discriminant-agnostic), so coexistence is expected but must be compiled and lint-checked.
- **`Icon` derive stack:** `Icon` will carry the strum variant-enumeration derive, a strum string derive (for `name()`), and `enum_map::Enum`, alongside its existing `Clone, Copy, Debug, PartialEq, Eq, Hash`. All must coexist and lint clean.
- **Doc gate:** `broken_intra_doc_links = "deny"` (Cargo.toml:42) + `RUSTDOCFLAGS="-D warnings"`. Every doc link to the removed `::ALL` must be repointed (to the chosen strum surface) or the doc build fails.
- **gp-core invariant preserved:** the integer-only, deterministic core is untouched — this change only swaps variant-enumeration machinery; `sim`/`geom` arithmetic and semantics are unchanged, and strum variant iteration is declaration-order with no added computation.
- **`Icon::ALL.len()` count usages** (icons.rs:189 `with_capacity`, icons.rs:267 test `== 5`) must have a working replacement under the chosen strum surface (or become redundant if `EnumMap`-based construction replaces the `with_capacity` build loop).

## Acceptance Criteria

| # | Criterion |
|---|-----------|
| AC1 | `strum` appears in both `crates/core/Cargo.toml` and `crates/render/Cargo.toml` at constraint `0.28`, and `enum-map` in `crates/render/Cargo.toml` at constraint `2`; `cargo update` touches only the intended edges and `cargo build` is clean. |
| AC2 | No `const ALL` remains on `Side`, `Action`, or `Icon` (grep-clean), and no `pub const ALL` compatibility alias exists. |
| AC3 | Every enumerated `::ALL` call site (production + test) compiles against the strum-provided API; the observable variant set is unchanged. |
| AC4 | `Action` iteration order is still exactly `Coast, East, West, North, South` (the policy logit / `legal_mask` order); a test asserts this order over the strum surface. |
| AC5 | `Icon::name()`'s hand-written `match` is gone, replaced by a strum string derive; it yields the exact keys (`play`, `pause`, `grid-3x3`, `zoom-in`, `settings`) as `&'static str` with **no heap allocation**; both call sites (icons.rs:191, :268) are migrated; and a test asserts each variant's key is byte-identical to today's. `Icon::svg_bytes()` is unchanged. |
| AC6 | `IconSet`'s backing store is `EnumMap<Icon, TextureHandle>` (no `HashMap`); `Icon` derives `enum_map::Enum`; the set still bakes every variant and its lookup returns the correct baked texture per icon. |
| AC7 | `cargo clippy --workspace --all-targets -- -D warnings` is clean (pedantic + nursery included), with any strum/enum_map generated-code allow justified by a `reason`. |
| AC8 | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` is clean — no broken intra-doc link left by the removed `::ALL`. |
| AC9 | `cargo test --workspace` is green; the `Icon`/`Action` tests that referenced `::ALL` (incl. the `Icon::ALL.len() == 5` count assertion and the `name()` distinctness test) and the `IconSet` tests are updated and still assert the same facts. |
| AC10 | `ai-docs/code-style.md` § "Deterministic collections" documents the enum-keyed-collection preference (`enum_map::EnumMap` for enum-keyed maps, `enumflags2::BitFlags` for enum sets, and the `[V; N]`-discriminant-surrogate replacement), cross-linked to § "Enum repr"; the Propagation grep for the changed rule (`grep -rn` across `.claude/` `AGENTS.md` `ai-docs/`) surfaces no un-updated enforcement reference (self-review.md / review-findings.md alignment checked); `ai-docs/code-style.md` stays < 40,000 chars (11,861 today — ample headroom). |

## Open questions

None — the round-1 `Icon::name()` scope question is resolved (in scope; see Scope A3). Remaining implementation forks (which strum variant-enumeration derive, keep-vs-remove the `name()` wrapper, `IconSet::get` signature, fallible `EnumMap` construction) are design-phase choices with defensible defaults, not spec blockers.

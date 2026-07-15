# Library Adoption Survey — graphite-gp

**Date:** 2026-07-15
**Scope:** PART 1 of `ai-docs/plans/2026-07-15-library-survey-workflow-tuning.spec.md` — survey candidate crates for reducing boilerplate, better collections, and idiomatic error handling; adopt the clearly-beneficial low-risk ones now; file near-term issues for the imminent next-crate candidates; leave the rest report-only.

## Verdict legend

- **adopt-now** — add the dependency and refactor call-sites in this PR.
- **defer-to-issue** — deferred; either tracked by a GitHub issue filed this PR (see [Near-term GitHub issues](#near-term-github-issues)) or report-only until its target crate is built.
- **reject** — not a good fit now (for the stated target); revisit only if the stated rationale changes.

Any crate version is intentionally omitted — per AGENTS.md § Dependency Versions, a version must be queried live (`crates.io` `max_stable_version`) at adoption time, never asserted from memory. This survey stays version-agnostic.

## Adopt-now set: EMPTY (honest outcome)

The `gp-core` adopt-now set is **empty**, and no other crate warrants an in-PR refactor. This is the honest outcome the spec permits (AC2) — no marginal adoption is forced merely to "adopt something". Concretely, `gp-core` today:

- has **zero** `[dependencies]` (`crates/core/Cargo.toml`);
- uses `HashMap`/`HashSet` **only** under `#[cfg(test)]` (`crates/core/src/geom/mod.rs`, `crates/core/src/geom/graph.rs`) — no production nondeterministic collection to migrate;
- defines **no** error enum/struct — nothing for `thiserror` to wrap yet;
- returns the legal-action set as `[bool; 5]` (`crates/core/src/sim.rs` `legal_mask`) — no bitflags contract yet;
- is integer-only and deterministic (`docs/design.md` §3a) — float-vector crates (`glam`) never target it.

Because the adopt-now set is empty, **AC3 is vacuously satisfied** — there is no adopt-now line whose refactor must be verified, and this PR ships **no** `*.rs` changes.

## Survey table

| Candidate | Verdict | Target crate | One-line rationale | Tracking |
|---|---|---|---|---|
| `roaring` | reject | `gp-core` | Wrong tool for small, dense, bounded grid corridors; a `Vec<bool>` / plain bitset over the corridor rect is simpler and faster than a compressed roaring bitmap. | — |
| `enumflags2` | defer-to-issue | `gp-core` (`legal_mask`) | `legal_mask` returns `[bool; 5]` today; a typed 5-action bitflag is cleaner and enables `#[repr]`-backed masks, but is preventive until the action set is locked. | [#51](https://github.com/maratik123/graphite-gp/issues/51) |
| `indexmap` / `IndexSet` | defer-to-issue | `gp-gen` | Deterministic-iteration set for track generation (ring/repair frontier), avoiding `std::HashSet`'s nondeterministic order; no production set exists in the core yet. | [#50](https://github.com/maratik123/graphite-gp/issues/50) |
| `thiserror` | reject (for now) | `gp-core` / `gp-gen` | No error enum exists yet; AGENTS.md § Code Style already mandates `thiserror` on the **first** error type — adopt then; nothing to refactor now. | — |
| `itertools` | reject | `gp-core` | Marginal: would add a dependency to the otherwise dep-free core for a single grid-walk call-site; std iterator adapters already suffice. | — |
| `anyhow` | reject (for now) | `gp-game` | Library crates (core/gen/render/ai) use typed errors, not `anyhow`; the binary's top-level orchestration could use it once it has fallible flows — none exist yet. | — |
| `smallvec` | defer-to-issue | `gp-render` / `gp-ai` | Small-vector optimization for hot per-cell / per-step buffers; the target hot paths are not built yet. | Report-only this PR |
| `rand` / `rand_chacha` | defer-to-issue | `gp-gen` | Seeded, reproducible RNG for track generation and replay determinism; `rand_chacha` gives a portable, version-stable stream across platforms. | [#49](https://github.com/maratik123/graphite-gp/issues/49) |
| `serde` | defer-to-issue | `gp-gen` / `gp-game` | Track / replay (de)serialization and save format; no persisted format is defined yet. | Report-only this PR |
| `glam` | defer-to-issue | `gp-render` / `gp-ai` | Float vector/matrix math for rendering and policy features; the integer core never uses it (`docs/design.md` §3a). | Report-only this PR |
| `rayon` | defer-to-issue | `gp-ai` / `gp-gen` | Data-parallel self-play / batch generation; premature before a profiled bottleneck exists (determinism must be preserved). | Report-only this PR |
| `candle` / `burn` | defer-to-issue | `gp-ai` | Neural-net training backend for the self-taught policy; `crates/ai/Cargo.toml` `TODO(4)` already flags a profile-simulation-vs-network decision first. | Report-only this PR |
| `macroquad` / `egui` | defer-to-issue | `gp-render` | Rendering / UI backend; `crates/render/Cargo.toml` `TODO(2)` flags the backend choice — pick on the first real render work. | Report-only this PR |

## Near-term GitHub issues

Per Key Decision Q2 (near-term only), exactly three issues are filed by this PR for the imminent next-crate candidates. The render / AI-stack candidates (`smallvec`, `serde`, `glam`, `rayon`, `candle` / `burn`, `macroquad` / `egui`) stay **report-only** — they target code that does not exist yet, so no issue is filed for them this PR.

- **`gp-gen` — seeded replay RNG (`rand` / `rand_chacha`):** [#49](https://github.com/maratik123/graphite-gp/issues/49)
- **`gp-gen` — deterministic-order set (`IndexSet`):** [#50](https://github.com/maratik123/graphite-gp/issues/50)
- **`gp-core` — `enumflags2` for `legal_mask`:** [#51](https://github.com/maratik123/graphite-gp/issues/51)

## Notes

- **Deterministic collections:** because no production `HashMap`/`HashSet` exists in `gp-core` (test-only uses excepted), there is nothing to migrate today. The preventive deterministic-collections code-style rule (forbid production `std::HashMap`/`HashSet`; prefer `IndexMap`/`IndexSet`/`BTree*`) covers future additions and is added under PART 4 (CS-0 group), not here.
- **`gp-core` "std-only" note** (`crates/core/src/geom/graph.rs`) restricts numeric types to integers, not external crates — a crate may be adopted in the core if genuinely beneficial. None currently is.

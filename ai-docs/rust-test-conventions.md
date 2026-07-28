# Rust test conventions — detail

> Extracted from `AGENTS.md` § *Rust Test Conventions* so the always-loaded file carries the rules and this page carries the reasoning. Read on demand — when choosing a test crate, writing an assertion whose type bounds matter, or bounding a property-test input space.

## `proptest` is for DIFFERENTIAL properties

`proptest` (a `gp-core` dev-dep) earns its place when a rewrite must be pinned to the implementation it replaced — the old code is kept verbatim as an oracle and the new code is asserted equal to it across generated inputs. That is the shape it is for; a free-floating "this function shouldn't panic" property is usually a unit test wearing a costume.

**Bound the input space by what the ORACLE costs, not by what the type allows.** The generated range is not free: a kept-verbatim `O(bbox)` reference turns an innocuous `±10_000` coordinate pair into a 4·10⁸-cell scan *per case*, and proptest runs hundreds of cases. The type's range (`i32`) is not the budget; the oracle's per-case cost is.

Precedent in-tree: `crates/core/src/geom/supercover.rs` § `supercover_equivalence`.

Other crates, for orientation: `rstest` for parameterized cases when a table beats duplication; `mockall` for mocking traits; `pretty_assertions` encouraged wherever a diff is easier to read than an equality failure.

## `assert_matches!` imposes a `Debug` bound that `assert!(matches!(…))` does not

`assert_matches!` formats the scrutinee with `{:?}` on mismatch, so **the scrutinee's type MUST implement `Debug`**:

- a `Result` needs `Debug` on **both** `T` and `E`;
- a `Box<dyn Trait>` needs a `Debug` supertrait.

`assert!(matches!(...))` imposes no such bound, because it never formats the value.

**If the scrutinee is non-`Debug`, leave `assert!(matches!(...))` as it is.** Do **not** add a production `#[derive(Debug)]` to satisfy a test-only assertion — that widens a public API's trait surface to serve a convenience in the test module, and the API-stability latitude this project has is not a licence to grow types for test ergonomics.

### Counting `assert!(matches!)` sites before a migration

The multi-line message form is **invisible** to a single-line `rg 'assert!\(matches!'` — use `rg -U`.

That is one instance of a general rule: **a search miss on a construct that SHOULD exist is a search-method failure first, code-absence second.** Its causes and fixes live in [`.claude/rules/ast-index.md` → Negative results are NOT evidence](../.claude/rules/ast-index.md#negative-results-are-not-evidence) — consult that table before concluding a construct is absent.

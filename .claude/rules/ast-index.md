# ast-index Rules

## Mandatory Search Rules

1. **ALWAYS use ast-index FIRST** for any code search task
2. **NEVER duplicate results** — if ast-index found usages/implementations, that IS the complete answer
3. **DO NOT run grep "for completeness"** after ast-index returns results
4. **Use grep/Search ONLY when:**
   - ast-index returns empty results
   - Searching for regex patterns (ast-index uses literal match)
   - Searching for string literals inside code (`"some text"`)
   - Searching in comments content

## Negative results are NOT evidence

A search that comes back **empty or short** for a construct that SHOULD exist is a
**search-method failure first, code-absence second.** Never conclude "X does not
exist" from a miss — re-run with a different method, or read the region.

| Cause of the false negative | Fix |
|---|---|
| Multi-line construct (rustfmt-split `#[cfg_attr(\n    miri, …)]`, `assert!(matches!` with a message, wrapped macro call) | `rg -U` (multiline), or read the region |
| Hand-rolled identifier class — `[a-z_]*` excludes digits, and Rust `snake_case` routinely carries them (all three verified in-tree: `ac7_v1_liveness_is_equivalent_to_full_oracle_lappability`, `p0_at_v1`, `trap_ring_is_v1_lappable_and_has_an_unbrakeable_hazard`) | `[A-Za-z0-9_]+`, or `ast-index symbol` / `ast-index outline`, which needs no hand-written pattern |
| Wrong crate version / wrong path on disk; aliased or rewritten output | Read the actual source file |
| Case-sensitive pattern over **prose** — instruction text, comments and headings capitalise mid-sentence words freely, so the emphatic occurrence is the one that escapes | `grep -rni` / `rg -i`; a clean sweep is evidence about your *pattern* until you have varied its case |

**MUST — a claim that an API, symbol, flag, or precedent does NOT exist requires a
raw read of the source (or offline rustdoc), never a search tool's silence.** This
binds `[measured:]` tags in a design doc especially: a `[measured:]` negative backed
by a grep miss is untagged. Prescribing a *replacement* off such a negative
compounds it by inventing a second nonexistent symbol.

## Why ast-index

ast-index is 17-69x faster than grep (1-10ms vs 200ms-3s) and returns structured, accurate results.

## Command Reference

| Task | Command | Time |
|------|---------|------|
| Universal search | `ast-index search "query"` | ~10ms |
| Find struct/trait | `ast-index class "StructName"` | ~1ms |
| Find symbol | `ast-index symbol "SymbolName"` | ~1ms |
| Find usages | `ast-index usages "SymbolName"` | ~8ms |
| Find implementations | `ast-index implementations "Trait"` | ~5ms |
| Call hierarchy | `ast-index call-tree "function" --depth 3` | ~1s |
| Find callers | `ast-index callers "functionName"` | ~1s |
| Module deps | `ast-index deps "module-name"` | ~10ms |
| File outline | `ast-index outline "lib.rs"` | ~1ms |

## Rust-Specific Commands

| Task | Command |
|------|---------|
| Find structs | `ast-index class "User"` |
| Find traits | `ast-index class "Repository"` |
| Find impl blocks | `ast-index search "impl"` |
| Find macros | `ast-index search "macro_rules"` |
| Find derives | `ast-index search "#[derive"` |
| Find tests | `ast-index search "#[test]"` |

## Index Management

- `ast-index rebuild` — Full reindex (run once after clone)
- `ast-index update` — After git pull/merge
- `ast-index stats` — Show index statistics

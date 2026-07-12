# API Naming

This page extracts the body of [`AGENTS.md` § API Naming](../AGENTS.md#api-naming). AGENTS.md keeps the `## API Naming` heading and a one-line stub-link back to this page.

## The _unchecked AXIOM

> **AXIOM — `_unchecked` means `unsafe` AND UB-on-failure. Period.**
> The suffix is reserved exclusively for `unsafe fn` whose contract documents Undefined Behaviour on caller-invariant violation. **NEVER** apply it to a safe fn — even one that "skips a runtime check" — because the suffix carries unsafety implications that mislead readers and reviewers.
>
> | Your fn... | Suffix |
> |---|---|
> | Is `unsafe`, UB on caller violation (`# Safety` section required) | `_unchecked` ✓ (e.g., `slice::get_unchecked`, `str::from_utf8_unchecked`) |
> | Is **safe**, skips a non-safety check (validation, sort-order, etc.) | A descriptive suffix like `_unverified` / `_skip_validation` / `_unsorted` — **NEVER** `_unchecked` |
> | Is **safe**, returns `Result` / `Option` on failure | Unsuffixed (`do_something`); add a `try_*` variant if a panicking sibling exists |

Follow `std` ecosystem conventions. The unsuffixed name is the **safe, ergonomic default**; suffixes mark deviations. Path of least resistance must be the safe path.

## Naming rules

- **`_unchecked` is reserved for `unsafe` fns only.** Every `_unchecked` fn must be marked `unsafe` and document a `# Safety` section listing the invariants the caller must uphold to avoid Undefined Behavior. Examples: `slice::get_unchecked`, `str::from_utf8_unchecked`. **Never use `_unchecked` on a safe fn** — the suffix carries an unsafety implication; co-opting it for "skips an unrelated runtime check" misleads readers and reviewers.
- **Default safe + checked, returns `Result`/`Option` on failure:** safe `do_something()` plus `unsafe do_something_unchecked()`.
- **Prefer non-panicking APIs for libraries:** implement `try_do_something()` returning `Result`/`Option` as the default; leave it to the caller to decide how to handle failure. A panicking `do_something()` convenience wrapper may be added alongside but must not be the only option. Panicking is acceptable only when a fundamental invariant is broken and continuing would leave the application in an inconsistent state — even then, get explicit user approval before adding a panicking API. Document panicking behaviour in the `# Panics` section of the fn doc. Optional `unsafe do_something_unchecked()` for UB-on-failure (e.g. `unchecked_add`).
- **Other "with-vs-without runtime behavior X" variants** (neither `unsafe` nor panicking — e.g. flag-aware vs. flag-bypassing): pick descriptive names that say what each variant *does*. Do **not** repurpose `_unchecked`/`_checked`. If one variant is overwhelmingly more common, give it the unsuffixed name and suffix the rare one.

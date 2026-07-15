# Documentation Conventions — graphite-gp

The canonical reference for `///` and `//!` doc-comment style across the
workspace. Every public item in every crate (`gp-core`, `gp-gen`,
`gp-render`, `gp-ai`, `gp-game`) conforms. The rules grow through the
learning loop (`/improve`).

The AGENTS.md § Code Style "Documentation" bullet is the one-line quick
reference; this file is the detail. The `project-review` skill and the
`review-findings` / `self-review` agents read this file as their reference
and check the lint-invisible rules (summary tense, section order,
`# Parameters` content quality) on every PR.

## Scope

- **Applies to:** every public item — `pub fn`, `pub struct`, `pub enum`,
  `pub trait`, `pub union`, exported macros, and every method declared
  inside a `pub trait` body.
- **Does NOT apply to:**
    - `#[doc(hidden)]` items.
    - Private items (`fn`, `struct`, … without `pub`).
    - Methods inside `impl Trait for Type { … }` blocks — they inherit the
      trait's docs (see [DOC-8 — Trait-impl exemption](#doc-8--trait-impl-exemption)).

## DOC-1 — Summary line

- One sentence on the first line of the doc comment.
- **Third-person singular present indicative.** Write `Returns the…`,
  `Creates a new…`, `Computes the…`, `Sets the…`. Not imperative
  (`Return…`), not progressive (`Returning…`), not future (`Will return…`).
- Terminal period at the end.
- **American English spelling:** `behavior`, `color`, `serialize`,
  `initialize`, `neighbor` — not `behaviour`, `colour`, `serialise`,
  `initialise`, `neighbour`. (`clippy::doc_markdown` is `en-us`-aware; see
  [DOC-6](#doc-6--lints-that-mechanically-enforce).)

## DOC-2 — Section order (strict)

When multiple `#`-headed sections are present they MUST appear in this
order. Reordering is checked mechanically by reviewers:

1. Summary line (no heading).
2. Free-form prose paragraphs (optional).
3. `# Parameters`
4. `# Returns`
5. `# Type parameters`
6. `# Lifetimes`
7. `# Errors`
8. `# Panics`
9. `# Safety`
10. `# Examples`
11. `# See also`

Omitting a section that is not required by the rules below is fine.
Reordering is not.

## DOC-3 — `# Errors` / `# Panics` / `# Safety`

- **`# Errors` — required on every `Result`-returning public fn.** List
  each error variant and the precondition that produces it. Enforced by
  `clippy::missing_errors_doc` (via `pedantic`).
- **`# Panics` — required when the fn can panic on a precondition the
  caller controls** (`unwrap` / `expect`, indexing, arithmetic overflow,
  an asserted invariant). Enforced by `clippy::missing_panics_doc` (via
  `pedantic`). `gp-core` is integer-only: document the coordinate/velocity
  or index range whose violation panics.
- **`# Safety` — dormant until real `unsafe` exists.** Required on every
  `unsafe fn` and every `_unchecked` variant, listing the invariants the
  caller must uphold to avoid undefined behavior. The workspace currently
  ships **no `unsafe`**, so no item needs `# Safety` today; the section
  activates the first time an `unsafe fn` / `_unchecked` API lands.
  Enforced then by `clippy::missing_safety_doc`, with
  `clippy::undocumented_unsafe_blocks = "deny"` additionally requiring a
  `// SAFETY:` comment on each `unsafe` block.

## DOC-4 — `# Parameters` / `# Returns` (recommended)

A **recommended, not required** convention — but a strong fit for
`gp-core`'s integer coordinate/velocity/index arguments, where the unit
and valid range are load-bearing and not obvious from the type.

- **`# Parameters`** — one bullet per argument (other than the receiver
  `self` / `&self` / `&mut self`). Backtick the identifier; describe units,
  ranges, and ownership semantics after a colon and single space:

    ```text
    /// # Parameters
    ///
    /// - `a`: start cell, inclusive; grid coordinates.
    /// - `b`: end cell, inclusive; grid coordinates.
    ```

- **`# Returns`** — when the return type is non-trivial and not already
  obvious from the summary. Skip it for a simple getter whose summary
  already names the returned value.

## DOC-5 — Backticking and intra-doc links

- **Backtick every Rust identifier in prose** — type names, function
  names, module names, build-config tokens (`no_std`), third-party crate
  types. `clippy::doc_markdown` enforces this for `CamelCase` identifiers.
- **Prefer intra-doc links** over bare backticks when the reference is a
  navigation target: `` [`Point`] ``, `` [`Grid::index`] ``,
  `` [`gp_core::supercover`] ``. `rustdoc::broken_intra_doc_links = "deny"`
  catches stale links.
- **Use the inline link form `` [`Foo`](path) `` — not the reference form
  `` [`Foo`][path] ``.** Both render identically, but the workspace
  convention is the inline form so readers and reviewers see one consistent
  shape. When editing a file that still uses the reference form, convert it
  in the same edit.
- Cross-crate links use the workspace crate name (`` [`gp_core::Corridor`] ``)
  and must target a **direct** dependency; a link into a transitive dep
  fails resolution under the doc gate. Inside the same crate, prefer the
  shortest unambiguous form.

## DOC-6 — Lints that mechanically enforce

CI runs `cargo clippy --workspace --all-targets -- -D warnings` and the
doc gate `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`, so
every lint below is a hard error in practice. The workspace declares them
once in the root `Cargo.toml` `[workspace.lints.*]`; each crate opts in via
`[lints] workspace = true`. (Full policy and carve-outs:
[`code-style.md` → Lints that mechanically enforce](code-style.md#lints-that-mechanically-enforce-parts-of-this-convention).)

| Rule | Lint |
|------|------|
| Every public item has ≥ 1 line of docs | `missing_docs = "deny"` (rust) |
| Every intra-doc link resolves | `rustdoc::broken_intra_doc_links = "deny"` (rustdoc) |
| `# Errors` on every `Result`-returning pub fn | `clippy::missing_errors_doc` (via `pedantic`) |
| `# Panics` on every caller-precondition panic | `clippy::missing_panics_doc` (via `pedantic`) |
| Backtick `CamelCase` identifiers in prose | `clippy::doc_markdown` (via `pedantic`) |

`clippy::doc_markdown`'s heuristic ignores pure all-caps acronyms (`GPU`,
`JSON`, `URL`) — they need no backticks or allow-listing. The workspace
ships no `clippy.toml` `doc-valid-idents` list; add one (narrowest
possible) only if a genuine non-code-token false positive appears.

**Lints cannot check** summary-line tense, section order, or
`# Parameters` content quality — those are reviewer duties.

## DOC-7 — Doctest policy

- Doctests in `# Examples` **must compile**.
- Use a plain ` ``` ` fence for pure-library types that compile **and**
  run (most of `gp-core` — deterministic integer types with no runtime
  dependency).
- Use `` ```no_run `` **only** for an example that can compile but cannot
  run — e.g. one that needs a window / event loop (`gp-render`) or opens a
  file. `no_run` is for "compiles but must not run", never for "should not
  even compile."
- Include `# Examples` with a compiling doctest wherever a runnable example
  is meaningful; the plural heading form (`# Examples`) is used even for a
  single example.

## DOC-8 — Trait-impl exemption

- Methods inside `impl Trait for Type { … }` blocks are **exempt** from the
  full convention — they inherit docs from the trait definition. This
  covers both hand-written impls (`impl Display for Track`) and
  compiler-generated derives (`#[derive(Debug, Clone, Default, …)]`), and
  both std-lib traits (`From`, `Into`, `Drop`, `Display`, `Debug`,
  `Default`) and user-defined traits.
- The trait **definition** (`pub trait Foo { fn bar(…); }`) is **NOT**
  exempt: every method declared in a `pub trait` body carries the full
  convention (summary, `# Parameters`, conditional sections, `# Examples`).
- `#[doc(hidden)]` impls are out of scope entirely.

## DOC-9 — Design-doc citations

- **KEEP citing the design doc where a rule is load-bearing.** Cite the
  `docs/design.md §N` section in module/item docs so the invariant's source
  is one click away — e.g. the `supercover` contract cites `docs/design.md`
  §3, the reward invariant cites §5. This convention stays for now.
- **Pre-publish strip note (future, one-time).** graphite-gp is a game
  application and is not published to crates.io today. **Before the first
  ever `cargo publish`**, strip the `docs/design.md §N` / `ai-docs/…`
  repo-internal path citations from the rustdoc surface (they mean nothing
  to a docs.rs reader). This is a single pre-publish action, not a per-PR
  gate — do not enforce it as a review defect while the project stays
  unpublished.
- graphite-gp does **not** require the rustdoc surface to stand alone for a
  docs.rs reader while the project is unpublished: repo-internal citations
  are permitted in the pre-publish rustdoc surface, and no reviewer check
  treats a `docs/design.md §N` citation as a defect.

## Conforming example

```rust
/// Returns the cells a straight edge from `a` to `b` covers.
///
/// Implements the supercover predicate (`docs/design.md` §3): every grid
/// cell the segment touches, endpoints included.
///
/// # Parameters
///
/// - `a`: start cell, inclusive; grid coordinates.
/// - `b`: end cell, inclusive; grid coordinates.
///
/// # Returns
///
/// The covered cells in order from `a` to `b`.
///
/// # Examples
///
/// ```
/// use gp_core::geom::{supercover, Point};
///
/// let cells = supercover(Point::new(0, 0), Point::new(2, 0));
/// assert_eq!(cells.len(), 3);
/// ```
pub fn supercover(a: Point, b: Point) -> Vec<Point> {
    // …
}
```

## Non-conforming example (annotated)

```rust
/// Compute the covered cells.       // ← imperative; should be "Returns" / "Computes"
///
/// # Examples                       // ← Examples before Parameters: wrong order
/// // …
///
/// # Parameters
/// - a: start cell                  // ← `a` must be in backticks
pub fn supercover(a: Point, b: Point) -> Vec<Point> { /* … */ }
```

Fixes: third-person present summary with a terminal period; move
`# Parameters` above `# Examples`; backtick each parameter name.

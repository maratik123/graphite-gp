# Doc conventions — graphite-gp

Quick rules; grows via `/improve`.

- Every public item has at least a one-line `///`.
- `cargo doc` is a gate: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` must pass (broken intra-doc links denied).
- New public items with single-line docs get a `# Examples` block where a runnable example is meaningful.
- Cite the design-doc section in module/item docs where a rule is load-bearing (e.g. the `supercover` contract cites `docs/design.md` §3; the reward invariant cites §5).
- Link items with intra-doc links (`` [`Type`] ``) rather than bare names.

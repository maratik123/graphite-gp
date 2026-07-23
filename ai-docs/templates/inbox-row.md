# `_inbox.jsonl` — row template

> Canonical row shape for `ai-docs/deferred/_inbox.jsonl`. **Consult this template to inspect the format — do NOT open the live `ai-docs/deferred/_inbox.jsonl` to reverse-engineer it** (a fresh repo's inbox may be empty, showing no shape at all). This file is written **only** by `/task` Step 12 and `/triage` (AGENTS.md AXIOM — never hand-edited). Field-derivation rules (how `/task` computes each field from a spec): [`inbox-propagation.md`](../../.claude/skills/task/inbox-propagation.md). Drain / dedupe / thematic migration: [`triage-runner.md`](../../.claude/agents/triage-runner.md).

## Row shape — one JSON object per line (JSONL)

```
{"item": <string>, "source_label": <string>, "source_path": <string>, "section": <"out-of-scope"|"deferred"|"open-question">, "tracked": "—"}
```

**Field key**

- **`item`** — the deferred item text, stored verbatim as a JSON string (a literal `|` is the raw byte `|`; no markdown-table escaping).
- **`source_label`** — derived from the source filename: strip the `YYYY-MM-DD-` prefix and the `.spec.md` / `.design.md` suffix, append ` spec` or ` design` (e.g. `2026-07-13-core-supercover.spec.md` → `core-supercover spec`).
- **`source_path`** — `../plans/done/<filename>`, relative to `_inbox.jsonl`'s location in `ai-docs/deferred/`.
- **`section`** — one of three literal tokens: `## Out of scope` → `out-of-scope`; `## Deferred` → `deferred`; `## Open questions` → `open-question` (singular).
- **`tracked`** — the un-triaged marker: the literal em-dash string `"—"` until `/triage` promotes the row (then `#<N>`).

## Thematic variant — after `/triage` drains a row into a topic `.jsonl`

The `section` key is dropped and a `status` key is added:

```
{"item": <string>, "source_label": <string>, "source_path": <string>, "status": "", "tracked": "—"}
```

## Example

```
{"item": "`Corridor::contains` / the `D` membership machinery — already present in `geom.rs`.", "source_label": "core-supercover spec", "source_path": "../plans/done/2026-07-13-core-supercover.spec.md", "section": "out-of-scope", "tracked": "—"}
```

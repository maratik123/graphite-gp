# Inbox propagation — parser rules and per-row mapping

Reference for `.claude/skills/task/SKILL.md` Step 12 sub-step 5. Walks one or more spec/design files, locates each of the three target headings (`## Out of scope` / `## Deferred` / `## Open questions`, exact h2 match anchored on `^## <Heading>$`), classifies the body shape using the six ordered rules below, and emits one JSON line to `ai-docs/deferred/_inbox.jsonl` per parsed item. The rules are ordered — the first match wins; rule ordering matters (pipe-bullet and bold-bullet shapes take precedence over plain bullets). For mixed-shape bodies (rare; a single section containing two shapes), classify per line using the same ordering — every line goes through the same six rules.

**Shape rule 1 — NONE sentinel.** Body matches one of: `_None._` / `None.` / `None`; `(none — …)` / `(none -- …)` / `_(none)_`; `None — …` / `None - …` (the word `None` followed by an em-dash or hyphen and prose); `None at spec time.` / `None blocking …`. Matcher: collapse whitespace, lower-case, strip wrapping `_` and `()`, then test `^none\b.*$` with no `- ` bullet lines and no `|` table pipes. **Emit zero rows; emit zero warnings.** The only silent case.

**Shape rule 2 — TABLE.** Section body's first non-blank line starts with `|` and is followed by a `|---|...|` separator line. Per-row extraction: skip header + separator; for each data row `| C1 | C2 | C3? | ... |`, `Item` = `C1` (verbatim, including markdown formatting); if C2 is non-empty, append ` — <C2>` to `Item` (carries rationale into the inbox row). Subsequent columns are ignored. Terminate row collection on first blank line or first `^## ` heading.

**Shape rule 3 — PIPEBULLET3.** Bullet lines match `^- (.+?) \| (.+?) \| (.+?)$` — three `|`-separated fields. Per-row extraction: `Item` = field 1 + ` — ` + field 2 (the third field is metadata such as "Separate issue needed?" and is dropped).

**Shape rule 4 — PIPEBULLET2.** Bullet lines match `^- (.+?) \| (.+?)$` — two `|`-separated fields. Per-row extraction: `Item` = field 1 + ` — ` + field 2.

**Shape rule 5 — BOLDBULLET.** Bullet lines match `^- \*\*([^*]+)\*\*\s*[—-]\s*(.+)$` — a bolded leading term followed by an em-dash or hyphen and prose. Per-row extraction: `Item` = `**<term>**` + ` — ` + `<prose>` (the leading bold survives into the inbox so the term/explanation split is preserved).

**Shape rule 6 — PLAINBULLET.** Bullet lines match `^- (.+)$` and do NOT match rules 3 / 4 / 5 above. Per-row extraction: `Item` = the entire bullet text, verbatim. **Multi-line continuation:** lines starting with 2+ spaces of indentation and no leading `- ` are joined into the preceding bullet with a single space — common in wrapped real-corpus bullets.

**Unrecognised shape.** A section with a non-blank body that matches none of the six rules emits one stdout warning and zero rows:

```
WARN: <spec-path> :: <section heading> — unrecognised body shape, no rows emitted
```

Step 12 continues normally; `_inbox.jsonl` is unchanged for that section.

**Per-row mapping.** Each parsed item becomes one JSON object appended as a single line to `ai-docs/deferred/_inbox.jsonl`. The canonical row shape + field key + example live in [`ai-docs/templates/inbox-row.md`](../../../ai-docs/templates/inbox-row.md) (the `_inbox` row carries `section`, not `status` — see the design § Approach (c) key set). The `/task`-specific per-field derivation:

- **`item`** — value produced by the matched shape rule, stored verbatim as a JSON string. A literal `|` is stored as the raw byte `|` (JSON has no pipe-escaping; the `\|` markdown-table escaping is NOT applied here — that was a markdown-table artefact, gone with the table sink).
- **`source_label`** — derived from the source filename: strip the `YYYY-MM-DD-` date prefix and the `.spec.md` / `.design.md` suffix; append ` spec` or ` design` accordingly. Examples: `2026-05-09-paint-style.spec.md` → `paint-style spec`; `2026-05-01-auto-connection.design.md` → `auto-connection design`.
- **`source_path`** — `../plans/done/<filename>` (relative to `_inbox.jsonl`'s location in `ai-docs/deferred/`).
- **`section`** — one of three literal tokens: `## Out of scope` → `out-of-scope`; `## Deferred` → `deferred`; `## Open questions` → `open-question` (singular `open-question`, NOT `open-questions`, matches the AGENTS.md AXIOM table cell text).
- **`tracked`** — the literal em-dash string `"—"`, identical to the un-triaged marker used in the thematic files' `tracked` field. Un-triaged invariant honoured.

Append form (`/task` is NOT a triage instruction, so the no-`>` rule does not apply here — see design Risk R5): compose the one-line object and append it, e.g. `printf '%s\n' "$line" >> ai-docs/deferred/_inbox.jsonl`.

**Dedupe rule (file-level).** Before running the parser, build the set `H` = every `.source_path` value appearing in the thematic `.jsonl` files in `ai-docs/deferred/` — every `*.jsonl` sibling of `_inbox.jsonl` (these are created by `/triage` as it drains the inbox into topic files; none may exist yet in a fresh repo, in which case `H` is empty) — harvested via `jq -r '.source_path' <file>.jsonl | sort -u`. Normalisation: strip trailing `/`, strip `#...` anchor fragments, strip leading `./`, strip leading/trailing whitespace; preserve case. For each candidate row, if its `source_path` is in `H`, skip the **entire file** (all of its sections) — file-level dedupe avoids re-harvesting any portion of a file whose other sections were already drained into thematic files by earlier triage passes.

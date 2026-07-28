#!/usr/bin/env bash
#
# Citation-namespace guard for issue #77.
#
# INVARIANT: every citation in the harness must resolve for its reader.
#   A bare `#N` / `PR #N` is repo-relative -> must resolve in THIS repo.
#   If the referent lives in the sibling project (quartzite, whose harness
#   was imported wholesale by commit 9077bfb), the citation MUST name it
#   (`maratik123/quartzite#N`, "quartzite's `ai-docs/learnings.md` <date>",
#   or a full path under quartzite's memory namespace).
#
# Run by `/ai-audit` Phase 2 (checklist item P) against the full instruction
# surface. Also runnable standalone: `bash .claude/skills/ai-audit/scripts/check-citations.sh`.
#
# On FAIL: qualify the offending citation with its namespace. NEVER drop a
# citation to make this pass -- dropping destroys verified history and is
# the bug this guard exists to catch, not a valid fix.
#
# Exclusions kept from the original red test (issue #77 Step 3):
#   - ai-docs/learnings.md itself (append-only local log; not a citation site)
#   - ai-docs/bugfix/** (deleted-on-resolution trace files, not durable)
#   - illustrative example filenames matching YYYY-MM-DD-slug.ext (no authority
#     claim -- e.g. a sample spec filename in a template or table row)
#   - already-namespace-qualified forms (contain "quartzite")
# One additional targeted exclusion added when this guard was promoted:
#   - corrections-log.md's `Superseded by:` field-spec row is FORBIDDEN to
#     touch (see issue #77's resolution) -- it is a format-spec example
#     illustrating that field's date-ref syntax, not a citation with a real
#     referent. Excluded by matching the ROW'S OWN TEXT within that one file,
#     so any *other* date citation later added to corrections-log.md still
#     gets checked.
#     NOT by line number: this exclusion was originally pinned to
#     `corrections-log.md:47`, an unrelated commit inserted rows above it, and
#     the pin silently re-pointed at a dateless neighbour -- un-excluding the
#     example (guard RED on a clean tree) while appearing to still work.
#     Locked by case 2 of scripts/test-check-citations.sh.
#
# SCOPE -- what this guard does NOT cover, and why. State this openly: a
# silent exclusion reads as "covered everything" when it did not.
#   - ai-docs/plans/**  : frozen historical records. A merged spec/design
#     documents what was decided AT THE TIME; rewriting its references
#     edits history rather than an instruction. Issue #77's inventory
#     listed no site here. KNOWN LIVE REFS: 7 unqualified occurrences across
#     6 lines (spec.md carries 2 lines, one of them doubled; design.md 4),
#     all naming maratik123/quartzite#340 ("ci: macOS Clippy spuriously
#     fails"), in 2026-07-14-import-ci-workflows.{spec,design}.md -- real,
#     accurate, unqualified. A follow-up may qualify them; this guard will
#     not. Counted, not estimated: occurrences != lines, and the earlier
#     "6" here conflated the two.
#   - ai-docs/deferred/**: `_inbox.jsonl` is AXIOM-protected -- AGENTS.md
#     § Workflow: "written ONLY by /task Step 12 and /triage. Hand-edits
#     defeat the propagation contract." 2 more maratik123/quartzite#340 rows.
#     A guard that fails on a file nobody may hand-edit is a guard that
#     teaches people to ignore it.
set -uo pipefail

LOCAL_MAX=$(gh pr list --state all --limit 1 --json number --jq '.[0].number')
fail=0

echo "== local high-water mark: PR #${LOCAL_MAX} =="
echo
echo "--- (1) unqualified 'PR #N' / bare '#N' claiming to be local but exceeding it ---"
# Matches BOTH `PR #N` and a bare `#N`. Bare `#N` is the same defect: GitHub
# resolves it repo-relative, so an imported bare ref silently rebinds. A
# `PR #N`-only regex is blind to it -- that blind spot shipped once already
# (self-review Round 1), because the sweep inherited AC3's `PR #N` command
# and inherited its aim. Match the DEFECT, not the phrasing that first
# surfaced it.
#
# EXAMPLE sites carry no authority claim and MUST stay green -- see (a)/(b)
# below. `#N <= LOCAL_MAX` is skipped too: it may be a real local ref.
# Hex colours need TWO defences, not one -- see (c). Do not "simplify" that
# to `\b` alone; it looks sufficient and is not.
while IFS=: read -r file line cite; do
  n=${cite##*#}
  [ "$n" -le "$LOCAL_MAX" ] 2>/dev/null && continue
  txt=$(sed -n "${line}p" "$file")
  # CONTEXT-QUALIFIED: the line already names the sibling namespace, so a bare
  # `#N` beside it inherits that scope and a reader can resolve it. Required for
  # quoted PR-body syntax (`Closes #289` on maratik123/quartzite#295) and for
  # run-on refs (`maratik123/quartzite#168`, `#169`) -- rewriting either to
  # spell the qualifier per-number would falsify the quote or bloat the prose.
  case "$txt" in *maratik123/quartzite*) continue ;; esac
  # EXAMPLE shapes carry no authority claim. Two kinds, handled differently:
  #
  # (a) TEMPLATE FIELD lines -- ANCHORED to the line start, so the key must BE
  #     the field, not merely appear in prose. Unanchored, `detail:` matched
  #     mid-sentence and silently swallowed real citations: any prose line
  #     opening "In detail: ..." and then citing a high-numbered ref was
  #     skipped. That is the substring-blacklist trap issue #77 warned about;
  #     anchoring closes it.
  echo "$txt" | grep -qE '^[[:space:]]*[-*]?[[:space:]]*(issue_ref|linked_prs|tracked_in|detail):' && continue
  # (b) PROSE specimens -- two lines quote an illustrative `#N` inside example
  #     text, where no field key exists to anchor to. Excluded by exact
  #     file:line rather than by a phrase, so a REAL citation later added to
  #     either file still gets checked. NOTE: check (2) deliberately does NOT
  #     use this mechanism -- its line pin drifted and broke (see the header).
  #     These two are NOT safer by nature; they are unrepaired for two
  #     different accidental reasons, neither of which is stability:
  #       - spec-writer.md's pin is INERT. Its specimen ref is below the live
  #         high-water mark, so the LOCAL_MAX test above `continue`s and
  #         execution never reaches this `case`. Its file HAS been edited since
  #         the pin was written; the pin survived only because that edit was
  #         line-count-neutral -- luck, not stability.
  #       - task/reference.md's pin is the only live one, and survives only
  #         because nothing has yet been inserted above it.
  #     Content-address either one the moment it drifts, or preferably before;
  #     do NOT re-pin. Same failure mode as the header's:
  #       spec-writer.md:161    -- a `detail`-field shape demo, quoting a
  #                                specimen ref inside example prose
  #       task/reference.md:179 -- an entry_args format demo, showing the
  #                                bare-vs-plain issue-ref argument forms
  #     (Both are described, not spelled: a comment that quotes the bad shape
  #     IS the bad shape, and this script would flag its own source. See
  #     reference.md Checklist P -- "describe the bad shape; spell it only
  #     alongside its fix." That rule applies here too.)
  case "$file:$line" in
    .claude/agents/spec-writer.md:161|.claude/skills/task/reference.md:179) continue ;;
  esac
  # (c) HEX COLOUR shape. `\b` alone is NOT sufficient: it excludes #93A2B8
  #     (letters break the digit run) but an ALL-NUMERIC hex like #123456 or
  #     #000000 DOES match it. Exclude the 6-digit shape explicitly -- no
  #     plausible issue/PR number here is 6 digits (local max 78; quartzite
  #     max 628). Verified, not assumed: an earlier comment here claimed \b
  #     made hex unmatchable, which is false.
  case "$n" in [0-9][0-9][0-9][0-9][0-9][0-9]) continue ;; esac
  printf '  RED  %s:%s  -> %s (local max %s; does not resolve here)\n' "$file" "$line" "$cite" "$LOCAL_MAX"
  fail=$((fail + 1))
done < <(grep -rnoE '(^|[^a-zA-Z0-9/_-])#[0-9]+\b' .claude/ AGENTS.md ai-docs/ 2>/dev/null \
           | grep -v learnings.md | grep -v '^ai-docs/bugfix/' \
           | grep -v '^ai-docs/plans/' | grep -v '^ai-docs/deferred/' \
           | sed -E 's/:([^:]*)(#[0-9]+)$/:\2/')

echo
echo "--- (2) 'learnings.md <date>' citations outside this log's range ---"
# This repo's log starts 2026-07-13. A 2026-0[1-6] date cannot be a local entry.
# CITATION only: require a 'see|entry|validated|recurrence|added' cue on the
# line, so illustrative example FILENAMES (2026-05-01-paint-style.spec.md) do
# not fire.
while IFS=: read -r file line _; do
  txt=$(sed -n "${line}p" "$file")
  # Targeted exclusion: the `Superseded by:` field-spec row illustrates the
  # same-date disambiguation syntax — `YYYY-MM-DD ("slug")` — so its date is a
  # format example, not a citation.
  # KEEP THIS COMMENT FREE OF LITERAL IN-RANGE DATES AND CUE WORDS: this file
  # is itself scanned by check (2) below, so writing the shape concretely here
  # would make the guard flag its own source.
  # Addressed by CONTENT, never by line number: this exclusion was previously
  # pinned to `corrections-log.md:47`, and an unrelated commit inserting rows
  # above it moved the row to :49, which both un-excluded the example (RED on a
  # clean tree) and silently re-pointed the pin at a dateless neighbour.
  # Locked by case 2 of test-check-citations.sh.
  # shellcheck disable=SC2016  # literal backticks are the pattern, not an expansion
  [ "$file" = "ai-docs/corrections-log.md" ] &&
    printf '%s' "$txt" | grep -q '^> `Superseded by:`' && continue
  echo "$txt" | grep -qiE 'see |entry|validated|recurrence|added ' || continue
  echo "$txt" | grep -qiE 'quartzite' && continue     # already qualified
  echo "$txt" | grep -qE '[0-9]{4}-[0-9]{2}-[0-9]{2}-[a-z]' && continue  # example filename
  printf '  RED  %s:%s  -> cites a learnings.md date this log never had\n' "$file" "$line"
  fail=$((fail + 1))
done < <(grep -rnoE '2026-0[1-6]-[0-9]{2}' .claude/ AGENTS.md ai-docs/ 2>/dev/null | grep -v learnings.md | grep -v '^ai-docs/bugfix/')

echo
echo "--- (3) 'feedback_*.md' cited without its owning namespace ---"
while IFS=: read -r file line _; do
  txt=$(sed -n "${line}p" "$file")
  echo "$txt" | grep -q 'projects/-home-syt-RustroverProjects-quartzite' && continue
  printf '  RED  %s:%s  -> cites a memory file without naming whose namespace holds it\n' "$file" "$line"
  fail=$((fail + 1))
done < <(grep -rnoE 'feedback_[a-z_]+\.md' .claude/ AGENTS.md 2>/dev/null)

echo
if [ "$fail" -gt 0 ]; then
  echo "FAIL: ${fail} unresolvable citation(s). Each says 'here' but means 'quartzite'."
  exit 1
fi
echo "PASS: every citation resolves for its reader."

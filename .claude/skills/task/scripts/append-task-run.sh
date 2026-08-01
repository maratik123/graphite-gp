#!/usr/bin/env bash
# Append one task-run telemetry record to ai-docs/metrics/task-runs.jsonl.
#
# WHAT THIS IS FOR
#   /task Step 12 sub-step 5a is the SINGLE writer of the task-run corpus. This
#   script derives one JSON object from the run's progress file plus ambient git
#   facts, and appends it as one line. The corpus exists so an /improve
#   escalation into the instruction corpus can later be shown to have moved, or
#   not moved, the per-task review cost.
#
# WHO RUNS IT
#   /task Step 12 sub-step 5a, once per completed run, before the staging
#   sub-step so the append reaches the PR diff. Nothing else writes this file —
#   not /pr-commented, not /pr-ci-failed, not /main-ci-failed, not
#   /project-review, not /bugfix.
#
# USAGE
#   .claude/skills/task/scripts/append-task-run.sh <progress-file> [<target-jsonl>]
#
#   <target-jsonl> defaults to ai-docs/metrics/task-runs.jsonl and exists so the
#   fixture test can point the append at a sandbox.
#
# EXIT CODES — a PARSE PROBLEM IS NEVER AN ERROR
#   0  appended, full OR degraded (degraded records carry "incomplete": true)
#   2  usage error (no progress-file argument)
#   3  jq not on $PATH
#   4  jq could not compose the record
#   5  could not append to the target
#   Only "cannot append at all" is non-zero. On non-zero, /task Step 12 writes
#   the nine fallback-required fields by hand and continues; under no path does
#   Step 12 halt on this sub-step.
#
# WHAT THIS DELIBERATELY DOES NOT COVER
#   - Planning cost. `rounds` counts /task Step 10 self-review rounds ONLY;
#     /interview, design and design-review rounds appear nowhere in the record.
#   - Process and handoff soundness. No field encodes handoff state, so a run
#     whose durable state was never maintained emits a normal-looking line.
#   - Anything after Step 12. Reviewer and CI-fix rounds extend the progress
#     file after this record is written.
#   - The `## Decisions log`. Not parsed in v1, so spec churn during
#     implementation and reopened subtasks are invisible here.
#   The full list, written as open questions, is on ai-docs/task-run-schema.md
#   § "What this log does NOT measure". That page is the schema's home; this
#   header does not restate the field table.
#
# COUNTING UNITS — see the schema page before comparing two records. `findings`,
# `objections` and `objections_reopened` are summed across all rounds and
# therefore inflate with `rounds`; `findings_first_seen` counts only rows absent
# from the immediately preceding round, keyed on the `File:line` cell verbatim.
# That key drifts whenever a Step-11 fix shifts line numbers, which is the
# expected case when several findings share a file. The schema page carries the
# degeneracy signature that tells a measured run from a collapsed one, and the
# coupling clause that binds the field to it.

set -uo pipefail

CORPUS_DEFAULT="ai-docs/metrics/task-runs.jsonl"

die() { printf 'append-task-run: %s\n' "$1" >&2; exit "$2"; }

[ $# -ge 1 ] || die "usage: append-task-run.sh <progress-file> [<target-jsonl>]" 2
command -v jq >/dev/null 2>&1 || die "jq not found on \$PATH; cannot compose a record" 3

pf=$1
target=${2:-$CORPUS_DEFAULT}

incomplete=false
degrade() { incomplete=true; }

# --- Ambient facts -----------------------------------------------------------

rec_date=$(date -u +%F)
branch=$(git branch --show-current 2>/dev/null)
# Detached HEAD, or not a git work tree, yields "". `branch` is
# fallback-required and consumers key the last-line-wins dedup on it (schema
# page § Append-only + last-line-wins), so a silently empty key is worse than
# a flagged one.
[ -n "$branch" ] || degrade

spec_base=$(basename "$pf")
spec_base=${spec_base%.progress.md}

# --- Header fields -----------------------------------------------------------

issue_json=null
base=""

if [ -r "$pf" ]; then
  issue=$(sed -nE 's/^\*\*Issue:\*\*[[:space:]]*#([0-9]+).*$/\1/p' "$pf" | head -1)
  if [ -n "$issue" ]; then
    issue_json=$issue
  else
    # Absent, or the canonical template's URL form rather than #N. `null` is
    # honest and total; a URL regex would invent a number from any path segment
    # that happens to be digits.
    degrade
  fi
  base=$(sed -nE 's/^\*\*base_commit:\*\*[[:space:]]*([^[:space:]]+).*$/\1/p' "$pf" | head -1)
else
  degrade
fi

# --- The diff-size trio ------------------------------------------------------
# Two-source base rule: prefer the progress file's **base_commit:**, fall back to
# `git merge-base main HEAD` (which needs no progress file) and flag the record,
# so the looser base is never passed off as the precise one. Both unobtainable
# -> 0/0/0, which is indistinguishable from a genuine no-change diff, so
# `incomplete` is what carries the difference.

if [ -n "$base" ] && git rev-parse --verify --quiet "${base}^{commit}" >/dev/null 2>&1; then
  :   # the precise base, read from the progress file
else
  # **base_commit:** absent or unparseable -> the looser base, always flagged.
  # If that is unobtainable too (no `main` ref, detached HEAD, shallow clone,
  # not a git work tree) the trio stays 0/0/0 and the record is already flagged.
  base=$(git merge-base main HEAD 2>/dev/null)
  degrade
fi

files_changed=0
insertions=0
deletions=0

if [ -n "$base" ]; then
  shortstat=$(git diff --shortstat "${base}..HEAD" 2>/dev/null)
  # Parse BY KEYWORD, never positionally: a deletions-only diff omits the
  # insertions clause entirely, a single-insertion diff omits the deletions
  # clause, both nouns singularise at 1, and no change at all yields empty
  # output. Each clause defaults to 0, which also makes the empty case fall out
  # as 0/0/0 with no special case.
  files_changed=$(printf '%s' "$shortstat" | sed -nE 's/^[^0-9]*([0-9]+) files? changed.*/\1/p')
  insertions=$(printf '%s' "$shortstat" | sed -nE 's/.*[,[:space:]]([0-9]+) insertions?\(\+\).*/\1/p')
  deletions=$(printf '%s' "$shortstat" | sed -nE 's/.*[,[:space:]]([0-9]+) deletions?\(-\).*/\1/p')
  : "${files_changed:=0}" "${insertions:=0}" "${deletions:=0}"
fi

# --- Progress-file body ------------------------------------------------------

optional_json='{}'

if [ -r "$pf" ]; then
  parsed=$(
    awk '
      /^## / {
        if ($0 ~ /^## Self-Review \(Round [0-9]+\)[[:space:]]*$/) {
          rounds++; insec = 1; havev[rounds] = 0; next
        }
        insec = 0; next
      }
      insec && /^\*\*Verdict:\*\*/ {
        if (!havev[rounds]) {
          v = $0
          sub(/^\*\*Verdict:\*\*[[:space:]]*/, "", v)
          gsub(/[[:space:]]+$/, "", v)
          verdict[rounds] = v; havev[rounds] = 1
        }
        next
      }
      insec && /^\|[[:space:]]*[0-9]/ {
        # MASK ESCAPED PIPES BEFORE SPLITTING. `\|` is a cell CONTENT
        # character, not a delimiter, and it may appear in ANY cell -- the
        # Finding cell, the free-text tail of `⚠️ Objected: <reason>`, or a
        # path. Masking removes the column shift at its source, so no later
        # step has to compensate for it at some particular index.
        #
        # This is the third form of this fix and the first named after the
        # INPUT rather than after the cell where the shift was last seen: a
        # fixed index (c[6]) broke on a shifted Finding cell; "last cell"
        # broke on a `\|` inside the Status cell itself. Both were correct
        # about the instance they were written for and wrong about the class.
        row = $0
        gsub(/\\\|/, "\002", row)
        n = split(row, c, "|")
        # Status is the LAST cell -- but GFM makes a row trailing pipe
        # OPTIONAL and the matcher above requires only the leading one, so
        # `split` leaves c[n] empty on a trailing-pipe row and c[n] IS Status
        # without one. Take c[n] unless blank. This is a SEPARATE property
        # from the masking above (row shape, not cell content); case 18
        # guards it and case 19 guards the mask.
        key = c[3]; sev = c[4]
        stat = (c[n] ~ /^[[:space:]]*$/ ? c[n - 1] : c[n])
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", key)
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", sev)
        # Restore the masked delimiters so extracted values are faithful.
        gsub(/\002/, "\\|", key)
        gsub(/\002/, "\\|", sev)
        gsub(/\002/, "\\|", stat)
        printf "ROW\t%d\t%s\t%s\n", rounds, key, sev
        if (index(stat, "⚠️ Objected")) obj++
        if (index(stat, "🔁 Re-opened")) reop++
        if (sev != "blocker" && sev != "major" && sev != "minor" && sev != "nit") badsev++
      }
      END {
        printf "ROUNDS\t%d\n", rounds + 0
        for (i = 1; i <= rounds; i++) printf "VERDICT\t%s\n", (havev[i] ? verdict[i] : "")
        printf "OBJ\t%d\n", obj + 0
        printf "REOP\t%d\n", reop + 0
        printf "BADSEV\t%d\n", badsev + 0
      }
    ' "$pf"
  )

  rounds=$(printf '%s\n' "$parsed" | sed -nE 's/^ROUNDS\t([0-9]+)$/\1/p')
  objections=$(printf '%s\n' "$parsed" | sed -nE 's/^OBJ\t([0-9]+)$/\1/p')
  objections_reopened=$(printf '%s\n' "$parsed" | sed -nE 's/^REOP\t([0-9]+)$/\1/p')
  badsev=$(printf '%s\n' "$parsed" | sed -nE 's/^BADSEV\t([0-9]+)$/\1/p')
  : "${rounds:=0}" "${objections:=0}" "${objections_reopened:=0}" "${badsev:=0}"

  [ "$rounds" -gt 0 ] || degrade

  # An unbucketed severity cell drops its row out of both `findings` and
  # `findings_first_seen` silently — flag the record so a reader can tell a
  # degraded run from a genuinely small one.
  [ "$badsev" -eq 0 ] || degrade

  # Verdict tokens: anything that is neither APPROVE nor REJECT — including a
  # section with no **Verdict:** line at all — becomes "UNKNOWN".
  verdicts_json=$(
    printf '%s\n' "$parsed" \
      | sed -nE 's/^VERDICT\t(.*)$/\1/p' \
      | awk '{ print ($0 == "APPROVE" || $0 == "REJECT") ? $0 : "UNKNOWN" }' \
      | jq -Rsc 'split("\n") | map(select(length > 0))'
  )

  # Any UNKNOWN verdict token (absent line, or neither APPROVE nor REJECT)
  # means at least one round's verdict was not read cleanly.
  printf '%s' "$verdicts_json" | jq -e 'index("UNKNOWN")' >/dev/null 2>&1 && degrade

  # `findings` sums every bounded row across all rounds; `findings_first_seen`
  # counts only rows whose File:line cell is absent from the IMMEDIATELY
  # PRECEDING round's key set (round 1 contributes all of its rows). A row whose
  # severity cell is not one of the four buckets contributes to neither.
  counts=$(
    printf '%s\n' "$parsed" | awk -F'\t' '
      BEGIN { split("blocker major minor nit", b, " ") }
      $1 == "ROW" {
        r = $2 + 0; key = $3; sev = $4
        if (sev == "blocker" || sev == "major" || sev == "minor" || sev == "nit") {
          all[sev]++
          if (r == 1 || !(((r - 1) SUBSEP key) in seen)) first[sev]++
        }
        seen[r SUBSEP key] = 1
      }
      END {
        for (i = 1; i <= 4; i++) printf "%s %d %d\n", b[i], all[b[i]] + 0, first[b[i]] + 0
      }
    '
  )
  findings_json=$(printf '%s\n' "$counts" | jq -Rsc '
    split("\n") | map(select(length > 0) | split(" ")) | map({(.[0]): (.[1] | tonumber)}) | add')
  first_seen_json=$(printf '%s\n' "$counts" | jq -Rsc '
    split("\n") | map(select(length > 0) | split(" ")) | map({(.[0]): (.[2] | tonumber)}) | add')

  # /task Step 10 caps the loop at 3 rounds and a rejected round 3 takes the
  # forced-surface path, so this is what separates "closed on round 2" from
  # "ran out of rounds".
  hit_round_cap=false
  if [ "$rounds" -ge 3 ] && [ "$(printf '%s' "$verdicts_json" | jq -r '.[2] // ""')" = "REJECT" ]; then
    hit_round_cap=true
  fi

  files_touched_json=null
  if grep -qE '^## Files touched[[:space:]]*$' "$pf"; then
    # The canonical line shape is ``- `path` — what changed``; path only, the
    # description is dropped.
    # shellcheck disable=SC2016  # the backticks are markdown delimiters inside a
    # sed regex, not command substitution — single quotes are what keeps them so
    files_touched_json=$(
      awk '/^## Files touched[[:space:]]*$/ {f = 1; next} f && /^## / {exit} f' "$pf" \
        | sed -nE 's/^-[[:space:]]+`([^`]+)`.*$/\1/p' \
        | jq -Rsc 'split("\n") | map(select(length > 0))'
    )
  else
    degrade
  fi

  # The pinned corpus command, :(exclude) term INCLUDED. A pre-exclusion form
  # would put every record on a superseded, non-comparable basis. A real corpus
  # is never 0 lines, so 0 means the command could not reach it (not a git work
  # tree, or run outside the repo) -> omit the field and flag the record.
  corpus=$(git ls-files -z -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md' \
    ':(exclude)ai-docs/learnings.md' 2>/dev/null \
    | xargs -0 cat 2>/dev/null | wc -l | tr -d ' ')
  corpus_json=null
  case "$corpus" in
    ''|0|*[!0-9]*) degrade ;;
    *) corpus_json=$corpus ;;
  esac

  optional_json=$(
    jq -cn \
      --argjson rounds "$rounds" \
      --argjson hit "$hit_round_cap" \
      --argjson verdicts "$verdicts_json" \
      --argjson findings "$findings_json" \
      --argjson first_seen "$first_seen_json" \
      --argjson objections "$objections" \
      --argjson reopened "$objections_reopened" \
      --argjson files_touched "$files_touched_json" \
      --argjson corpus "$corpus_json" \
      '{rounds: $rounds, hit_round_cap: $hit, verdicts: $verdicts,
        findings: $findings, findings_first_seen: $first_seen,
        objections: $objections, objections_reopened: $reopened}
       + (if $files_touched == null then {} else {files_touched: $files_touched} end)
       + (if $corpus == null then {} else {instruction_corpus_lines: $corpus} end)'
  ) || die "jq failed to compose the optional field set" 4
fi

# --- Compose -----------------------------------------------------------------

record=$(
  jq -cn \
    --arg date "$rec_date" \
    --arg branch "$branch" \
    --argjson issue "$issue_json" \
    --arg spec_base "$spec_base" \
    --argjson incomplete "$incomplete" \
    --argjson files_changed "$files_changed" \
    --argjson insertions "$insertions" \
    --argjson deletions "$deletions" \
    --argjson optional "$optional_json" \
    '{schema_version: 1, date: $date, branch: $branch, issue: $issue,
      spec_base: $spec_base, incomplete: $incomplete,
      files_changed: $files_changed, insertions: $insertions,
      deletions: $deletions} + $optional'
) || die "jq failed to compose the record" 4

[ -n "$record" ] || die "jq produced an empty record" 4

printf '%s\n' "$record" >> "$target" \
  || die "could not append to '$target'" 5

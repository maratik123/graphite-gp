#!/usr/bin/env bash
# Fixture test for append-task-run.sh.
#
# Locks the extractor's contract for the task-run telemetry corpus
# (ai-docs/metrics/task-runs.jsonl). See ai-docs/task-run-schema.md for the
# schema those cases assert against.
#
# The case count is NOT written here. It is derived at the end of the run and
# asserted equal to the design's § Cases row count (that assertion IS AC6), and
# the closing banner prints the derived number. This header previously said
# "Eighteen cases" and was stale within one round -- a transcribed count in a
# comment is a claim nothing checks, which is the same defect AC6 exists to
# catch one layer down.
#
# What this test deliberately does NOT cover:
#   - the /task Step 12 sub-step 5a integration itself. No harness can execute a
#     skill sub-step; its coverage is the AC2 ordering grep plus the Step-12
#     verification block on the schema page.
#   - `instruction_corpus_lines` as a VALUE. It is environment-dependent (it
#     counts the live instruction corpus), so cases assert only that it is
#     present-and-integer where that is meaningful, never a fixed number.
#   - `date` / `branch` / the diff-size trio in case 1, for the same reason. The
#     trio gets its own purpose-built sandbox repo in case 12.
#
# FIXTURE STRATEGY — deliberately different from test-check-citations.sh, which
# mutates a tracked file under a trap triad. The entry point here takes explicit
# path arguments, so every fixture and every append target lives inside a
# `mktemp -d` sandbox and ZERO tracked files are touched. Case 13 asserts that
# property rather than assuming it.
#
# Fixture issue numbers are `#42`, never the real one: check-citations.sh skips
# any `#N` at or below the local PR high-water mark, so a low number keeps this
# file green under that guard.
#
# Usage: bash .claude/skills/task/scripts/test-append-task-run.sh
# Exit 0 = all cases pass. Exit 1 = regression.

set -uo pipefail

repo_root=$(git rev-parse --show-toplevel) || exit 1
cd "$repo_root" || exit 1

script=".claude/skills/task/scripts/append-task-run.sh"
schema="ai-docs/task-run-schema.md"
failures=0

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

status_before=$(git status --porcelain)

# Every assertion label begins `case <N>[<suffix>]:`. Recording N here makes the
# set of cases that ACTUALLY EXECUTED observable at runtime, which is what AC6
# compares against the design. Counting `# --- Case N` comments instead would
# count the inventory rather than the run: deleting a case body while leaving
# its banner comment would still read as present.
case_ids=""
_note_case() {
  case "$1" in
    case\ [0-9]*)
      _n=${1#case }
      _n=${_n%%[!0-9]*}
      case_ids="$case_ids $_n"
      ;;
  esac
}

pass() { _note_case "$1"; printf '  PASS  %s\n' "$1"; }
fail() {
  _note_case "$1"
  printf '  FAIL  %s\n' "$1"
  [ $# -gt 1 ] && printf '        %s\n' "$2"
  failures=$((failures + 1))
}

# $1 = label, $2 = observed exit, $3 = expected exit
assert_exit() {
  if [ "$2" -eq "$3" ]; then pass "$1 (exit $2)"; else fail "$1" "expected exit $3, got $2"; fi
}

# $1 = label, $2 = JSON line, $3 = jq filter that must yield true
assert_jq() {
  if printf '%s' "$2" | jq -e "$3" >/dev/null 2>&1; then
    pass "$1"
  else
    fail "$1" "filter [$3] did not hold for: $2"
  fi
}

# $1 = label, $2 = observed, $3 = expected
assert_eq() {
  if [ "$2" = "$3" ]; then pass "$1"; else fail "$1" "expected [$3], got [$2]"; fi
}

echo "== test-append-task-run =="
echo

# --- Fixtures ----------------------------------------------------------------

# A commit that certainly exists in THIS repo, so F1/F5's `**base_commit:**`
# resolves and the diff-size trio is computed from a real base rather than
# degrading (which would set `incomplete` and break case 1's `incomplete==false`).
real_base=$(git rev-parse HEAD)

f1="$tmp/f1.progress.md"
cat > "$f1" <<EOF
# Progress: fixture — ACTIVE
_Updated: 2026-07-31 00:00_

**Branch:** feat/fixture
**base_commit:** ${real_base}
**Issue:** #42
**Spec:** ai-docs/plans/fixture.spec.md

**current_step:** Step 12
**last_passed_gate:** (none)

## Files touched

- \`crates/gp-core/src/a.rs\` — added a thing
- \`crates/gp-core/src/b.rs\` — changed a thing
- \`crates/gp-core/src/c.rs\` — removed a thing

## Decisions log

- **Step 11**: accepted the ⚠️ Objected rationale on src/b.rs:20; later 🔁 Re-opened in Round 2

## AC Status

| AC | Status |
|----|--------|
| AC1 | PASS |

## Self-Review (Round 1)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:10 | blocker | Description | ✅ Fixed |
| 2 | src/b.rs:20 | major | Description | ⚠️ Objected: out of scope |
| 3 | src/c.rs:30 | major | Description | ✅ Fixed |
| 4 | src/d.rs:40 | nit | Description | ✅ Fixed |
# This row is the Step-11 fix whose line delta shifts src/g.rs:70 -> :73 between
# R1 and R2. It exists to instantiate key drift. Deleting it makes case 14 pass
# for the wrong reason and removes the only scenario findings_first_seen
# measures. If case 14 is red, the defect is in the parser or the gate -
# not in this row. See ai-docs/task-run-schema.md.
| 5 | src/g.rs:15 | minor | Description | ✅ Fixed |
| 6 | src/g.rs:70 | minor | Missing doc comment | ⬜ Open |

## Self-Review (Round 2)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/b.rs:20 | major | Description | ⬜ Open 🔁 Re-opened |
| 2 | src/e.rs:50 | major | Description | ⬜ Open |
| 3 | src/f.rs:60 | minor | Description | ⚠️ Objected: nit-level |
| 4 | src/g.rs:73 | minor | Missing doc comment | ⬜ Open |

## Self-Review (Round 3)

**Verdict:** APPROVE

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|

## Comment cycle round 1

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | f.rs:1 | major | decoy | ⚠️ Objected: x |
| 2 | f.rs:2 | minor | decoy | ⬜ Open 🔁 Re-opened |
EOF

f2="$tmp/f2-does-not-exist.progress.md"   # deliberately never created

f3="$tmp/f3.progress.md"
cat > "$f3" <<EOF
# Progress: fixture — ACTIVE

**Branch:** feat/fixture
**base_commit:** ${real_base}
**Issue:** #42

## Files touched

- \`crates/gp-core/src/a.rs\` — added a thing
EOF

# Three degradation paths fire at once: URL-form Issue, a verdict-less section,
# an unknown severity, a truncated row, and NO **base_commit:** line.
f4="$tmp/f4.progress.md"
cat > "$f4" <<'EOF'
# Progress: fixture — ACTIVE

**Branch:** feat/fixture
**Issue:** https://github.com/maratik123/graphite-gp/issues/42

## Self-Review (Round 1)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:10 | critical | Unknown severity | ⬜ Open |
| 2 | src/b.rs:2

## Self-Review (Round 2)

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:10 | major | No verdict line above | ⬜ Open |
EOF

f5="$tmp/f5.progress.md"
cat > "$f5" <<EOF
# Progress: fixture — ACTIVE

**Branch:** feat/fixture
**base_commit:** ${real_base}
**Issue:** #42

## Files touched

- \`crates/gp-core/src/a.rs\` — added a thing

## Self-Review (Round 1)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:10 | major | Description | ⬜ Open |

## Self-Review (Round 2)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:10 | major | Description | ⬜ Open |

## Self-Review (Round 3)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:10 | major | Description | ⬜ Open |
EOF

# F6 — a literal `|` inside a `Finding` cell shifts every later column right by
# one. A parser reading Status off a FIXED column index (e.g. `c[6]`) reads
# part of the Finding text instead; only "Status is the LAST cell" survives
# this. This fixture fails on the pre-fix form and must pass on the fix.
f6="$tmp/f6.progress.md"
cat > "$f6" <<EOF
# Progress: fixture — ACTIVE

**Branch:** feat/fixture
**base_commit:** ${real_base}
**Issue:** #42

## Self-Review (Round 1)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:1 | major | uses \`a | b\` here | ⚠️ Objected: x |
EOF

# F7 — an unbucketed severity cell, ISOLATED: everything else in this fixture
# is clean (parseable `#N` Issue, resolvable base_commit, a `## Files touched`
# section, a well-formed Verdict). Only the "critical" severity cell (none of
# blocker/major/minor/nit) is wrong. incomplete must be TRUE for this reason
# alone (M2/M3) — no other trigger in this fixture can produce it.
f7="$tmp/f7.progress.md"
cat > "$f7" <<EOF
# Progress: fixture — ACTIVE

**Branch:** feat/fixture
**base_commit:** ${real_base}
**Issue:** #42

## Files touched

- \`crates/gp-core/src/a.rs\` — added a thing

## Self-Review (Round 1)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:10 | critical | Unknown severity | ⬜ Open |
EOF

# F8 — a verdict-less Self-Review section, ISOLATED the same way: clean
# header fields, a `## Files touched` section, and a well-bucketed row. Only
# Round 2 carries no `**Verdict:**` line, so its verdict token becomes
# "UNKNOWN". incomplete must be TRUE for this reason alone (M2/M3).
f8="$tmp/f8.progress.md"
cat > "$f8" <<EOF
# Progress: fixture — ACTIVE

**Branch:** feat/fixture
**base_commit:** ${real_base}
**Issue:** #42

## Files touched

- \`crates/gp-core/src/a.rs\` — added a thing

## Self-Review (Round 1)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:10 | major | Description | ⬜ Open |

## Self-Review (Round 2)

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/a.rs:10 | major | No verdict line above | ⬜ Open |
EOF

# F9 — GFM makes a row's TRAILING pipe optional; the row matcher
# (`/^\|[[:space:]]*[0-9]/`) requires only the LEADING one, so a no-trailing-pipe
# row is accepted by the matcher and must still be read correctly. Two rows,
# neither closed with `|`: row 1's Status cell is the LAST split field (no
# empty trailing field to fall back past); row 2 likewise, carrying the
# re-opened marker instead. Independent of F6/case 15, which pins the
# trailing-pipe-plus-embedded-pipe shape.
f9="$tmp/f9.progress.md"
cat > "$f9" <<EOF
# Progress: fixture — ACTIVE

**Branch:** feat/fixture
**base_commit:** ${real_base}
**Issue:** #42

## Self-Review (Round 1)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/z.rs:99 | major | Do the thing | ⚠️ Objected: reason
| 2 | src/y.rs:5 | minor | Another finding | ⬜ Open 🔁 Re-opened
EOF

# F10 — an escaped pipe walked through EVERY non-final cell.
#
# The property, stated so the fixture cannot be reduced to the cell that
# happened to fail last: `\|` is cell CONTENT and may appear anywhere, so the
# parse must locate #, File:line, Severity, Finding and Status correctly with
# the escape in each of them in turn. Rounds 1 and 2 each shipped a fix named
# after one observed cell (Finding, then Status) and each moved the hole to
# another cell. A case pinning one more cell would repeat that; this one
# enumerates the positions instead.
#
# Row 6 carries the escape in the SEVERITY cell, where the correct behaviour
# differs: the cell is located correctly but its content is then not one of
# the four buckets, so the row contributes to no bucket and trips `incomplete`.
# That is the discriminator for mislocation -- a parser that read the wrong
# cell as severity would likely find a VALID bucket name there and bucket the
# row, so `([.findings[]] | add) == 5` fails if location is wrong.
f10="$tmp/f10.progress.md"
cat > "$f10" <<EOF
# Progress: fixture — ACTIVE

**Branch:** feat/fixture
**base_commit:** ${real_base}
**Issue:** #42

## Self-Review (Round 1)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1\\|x | src/a.rs:1 | blocker | plain finding | ⚠️ Objected: reason |
| 2 | src/b\\|c.rs:2 | major | plain finding | ⚠️ Objected: reason |
| 3 | src/d.rs:3 | major | uses a \\| b here | ⚠️ Objected: reason |
| 4 | src/e.rs:4 | minor | plain finding | ⚠️ Objected: use a \\| b |
| 5 | src/f.rs:5 | nit | uses p \\| q | ⬜ Open 🔁 Re-opened for r \\| s |
| 6 | src/g.rs:6 | ma\\|jor | plain finding | ⬜ Open |

## Files touched

- \`src/a.rs\` — a
EOF

# --- Case 1: F1 happy path (AC9) ---------------------------------------------
t1="$tmp/out1.jsonl"
bash "$script" "$f1" "$t1" >/dev/null 2>&1
assert_exit "case 1: F1 exits 0" "$?" 0
assert_eq   "case 1: exactly one line appended" "$(grep -c '' "$t1" 2>/dev/null)" "1"
l1=$(tail -1 "$t1" 2>/dev/null)

assert_jq "case 1: rounds == 3"                "$l1" '.rounds == 3'
assert_jq "case 1: verdicts in round order"    "$l1" '.verdicts == ["REJECT","REJECT","APPROVE"]'
assert_jq "case 1: hit_round_cap == false"     "$l1" '.hit_round_cap == false'
assert_jq "case 1: findings hand-count"        "$l1" '.findings == {"blocker":1,"major":4,"minor":4,"nit":1}'
assert_jq "case 1: findings_first_seen"        "$l1" '.findings_first_seen == {"blocker":1,"major":3,"minor":4,"nit":1}'
assert_jq "case 1: objections == 2"            "$l1" '.objections == 2'
assert_jq "case 1: objections_reopened == 1"   "$l1" '.objections_reopened == 1'
assert_jq "case 1: files_touched (3 paths)"    "$l1" \
  '.files_touched == ["crates/gp-core/src/a.rs","crates/gp-core/src/b.rs","crates/gp-core/src/c.rs"]'
assert_jq "case 1: incomplete == false"        "$l1" '.incomplete == false'
assert_jq "case 1: issue == 42"                "$l1" '.issue == 42'
assert_jq "case 1: spec_base from basename"    "$l1" '.spec_base == "f1"'
assert_jq "case 1: schema_version == 1"        "$l1" '.schema_version == 1'
assert_jq "case 1: corpus lines is an int > 0" "$l1" '.instruction_corpus_lines | type == "number" and . > 0'

# The corpus count must come from the PINNED command INCLUDING its
# `:(exclude)ai-docs/learnings.md` term. A script carrying the pre-exclusion form
# writes every record on a superseded, non-comparable basis — and the value is
# plausible either way, so nothing but this equality catches it. Derived live
# rather than hardcoded: the count is environment-dependent by construction.
corpus_excl=$(git ls-files -z -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md' \
  ':(exclude)ai-docs/learnings.md' | xargs -0 cat | wc -l | tr -d ' ')
corpus_broad=$(git ls-files -z -- 'AGENTS.md' 'CLAUDE.md' ':(glob).claude/**/*.md' ':(glob)ai-docs/*.md' \
  | xargs -0 cat | wc -l | tr -d ' ')
# Mutation guard for the assertion below: if the two forms ever agree, the
# equality cannot discriminate and would pass on the pre-exclusion script. A
# failure HERE means the excluded file vanished, not that the extractor is wrong.
if [ "$corpus_excl" != "$corpus_broad" ]; then
  pass "case 1: exclude-form corpus differs from broad form (assertion discriminates)"
else
  fail "case 1: exclude-form corpus differs from broad form (assertion discriminates)" \
       "both forms read [$corpus_excl]; the next assertion cannot fail"
fi
assert_jq "case 1: corpus uses the :(exclude) pinned form" "$l1" \
  ".instruction_corpus_lines == $corpus_excl"

# --- Case 2: F1 section bounding ---------------------------------------------
# F1 carries a `⚠️ Objected` + `🔁 Re-opened` bullet in `## Decisions log` (which
# `/task` Step 11 sub-step 4 MANDATES for any objection, so this is the live
# shape, not a contrivance) and a trailing `## Comment cycle round 1` table whose
# rows begin `| 1 |` / `| 2 |` and carry `major` / `minor` severity cells.
#
# A parser that scans the whole file instead of bounding each Self-Review section
# to the next `^## ` heading reads 12 findings (not 10) — shape-independently,
# since any `^| [0-9]` row matcher picks the decoy rows up — and 3-or-4 objections
# and 2-or-3 re-opens depending on its shape. All three assertions below fail.
assert_jq "case 2: findings total 10 despite decoys" "$l1" '([.findings[]] | add) == 10'
assert_jq "case 2: objections bounded to 2"         "$l1" '.objections == 2'
assert_jq "case 2: objections_reopened bounded to 1" "$l1" '.objections_reopened == 1'

# --- Case 3: F1 carry-forward (AC9's explicit clause) -------------------------
# `src/b.rs:20` appears in R1 (objected) and R2 (re-opened): counted TWICE in
# `findings`, ONCE in `findings_first_seen`. Without this pair the two fields are
# indistinguishable on any fixture whose rows are all unique.
assert_jq "case 3: carry-forward counted twice in findings"      "$l1" '.findings.major == 4'
assert_jq "case 3: carry-forward counted once in first_seen"     "$l1" '.findings_first_seen.major == 3'

# --- Case 4: F2 absent (AC7) --------------------------------------------------
t4="$tmp/out4.jsonl"
bash "$script" "$f2" "$t4" >/dev/null 2>&1
assert_exit "case 4: absent progress file still exits 0" "$?" 0
assert_eq   "case 4: exactly one line appended" "$(grep -c '' "$t4" 2>/dev/null)" "1"
l4=$(tail -1 "$t4" 2>/dev/null)
assert_jq "case 4: valid JSON object"     "$l4" 'type == "object"'
assert_jq "case 4: incomplete == true"    "$l4" '.incomplete == true'
assert_jq "case 4: all nine required keys present" "$l4" \
  '(["schema_version","date","branch","issue","spec_base","incomplete","files_changed","insertions","deletions"] - (keys)) == []'
assert_jq "case 4: spec_base from basename" "$l4" '.spec_base == "f2-does-not-exist"'
assert_jq "case 4: trio is integer-typed"   "$l4" \
  '(.files_changed|type)=="number" and (.insertions|type)=="number" and (.deletions|type)=="number"'
assert_jq "case 4: no bogus progress-derived optionals" "$l4" \
  '(has("rounds")|not) and (has("verdicts")|not) and (has("findings")|not) and (has("objections")|not) and (has("files_touched")|not)'

# --- Case 5: F3 no Self-Review sections (AC7) ---------------------------------
t5="$tmp/out5.jsonl"
bash "$script" "$f3" "$t5" >/dev/null 2>&1
assert_exit "case 5: no-sections file exits 0" "$?" 0
l5=$(tail -1 "$t5" 2>/dev/null)
assert_jq "case 5: incomplete == true"     "$l5" '.incomplete == true'
assert_jq "case 5: rounds == 0"            "$l5" '.rounds == 0'
assert_jq "case 5: verdicts == []"         "$l5" '.verdicts == []'
assert_jq "case 5: hit_round_cap == false" "$l5" '.hit_round_cap == false'

# --- Case 6: F4 garbled (AC7) -------------------------------------------------
t6="$tmp/out6.jsonl"
bash "$script" "$f4" "$t6" >/dev/null 2>&1
assert_exit "case 6: garbled file exits 0" "$?" 0
l6=$(tail -1 "$t6" 2>/dev/null)
assert_jq "case 6: line is valid JSON"                 "$l6" 'type == "object"'
assert_jq "case 6: incomplete == true"                 "$l6" '.incomplete == true'
assert_jq "case 6: unknown severity in no bucket"      "$l6" '([.findings[]] | add) == 1'
assert_jq "case 6: verdict-less section -> UNKNOWN"    "$l6" '.verdicts == ["REJECT","UNKNOWN"]'
assert_jq "case 6: URL-form Issue -> JSON null, not absent" "$l6" 'has("issue") and .issue == null'
assert_jq "case 6: trio present despite missing base_commit" "$l6" \
  '(["files_changed","insertions","deletions"] - (keys)) == []'

# --- Case 7: F5 round cap in its TRUE state -----------------------------------
# Paired with case 1's `false`, this makes the derivation testable in both
# directions — a parser hardcoding `false` passes every other case.
t7="$tmp/out7.jsonl"
bash "$script" "$f5" "$t7" >/dev/null 2>&1
assert_exit "case 7: round-cap fixture exits 0" "$?" 0
l7=$(tail -1 "$t7" 2>/dev/null)
assert_jq "case 7: hit_round_cap == true" "$l7" '.hit_round_cap == true'
assert_jq "case 7: verdicts all REJECT"   "$l7" '.verdicts == ["REJECT","REJECT","REJECT"]'

# --- Case 8: cannot append (AC8) ----------------------------------------------
# A non-existent PARENT DIRECTORY, not `chmod 000`: a root-run test would defeat
# a permission-based fixture.
t8="$tmp/does-not-exist/task-runs.jsonl"
err8="$tmp/err8.txt"
bash "$script" "$f1" "$t8" >/dev/null 2>"$err8"
rc8=$?
if [ "$rc8" -ne 0 ]; then pass "case 8: unwritable target exits non-zero (exit $rc8)"
else fail "case 8: unwritable target exits non-zero" "got exit 0"; fi
if [ -s "$err8" ]; then pass "case 8: diagnosis written to stderr"
else fail "case 8: diagnosis written to stderr" "stderr was empty"; fi
if [ ! -e "$tmp/does-not-exist" ]; then pass "case 8: no target created"
else fail "case 8: no target created" "$tmp/does-not-exist exists"; fi

# --- Case 9: append-only / last-line-wins -------------------------------------
t9="$tmp/out9.jsonl"
bash "$script" "$f1" "$t9" >/dev/null 2>&1
first_line=$(cat "$t9")
bash "$script" "$f1" "$t9" >/dev/null 2>&1
assert_eq "case 9: second run appends, total 2 lines" "$(grep -c '' "$t9" 2>/dev/null)" "2"
assert_eq "case 9: line 1 byte-identical after re-run" "$(head -1 "$t9" 2>/dev/null)" "${first_line%$'\n'}"

# --- Case 10: trailing newline (AC1) ------------------------------------------
nl_ok=1
for t in "$t1" "$t4" "$t5" "$t6" "$t7" "$t9"; do
  [ "$(tail -c1 "$t" 2>/dev/null | xxd -p)" = "0a" ] || nl_ok=0
done
if [ "$nl_ok" -eq 1 ]; then pass "case 10: every appending case ends with 0x0a"
else fail "case 10: every appending case ends with 0x0a"; fi

# --- Case 11: AC10 two-path containment ---------------------------------------
# All three key sets are derived MECHANICALLY from the artefacts — the required
# set from the schema page's own field table, never hardcoded here, so a field
# added to the schema cannot drift out of this assertion. Content-addressed on
# the `## Field table` / `### Worked fallback example` headings, never on line
# numbers: check-citations.sh's header documents at length how a line-pinned
# exclusion silently re-points after an unrelated insertion.
req_f="$tmp/keys-required.txt"; ex_f="$tmp/keys-example.txt"; sc_f="$tmp/keys-script.txt"
all_f="$tmp/keys-all.txt"
awk '/^## Field table$/{f=1;next} f&&/^## /{exit} f' "$schema" \
  | awk -F'|' '/^\| `/ && $(NF-1) ~ /fallback-required/ {gsub(/[ `]/,"",$2); print $2}' \
  | sort > "$req_f"
awk '/^## Field table$/{f=1;next} f&&/^## /{exit} f' "$schema" \
  | awk -F'|' '/^\| `/ {gsub(/[ `]/,"",$2); print $2}' \
  | sort > "$all_f"
awk '/^### Worked fallback example$/{f=1} f&&/^```json$/{g=1;next} g&&/^```$/{exit} g' "$schema" \
  | jq -r 'keys[]' | sort > "$ex_f"
printf '%s' "$l1" | jq -r 'keys[]' | sort > "$sc_f"

# Every count here is DERIVED from the schema page, never written down: a
# transcribed cardinality fails-on-correct the moment a field is legitimately
# added, and that is the failure mode this corpus keeps re-learning.
if [ -s "$req_f" ] && [ -s "$all_f" ]; then
  pass "case 11: field-table extraction is non-empty (required $(grep -c '' "$req_f") of $(grep -c '' "$all_f"))"
else
  fail "case 11: field-table extraction is non-empty" "the page's field table yielded no names"
fi
assert_eq "case 11: REQUIRED subset of EXAMPLE" "$(comm -23 "$req_f" "$ex_f" | tr '\n' ' ')" ""
assert_eq "case 11: EXAMPLE subset of SCRIPT"   "$(comm -23 "$ex_f" "$sc_f" | tr '\n' ' ')" ""
if [ -s "$sc_f" ] && [ -n "$(comm -13 "$ex_f" "$sc_f")" ]; then
  pass "case 11: EXAMPLE is a PROPER subset of SCRIPT"
else
  fail "case 11: EXAMPLE is a PROPER subset of SCRIPT" "script emitted no key beyond the fallback set"
fi
# WHITELIST, not a blacklist: the script's key set must EQUAL the page's field
# table. A blacklist can only name the fields someone already thought of; this
# rejects any unmandated field — including the three v1 explicitly excludes —
# and equally rejects a field the page mandates but the script never emits.
assert_eq "case 11: SCRIPT key set equals the schema page's field table" \
  "$(comm -3 "$all_f" "$sc_f" | tr -d '\t' | tr '\n' ' ')" ""

# --- Case 12: AC11a shortstat shapes ------------------------------------------
# A real throwaway repo, not a "parse this string" hook in the script: adding an
# API surface that exists only for the test was rejected.
#
# Each shape PINS `**base_commit:**` to the sandbox SHA. A `git init` repo has no
# `main` ref, so the script's merge-base last resort is unavailable there; without
# the pin the trio would degrade to 0/0/0 and all four sub-assertions would fail
# for a reason unrelated to keyword parsing.
sb="$tmp/sandbox"
mkdir -p "$sb"
git -C "$sb" init -q
git -C "$sb" config user.email 'fixture@example.invalid'
git -C "$sb" config user.name 'fixture'

sb_pf() {  # $1 = base sha -> echoes a progress-file path pinned to it
  local p="$tmp/sb-$2.progress.md"
  cat > "$p" <<EOF
# Progress: sandbox fixture

**Branch:** master
**base_commit:** $1
**Issue:** #42
EOF
  printf '%s' "$p"
}
sb_run() {  # $1 = progress file -> echoes the appended JSON line
  local t="$tmp/out12-$2.jsonl"
  ( cd "$sb" && bash "$repo_root/$script" "$1" "$t" >/dev/null 2>&1 )
  tail -1 "$t" 2>/dev/null
}

printf 'l1\nl2\nl3\nl4\n' > "$sb/f.txt"
git -C "$sb" add f.txt && git -C "$sb" commit -q -m base
base_a=$(git -C "$sb" rev-parse HEAD)

# (a) deletions only — the shape a positional parse inverts
printf 'l1\n' > "$sb/f.txt"
git -C "$sb" commit -aq -m del
l12a=$(sb_run "$(sb_pf "$base_a" a)" a)
assert_jq "case 12a: deletions-only -> insertions 0" "$l12a" '.insertions == 0'
assert_jq "case 12a: deletions-only -> deletions 3"  "$l12a" '.deletions == 3'

# (b) single insertion — singular `insertion` AND an absent deletions clause
base_b=$(git -C "$sb" rev-parse HEAD)
printf 'l1\nnew\n' > "$sb/f.txt"
git -C "$sb" commit -aq -m ins
l12b=$(sb_run "$(sb_pf "$base_b" b)" b)
assert_jq "case 12b: 1 insertion(+) parsed" "$l12b" \
  '.files_changed == 1 and .insertions == 1 and .deletions == 0'

# (c) no changes at all — empty shortstat must fall out as 0/0/0
base_c=$(git -C "$sb" rev-parse HEAD)
l12c=$(sb_run "$(sb_pf "$base_c" c)" c)
assert_jq "case 12c: empty shortstat -> 0/0/0" "$l12c" \
  '.files_changed == 0 and .insertions == 0 and .deletions == 0'

# (d) singular `deletion(-)`. Without this sub-assertion case 12 cannot fail for
# the reason it exists: a parser written `deletions\(-\)` with no `?` satisfies
# (a) (plural), and (b)/(c) expect 0, which a non-matching pattern produces by
# DEFAULTING. Only ` 1 deletion(-)` separates the two patterns — and the broken
# one would write a wrong number, once, into an append-only corpus no later gate
# re-derives.
base_d=$(git -C "$sb" rev-parse HEAD)
printf 'l1\na\nb\n' > "$sb/f.txt"
git -C "$sb" commit -aq -m mixed
l12d=$(sb_run "$(sb_pf "$base_d" d)" d)
assert_jq "case 12d: singular deletion(-) parsed" "$l12d" \
  '.insertions == 2 and .deletions == 1'

# (e) unobtainable branch (m1) — `git branch --show-current` returns "" on
# detached HEAD or outside a git work tree. `branch` is fallback-required and
# last-line-wins consumers key their dedup on it, so an unflagged "" is a
# silent key collision waiting to happen.
#
# ISOLATED via a `git` shim on $PATH rather than the throwaway sandbox: the
# sandbox's `corpus` command already reads 0 (it has none of AGENTS.md /
# ai-docs/*.md), which degrades the record for an UNRELATED reason and would
# make this assertion pass whether or not the branch fix exists. Run against
# F1 (real repo, every other trigger already closed per case 1) with only
# `git branch --show-current` intercepted, so `incomplete: true` here can be
# attributed to the empty branch alone.
real_git=$(command -v git)
mkdir -p "$tmp/bin"
cat > "$tmp/bin/git" <<EOF
#!/usr/bin/env bash
if [ "\$1" = "branch" ] && [ "\$2" = "--show-current" ]; then
  exit 0
fi
exec "$real_git" "\$@"
EOF
chmod +x "$tmp/bin/git"
t12e="$tmp/out12e.jsonl"
PATH="$tmp/bin:$PATH" bash "$script" "$f1" "$t12e" >/dev/null 2>&1
l12e=$(tail -1 "$t12e" 2>/dev/null)
assert_jq "case 12e: unobtainable branch -> branch is empty" "$l12e" '.branch == ""'
assert_jq "case 12e: unobtainable branch -> incomplete == true from branch alone" "$l12e" \
  '.incomplete == true'

# --- Case 14: F1 key drift — the over-count asserted as EXPECTED (AC9a) --------
# `src/g.rs:70` (R1) and `src/g.rs:73` (R2) are ONE finding: same file, same
# `Finding` text, at a line number the `src/g.rs:15` fix above them shifted. Under
# the shipped `File:line` identity key they are two different keys, so the row
# receives NO de-duplication and is counted twice in BOTH counters.
#
# THE OVER-COUNT BELOW IS NOT A BUG. It is measured behaviour, and it is
# expected under the current File:line key. See ai-docs/task-run-schema.md
# § "Counting units" — the frequency clause, the degeneracy signature, and the
# coupling clause all describe exactly this. Do NOT "repair" the parser to make
# the number look right: switching the key to path + Finding text would make
# `.findings_first_seen.minor` read 3, fail this case, and silently change what
# every record in the corpus means MID-SERIES. If this case is red, read the
# schema page before touching anything.
#
# Case 3's `major` bucket is the complement: there the key is STABLE across
# rounds and de-duplication works (4 -> 3). The two cases together pin the
# mechanism and its known limit.
assert_jq "case 14: drifted row counted twice in findings"          "$l1" '.findings.minor == 4'
assert_jq "case 14: drifted row NOT de-duplicated in first_seen"    "$l1" '.findings_first_seen.minor == 4'
# Stated as an equality so no literal is transcribed: the ABSENCE of
# de-duplication is what the test asserts, not an incidental number.
assert_jq "case 14: no de-duplication under a drifted key"          "$l1" \
  '.findings_first_seen.minor == .findings.minor'

# --- Case 15: pipe-in-Finding column shift (M1 regression guard) -------------
# Fails on the pre-fix `stat = c[6]` form: the escaped-looking pipe inside the
# Finding cell shifts Status to c[7], so the old code reads " b\` here " (part
# of the Finding text) as Status and never sees "⚠️ Objected". Passes under
# `stat = c[n - 1]`.
t15="$tmp/out15.jsonl"
bash "$script" "$f6" "$t15" >/dev/null 2>&1
assert_exit "case 15: F6 exits 0" "$?" 0
l15=$(tail -1 "$t15" 2>/dev/null)
assert_jq "case 15: objections == 1 despite the pipe in Finding" "$l15" '.objections == 1'
assert_jq "case 15: findings.major == 1"                          "$l15" '.findings.major == 1'

# --- Case 16: unbucketed severity ALONE triggers incomplete (M2/M3) ----------
t16="$tmp/out16.jsonl"
bash "$script" "$f7" "$t16" >/dev/null 2>&1
assert_exit "case 16: F7 exits 0" "$?" 0
l16=$(tail -1 "$t16" 2>/dev/null)
assert_jq "case 16: incomplete == true from the bad severity alone" "$l16" '.incomplete == true'
assert_jq "case 16: the bad-severity row lands in no bucket"        "$l16" '([.findings[]] | add) == 0'

# --- Case 17: verdict-less section ALONE triggers incomplete (M2/M3) ---------
t17="$tmp/out17.jsonl"
bash "$script" "$f8" "$t17" >/dev/null 2>&1
assert_exit "case 17: F8 exits 0" "$?" 0
l17=$(tail -1 "$t17" 2>/dev/null)
assert_jq "case 17: incomplete == true from the missing Verdict line alone" "$l17" '.incomplete == true'
assert_jq "case 17: verdict-less round -> UNKNOWN"                          "$l17" \
  '.verdicts == ["REJECT","UNKNOWN"]'

# --- Case 18: no-trailing-pipe row shape (M1' regression guard) --------------
# Fails on a `stat = c[n - 1]` form (assumes a trailing pipe unconditionally):
# with no trailing pipe, `c[n]` IS Status and `c[n-1]` is the Finding cell, so
# that form reads "Do the thing" / "Another finding" as Status and finds
# neither marker. Passes under the ternary that takes `c[n]` unless blank.
t18="$tmp/out18.jsonl"
bash "$script" "$f9" "$t18" >/dev/null 2>&1
assert_exit "case 18: F9 exits 0" "$?" 0
l18=$(tail -1 "$t18" 2>/dev/null)
assert_jq "case 18: objections == 1 with no trailing pipe"        "$l18" '.objections == 1'
assert_jq "case 18: objections_reopened == 1 with no trailing pipe" "$l18" '.objections_reopened == 1'
assert_jq "case 18: findings.major == 1 and findings.minor == 1"  "$l18" \
  '.findings.major == 1 and .findings.minor == 1'

# --- Case 19: escaped pipe in EVERY non-final cell (M1'' class guard) --------
# Guards the PROPERTY, not a cell. Falsification directions MEASURED, not
# reasoned -- the three shipped forms of this fix red three DISJOINT cases:
#   stat = c[6]           -> case 15 reds (1 assertion); 18 and 19 stay green
#   stat = c[n - 1]       -> case 18 reds (2 assertions); 15 and 19 stay green
#   mask removed          -> case 19 reds (4 assertions); 15 and 18 stay green
# So each case owns exactly one property and none subsumes another. Note what
# that means for the mask: cases 15 and 18 pass WITHOUT it, because "last cell"
# already absorbs a shift originating in the Finding cell. The mask is what
# handles an escape inside the STATUS cell itself, where the status text is
# split across cells and no single index holds it -- which is the axis rounds 1
# and 2 both left open. If a future edit reds ONE row here, read that row's
# cell position before touching the parser: it names the cell the edit dropped.
t19="$tmp/out19.jsonl"
bash "$script" "$f10" "$t19" >/dev/null 2>&1
assert_exit "case 19: F10 exits 0" "$?" 0
l19=$(tail -1 "$t19" 2>/dev/null)
# Four rows carry the objection marker in their Status cell (1, 2, 3, 4); row 5
# is Re-opened; row 6 is plain Open. A mislocated Status cell moves these.
assert_jq "case 19: objections == 4 with the escape in four positions" "$l19" '.objections == 4'
assert_jq "case 19: objections_reopened == 1"                          "$l19" \
  '.objections_reopened == 1'
# Bucketing proves Severity was located: rows 1-5 bucket, row 6 does not.
assert_jq "case 19: severity located under every escape position"      "$l19" \
  '.findings == {"blocker":1,"major":2,"minor":1,"nit":1}'
assert_jq "case 19: escaped-pipe severity lands in no bucket"          "$l19" \
  '([.findings[]] | add) == 5'
# `incomplete` comes from row 6's unbucketed severity ALONE -- the fixture has
# a resolvable base_commit, an #N Issue, a Verdict and a Files-touched section,
# so any other degradation path firing here is itself the bug.
assert_jq "case 19: incomplete traces to the bad severity alone"       "$l19" \
  '.incomplete == true'

# --- Case 20: escaped BACKSLASH before a real delimiter (M1''' class guard) --
# The failure this guards is a FALSE POSITIVE, not a parse error, which is why
# it survived case 19: `\\` is an escaped backslash and the `|` after it is a
# GENUINE delimiter. A single-stage pipe mask matches the second backslash
# together with that pipe, eats the delimiter, and MERGES Finding into Status
# -- so a Finding cell whose prose mentions the objection marker is counted as
# an objection while the row's real status is `✅ Fixed`. The count moves in the
# direction that looks like more review work, and nothing flags it.
#
# Row 1 is the discriminator: prose marker + a trailing `\\` + a real `✅ Fixed`
# status. Row 2 pins that a `\\` NOT adjacent to a delimiter still round-trips.
# Falsification: delete the `gsub(/\\\\/, "\001", row)` stage and row 1 reports
# objections == 1.
f11="$tmp/f11.progress.md"
cat > "$f11" <<EOF
# Progress: fixture — ACTIVE

**Branch:** feat/fixture
**base_commit:** ${real_base}
**Issue:** #42

## Self-Review (Round 1)

**Verdict:** REJECT

| # | File:line | Severity | Finding | Status |
|---|-----------|----------|---------|--------|
| 1 | src/w.rs:1 | major | prose naming ⚠️ Objected, ends in a backslash \\\\| ✅ Fixed |
| 2 | src/v.rs:2 | minor | a \\\\ mid-cell, then a real \\| escape | ⚠️ Objected: r |

## Files touched

- \`src/w.rs\` — w
EOF

t20="$tmp/out20.jsonl"
bash "$script" "$f11" "$t20" >/dev/null 2>&1
assert_exit "case 20: F11 exits 0" "$?" 0
l20=$(tail -1 "$t20" 2>/dev/null)
# Row 1's true status is ✅ Fixed; only row 2 is an objection. A merged
# Finding+Status cell makes this read 2.
assert_jq "case 20: escaped backslash does not merge Finding into Status" "$l20" \
  '.objections == 1'
assert_jq "case 20: both rows bucket normally"                            "$l20" \
  '.findings == {"blocker":0,"major":1,"minor":1,"nit":0}'
assert_jq "case 20: no degradation from either escape form"               "$l20" \
  '.incomplete == false'

# --- Case 13: no tracked-file mutation ----------------------------------------
# True by construction under the sandbox strategy; this case is what proves the
# construction held.
status_after=$(git status --porcelain)
assert_eq "case 13: working tree unchanged by the test run" "$status_after" "$status_before"

# --- AC6: the expected case count is DERIVED from the design, both sides ------
# This assertion is AC6. Before it existed the file ended with a hard-coded
# `echo "PASS: all 18 cases green."` -- a hand-typed string that agreed with
# nothing, so fixture-side drift (a case added, the banner not updated) was
# invisible and the design-side check ran only when a human typed it at a
# terminal. Both halves are derived now: `cases_run` from the labels emitted by
# this run, `C` from the design's § Cases table.
#
# The design moves to ai-docs/plans/done/ at /task Step 12, so both locations
# are probed. Absent design == FAIL, deliberately: AC6 IS the coupling, and a
# gate that quietly skips when its reference is missing is the defect this
# assertion was written to remove.
# These paths are $PWD-relative and that is SAFE, not an oversight: the script
# resolves `repo_root` and `cd`s to it at the top (see `set -uo pipefail`
# block), so $PWD is the repo root by the time any assertion runs, and a run
# from outside a work tree exits there rather than reaching this gate. Do not
# add a second anchoring here -- duplicating an invariant creates two places to
# keep in sync, and the copy is what goes stale.
cases_run=$(printf '%s' "$case_ids" | tr ' ' '\n' | sort -un | grep -c .)
design=""
for _cand in ai-docs/plans/2026-07-31-task-run-telemetry.design.md \
             ai-docs/plans/done/2026-07-31-task-run-telemetry.design.md; do
  if [ -r "$_cand" ]; then design="$_cand"; break; fi
done
if [ -z "$design" ]; then
  fail "AC6: design document not found in either location — cannot derive C"
else
  # `^\|[[:space:]]*[0-9*]`, NOT `^\| [0-9*]`. The narrow form requires exactly
  # one space and misses column-aligned (`|  19 |`) and tight (`|19|`) rows --
  # the same defect corrected on the schema page as its `findings` row matcher.
  # A gate against desync must not itself miscount when a row is reformatted:
  # C would under-read, this assertion would red, and the next reader would go
  # fix the fixture rather than the matcher.
  C=$(awk '/^### Cases/{f=1} f&&/^\|[[:space:]]*[0-9*]/{n++} f&&/^$/&&n{print n;exit}' "$design")
  assert_eq "AC6: cases exercised == design § Cases rows" "$cases_run" "${C:-0}"
fi

echo
if [ "$failures" -gt 0 ]; then
  echo "FAIL: ${failures} assertion(s) failed."
  exit 1
fi
echo "PASS: all ${cases_run} cases green (count derived from ${design} § Cases)."

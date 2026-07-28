#!/usr/bin/env bash
# Regression test for check-citations.sh.
#
# Locks ONE invariant: check (2)'s format-spec exclusion must identify the
# excluded text by its CONTENT, not by its line number.
#
# Why that invariant and not just "the gate is green": a fix that merely
# re-pins the hardcoded line number (47 -> 49) would make the gate green
# today and silently rot again on the next insertion above that row. Case 2
# below is what distinguishes a real fix from a re-pin — it shifts the row and
# requires the guard to still find it.
#
# Usage: bash .claude/skills/ai-audit/scripts/test-check-citations.sh
# Exit 0 = all cases pass. Exit 1 = regression.

set -uo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root" || exit 1

guard=".claude/skills/ai-audit/scripts/check-citations.sh"
target="ai-docs/corrections-log.md"
backup=$(mktemp)
failures=0

# Restore the pristine file however we exit — including on interrupt. A stray
# edit to a tracked doc is worse than a failed test.
# The signal handlers EXIT rather than sharing the EXIT body: a handler that
# only cleans up lets the script resume with its backup already deleted, so
# every later restore silently fails and the target is left mutated.
# shellcheck disable=SC2064  # intentional: expand $backup/$target now, not at trap time
trap "cp '$backup' '$target'; rm -f '$backup'" EXIT
# shellcheck disable=SC2064  # same
trap "cp '$backup' '$target'; rm -f '$backup'; exit 130" INT
# shellcheck disable=SC2064  # same
trap "cp '$backup' '$target'; rm -f '$backup'; exit 143" TERM
cp "$target" "$backup"
mode_before=$(stat -c '%a' "$target")

report() {
  # $1 = case name, $2 = observed exit, $3 = expected exit
  if [ "$2" -eq "$3" ]; then
    printf '  PASS  %s (exit %s)\n' "$1" "$2"
  else
    printf '  FAIL  %s — expected exit %s, got %s\n' "$1" "$3" "$2"
    failures=$((failures + 1))
  fi
}

echo "== test-check-citations =="
echo

# --- Case 1: the guard is green on the pristine tree -------------------------
# Failed before the fix: the exclusion was pinned to corrections-log.md:47
# while the format-spec example it means to exclude had drifted to :49.
bash "$guard" >/dev/null 2>&1
report "case 1: green on pristine tree" "$?" 0

# --- Case 2: the exclusion survives line drift --------------------------------
# Insert blank lines above the glossary so every row below shifts down. A
# content-based exclusion still finds its target; a line-pinned one does not.
# Blank lines are used deliberately — they add no text that could trip any
# other check, so a RED here isolates the drift behaviour.
# shellcheck disable=SC2016  # literal backticks in the markdown row are the pattern, not an expansion
row_before=$(grep -n '^> `Superseded by:`' "$target" | cut -d: -f1)
shifted=$(mktemp)
{ head -n 1 "$target"; printf '\n\n\n'; tail -n +2 "$target"; } > "$shifted"
# `cat >` into the existing path, never `mv` onto it: `mv` would replace the
# tracked file with mktemp's inode and permanently drop its mode 644 -> 600.
# Git does not track that bit, so `git status` stays clean and the damage is
# invisible to the obvious check.
cat "$shifted" > "$target"
rm -f "$shifted"
# shellcheck disable=SC2016  # same literal-backtick pattern as above
row_after=$(grep -n '^> `Superseded by:`' "$target" | cut -d: -f1)

if [ "$row_before" = "$row_after" ]; then
  printf '  FAIL  case 2 setup — row did not move (%s); the drift was never exercised\n' "$row_before"
  failures=$((failures + 1))
else
  printf '  ....  case 2 setup: Superseded-by row moved %s -> %s\n' "$row_before" "$row_after"
  bash "$guard" >/dev/null 2>&1
  report "case 2: green after the excluded row drifts" "$?" 0
fi

cp "$backup" "$target"

# --- Case 3: a REAL unresolvable citation is still caught ---------------------
# Guards against over-correcting case 1 into a blanket skip of the whole file.
#
# The fixture date is ASSEMBLED AT RUNTIME, never written as a literal. This
# file lives under .claude/, which check-citations.sh scans — a literal
# out-of-range date here would make the guard flag its own test as a bad
# citation. Excluding this path in the guard was the alternative and was
# rejected: a path is another pinned identifier, and a rename would break it
# silently, which is the exact bug class this test exists to prevent.
bad_year="2026"
bad_date="${bad_year}-03-14"
printf '\n> See the %s learnings entry for context.\n' "$bad_date" >> "$target"
bash "$guard" >/dev/null 2>&1
report "case 3: genuine bad citation still RED" "$?" 1

cp "$backup" "$target"

# --- Case 4: this test must not mutate the tracked file's MODE ---------------
# `git status` cannot see a permission change, so a test that quietly drops
# 644 -> 600 (as `mv` from mktemp does) leaves damage no obvious check reports.
mode_after=$(stat -c '%a' "$target")
if [ "$mode_before" = "$mode_after" ]; then
  printf '  PASS  case 4: target mode preserved (%s)\n' "$mode_after"
else
  printf '  FAIL  case 4: target mode changed %s -> %s\n' "$mode_before" "$mode_after"
  failures=$((failures + 1))
fi

echo
if [ "$failures" -gt 0 ]; then
  echo "FAIL: ${failures} case(s) regressed."
  exit 1
fi
echo "PASS: exclusion is content-addressed and drift-proof."

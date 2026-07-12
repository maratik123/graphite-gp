# Instruction-File Validation Methodology

This doc owns the **v5 dual-model instruction-file validation methodology** — the
workflow used to stress-test whether two correlated readers (Opus 4.7 and
Sonnet 4.6) land on the same interpretation of a workspace instruction file.
Scope: any forward-living instruction file under `AGENTS.md`, `CLAUDE.md`,
`.claude/agents/`, `.claude/skills/`, `.claude/rules/`, or
`ai-docs/{code-style,doc-convention,agent-writing-style,corrections-log}.md`
that an agent reads at session start or on demand. The methodology evolved
v1 → v5 across an initial instruction-file-rewrite effort; each v5 addition
(Class D calibration probes, open-ended probe quota, decoupled probe-author
flow, historical replay testing) was motivated by the Phase 1 retrospective.
Treat the sections below as the prescriptive reference.

---

## Probe taxonomy

Each comprehension test contains three classes of probe, randomly interleaved:

### Class A — Failure-targeted (4–6 probes)

Questions derived from documented misread events in `ai-docs/learnings.md`.
One probe per learnings entry that traces back to the file under rewrite.
These are the rules where ambiguity has *already* cost a session.

**Example (AGENTS.md):**

> The user says "add this to learnings: \<some rule\>". You write the entry.
> Are you authorised to also update AGENTS.md in the same turn? Yes / No, and
> why?

(Targets entry L13 — "on corrections, write to learnings only".)

### Class B — Failure-likely (1–3 probes)

Questions targeting complex/non-trivial logic in the file that *hasn't* failed
yet but has the structural shape that produces dual-model divergence: nested
conditionals, multi-axis decision trees, carve-out exemptions, cross-references
requiring synthesis.

**Example (api-naming.md decision tree):**

> A safe fn `fn nth(&self, i: usize) -> Cell` skips its bounds check for
> speed but returns a default `Cell` (never UB) when `i` is out of range.
> Which suffix does it carry — `_unchecked`, `_unverified`, or none — and
> why?

(Targets the carve-out boundary that's structurally tricky but hasn't failed
yet: `_unchecked` is reserved for `unsafe`, UB-on-failure fns, so a safe
check-skipping fn must NOT use it.)

### Class C — Control / A-B sanity (1 probe)

A read-the-file-literally question whose answer is a single explicit token
from the rewritten file. Serves as a control: both models should always
answer this identically. If they diverge on the control, the test setup
itself is broken (model didn't read, model misread the prompt) — not the
file.

**Example (AGENTS.md):**

> Per AGENTS.md § *Project*, what file should you read on demand for project
> purpose, entities, architecture, and design decisions? Quote the path
> verbatim.

(Answer: literally `ai-docs/context.md`. If models give different answers,
throw out the test run and re-spawn — file isn't to blame.)

### Class D — Calibration / setup-bias (1 probe) — v5

A probe whose answer is **intentionally underspecified** in the file — a rule
the file does NOT actually pin down, where any of several answers could
reasonably be defended. If both models confidently converge on the *same*
answer to a deliberately-ambiguous question, the test setup itself is
converging (training-data biases, prompt-shape biases) — strong signal that
the v5 PASS verdicts on this run are suspect.

**Why:** v4.2's all-PASS-round-1 result across 5 files is either evidence the
files were genuinely clear OR evidence the test isn't sensitive enough.
Without a calibration probe we can't distinguish. Class D distinguishes.

**Example (AGENTS.md):**

> Per AGENTS.md, when a public fn documents BOTH a `# Safety` section AND a
> `# Panics` section, what's the recommended order of the two within the doc
> comment, and why?

(File doesn't actually specify this — the ordering between `# Safety` and
`# Panics` isn't pinned down. Reasonable answers include "`# Safety` first",
"`# Panics` first", or "no fixed rule". If both Opus and Sonnet confidently
say the same thing without hedging, that's training-data convergence, not
file clarity.)

**Verdict logic:**
- Both models hedge / acknowledge the file doesn't specify → ✅ healthy (file is honest about what it does and doesn't pin down; setup is reading correctly)
- Both models confidently converge on the same answer → ⚠️ SETUP-SUSPECT — flag the run; the test bias may have masked real ambiguity in A/B/C
- Models diverge → 🟡 expected (the rule is genuinely ambiguous; either is defensible)

Class D failure does NOT count against the iteration cap — it's a setup
signal, not a file-clarity signal. But it should be surfaced to the user
who decides whether to trust the run's A/B/C verdicts.

### Open-ended probe requirement — v5

In v5, **at least 1–2 of the Class A + Class B probes must be open-ended**:

> "Explain in your own words how rule X applies in scenario Y"

NOT yes/no, NOT multiple-choice, NOT specific-fact. Closed forms produce
convergence too easily because the answer space is narrow. Open-ended forms
require the model to articulate the rule's substance, surfacing
interpretation differences that closed forms hide.

**Rubric for open-ended probes** is necessarily looser:

- Required-present: load-bearing concepts that must appear (any phrasing)
- Required-absent: concepts that, if present, indicate misread
- Verdict: same binary CORRECT / WRONG, but the grader (parent agent)
  applies semantic judgment rather than substring matching for required-
  present concepts. State the judgment criterion at probe-draft time
  (Gate 1 approval).

Divergence on open-ended probes is a much stronger ambiguity signal than
divergence on yes/no.

### Random ordering

Each iteration shuffles probe order. Detects order-dependence (where a
model's answer to probe N is biased by what it saw in probe N-1). If round 2
with reshuffled order produces a *different* outcome than round 1 with the
same probes, that's diagnostic information about ordering effects in the
file's logical flow.

### Total per file (v5)

| Class | Count | Notes |
|---|---|---|
| A — Failure-targeted | 4–6 | At least 1 must be open-ended (counted toward A+B 1–2 open-ended quota) |
| B — Failure-likely | 1–3 | At least 1 of A+B must be open-ended (combined quota) |
| C — Control | 1 | Literal-token sanity check (unchanged from v4.2) |
| D — Calibration | 1 | Intentionally-ambiguous answer (NEW in v5; surfaces setup-bias) |
| **Total** | **7–11** | |

**Open-ended quota:** at least 1–2 probes from A+B combined must be open-
ended (state-the-rule-in-your-own-words style). Closed forms
(yes/no, multiple-choice, specific-fact) for the remaining A/B probes are
fine but should not be the entire test.

---

## Coverage rules

Probes must demonstrate the file is comprehensible end-to-end, not just at
known-bad spots.

### Hard requirements (checked at Gate 1)

| Rule | Threshold |
|---|---|
| Section coverage | At least **60% of top-level (`##`) sections** are targeted by at least one probe |
| Anti-clustering (default) | No more than **40% of probes** target a single section |
| Anti-clustering (heavy-section override) | If a section's line range is > 50% of the file (mechanical) OR is judged rule-dominant at Gate 1 (judgment-call), the per-section cap relaxes to `min(section_share + 10%, 70%)` |
| Class B placement | At least **one Class B probe** targets a section that has zero Class A probes |
| Class C placement | The control probe's anchor section must NOT be a section targeted by any Class A or B probe |

### Section-share-aware anti-clustering — two qualifying paths

**(a) Mechanical override** — section line range / total file line range > 50%.
Auto-detected during Section Inventory (step 3 above).

**(b) Judgment-call override** — section is ≤ 50% by lines but rule-dense to
the point that its share of the rules worth probing exceeds 50%. Invoking (b)
requires an explicit one-line justification at Gate 1 (e.g., "section is 30%
by lines but contains 4 of 5 candidate rules with documented misreads + 2 of
3 candidate failure-likely hot spots — declaring rule-dominant").

### Worked examples

**Example 1 — small variation (mechanical override)**

```
File: hypothetical-rule-doc.md (300 lines)
Section X: lines 100-280 = 180 lines = 60% of file → mechanical override

Cap on Section X:           min(60% + 10%, 70%) = 70%
Cap on every other section: 40%

If 8 probes total: up to 5 in Section X (62.5%), at least 3 spread elsewhere.
```

**Example 2 — AGENTS.md § Workflow (judgment-call override)**

```
File: AGENTS.md (~280 lines)
Section "Workflow": ~70 lines = 25% by lines

NOT > 50% by lines → no mechanical override.

Justification at Gate 1: "Section is 25% by lines but contains three
fail-loud AXIOM blockquotes (branch-before-edit, read-PR-body-after-push,
self-review-before-push) plus the commit-hygiene and _inbox.jsonl rules.
Of 5 candidate Class A probes from learnings.md, 4 trace here. Of 3
candidate Class B hot spots, 2 are inside this section. Declaring
rule-dominant."

Cap on this section (judgment-call):  60% (declared share)
If 8 probes total: up to 5 in this section, at least 3 spread elsewhere.
```

**Example 3 — flat file, no override**

```
File: hypothetical-flat.md (200 lines, 5 ## sections, ~40 lines each)
No section qualifies → default 40% cap applies.
If 8 probes total: max 3 per section.
```

### Coverage map deliverable at Gate 1

Surfaced as a table:

```
Section                               | Share | A probes  | B probes | C  | Cap | OK?
--------------------------------------|-------|-----------|----------|----|-----|----
Scope                                 | 3%    | -         | -        | -  | 40% | ✓
Source files                          | 2%    | -         | -        | -  | 40% | ✓
...
Workflow                              | 25%   | A1,A2,A3,A4 | B1,B2  | -  | 60% | ✓ (judgment override)
...

Coverage:                7 / 11 sections targeted (64%) → PASS (≥ 60%)
Heaviest single section: 6/10 = 60% in Workflow section → PASS (cap 60%)
Coverage rule:           PASS
```

The override declaration is part of Gate 1 — user sees the justification
text, agrees or pushes back.

---

## Rubric framework

Comparison is rubric-based, not prose-vs-prose. Each model's answer is
graded independently against the probe's pre-defined answer key. Wording
deviation between two correct answers is irrelevant.

### Probe answer key (per probe)

#### Class C (control) — exact-token match

```
Type: literal-token
Required value: <verbatim string from the file>

Comparison: case-sensitive substring search for the literal value
in each model's answer.
```

#### Class A and B — required-present / required-absent rubric

```
Type: prose-with-rubric

Required-present (each must appear in some form):
  1. <key concept 1>
  2. <key concept 2>

Required-absent (none of these may appear):
  1. <forbidden concept 1>
  2. <forbidden concept 2>

Verdict logic:
  - All required-present matched AND no required-absent matched → CORRECT
  - Anything else → WRONG
```

### Convergence verdict (across both models)

After both models produce answers and each is independently graded:

| Opus | Sonnet | Verdict |
|---|---|---|
| CORRECT | CORRECT | **CONVERGE** — file is clear on this rule |
| WRONG | WRONG | **DIVERGE** — rule is unclear to both models |
| CORRECT | WRONG | **DIVERGE** — rule reads differently to Sonnet |
| WRONG | CORRECT | **DIVERGE** — rule reads differently to Opus |

Both-CORRECT counts as CONVERGE *regardless of how their wording differs.*

### Reporting format

For each probe, after the round completes:

```
Probe N (Class A, derived from L13)

Question: <verbatim>

Opus answer: "<verbatim>"
  Rubric check:
    required-present 1: ✓ matched
    required-present 2: ✓ matched
    required-absent  1: ✓ none
    required-absent  2: ✓ none
  Verdict: CORRECT

Sonnet answer: "<verbatim>"
  Rubric check:
    required-present 1: ✓ matched
    required-present 2: ✓ matched
    required-absent  1: ✓ none
    required-absent  2: ✗ <forbidden concept> present
  Verdict: WRONG

Convergence: DIVERGE — Sonnet read the rule as admitting a carve-out
that does not exist. Source paragraph: <file>:<line range>.
Diagnosis: <text>. Proposed revision: <text>.
```

### Edge cases

| Case | Handling |
|---|---|
| Model gives correct answer + adds a wrong qualifier | Required-absent hit → WRONG |
| Both correct but verbose vs terse | Both CORRECT → CONVERGE |
| Model answers in list, other in prose | Format irrelevant — rubric scans concepts |
| Model partially answers (1/2 required-present) | WRONG — partial credit isn't enough |
| Model adds extra correct context not asked for | No required-absent hit → CORRECT |
| Rubric itself too lenient (catches false-CORRECT) | Caught at PR review or future learnings; tighten rubric next round |
| Rubric itself too strict (false-WRONG) | Caught when clearly-equivalent answer judged WRONG; amend rubric on the spot, re-evaluate |

### What is NOT used

- **No third LLM judge.** A judge subagent would itself have model-shape
  bias (an Opus judge would prefer Opus-shaped answers, vice versa) —
  defeating the dual-model neutrality. The driver evaluates against the
  rubric mechanically; the user signs off on the rubric.
- **No semantic-similarity scoring** (cosine/embedding distance). Too noisy
  at the granularity needed; would obscure substantive divergences.
- **No partial-credit grading.** Probes are binary CORRECT/WRONG.

---

## Workflow gates

### Approval gates

| Gate | When | Deliverables |
|---|---|---|
| Phase 0 PR | After style-spec draft | Approve / amend / reject `agent-writing-style.md` content |
| **Gate 1** (per file, before rewrite) | After step 7 of per-file workflow | Probes A+B, rubrics, coverage map (with override declarations) |
| **Gate 2** (per file, after rewrite) | After step 10 of per-file workflow | Class C probe + answer |
| Iteration-cap escalation (per file) | After round 3 of comprehension test fails | Diagnosis, divergent answers, proposed revision |
| Per-file PR | After self-review passes | Standard GitHub PR review |

### Iteration cap = 3

Per file, max 3 comprehension rounds. After round 3 if still diverging on
Class A or B, surface to user with:

- All three rounds' answers (Opus + Sonnet, side-by-side)
- The source paragraph
- Diagnosis: ordering effect (round 1 vs 2 vs 3 differed)? Persistent ambiguity
  (all rounds same wrong answer)?
- Proposed fourth revision

User decides: accept proposed revision, manually rewrite that section,
abandon fail-loud-ify of that section, or pause file pending judgment.

Class C divergence does NOT count against the cap — it's a setup-broken
signal, not a file-broken signal. Re-spawn subagents instead.

---

## Templates

Concrete patterns derived during the first execution of the per-file workflow
(issue #167, AGENTS.md rewrite). Future sessions applying this workflow to
other Phase 1 files (#168–#171) can imitate these for consistency.

### Branch naming

Pattern: `chore/<YYYY-MM-DD>-rewrite-<filename-kebab-case>` — drop the file
extension; convert dots and slashes to hyphens.

| File under rewrite | Branch name |
|---|---|
| `AGENTS.md` | `chore/2026-05-08-rewrite-agents-md` |
| `ai-docs/code-style.md` | `chore/2026-05-08-rewrite-code-style` |
| `ai-docs/doc-convention.md` | `chore/2026-05-08-rewrite-doc-convention` |
| `.claude/agents/self-review.md` | `chore/2026-05-08-rewrite-self-review` |
| `.claude/agents/review-findings.md` | `chore/2026-05-08-rewrite-review-findings` |

### Section inventory format

At workflow step 3, build a table with columns: `Section | Lines | Range | Share`.
Compute `Share = section_lines / total_file_lines`. Identify the largest
section — that's the candidate for the heavy-section override (mechanical >50%
or judgment-call rule-dominant) per the coverage rules.

### Cross-reference link audit

Pre-rewrite (after step 4) and post-rewrite (after step 17):

```bash
grep -rn '<file>#' ai-docs/ .claude/skills/ .claude/agents/
```

Pre-rewrite enumerates incoming anchor links. Post-rewrite verifies each
still resolves. Renaming or restructuring a `## Section` heading changes its
GitHub-style anchor (`#section-heading`); preserve the heading or update
siblings in the same PR.

### Probe drafting heuristics

**Class A (failure-targeted, 4–6 probes):**
- One per `learnings.md` misread event traceable to the file under rewrite
- Phrase as a realistic scenario asked as yes/no or what-do-you-do
- Include at least one detail that disambiguates the misread direction
  (e.g., explicitly offer (a)/(b)/(c) options to force a choice)

**Class B (failure-likely, 1–3 probes):**
- Target structural-complexity hot spots: nested conditionals, multi-axis
  decision trees, carve-out exemptions, cross-references requiring synthesis
- Place at least one Class B in a section with zero Class A probes —
  extends coverage beyond hindsight

**Class C (control, 1 probe):**
- Anchor on a stable, low-rule-density section (one that primarily points
  at another doc, or a short orientation paragraph)
- Required answer: a single literal token (file path, command, version)
- Verifies the test setup itself is intact

### Rubric authoring heuristics

For **Class A and B (prose-with-rubric):**

| Field | Items | Rules |
|---|---|---|
| Required-present | 2–4 | Each must be load-bearing (rule cannot be understood without it). Match leniently on synonyms (negation: `no` / `not authorised` / `forbidden` / `MUST NOT` all count). Match strictly on substantives (file paths, command names, specific triggers — verbatim). |
| Required-absent | 1–2 | Concepts that, if present, indicate the rule was misread in a known direction. Avoid common stop-words (don't required-absent `not` or `for example`). |
| Verdict | binary | ALL required-present matched AND NO required-absent matched → CORRECT. Anything else → WRONG. |

**Anti-rubrics — do NOT do:**

- Grade for exact wording (defeats dual-model neutrality)
- List required-present items that are synonyms of each other (graders end up checking the same concept twice)
- Required-absent on natural English phrases (`however`, `for example`)

For **Class C (literal-token):**

- Type: `literal-token`
- Required value: verbatim string from the rewritten file
- Comparison: case-sensitive substring search — model's answer must contain
  the literal value as a substring

### Subagent spawn

Spawn both models **in parallel** — single tool-use message with two `Agent`
calls:

```
Agent(
  description = "Comprehension test (Opus)",
  subagent_type = "general-purpose",
  model = "opus",
  prompt = <prompt template, see below>,
)

Agent(
  description = "Comprehension test (Sonnet)",
  subagent_type = "general-purpose",
  model = "sonnet",
  prompt = <IDENTICAL prompt>,
)
```

Both subagents receive identical prompts including the same shuffled probe
order — divergence is then attributable to the file's wording, not to setup
variance.

### Subagent prompt template

```
You are a comprehension-test subagent for the graphite-gp project's <FILE>
rewrite (issue #<N>). Your task is narrow and self-contained.

INSTRUCTIONS:

1. Read the file at <ABSOLUTE-PATH> — that file is the rewritten version
   under test.
2. Do NOT read any other file in the codebase.
3. Do NOT search for additional context, run grep, or open the web.
4. Answer each probe below INDEPENDENTLY, based ONLY on what's in the file.
5. Be brief: 1–4 sentences per answer. State the answer first, then briefly
   justify by referring to the relevant section.

OUTPUT FORMAT — for each probe, exactly this shape:

PROBE <N>:
<your answer>

PROBES (randomized order):

Probe 1: <verbatim Q>
Probe 2: <verbatim Q>
...

After answering all <N> probes, output a final line: `END OF PROBES`.
```

**Why each instruction is load-bearing:**

- "Do NOT read other files / grep / web search" — without this, subagents
  read sibling instruction files to "verify" cross-references and
  contaminate the test (model answers based on what *should* be true,
  not what the rewritten file says)
- "1–4 sentences" cap — keeps answers short enough for mechanical
  rubric-checking; longer answers risk burying the verdict-relevant token
- "State the answer first" — forces the verdict signal early; rubric
  matching becomes more reliable
- `END OF PROBES` sentinel — makes truncation detectable

### Probe order randomization

Per round, generate a fresh permutation of the probe set. Both subagents
get the **same** permutation in a given round — different orders across
models would introduce a confounding variable.

If round 2 is needed (some probes failed in round 1), shuffle again. If
round 2 with reshuffled order produces *different* convergence than round 1
on the same probes, that's diagnostic information about order-dependence in
the file's logical flow.

### Semantic-preservation subagent prompt template

Used at workflow step 16. Spawn with `model="opus"` — this review benefits
from deeper reasoning than the dual-model comprehension test does.

```
You are a semantic-preservation review subagent for the graphite-gp project's
<FILE> rewrite (issue #<N>). Your task is narrow and self-contained.

INSTRUCTIONS:

1. Read the OLD version of the file:
   `git show <BASE_COMMIT>:<RELATIVE-PATH>`
   This is the version before this PR's rewrite.
2. Read the NEW version of the file:
   `<ABSOLUTE-PATH>` (filesystem state — includes uncommitted edits)
   This is the rewritten version under review.
3. Do NOT read any other file (the rewrite is self-contained for this check).
4. Do NOT search for additional context, run grep on the wider tree, or
   open the web.

OBJECTIVE:

For every load-bearing rule present in OLD, verify whether it survives in
NEW. A "load-bearing rule" is anything an agent reading the file would be
expected to act on: prohibitions, requirements, command lists, file path
references, decision-tree branches, exemptions, edge cases.

OUTPUT FORMAT:

| # | Rule (from OLD) | OLD location | Verdict | NEW location |
|---|---|---|---|---|
| 1 | <one-line rule statement> | `<file>:<line>` | PRESERVED / WEAKENED / DROPPED | `<file>:<line>` or `—` |

Then a final summary block:

```
SEMANTIC PRESERVATION VERDICT: <PASS | FAIL>
- N rules in OLD
- M PRESERVED
- K WEAKENED  (must be 0 for PASS)
- L DROPPED   (must be 0 for PASS)
```

Definitions:

- PRESERVED: rule is unambiguously present in NEW with the same or stronger
  binding force (e.g., a paragraph rule promoted to AXIOM is PRESERVED).
- WEAKENED: rule is present but with reduced binding force (e.g., a
  "MUST NOT" softened to "should avoid", or a binary rule converted to a
  guideline). Or the rule lost an essential qualifier (e.g., an exemption
  was kept but its scope was widened/narrowed).
- DROPPED: rule has no equivalent in NEW.

Be skeptical. The rewrite was meant to clarify, not to soften. If a rule
appears in NEW but reads less binding than in OLD, mark it WEAKENED with a
specific reason.

After the table and summary, output a final line: `END OF REVIEW`.
```

**Why each instruction is load-bearing:**

- "Do NOT read other files" — prevents the subagent from cross-referencing
  sibling instruction files and judging based on their content rather than
  the file under review
- Forces a per-rule table — prevents narrative summaries that obscure
  whether specific rules survived
- Three-state verdict (PRESERVED / WEAKENED / DROPPED) is binary at the
  rule level — no partial credit; ambiguity defaults to flag-as-WEAKENED
- The summary's PASS/FAIL gate is explicit — N WEAKENED + L DROPPED must
  both be 0 for PASS; otherwise the parent agent must revise

### Class D calibration probe template (v5)

A calibration probe is a question whose answer is intentionally NOT pinned
down by the file under test. Both models converging confidently = test
setup is biased.

**Drafting heuristic:**
1. Read the file
2. Identify a topic the file mentions but doesn't pin down (e.g., ordering between two optional sections; severity assignment for an edge case; default behaviour where the file is silent)
3. Phrase as a question that could plausibly have multiple defensible answers
4. Document the verdict logic explicitly: "if both models confidently say X, that's training-data convergence not file clarity"

**Anti-patterns for Class D:**
- Drafting a probe whose answer IS in the file but obscure (that's a Class A/B difficulty probe, not calibration)
- Drafting a probe whose answer is genuinely random/unpredictable (no one model would converge confidently — defeats the purpose)
- Tying the calibration to a topic the file is silent on by oversight (it should be a deliberate non-coverage; otherwise calibration becomes a backdoor coverage test)

**Verdict:**
- Both models hedge or acknowledge the file's silence → ✅ healthy
- Both models confidently converge on same answer → ⚠️ SETUP-SUSPECT
- Models diverge (one says X, other says Y) → 🟡 expected

Class D failure does NOT count against iteration cap (it's setup signal).

### Open-ended probe template (v5)

```
Probe (Class A or B, open-ended):

Question: "Explain in your own words how <RULE> applies when <SCENARIO>.
What does the agent do, and why?"

Rubric (looser than closed-form):
  Type: prose-with-semantic-judgment
  Required-present (load-bearing concepts, any phrasing):
    1. <concept 1 — load-bearing>
    2. <concept 2 — load-bearing>
  Required-absent (concepts that indicate misread):
    1. <forbidden concept>
  Grader: parent agent applies semantic judgment for required-present
    (substring matching is too strict for open-ended). State the
    judgment criterion at probe-draft time (Gate 1).
  Verdict: same binary CORRECT / WRONG.
```

### Honest closing-comment template (v5)

When a file passes v5 and you write the issue closing comment, use this
language template instead of "the file is solid" or "the file is unambiguous":

```
The v5 methodology found no divergence on this file under the documented
test conditions:
- Step 12 dual-model comprehension test: <X>/<N> probes CONVERGE on round 1
- Step 12.5 historical replay: <Y>/<Z> documented misread cases caught
  by both models (or: not feasible — no documented misreads on this file's
  rules)
- Step 16 Opus semantic-preservation: <PRESERVED count>/<total> rules

This validates clarity-in-isolation against correlated readers and (where
replay was run) rule-fires-on-real-input on a sample of N documented cases.
It does NOT prove the file is unambiguous in production conditions.

See § Methodology limitations in the plan doc for the limits this
methodology has not addressed.
```

### Decoupled probe-author flow (v5)

To address probe-author selection bias (per § Methodology limitations), v5
prescribes that a separate Opus subagent drafts the probe set BEFORE the
parent agent runs the workflow:

1. **Probe-drafting subagent** (model="opus"): reads file + learnings.md + style spec, drafts probes A/B/C/D + rubrics + coverage map, surfaces to user (Gate 1).
2. **Workflow-execution agent** (parent / current session): receives the approved probe set, runs steps 9–18 against it, cannot influence probe wording.

This breaks the I-drafted-probes-I-expected-to-pass loop. The trade-off is one extra subagent invocation upfront (~15-30 min).

For Phase 1 retrospective (where this didn't fire), the existing replay batch (PR #176) provides external validation against ground truth — partial but real check on parent-agent bias.

### Reporting format for dual-model results

For all-CONVERGE rounds, a per-probe summary table is sufficient:

| Probe | Class | Topic | Opus | Sonnet | Verdict |
|---|---|---|---|---|---|
| A1 | failure-targeted | <topic> | CORRECT | CORRECT | ✅ CONVERGE |

For DIVERGE outcomes, add a per-probe detail block:

```
Probe N (Class X — derived from <source>)

Question: <verbatim>

Opus answer: "<verbatim>"
  Rubric check:
    required-present 1: ✓/✗ <reason>
    required-absent  1: ✓/✗ <reason>
  Verdict: CORRECT / WRONG

Sonnet answer: "<verbatim>"
  Rubric check: ...
  Verdict: ...

Convergence: DIVERGE — <one-sentence diagnosis>
Source paragraph: <file>:<line range>
Proposed revision: <text>
```

---

## Known biases & limitations

This section was added after Phase 1 completed (5 files all PASS round 1 with
no iteration). Reflecting on the all-PASS rate honestly: it is **either**
evidence the files were already clear, **or** evidence the test was not
sensitive enough to surface ambiguity. The available evidence does not
distinguish between these. Future rewrites and re-runs should account for
the limitations below.

### What the v4.2 methodology validates

A PASS verdict in v4.2 means: *current methodology found no divergence under
isolation conditions with two correlated models, on probes drafted by the
parent agent that drove the workflow.* It does **not** mean: *the file is
unambiguous in production conditions.*

### Known limitations

| Limitation | What it means | Why it matters | Future improvement |
|---|---|---|---|
| **Context-isolation bias** | Subagent prompts strip context (`Do NOT read any other file`) | Real `self-review` / `review-findings` agents read AGENTS.md, the diff, spec, and design before reaching the rule under test — by which point thousands of tokens have shaped their interpretation. The test answers "is this file clear when read alone?", not "is this file clear under realistic context load?" | Add a context-loaded probe round per file: subagent reads file + sister files (the ones it cross-references) + a synthesised realistic diff that exercises the rule. Convergence on isolation but divergence on context = "rule is clear in the abstract but gets lost in workflow noise." See [`## Phase 2 / future improvements`](#phase-2--future-improvements) below. |
| **Model-correlation bias** | Opus 4.7 and Sonnet 4.6 share training distribution and inductive biases | Identical prompts to two correlated models produce convergent answers via shared bias, not file clarity. A truly independent test needs an out-of-family model (GPT-class, Gemini-class). | **No current access to out-of-family models** in this repo's tooling. Acknowledged limitation, not addressable via methodology change. When the option becomes available, add a third subagent run per file. Until then, weight convergence between Opus + Sonnet as **"two highly correlated reads agree"**, not as "file is unambiguous to readers in general." |
| **Probe-author selection bias** | The parent agent (driving the workflow) drafts the probes after reading the file | Risk that probes are unconsciously phrased to surface the rules I expect to be clear, not the rules at risk. The Gate 1 user-approval step partially counters this but doesn't eliminate it. | Decouple probe-authoring from rewrite-execution: a separate Opus subagent reads the file + learnings + style spec, drafts probes blind to whether the file is "expected" to pass. The rewrite-execution agent (parent) cannot influence probe wording. |
| **Closed-question bias** | Probes are mostly yes/no, multiple-choice, or specific-fact | Closed answers produce convergence easily because the answer space is narrow. Open-ended probes ("explain how rule X applies to scenario Y in your own words") would surface interpretation differences that closed forms hide. | Mix in 1–2 open-ended probes per file. Rubric for open-ended is necessarily looser ("must mention concept A; must not contradict concept B") but divergence on open-ended probes is a stronger ambiguity signal than divergence on yes/no. |
| **No post-validation feedback loop** | After a file passes, we don't track whether real `learnings.md` events keep happening on its rules | The actual test of clarity is "do new misread events on this file's rules occur after validation?" If yes, the test was a false-positive of clarity. Without surveillance, validation is one-shot and unverified. | Post-validation surveillance: monitor `learnings.md` for ~30 days after a file is closed as PASS. New entries targeting that file's rules → add to the file's issue as a re-open trigger. Codify in this plan as a "post-validation surveillance" rule before next use. |
| **Step 16 minimal sample** | Opus semantic-preservation gate (the v4.2 distinctive contribution) fired exactly once across Phase 1 (#174 only — others had empty diffs) | Calling v4.2 "validated across 5 files" overstates evidence. Step 16 has 1 data point. | Run Step 16 explicitly on every Phase 2 file even when the diff seems trivially-preserving. Track Opus self-review verdicts across runs to build a track record on whether Opus catches subtle weakening Sonnet would miss. |
| **All-PASS-round-1 rate is a smell** | 45/45 probe convergences across 5 files, no iteration triggered | When every test passes on round 1 with no iteration, it's either (a) files genuinely clean, or (b) test not sensitive enough. Priors should split between these; Phase 1's results don't update strongly toward (a). | Add the calibration probe (below). If models converge confidently on a calibration-probe wrong answer, the test setup itself is converging on biases — strong signal that other PASS results are suspect. |
| **No calibration probe** | No probe with intentionally-ambiguous file content | Without one, we cannot detect when convergence is the test's bias rather than the file's clarity. | Per file, include 1 calibration probe whose answer is **intentionally underspecified** in the file (a rule the file doesn't actually pin down). Both models converging on the same confident answer = setup itself is converging on training-data biases. Calibration failure = strong signal that PASS results from other probes are suspect. |

### Honest framing in PR / issue closing comments

When a file passes v4.2, the closing comment should say:

> The v4.2 methodology found no divergence on this file under the
> documented test conditions. This validates clarity-in-isolation against
> correlated readers, not clarity-in-production-context. See
> [`## Known biases & limitations`](#known-biases--limitations).

It should NOT say "the file is solid" or "the file is unambiguous". The
Phase 1 closing comments (#168, #169, #170, #171) do use the stronger
phrasing — those wordings overstate what the methodology proves.

### Phase 2 / future improvements

When v4.2 is next applied (Phase 2 procedural skills, or a new instruction
file added later), the workflow should be amended to address the limitations
above. Concrete changes for next use:

1. **Two probe rounds per file** — isolation round (current) + context round
   (file + sister files + synthesised realistic diff).
2. **Decoupled probe-author** — separate Opus subagent drafts probes, parent
   agent runs the workflow.
3. **Mix in open-ended probes** — 1–2 per file, looser rubric.
4. **Add a calibration probe** — 1 per file, intentionally underspecified.
5. **Post-validation surveillance** — monitor `learnings.md` ~30 days
   post-PASS; new entries trigger re-open.
6. **Step 16 always fires** — even on empty diffs (returns trivial PASS but
   builds the run record).
7. **Honest framing** — closing comments distinguish "no divergence under
   test conditions" from "file is unambiguous in production".

Out-of-family model is **aspirational, not currently feasible** — no API
access in this repo's setup at present. When the tooling supports it, add
as an 8th change (third subagent run per file).

### Phase 1 retrospective summary

| Aspect | Verdict |
|---|---|
| Probe + rubric + structured-answer framework | ✅ Genuine improvement over no testing |
| Style-violation scan (step 9) | ✅ Mechanical, cheap, catches real anti-patterns |
| Heavy-section override mechanism | ✅ Worked correctly on `code-style.md` |
| Cross-reference link audit | ✅ Mechanical, would have caught real anchor breakage |
| Probe-design lessons (avoid symmetric multi-choice; confusion-traps in required-absent) | ✅ Concrete portable knowledge |
| Issue-tracking decoupled from PRs (closing comments as record) | ✅ Future readers can audit |
| Strong validation of file clarity | ❌ **Overstated** — see limitations table above |
| Step 16 (Opus self-review) cross-run track record | ❌ Only one data point (#174) |
| Detection of methodology biases | ❌ Not built in — discovered post-hoc |

Phase 1 was a **methodology shakedown** that produced good infrastructure
and concrete lessons. The infrastructure is reusable; the file-clarity
claims are weaker than the closing comments suggest.

---

## Historical replay testing

Phase 1 retrospective ([`## Known biases & limitations`](#known-biases--limitations))
identified that v4.2 synthetic-probe methodology validates clarity-in-isolation
against correlated readers, but doesn't measure whether rules actually fire on
real input. **Historical replay testing** addresses this directly: feed a real
historical buggy state to the current `self-review` / `review-findings` agent
and check whether it catches the documented misread.

Treat as a **complement** to v4.2, not a replacement. v4.2 = cheap clarity
stress-test for new files; replay = expensive but high-information-value deep
validation against documented misreads.

### When to run

- After a major instruction-file rewrite — confirms whether the rewrite
  preserved enforcement, not just clarity
- When `/improve` proposes escalating a rule that was previously v4.2-PASS —
  replay confirms whether the proposed escalation is necessary, or whether
  the existing rule was already sufficient and the misread was a one-off.
  **A `/improve`-proposed escalation on a v4.2-PASS rule is the canonical
  re-open trigger** for the corresponding closed Phase 1 issue.
- Periodic surveillance on a rotating set of historical cases

### Methodology

**Inputs per case:**
1. A `learnings.md` entry with file/line locations of the documented misread
2. The buggy commit SHA (state where the misread was present)
3. (Optional) historical context — spec/design from the PR

**Setup — git worktree:**

```bash
# 1. Create replay worktree at the buggy state
git worktree add /tmp/replay-<short-sha> <BUGGY_COMMIT>

# 2. (Variant A — recommended) Overlay CURRENT instruction files
cd /tmp/replay-<short-sha>
git checkout main -- AGENTS.md \
  ai-docs/code-style.md ai-docs/doc-convention.md ai-docs/agent-writing-style.md \
  .claude/agents/review-findings.md .claude/agents/self-review.md
```

**Variants:**

| Variant | Tests |
|---|---|
| **A — replay-on-current-rules** (overlay current instruction files) | Whether *current* rules would catch the misread. Higher-info test for retrospective. |
| **B — replay-on-historical-rules** (no overlay) | How the agent actually performed at the time. Baseline / regression check. |

**Subagent prompt** (review-findings, whole-codebase):

```
You are running as the review-findings agent on a graphite-gp codebase review.

CRITICAL paths:
- Codebase under review: /tmp/replay-<sha>
- Use ABSOLUTE paths starting with /tmp/replay-<sha>/ for ALL reads
- Do NOT read from the main checkout — different unrelated codebase

PROCEDURE:
1. Read /tmp/replay-<sha>/.claude/agents/review-findings.md and follow it
2. Read AGENTS.md, ai-docs/code-style.md, ai-docs/doc-convention.md from worktree
3. Read every *.spec.md and *.design.md in /tmp/replay-<sha>/ai-docs/plans/done/
4. Walk the source tree
5. Apply checklist mechanically. Output findings.
```

For `self-review` (diff-based), additionally specify the base_commit and HEAD.

**Dual-model:** spawn Opus + Sonnet in parallel, same as v4.2.

**Ground-truth comparison:**

| Subagent flagged the documented violation? | Outcome |
|---|---|
| Yes, correct location + severity | ✅ True positive — rule fires on real input |
| Yes, wrong severity | ⚠️ Partial — caught but undermarked |
| No (or different findings, missed the known violation) | ❌ False negative — **re-open trigger** for the corresponding v4.2-PASS Phase 1 issue |

**Cleanup:**

```bash
git worktree remove /tmp/replay-<sha> [--force]
```

### Methodology lessons (from replay experience)

- **Replay surfaces both ground-truth signal AND noise.** Current-rules-on-historical-code may flag historical code that was correct under historical rules. Frame ground truth as the test signal; surface additional findings separately — they're audit byproduct, not test failure.
- **Ground-truth labelling requires care.** Pull file/line and rule from the `learnings.md` entry + the fix-PR's diff + the escalation PR's instruction-file update.
- **Spec/design unavailability:** older PRs may not have these. `review-findings` (whole-codebase) avoids spec-construction; `self-review` (diff-based) needs a synthesised minimal spec.
- **Methodology divergence vs file-clarity divergence:** if Opus and Sonnet flag the violation but disagree on grouping or severity, that's a rule-wording question, not a file-clarity failure. Don't conflate.

### Cost vs v4.2 synthetic-probe rerun

Roughly the same cost (~1–2 hours per case for setup + dual-model run + analysis), but **much stronger evidence per case** because the test signal connects to documented ground truth rather than author-selected probes.

### Illustrative replay case (schematic)

graphite-gp is a fresh repo with no replay history yet; the shape below is
illustrative, drawn from prior application of the methodology. A single
replay case looks like:

**Setup:**
- Buggy commit: `<BUGGY_SHA>` (state where the documented misread was present)
- Variant A — current instruction files overlaid on the historical worktree
- Documented misread (ground truth): a `learnings.md` entry naming the exact
  file/line and the rule it violated (e.g., a safe fn wrongly suffixed
  `_unchecked`, or a float used inside `gp-core` `geom` / `sim`).

**Subagents:** Opus and Sonnet spawned in parallel via `general-purpose`
`Agent` calls with `model` overrides.

**Result taxonomy:**

| Subagent flagged the documented violation? | Outcome |
|---|---|
| Yes, correct location + severity | ✅ True positive — rule fires on real input |
| Yes, wrong severity | ⚠️ Partial — caught but undermarked |
| No / missed the known violation | ❌ False negative — **re-open trigger** for the corresponding closed clarity issue |

### Further methodology lessons

- **Grouping-strategy divergence is recurring, not random.** Opus and Sonnet
  tend toward different default groupings (by-pattern-type vs by-file), both
  consistent with the same rule wording. Future probe-design and rule-wording
  work should expect this divergence and either tighten the grouping rule or
  accept both forms.
- **The methodology-fit limitation is real but bounded.** Rules whose
  enforcement is in shell-command form (clippy invocations, CI workflow
  gates) aren't replayable — `review-findings` / `self-review` review source
  code, not shell commands. Document such rules as out-of-scope for replay.
- **Replay surfaces audit byproducts.** Every run produces additional
  findings beyond ground truth (current rules applied to historical code).
  For replay-test purposes these are noise; for codebase-quality purposes
  they may be real ongoing issues or rules that postdate the historical
  commit. Each needs separate adjudication.

### Future replay candidates (if undertaken)

Once graphite-gp accumulates a correction history, target rules with the
highest correction frequency. Candidate shapes:

| Candidate | Why | Tests rule |
|---|---|---|
| Integer-only `gp-core` (`geom` / `sim`) | Core determinism invariant | No-floats-in-core rule |
| `_unchecked` suffix on a safe fn | Naming / safety carve-out | The `_unchecked` AXIOM |
| Magic number left inline instead of a `const` | High-frequency style rule | Magic-numbers rule |
| Missing `#[cfg(test)] mod tests` on a ~50+-line logic file | Test-coverage carve-out | Test-coverage rule |

Each ~1–2 hours per case if rule-shaped (vs process-shaped).

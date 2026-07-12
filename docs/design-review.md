# Design review — `racetrack_design.md`

Reviewer pass over [`design.md`](design.md). Goal: pressure-test the invariants and
the load-bearing claims, surface gaps before they become code. Findings are ranked
by severity, each with a concrete recommendation. Section refs (§) and phase refs
(Ф) point into the design doc.

**Overall verdict.** The design is strong and unusually coherent — the point/wall
duality is a genuinely good load-bearing invariant, and the 3a/3b split, the
masked-softmax policy, and the "oracle as certifier, not generator" philosophy are
all the right calls. The issues below are mostly (a) two internal contradictions,
(b) a few load-bearing pieces that are underspecified, and (c) one design tension
between the track model and the collision model that undercuts a stated goal.

---

## Claims I checked and believe are sound

Worth stating explicitly, because the rest of the doc leans on them:

- **`dilate(P) \ P` is an annulus** (§2 Ф1). Dilating a simply-connected polyomino
  by a convex block stays simply connected; subtracting `P` leaves exactly one hole
  (`P` itself, ≥1 cell). Annulus-by-construction holds.
- **Signed S/F counter sufficiency** (§3). The full-chord cut argument is correct: a
  full chord turns the annulus into a simply connected strip, so you can only return
  to the line by backing off (−1 cancels +1) or a full lap. Good.
- **Oracle iterative-deepening termination** (§3). "Stop when `max|v|` among *live*
  states `< V`" is sound: higher speed never helps braking, so a state that can't
  reach closure at speed `s` can't at `s' > s`; if no live state hit the ceiling, the
  ceiling wasn't binding → true `Vmax_attain`. The `V = 1,2,4,8…` doubling can run the
  final BFS at up to 2× the needed ceiling (minor waste), but the result is correct.
- **Live set = `R ∩ B`** (forward-reachable ∩ backward-reachable-to-closure). Correct
  notion; provably-crashing high-speed states fall out of `B`. Clean.
- **V=1 liveness is sound *and* complete for "a lap exists"** (§3) — but see M4: it is
  essentially equivalent to the Ф4 flood-fill, so its marginal value is small.

---

## 🔴 Correctness / soundness

### C1 — The progress reward contradicts itself (PBS vs. "new-max" ratchet)

§5 specifies the progress term two different, incompatible ways:

- In the reward block: `w_prog · Δs_normalized  # ТОЛЬКО за новый максимум s` — a
  positive-only **ratchet** (backward motion costs nothing, forward-past-max pays).
- Later: *"Безопасный shaping — potential-based (`γΦ(s') − Φ(s)`, `Φ = s`): не меняет
  оптимум."* — classic **potential-based shaping** (PBS), which is signed and
  telescoping.

These are not the same mechanism and cannot both hold:

- PBS with `Φ = s` is provably policy-invariant (Ng–Harada–Russell 1999). It is
  *signed*: moving backward yields negative shaping, so a forward-then-back excursion
  telescopes to ≈ 0. **PBS already prevents reverse-farming on its own** — the "new
  max" rule is then redundant.
- The "only new max s" ratchet is *not* of the form `γΦ(s') − Φ(s)` (it's clipped at
  0, non-telescoping), so it is **not** PBS and carries **no** optimum-preservation
  guarantee. It can bias the optimal policy.

The only way to salvage the ratchet as PBS is to make the potential `Φ = s_max` *and*
add `s_max` to the state (then it's `γΦ(s')−Φ(s)` with `Φ` = running max, valid but
`Φ ≠ s`). The doc writes `Φ = s` **and** "only new max," which is the inconsistent mix.

**Recommendation.** Drop the ratchet, use pure PBS `r = γΦ(s') − Φ(s)`, `Φ = s`. It is
provably safe and already kills reverse-farming. This also removes the need to track a
per-episode running max.

### C2 — Lap counter vs. "cars start on the S/F line" is an off-by-one

§3 initializes the counter to `−1` ("first forward cross = race start"). But Ф3 places
cars *"на самой линии"* (on the line itself), with extra rows *behind* it. A car sitting
exactly **on** the timing line at `t=0` has an ill-defined "first forward crossing":

- If leaving-the-line-forward from an on-line start is **not** a transversal crossing
  (its segment's endpoint lies *on* the line), the front row only scores its first `+1`
  after a full loop → **the first completed lap registers as 0**.
- If it **is** counted, then front-row cars (on the line) and back-row cars (behind the
  line, which genuinely cross it on lap 1) get **inconsistent** initial counts.

Either way the semantics are undefined for the front row, and the degenerate cases
(segment endpoint exactly on the line, or grazing the chord's end) need a tiebreak.

**Recommendation.** Decouple the **start grid** from the **timing line**: put the timing
line a fraction ahead of the front row so every car is strictly behind it at `t=0`. Then
every car's first forward crossing is unambiguous and uniform, and the `−1` init is clean.
Specify the segment/chord crossing test as a half-open interval (e.g. count a crossing
iff `from` is strictly on the −side and `to` is on the +side or on the line) to kill the
endpoint-on-line degeneracy.

### C3 — Ф6's "local oracle" is not sound for run-out fixes

§2 Ф6 claims each edit needs only *"затронутые чеки + локальный оракул."* For pure
**validity** (does a lap still exist) this is mostly fine — widening never destroys an
existing slow completing lap, and cell removals have locally-checkable connectivity/hole
effects. The problem is the **dynamic** repairs, e.g. *"поворот не тормозится → удлинить
предшествующую прямую."*

Braking feasibility at a corner is a function of the **maximum reachable entry speed**,
which is set by the *entire upstream* straight, not the local neighborhood. Lengthening a
straight to fix corner X's run-out simultaneously *raises* the speed cars arrive at X
with. A fixed-radius "local oracle" that doesn't propagate the new upstream speed can
report X as fixed while it is still un-brakeable at the new max entry speed.

**Recommendation.** After a dynamic edit, re-run reachability not over a fixed radius but
from the nearest **upstream speed sink** (a point where all live states are already slow —
a hairpin or the S/F accel-zone start) forward through the edited region to the next sink.
That is the correct locality for `race_dir`-directed dynamics; "local" needs defining as
"sink-to-sink," not "N cells around the edit."

### C4 — Supercover's exact-corner rule is the single most correctness-critical detail, and it's only described in prose

Everything (`legal_move`, the oracle edge, the "needle's-eye" guarantee) rides on
**strict** supercover including corner-clipped cells (§3). The behavior that actually
matters is the tie case: a segment passing **exactly through a dual vertex** (e.g. a pure
`(1,1)` diagonal move through the corner shared by 4 cells). To forbid squeezing between
two diagonally-placed walls, strict supercover must require **all four** cells sharing that
vertex, not just the two endpoints. Off-by-one here silently allows illegal diagonal slips
or forbids legal moves — and it's the same code in the runtime rule *and* the oracle, so a
bug corrupts validation and play identically.

**Recommendation.** Specify supercover as an exact integer predicate (no floating point):
define precisely what "touch" means at exact corner/edge crossings, and lock it with a
table of hand-worked cases (axis-aligned, `(1,1)`, `(2,1)`, a chord grazing one corner, a
chord through two collinear vertices). This is the first thing to unit-test in 3a. *(The
`supercover` stub in `crates/core/src/geom.rs` already carries this TODO — this review
raises its priority to "spec + test before anything else in 3a.")*

---

## 🟠 Design tensions / underspecified load-bearing pieces

### D1 — The collision model dissolves the very contention the width model creates

The width design deliberately manufactures scarcity: `n = ⌈m/2⌉` bottlenecks that are
*half* the width of the `≥ m` start (§1), so `m` cars funnel into a space that fits `⌈m/2⌉`
abreast — intended overtaking drama. But the collision model (§3) resolves only cars that
**end a turn in the same cell**, via nearest-free geodesic teleport, **keeping velocity**,
and does **not** model mid-segment interaction (two cars whose move-segments overlap or
swap cells pass through each other). Net effect: a narrow section imposes no real queuing —
cars overlap through it and get teleported apart at turn end with speed intact. **The
bottleneck drama the track model works to create is undercut by the collision model.**

This is a legitimate v1 simplification (cars are points; only final positions deconflict),
but it conflicts with the stated purpose of the width rules.

**Recommendation.** Decide explicitly which world you're in: (a) accept that narrows are
positional/visual only and stop justifying `⌈m/2⌉` as a passing-room constraint; or (b) add
real occupancy — a car may not *end* (and ideally not *pass through*) a cell held by a
non-yielding car — which makes bottlenecks bite but is a real physics change (and needs a
mid-segment blocking rule). At minimum, add a same-turn **swap/pass-through** check so two
cars can't trade cells or thread through each other unmodeled.

### D2 — "centerline(s)" is load-bearing but its construction is unspecified — and it's conflated with the medial axis

`centerline(s)` feeds the AI frame (`v_tangent/v_normal`, lateral distances), the reward
(`Δs`), and the render ideal line, and §6 correctly flags it as a first-class product. But:

- **Construction is undefined.** For a variable-width, non-convex, S-shaped annulus, a
  *unique, monotone-parameterized* centerline is nontrivial. `Δs` progress needs a globally
  monotone `s`; naïve "nearest point on a polyline" makes `s` jump or fold near wide bays
  and hairpins, corrupting the reward.
- **Two different "centers" are conflated.** Width validation (§2 Ф4) wants the **medial
  axis / distance-transform ridge** (a geometric object, available in Ф4 with no
  parameterization). The **racing centerline** parameterized by `s` along `race_dir` is a
  separate Ф7 object for AI/reward/render. The doc uses "centerline" for both; they are not
  the same and shouldn't share a definition. (This also resolves the apparent Ф4→Ф7 ordering
  circularity: Ф4 uses DT/medial-axis, not the `s`-parameterized centerline.)

**Recommendation.** Specify the racing centerline as an explicit construction (e.g. medial
axis → prune to the loop → arc-length resample → monotone `s` closed on itself) with a
stated guarantee that projection `pos ↦ s` is single-valued and monotone within the
corridor. Name the width object "medial axis" separately.

### D3 — `V` is used as both "arbitrary scaffolding" and a geometry-sizing constant

§3 insists `Vmax` is *derived*, and `V` is only "строительные леса" (scaffolding) for the
finite BFS — arbitrary. Yet §2/§3 size real geometry to it: acceleration zone `~V²/2`,
run-out `~v²/2`. If `V` is an arbitrary search bound, sizing the start straight to `V²/2` is
circular; if it's a gameplay target top speed, it's not arbitrary. The two roles are
conflated under one symbol.

**Recommendation.** Split into two symbols: `V_target` (a *design* top speed used to size
straights and the accel zone in generation) and `V_ceil` (the BFS scaffolding bound, driven
by iterative deepening and not a geometry input). Size geometry to `V_target` / the longest
straight; let `V_ceil` float.

### D4 — The crash anti-abuse argument is incomplete

§3 rejects in-place `v=0` as "brake-by-crash" abuse, then leans toward direction-preserving
damping (zero normal, tangential `/2`). But the abuse isn't only about reaching a *controlled*
`v=0` — it's about **cheap deceleration**. Halving tangential speed in a single tick is a
larger decel than honest braking (`−1`/turn) can ever achieve, so "clip the wall to scrub
speed" can still be net-cheaper than braking for high entry speeds. The claim *"Краш не даёт
контролируемого v=0 → абуза нет"* addresses the wrong axis.

Also underspecified: when **all 5 moves leave `D`**, they may exit through **different** edges
with different normals — "the normal of the edge supercover exited through" is ambiguous
(which edge? the first hit along the swept segment? the corner case hits two).

**Recommendation.** Make the deterrent a **cost**, not a kinematic detail: the real anti-abuse
lever for the AI is `P_crash` (already there); for a human, it's the time/position loss from
repositioning. Add an explicit cost so a crash is *strictly* worse than honest braking at
every speed — e.g. respawn at last valid cell with normal→0, tangential→`⌊t/2⌋`, **plus one
tick of no re-acceleration** (a "scrub" turn). Define the crash normal as the wall hit first
along the swept segment; for a corner (two walls), zero both offending components. See the
Open-questions recommendation below.

---

## 🟡 Minor / polish

- **M1 — `v=0` degeneracies in the feature set (§5).** Several features are built on a
  *velocity* heading and blow up at the start state (which is `v=0` for *all* cars): look-ahead
  distance `d ∝ v` collapses to 0 (blind exactly at launch), `free_dist_ahead` has no heading,
  `free_dist_ahead/(v²/2)` is `0/0`. Anchor all of these to the **centerline tangent** (always
  defined) rather than velocity, and floor `d` at a small minimum. (The doc already uses a
  centerline frame — just make "ahead" mean centerline-ahead, and guard the divisions.)
- **M2 — Rival intent is genuinely partially observable (§5).** "No memory needed" is airtight
  for the single-agent Markov argument, but a rival's *pending action* (about to brake / turn
  into you) is not recoverable from its position+velocity. Wheel-to-wheel, a rival's relative
  *acceleration* or a 2–3 frame stack may matter. Keep the no-LSTM stance, but treat the frame
  stack as likely-needed for the multi-agent phase, not just a lookahead-truncation fallback.
- **M3 — Replay determinism vs. float features.** 3a is integer and bit-deterministic (good),
  and collision RNG is seeded (good) — but bot actions depend on `f32` features, which are not
  guaranteed identical across platforms/compilers. A replay that includes bots can therefore
  diverge even with a fixed seed. Either compute features in fixed-point, or store bot actions
  in the replay (not just the seed).
- **M4 — V=1 liveness ≈ Ф4 flood-fill.** Sound and complete (verified above), but because you
  can always brake to `v=0` and step 4-connectedly, V=1 liveness reduces to "`D` is a
  4-connected annulus with a crossable S/F" — which Ф4 already checks. Its real added value is
  re-validating the S/F crossing mechanics, not liveness. Fine to keep, but the framing
  oversells it as a distinct dynamic check.
- **M5 — "Связность движения — 4-связность" is a misnomer (§1).** Cars do **not** move
  4-connectedly — velocities can be large and diagonal (e.g. `(2,3)`); only *acceleration* is
  von-Neumann (no diagonal accel per turn), and motion legality is chord+supercover. The
  4-connectivity is a property of the **corridor/analysis graph** (topology, width, V=1
  liveness), not of vehicle motion. Reword to avoid implying cars step to 4-neighbors.
- **M6 — Cosmetic wall smoothing vs. supercover-grazing (§4).** Chaikin-rounded walls at
  concave corners can visually appear to clip a car that legally grazes the corner (or vice
  versa), since the logical `D`/supercover is untouched. Low-priority UX mismatch; worth a note
  so the renderer doesn't round *into* a cell the physics treats as grazeable.
- **M7 — "Единственный failure mode" (§6)** is a mild oversimplification — it covers validity
  failures (merge / pinch / impassable corner) but not *quality* failures (passable but boring:
  a single viable line, or a track whose only completing lap is trivial). Those are metric, not
  validity, issues — fine to exclude, but don't imply the three variants exhaust everything that
  can go wrong with a generated track.

---

## Open questions — recommendations

**Crash rule (§3, marked [OPEN]).** Recommend: respawn at last valid cell; zero the
into-wall normal component; keep tangential with strong damping (`⌊t/2⌋`); **add a one-tick
"scrub" where the car cannot re-accelerate**; fail-safe halves again down to `v=0` if no legal
move exists. The scrub tick is what makes a crash *strictly* dominated by honest braking at
every speed (fixing D4). If the damping calibration proves finicky in practice, fall back to
the simpler, more predictable option 1 (`v=0` at last valid cell + skip `P` turns) — it has the
same anti-abuse property via time cost and no direction-ambiguity to resolve.

**Width taper thresholds (§2).** Keep the nominal `k` everywhere except designated
"technical" sections; only there taper down toward `⌈m/2⌉`. Enforce: no concave width *step*
larger than 1 point, taper spread over ≥ a few columns, and a post-taper check that no
concave corner is one that supercover would clip at plausible entry speeds. Tie the "designated
technical section" choice to the generator so bottlenecks land where overtaking is intended
(and revisit given D1).

**Reward weights (§5).** Resolve C1 first (pure PBS, `Φ = s`). Then the doc's component order
is right — `w_prog` + `c_time` (drives, and fast?) → `P_crash` (risks but doesn't ram or
freeze?) → `B_lap`/`B_rank` (races for position?). Start with the terminal weighted to clearly
dominate the *accumulated* dense return over a full lap (so the bot optimizes winning, not
"driving prettily"), and set `P_crash` just above the progress lost to an honest braking
sequence from typical corner-entry speed.

---

## Suggested priority order

1. **C4** (supercover exact spec + tests) — foundational; blocks all of 3a.
2. **C1** (pick pure PBS) — cheap, removes a real inconsistency before any RL runs.
3. **C2** (decouple timing line from start grid) — cheap, prevents an off-by-one baked into
   both the counter and Ф3 placement.
4. **D2** (define the racing centerline construction + separate it from the medial axis) —
   unblocks AI features, reward, and Ф4 width.
5. **D1** (decide the collision/contention model) — affects both track-width rules and physics.
6. **C3 / D3 / D4** and the open-questions recommendations as those blocks come up.

---

# Round 2 — re-review of the revised doc (+ generation pseudocode)

The doc was revised against Round 1. **All of C1–C4, D1–D4, M1–M7 are addressed**, and
two are resolved better than proposed:

- **C2** — the timing gate is now a **dual edge on the half-grid**, so a car (always on
  an integer point) can never be geometrically "on" it. Degeneracy removed at the source,
  not just offset.
- **D2** — `s` is now a **scalar field** on `D` (graph-distance / potential from the gate),
  not a nearest-point projection onto a curve. This is the right call — it sidesteps the
  fold-near-hairpins problem completely. (But see N1.)

Verification: C1 pure PBS ✅ · C3 sink-to-sink + cheap-global fallback ✅ · C4 exact integer
predicate with tie-case + test table ✅ · D1 option (a) + mandatory swap check ✅ · D3
`V_target`/`V_ceil` split ✅ · D4 scrub-tick rule finalized ✅ · M1–M7 all folded in ✅.

New findings, from the added pipeline pseudocode (§2) and the finalized rules:

## 🟠 N1 — the `s`-field folds on a loop unless it's computed on the annulus *cut at the gate*

`s_field = graph_distance_from(sf, along=skel.dir)` (Ф7) is underspecified in a way that
breaks the reward. Undirected BFS distance from the gate over the **full** ring is *not*
monotone in `race_dir`: distance grows going both CW and CCW away from the gate and meets at
the **antipode**. Past the antipode, moving forward in `race_dir` makes naive distance-to-gate
*decrease* → `Δs < 0` → a false "reverse" signal for the entire second half of every lap.
(The discontinuity *at the gate*, where `s` resets `L → 0`, is expected and fine — the lap
counter handles the lap; the problem is the fold at the antipode.)

**Fix:** define `s` as BFS distance on the annulus **cut at the gate** — seed at the gate's
forward (`+race_dir`) face and treat the gate edges as barriers so propagation can't wrap
around. That yields a single-valued, monotone `0 → L` coordinate, which is what `Δs` needs.
"graph_distance_from(sf)" should read "graph_distance on `D \ gate` from the forward face."

## 🟡 N2 — swap/pass-through is specified as *detection*, not *resolution*

§3 (D1) correctly mandates a same-turn check that two cars don't trade cells or thread through
each other — but only the **detection** is specified. What *happens* on a detected swap is
undefined (who yields? does it become a same-cell collision routed through the nearest-free
BFS? a mutual crash? a no-op block?). Pick a rule; the natural one is to fold a detected swap
into the existing collision layer (treat the contested edge as a same-cell contest and run the
seeded nearest-free placement), so there's one resolution path.

## 🟡 N3 — the real risk in generation is `frontier_gap → concrete edit`, and it's abstracted away

`phase5_full_oracle` returns `break_points = frontier_gap(R, goal)`, and `phase6_local_repair`
matches issue types to edits. Mapping a *reachability shortfall* (the oracle stalled here) back
to the *right dual-edge to move* is the crux of whether "almost-valid by construction + oracle
certifies + local repair" actually converges — and it's the one step treated as a given helper.
Recommend prototyping this mapping first when block 1 starts (on a hand-built almost-valid
track), before the rest of the pipeline; it's where the approach most plausibly fails to
converge and falls back to full reseeds.

## 🟡 N4 — small loose ends in the pseudocode

- **Unbounded outer loop.** `loop:` (reseed) has no max-seed budget; only the inner repair
  loop is bounded. Add a seed budget and a failure return so `generate_track` can't spin forever.
- **`race_dir` not threaded.** `oracle_liveness_V1` / `phase5_full_oracle` reference `race_dir`
  in their bodies but it isn't in their signatures (only `skel` / `sf` are passed). Thread it
  explicitly (carry it on `sf`, or pass `skel.dir`).
- **Seed not threaded.** The reseed loop and `random_*` calls need an explicit seeded RNG for
  the replay-determinism the design relies on elsewhere.
- **Start straight behind the grid.** Ф3 ensures the accel zone *ahead* of the gate is
  `≥ V_target²/2`, but with back-rows extending `-race_dir` from the front row, nothing checks
  there's enough straight *behind* the gate to hold all start rows in `D`.

## 🟡 N5 — D4 "scrub-tick makes a crash strictly worse than braking at any speed" is slightly overstated

The scrub-tick is a *constant* one-tick cost; the crash's tangential halving is a *geometric*
speed cut. At high enough entry speed, "clip the wall to halve speed + 1 scrub tick" can still
beat linear `−1`/tick braking on tick-count. What actually makes crashing dominated in a *race*
is that it pins your progress (`s` doesn't advance) and costs position — i.e. `P_crash` (AI) and
the time/position loss (human), which the doc already names as the primary levers. Recommend
softening the claim to rest on the progress/position cost, not the kinematics.

---

# Round 3 — converged

All of N1–N5 are incorporated correctly (N1 as a cut-annulus `s`-field, N2 swap→nearest-free,
N3 as an explicit `map_frontier_gap_to_edge` + "prototype first" callout, N4 seed/rng/`race_dir`
threading + straight-behind check, N5 softened). The design is coherent enough to build from.
Two precision notes, neither blocking:

## P1 — under pure PBS, N1 was a *signal-quality* fix, not an optimum-correctness one (and that's reassuring)

The status line calls the antipode fold a "correctness bug." More precisely: because the reward is
pure potential-based shaping (C1), the optimal policy is invariant for **any** potential `Φ`
(Ng–Harada–Russell) — so *no* `s`-field, folded or not, can move the optimum. The fold still had to
be fixed because a `Δs ≈ −L` spike once per lap is an enormous *misleading gradient* (variance,
unstable learning), not because it shifted the optimum. Useful corollary: **minor residual
non-monotonicities in `s`** (small BFS-geodesic wobble across wide pockets) **are harmless** —
they're shaping-only. `s` needs to be fold-free, not perfect.

## P2 — one loose thread between D2 and M1: where does the AI's centerline *frame* come from?

D2 says the racing-line **curve** is "only for render" (pseudocode line 186), but the AI features
(M1) need a centerline **frame** — `v_tangent`, `v_normal`, `dist_left/right` — i.e. a tangent/normal
at the car's cell. If that tangent is read off the render curve, the AI depends on the curve and
D2's "render-only" separation is incomplete; block 1 would then owe block 4 a high-quality
parameterized curve, not just the field.

**Recommendation:** derive the AI frame from the **`s`-field gradient** — `t̂ = normalize(∇s)`
(always defined in the strip interior; at the gate the tangent is just `race_dir`), `n̂ ⟂ t̂`,
lateral distances along `±n̂`. Then the curve stays genuinely render-only and block 1's contract to
block 4 is exactly `{ D, walls, sf, race_dir, s_field }` — no curve. Worth one sentence in §2/§5 so
block 4 doesn't reinvent the tangent or accidentally couple to the render spline.

**Status:** P1 folded in (§2, "[P1]"). P2 folded into §2 (Ф7 export split; "[P2]" note) and §5/M1
(tangent from `∇s`). ✅

## P3 — the P2 rename didn't reach §6; the architecture diagram still says `centerline(s)`

Found while verifying the P2 application. §2 and §5 now agree that the block1→block4 contract is
`{ D, walls, sf, race_dir, s_field }` and the racing-line curve is render-only — but §6 still
carries the old bundled `centerline(s)` in two spots, contradicting that contract:

- **Diagram edge `[1 Generation] ──…, centerline(s)──▶ [3a Core]`.** Post-P2 the product flowing to
  the core/AI is `s_field`; the `centerline` curve belongs on the `[1] ──▶ [2 Render]` edge (the
  ideal line), not this one.
- **Work-order bullet "`centerline(s)` на блок 1".** The backward requirement block 4 imposes on
  block 1 is the **field**, not the curve → should read `s_field`.

Cosmetic (it's the diagram, not the spec text), but it would mislead whoever implements block 1's
output. Fix: swap both `centerline(s)` references in §6 to `s_field`, and optionally add
`centerline` to the `[1]→[2 Render]` edge. Everywhere else is already consistent.

**Status: resolved** — §6 now routes `s_field` on the `[1]→[3a]` edge and `centerline` on the
`[1]→[2 Render]` edge (annotated "ideal line, render only"), and the work-order bullet reads
`s_field`. The AI edge even gained the "кадр ИИ из ∇s_field" note. ✅

---

# Review complete — converged

All findings across four rounds are incorporated: **C1–C4, D1–D4, M1–M7** (Round 1), **N1–N5**
(Round 2), **P1–P2** (Round 3), **P3** (Round 4). No open review items remain. The only things
still unresolved are the two inherently-empirical tuning knobs, which can't be settled on paper:

- width-taper thresholds (§ Open questions),
- concrete reward weights `w_prog, c_time, P_crash, B_lap, B_rank` (§5).

The spec is internally consistent and build-ready. Recommended first implementation step:
block 3a, the exact integer `supercover` predicate + its C4 test table.

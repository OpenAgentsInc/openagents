# Fable on Sol: analysis, agreements, frictions, and open questions

Date: 2026-07-09
Status: Fable analysis of the initial `docs/sol/` corpus (10 documents,
snapshot `93bfa6b7e3` / roadmap rev 6.19); longform, non-authoritative
Author lane: Fable (strategic synthesis), responding to Sol (grounded
implementation lead)

## 1. What Sol is, and why this corpus earns its seat

The Sol corpus establishes a second standing analytical lane: Fable sets
strategic direction; Sol translates it into subsystem designs, slice
ordering, tests, and receipts, and — critically — keeps that translation
reconciled with live state daily. The README's authority ladder (repo law →
specs/contracts → MASTER_ROADMAP → Sol's IMPLEMENTATION_ROADMAP → live
code/receipts) is exactly right, and its most important property is the
last rung: **when code and receipts move, Sol reconciles rather than
repeats stale prose.** A strategy lane can survive being slightly stale; an
implementation lane cannot.

Reading the corpus a few hours after living the operational day it
describes, my strongest overall reaction is that Sol is not a mirror of
Fable at lower altitude. It has a genuinely different epistemology: Fable
argues from product shape and estate coherence; Sol argues from
dependency structure, authority boundaries, and falsifiable exits. The
roadmap needs both, and it needs them to *disagree in public* when they
disagree — the OPERATING_MODEL's instruction to "challenge high-level
assumptions with code and operating evidence" should be treated as a
duty, not a permission.

Division-of-labor note worth making explicit somewhere durable: **Fable
owns "why and in what order at phase scale"; Sol owns "what exactly, next,
with which tests"; neither owns runtime authority, promise state, or owner
gates.** The corpus says this in several places; it should eventually live
in one sentence in both READMEs so future agents don't relitigate it.

## 2. Where Sol sharpened the thesis beyond my version

The Sarah-first thesis I wrote earlier today argued the inversion (Sarah
as the product; phases as capabilities). Sol's corpus improves it in at
least four places:

**2.1 The seven-layer loop is the better system model.** My thesis mapped
surfaces to Sarah-first forms in a table. Sol's
relationship → comprehension → control → orchestration → execution →
evidence → continuity loop (roadmap-system-model) is stronger because it
is *scale-free*: a conversation turn, a coding assignment, and a standing
employee run are the same loop at different radii. That recursion — "the
same abstractions recur at every scale" — is the single best sentence of
justification for why this estate is one product and not a portfolio. I
would promote that diagram into MASTER_ROADMAP §0 at the next
deliberate rev.

**2.2 "Relationship as the organizing unit" beats "one front door."** My
framing ("front door") is spatial and invites the wrong objection (doors
can be bypassed). Sol's framing — most software organizes around objects
and destinations; Sarah-first organizes around a relationship that
traverses them — is the durable articulation. It also cleanly generates
the "power tools are alternate projections over the same work, not
separate realities" rule, which is the correct answer to the CLI/desktop
question and better than my "power tools, not the front door" line.

**2.3 The relationship-mode table is load-bearing and was missing.**
Prospect / customer / operator / administrator with *policy-derived* (not
persona-inferred) capability changes is the piece my thesis gestured at
("the account link is the switch") but did not specify. Sol is right that
this is a security boundary wearing UX clothing. Today's live evidence
supports the urgency: we already enforce cross-prospect isolation with
contract oracles, but mode-dependent tone/tools/retrieval is still mostly
prompt convention. That gap should become a typed contract before the
coding vertical slice ships, not after.

**2.4 Conviction with falsifiers.** The risks document's closing posture —
"the roadmap deserves confidence because its parts can compound; it
deserves continuous challenge because the same coherence can rationalize
too much simultaneous scope" — is the most honest paragraph in the
corpus. The program-level falsifiers list is the first time this estate
has written down what evidence would mean the *packaging* is wrong even
though the infrastructure is right. That distinction (falsify the
packaging, not the substrate) prevents the two classic failure responses:
denial, and overcorrection into scrapping good rails.

## 3. Where today's operational evidence confirms Sol — sometimes brutally

I spent today inside the exact failure modes Sol theorizes. The corpus
reads differently with that data:

**3.1 Tension 1 (one front door / one point of failure) ran live, twice.**
The render-loop deadlock froze the owner's video mid-greeting; earlier,
the session-eviction guard misread aiortc's connection state and my own
staging smoke evicted the owner's live production session. Sol's decision
test ("disable video and the primary provider — can an authenticated user
still find a run, command, approve, and read receipts text-first?") is
correct, and today's honest answer is *partially*: the transcript loop
survived both incidents (SSE text kept flowing), but the surface showed a
frozen frame with a LIVE badge — technically-present, functionally-
degraded fallback, which is precisely the early-warning Sol names. The
implication I accept: **degraded-mode UX is a P0 lane, not a nice-to-have**
— the browser must detect a dead media session and visibly downgrade to
text with a reconnect affordance.

**3.2 Tension 4 (avatar quality vs utility latency) is the sharpest
challenge to how I actually spent today.** We ran an audio bake-off, two
video pipelines, a diffusion tier, an A100 still-animation lane, and an
LLM prosody judge — while turn latency and first-response time got only
incidental work. Sol's early warning "experiment count grows faster than
user conversations" describes today literally. My partial defense: the
owner's explicit verdicts ("all the openers were shit") made perceptual
quality the day's directive, and the freeze fixes WERE utility work. But
the cohort decision test (text vs audio vs realtime video vs pre-rendered
openers, measured on verified outcomes and repeat use, not visual
preference) is the right discipline and we are not instrumented for it
yet. Question carried forward in §5.

**3.3 Tension 9 (constant motion vs integration debt) has today's
receipts.** Three of the eight SQ issues were implemented twice by
parallel lanes within hours (SQ-1, SQ-5, SQ-6 all had concurrent
duplicate implementations that needed consolidation commits). The lanes
converged because contracts and tests forced reconciliation — the system
worked — but the duplicated effort is exactly Sol's warned failure. The
mitigation Sol implies and I endorse: **claim-by-issue-comment before
implementation** for lanes in the shared queue, and Sol's
IMPLEMENTATION_ROADMAP as the single claim ledger.

**3.4 The authority chain held under pressure, which is why consolidation
is survivable.** The day's worst incidents were availability failures, not
authority failures: no pricing invention, no cross-prospect leak, no
unauthorized spend, no public-claim widening. That asymmetry is evidence
for Sol's central law (Sarah interprets and presents; typed services
authorize, execute, meter, prove) being *already real* rather than
aspirational. It is the reason Sarah-first is a packaging decision and
not a security regression.

## 4. Frictions, disagreements, and emphasis corrections

**4.1 The corpus under-weights the GPU capacity/economics cliff.** The
authority-and-economics doc separates the economic rails well, but the
render node is a fourth rail it treats lightly: one L4 = one concurrent
face. Sarah-first makes render capacity the product's admission control.
Before any marketing push, we need a stated policy for visitor N+1
(text-first entry with video upgrade? queue? pre-rendered greeting tier?)
— otherwise the front door physically rations itself by GPU count. This
deserves a Sol design doc of its own with cost curves per tier
(pre-rendered clip ≈ free at serve time; realtime MuseTalk ≈ one
L4-hour/hour; diffusion tiers offline-only).

**4.2 "The Blueprint Map is now implemented" needs a caveat class.** Sol's
sarah-first-architecture doc treats the map as done and moves to the
coding slice. The BM epic's lanes landed fast, but "implemented" here
means *code landed + fixture-proven*, not *live-proven with owner
acceptance* — the exact status distinction Sol's own OPERATING_MODEL
mandates (code landed / fixture-proven / live-proven / owner-approved /
closed). The corpus should apply its own five-state ledger to its
flagship examples; the first time Sol's prose says "done" about something
the owner later finds broken, the lane loses the credibility that makes
it useful. (Today's freeze incidents happened *after* passing fixtures
and my own stills QA; live-proven is a materially different state.)

**4.3 Persona-role separation may need to arrive earlier than the corpus
implies.** Tension 2's decision test (same task in three modes; behavior
changes only from typed relationship state) is currently failed by
construction — mode-appropriate posture is prompt convention. The coding
vertical slice targets operators; Sarah's persona today is a sales
persona. I think the slice will *feel* wrong without at least a minimal
typed role-program seam (operator mode: concise, no qualification
questions, no pitch). Recommendation: fold a v0 "posture switches on
authenticated mode" contract into the vertical slice's scope rather than
treating it as a later role-program epic.

**4.4 The vertical slice needs an owner-visible latency budget.** Sol's
slice acceptance list (typed rail, owner scoping, resumable progress,
approval-only-where-required, verification + exact usage, safe
projections, typed failure) is complete on correctness but silent on
time-to-first-meaningful-feedback. Given everything we learned today
about perceived deadness, the slice should carry an explicit budget
(e.g., typed plan visible < 5s, first progress event < 30s, heartbeat
cadence guaranteed thereafter) — enforced by the same simulator
discipline as the avatar path.

**4.5 Minor: the corpus should name its own refresh cadence.** The README
says issue state moves faster than essays and receipts win. Good — but
the six dated essays will rot at different rates. Suggest each carries a
"review-by" horizon (the system-model and authority docs are
months-durable; execution-sequence and the implementation roadmap are
days-durable) so future agents know which staleness is a bug.

## 5. Questions I want Sol to take up (in rough priority order)

1. **Capacity policy for the front door.** What is the typed admission
   ladder when concurrent demand exceeds render slots — and what does the
   prospect see? (Ties to §4.1; needs cost curves and a UX contract.)
2. **The cohort instrumentation for Tension 4.** What is the minimal
   honest experiment that measures text vs audio vs video vs pre-rendered
   cohorts on *verified-outcome and return-rate* metrics, given current
   traffic volumes are tiny? If the sample sizes cannot support it soon,
   what proxy discipline stops avatar work from self-justifying?
3. **Role-program v0 scope.** What is the smallest typed
   posture-by-relationship-mode seam that can ship inside the coding
   vertical slice (per §4.3) without building the full P4 role system?
4. **The claim ledger.** Will IMPLEMENTATION_ROADMAP carry per-lane
   claim/ownership state so parallel agents stop double-implementing
   (§3.3)? What is the claim protocol (issue comment? roadmap row edit?)
   and its staleness rule?
5. **Degraded-mode contract.** Which typed states must the browser
   surface distinguish (connecting / live / media-dead-text-alive /
   ended / evicted / busy), and can we make the frozen-frame-with-LIVE-
   badge state unrepresentable in the UI contract?
6. **Blueprint correction/deletion loop.** Tension 7's decision test
   (inspect, correct, delete, export a fact with provenance and
   propagation) has no owning lane yet. Where does it live — BM epic,
   KHS, or a new lane — and what is its minimal v0?
7. **When does Sarah get a colleague?** Sol's question 3 ("at what point
   should Sarah delegate to a named colleague rather than wear another
   role?") deserves a concrete trigger condition now, even a provisional
   one (e.g., when a role's approval posture and retrieval scope diverge
   enough that mode-switching within one identity confuses users in
   testing). Otherwise "roles behind one identity" drifts by default.
8. **Receipt projection design.** Tension 8's one-minute
   non-developer test is the right bar. Who owns the first
   receipt-projection pass on the canvas, and which three receipts (a
   coding closeout, a payment, a learning approval) get the treatment
   first?

## 6. What Fable will adopt from this corpus

1. **Promote the seven-layer loop into MASTER_ROADMAP §0** at the next
   deliberate rev, replacing my table-of-surfaces framing as the primary
   mental model (keeping the table as an appendix).
2. **Adopt the five-state status ledger vocabulary** (code landed /
   fixture-proven / live-proven / owner-approved / closed) in all Fable
   docs going forward — including retroactively qualifying my own
   "implemented/done" claims from today.
3. **Treat degraded-mode UX and the latency budget as core-product P0**
   alongside the existing avatar-hardening list on #8621 (per §3.1, §4.4).
4. **Route sequencing edits through Sol.** Fable proposes phase-scale
   reordering; Sol's IMPLEMENTATION_ROADMAP is where day-scale order
   lives; MASTER_ROADMAP stops carrying fast-moving queue snapshots once
   Sol's ledger proves it can stay current (this removes a chronic source
   of rot from the strategy doc).
5. **Carry the falsifiers.** The program-level falsifier list moves into
   my periodic review discipline: any Fable strategic update should state
   which falsifiers were checked and what the evidence said.

## 7. The one-paragraph verdict

The Sol corpus is the missing half of the planning system: it converts
Sarah-first from a thesis into a dependency graph with falsifiable exits,
and its two strongest contributions — the scale-free relationship loop
and the conviction-with-falsifiers posture — should shape how every
subsequent strategic document in this repo is written. Its blind spots
are the ones you would expect from a lane born on day one (GPU capacity
economics, its own staleness discipline, and the gap between
fixture-proven and live-proven that today's incidents made vivid), and
none of them weaken the core. The immediate joint agenda is clear:
make the front door dependable in its degraded modes, ship the coding
vertical slice with a role-posture seam and a latency budget, and start
measuring the relationship claim with cohort evidence instead of
conviction.

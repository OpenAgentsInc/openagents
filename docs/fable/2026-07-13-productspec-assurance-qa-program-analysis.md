# ProductSpec, AssuranceSpec, and the QA program — full landscape analysis

Date: 2026-07-13
Status: strategy analysis with recommendations; §4 contains opinions and is
labeled as such; no section makes an implementation claim beyond those that
name exact paths
References: upstream `gokulrajaram/ProductSpec` `origin/main` (parser 0.20.0);
`docs/assurance/` (all six dossiers plus the new
[`GAP_ANALYSIS.md`](../assurance/GAP_ANALYSIS.md) and
[`AGENT_TOOLING.md`](../assurance/AGENT_TOOLING.md));
`packages/assurance-spec/`; `packages/product-spec/`;
[`2026-07-08-productspec-adoption-analysis.md`](./2026-07-08-productspec-adoption-analysis.md)
(#8593); `ROADMAP_QA.md`;
[`2026-07-02-qa-swarm-product-plan.md`](./2026-07-02-qa-swarm-product-plan.md);
[`2026-07-03-behavior-contracts-and-customer-invariants.md`](./2026-07-03-behavior-contracts-and-customer-invariants.md);
[`2026-07-06-seam-testing-audit-qa-swarm-gaps.md`](./2026-07-06-seam-testing-audit-qa-swarm-gaps.md);
issue #8756
Audience: owner + fleet agents

## 0. Verdict up front

We are in an unusually good position that we could squander in exactly one
way. The good position: we adopted ProductSpec's format early (#8593, five
days ago), we designed a proof-design companion (AssuranceSpec) that goes a
full layer deeper than anything upstream attempts, we shipped an honest
bounded slice of it (~1,290 LOC that refuses to pretend), and our Desktop MVP
gives us a real, live, 18-criterion subject to dogfood against. The one way to
squander it: keep designing. The AssuranceSpec dossier set is now roughly
90KB of specification against 1,290 lines of code, and upstream is
demonstrating every day what the opposite ratio buys — v0.7.0 to v0.20.0 in
the five days since our adoption analysis, with a 13-tool MCP server, spec
sessions, two skills, a starter kit, and a GitHub Action, all shipped while we
wrote sections. Their format quality went *down* in ways we should not copy
(§4.1); their adoption surface went up in ways we must answer
([`AGENT_TOOLING.md`](../assurance/AGENT_TOOLING.md) is the answer). The
program for the next stretch is three moves, in order: **AT-1 agent tooling
now** (pure functions over code that already exists), **PSEL-0 parity next**
(it unblocks three other lanes), and **the AO-CW-AC-04-01 vertical slice
before any further taxonomy** (one obligation, admitted, compiled, executed,
receipted, bridged — the first time any of the 14 laws touches reality).

## 1. The landscape, read end to end

### 1.1 Upstream ProductSpec's trajectory, and what the velocity means

When #8593 was written (2026-07-08), ProductSpec was a well-shaped v0.7.0
format with a reference parser: "adopt the format, not the tooling" was the
right call and remains right. Five days later the same repo is at parser
0.20.0 and the center of gravity has visibly moved from *format* to *agent
tooling*: `productspec mcp` (13 deterministic tools over stdio JSON-RPC,
hand-rolled, no SDK), `begin_spec_session`/`check_spec_session` content-hash +
revision pinning with typed `recommended_action`s, an implementing skill and
an authoring skill, a starter kit that makes any repo adoptable in one commit,
a composite GitHub Action, a spec dependency graph
(buildable/blocked/topological order), Decision Trace validation, and an
`npx --yes` distribution path built for non-interactive agents.

Three readings of that velocity, all of which I think are true at once:

- **The thesis is agent adoption, and it is correct.** Every one of those
  thirteen tools exists so that an agent already working in a repo can load
  the intent contract in seconds and be checked against it at "done" time.
  That is the same bet our whole company makes. Upstream got there faster
  because they have one artifact and no execution layer to be honest about.
- **The velocity bought drift.** Twenty minor versions while
  `spec_format_version` stayed `"0.1"`; documents valid under v0.5 fail v0.20
  validation because required structured blocks appeared without a format
  version bump. A single-maintainer standard moving this fast is a standard
  whose stability promise is aspirational. Consequence for us: pin our
  compatibility target per upstream release, treat their `conformance/`
  corpus as the oracle for the pinned version, and never chase minor releases
  reactively. (We already learned this passively — our
  `packages/product-spec` is "behind 0.19.0," which is a milder problem than
  having auto-tracked twenty breaking minors.)
- **The commercial arc is visible.** The open repo deliberately stops at
  structure; "quality" is reserved for the hosted ProductSpec.io editor and a
  "managed implementation" described in their vision doc. That is a
  reasonable open-core line. It also means the *proof* layer — everything
  AssuranceSpec is — is not on their roadmap in any open form. The layer is
  genuinely unclaimed.

### 1.2 Our stack, bottom to top

- **`packages/product-spec`** — our own Bun/Effect ProductSpec implementation
  per #8593's "implement our own validator" decision. Real, in the test
  sweep, validating `specs/` and the MVP spec. Behind upstream: no structured
  AC/EVAL/SM item model, no Related Artifacts, no evidence checklist.
- **The Desktop ProductSpec workroom** — the sleeper asset in this whole
  landscape. `apps/openagents-desktop/src/product-spec-workroom*` implements
  accepted plans, work packets, leases, evidence envelopes,
  independent-verification refs, and owner disposition, with the builtin
  `productspec-work` skill enforcing identity discipline
  (`path@revision+digest#criterion-id`) and a hard authority boundary. Nobody
  else — including upstream — has a shipping runtime loop where agent work is
  leased and dispositioned against a digest-pinned spec. #8756 carries it:
  RC6 built, notarized, 8/8 release-preflight gates green, honestly held open
  because the real installed real-Codex vertical journey has not passed
  (Codex account availability, not code, is the stated blocker).
- **AssuranceSpec (designed)** — `docs/assurance/ASSURANCE_SPEC.md` and
  companions: 14 laws, the 9-section document, obligations with oracles/
  falsifiers/independence/proof rungs, seams as first-class objects,
  digest-pinned Environment Profiles, a pure compiler to an immutable
  manifest, receipts with 8 non-collapsing status axes, 4 conformance levels,
  a 12-layer authority matrix. Observer is the product codename;
  Observatory the possible future public evidence surface.
- **AssuranceSpec (implemented)** — the AS-1 slice: schema, parser,
  serializer, structural validator, adequacy assessment, deterministic
  proposal (one `AO-<criterion>-01` per criterion, zero proof inference),
  committed-HEAD repository inventory, CLI. Plus the generated MVP proposal:
  18/18 obligations `needs_design`, structurally valid, not admitted. The
  package README's negative-claims list ("does not use a model, infer
  semantics, map tests to criteria, …") is the house style at its best.
- **The QA antecedents** — this is the part I want to name clearly, because
  AssuranceSpec is not a new idea for us; it is the *convergence* of three
  documented failures/decisions:
  - The **QA swarm plan** (2026-07-02) wanted swarms of agents producing
    verification evidence — and immediately hit "who reviews what the swarm
    asserts."
  - **Behavior contracts** (2026-07-03, now standing law in CLAUDE.md) made
    owner-stated expectations land as verbatim registry entries with oracle
    tests in the same change — the first "proof intent is a committed,
    reviewed artifact" mechanism we shipped.
  - The **seam-testing audit** (2026-07-06) found the failure mode that
    AssuranceSpec's Law 5 codifies: green component tests on both sides of a
    boundary with nothing proving the boundary. Mock-only coverage
    masquerading as integration proof.
  AssuranceSpec's obligations/oracles/falsifiers/seams model is those three
  lessons given a portable document format. That grounding is why I believe
  in the design even while criticizing its size.

## 2. What we have today — honest inventory

| Layer | State |
| --- | --- |
| ProductSpec format adoption (#8593) | Real: `specs/` conventions, own validator, MVP spec rev 6 (`CW-AC-01…18`, digest `fba79633…`) in the test sweep |
| Upstream parity | Behind: no structured items, no Related Artifacts, no sessions/MCP/graph/skills/action locally |
| Desktop workroom loop | Shipping: plans/packets/leases/evidence/verification-refs/disposition + builtin skill; RC6 partial evidence, #8756 open on real-journey blockers |
| AssuranceSpec design | Complete as proposal: format doc, Observer plan, system map, evidence-loop boundary, MVP dogfood plan |
| AssuranceSpec implementation | AS-1 bounded slice (~1,290 LOC, 11 tests) + generated MVP proposal (18/18 `needs_design`) |
| Semantic planning, admission, compiler, environments, adapters, receipts, workroom bridge, QA-swarm consumption, Observatory | Not implemented — designed only |
| AssuranceSpec agent tooling | Not implemented — designed as of today in [`AGENT_TOOLING.md`](../assurance/AGENT_TOOLING.md) |
| Observer marketing surface | In flight in a concurrent lane; as of this writing `openagents.com/observer` 302-redirects to the homepage, so "live" awaits its own deploy receipt |
| Tracking | #8756 is the only open issue in this territory; AssuranceSpec has no issue of its own yet |

## 3. What we're building next (the committed ladders)

Already written down, restated here only for one-page orientation:

- **PSEL-0…4** (`PRODUCTSPEC_EVIDENCE_LOOP.md`): catch `packages/product-spec`
  up to upstream structured items + Related Artifacts; compute the dual
  document/intent digest; migrate `CW-AC-01…18` → `AC-1…18` with a
  machine-readable ID map + Decision Trace (PSEL-2).
- **AS-0…6** (`ASSURANCE_SPEC.md` §16): AS-1 document core (partially done) →
  AS-2 deterministic compiler → AS-3 thin adapters over existing harnesses →
  AS-4 Desktop/Effect Native dogfood with real seam + release + sensitivity
  evidence → AS-5 portability/self-hosting → AS-6 hosted + Observatory.
- **AS-MVP-0…7** (`MVP_FIRST_ASSURANCESPEC.md`): the first vertical slice —
  design, admit, compile, and execute exactly `AO-CW-AC-04-01` (stable
  criterion identity, oracle = the existing ProductSpec executability test,
  falsifier = the existing duplicate-ID rejection) against
  `ENV-OA-LOCAL-BUN-1` via `openagents.bun_test.v1`, landing one receipt
  through `openagents.assurance_receipt_bridge.v1` into the workroom.
- **AT-1…6** (new, `AGENT_TOOLING.md` §6): CLI session/obligation/ledger/
  checklist/claim commands + the read-only 17-tool MCP server now; skills and
  the starter kit as the format layers beneath them become real.

My recommended interleave, concretely: AT-1 immediately (it needs nothing new
and it is the adoption surface); PSEL-0 in parallel (different files,
different lane); then AS-MVP straight through before AS-1's remaining
completeness items (conformance corpus, custom sections) — with the corpus
allowed to ride along AS-MVP since admission review will shake out format
bugs anyway. Open an AssuranceSpec tracking issue so this stops living only
in docs and one package; #8756 should stay the MVP's issue, not absorb this.

## 4. Directions to evolve — opinions

Everything in this section is my judgment. Where I am speculating beyond what
I read, I say so.

### 4.1 Where AssuranceSpec should deliberately diverge from upstream

- **Format-version discipline.** Upstream's one unambiguous mistake: semantic
  drift under a frozen version string. Our rule should be mechanical — any
  change that can make a previously-valid document invalid bumps
  `assurance_spec_format_version`, and the conformance corpus carries fixtures
  per version. We have the luxury of learning this before we have external
  users; upstream learned it after.
- **Stateless-first sessions.** Upstream's `begin_spec_session` stores state
  in server memory and offers a stateless fallback. AGENT_TOOLING.md inverts
  that: the full pin is always returned and the stateless path is primary.
  For long-running, restartable, multi-harness agent loops (our actual
  workload), server-side session memory is a liability, not a convenience.
- **Dual digest.** Content-hash pinning treats an evidence-attachment edit
  and an intent change identically — both "spec changed, replan." The
  document/intent digest split is our single best format-layer idea: it makes
  "keep working, only the evidence index moved" a *typed, provable* state
  (only after a semantic diff proves it — never inferred). Upstream cannot
  build this without structured intent projection; we can, after PSEL-0.
- **Never expose a rounded number.** Upstream's surface is honest mostly by
  omission (it has little state to round). Ours will be honest by
  construction: three ledgers that never blend, eight axes that never
  collapse. This is a divergence worth being loud about publicly — it is the
  brand.

### 4.2 Should we upstream anything?

Selectively, yes — small format-layer gifts, not our core. Two candidates I
would actually send: (a) a proposal for an `assurance` (or `verification_run`)
Related Artifact target kind, so AssuranceSpec receipts have a first-class
attachment type in *their* vocabulary instead of `other` — cheap for them,
legitimizing for the companion-artifact pattern; (b) our session-staleness
taxonomy if we find real cases their three `recommended_action`s misclassify.
I would **not** upstream the intent digest (it requires canonical semantic
projection they don't have and we'd be committing to maintain it in their
repo), and obviously not obligations/oracles/seams — that layer is the
company. Speculation flag: whether a single-maintainer repo accepts outside
proposals at all is untested; the attempt itself is cheap and is also how we
find out whether AssuranceSpec-as-companion gets acknowledged or cloned.
Watch for the clone case: their vision doc's "managed implementation" arc
could grow a quality/verification story, and our defense is not secrecy (the
docs are public) but shipped receipts — nobody clones an evidence trail.

### 4.3 The taxonomy-before-vertical-slice risk is our biggest risk

The Observer plan names it; I want to sharpen it. We currently have: 24
candidate acceptance criteria for a product with no compiler, 6 receipt type
names with no receipts, 4 conformance levels with no corpus, a 12-layer
authority matrix with 2 implemented layers, and a 155KB generated proposal
whose every obligation says `needs_design`. Each artifact is individually
honest — the house style's negative-claims discipline is genuinely upheld —
but the *portfolio* is lopsided, and specification debt compounds like any
debt: every future implementation slice now carries the burden of either
conforming to or formally amending ~90KB of prose. The counterweight is
already written (AS-MVP), it is small, and everything in it binds to tests
that exist today. I would treat any new `docs/assurance/` document that is
not a receipt, a conformance fixture, or a design *amendment forced by
implementation* as scope creep until AO-CW-AC-04-01 has run. (This document
and its two siblings squeak in as the adoption/strategy layer — and I'd apply
the same freeze to us next.)

### 4.4 What the 13-tool MCP pattern should teach ours

Beyond what AGENT_TOOLING.md already encodes (read-only, deterministic,
root-confined, stateless pins), the deeper lesson: upstream's
`check_completion_claim` does not evaluate the claim — it returns the
checklist of what still needs verification, with the claim echoed for the
record. That looked like a weakness in the gap analysis; for *their* layer
it is actually correct, and for ours it is Law 10 in miniature: the tool's
job is to make it impossible to claim done *without seeing* what is undone,
not to become a verdict oracle an agent can launder authority through. Our
`check_completion_claim` keeps exactly that shape with eight axes of "here is
what is not confirmed." The moment someone proposes making it "smarter"
(semantic claim matching, LLM judging inside the server), the answer is Law 2:
that is Observer's separately reviewable planning step, never ambient tool
behavior.

### 4.5 Public traces / Observatory as the wedge — yes, but evidence-first

Opinion: the most differentiating product surface in the whole plan is the
criterion-first public trace — per criterion: mapped / executable / observed /
accepted, four facts that never blend, with "which criteria are
fixture-proven, missing real-seam evidence, stale, blocked, or owner-gated"
as the default summary. Nobody shows this. Every competitor surface shows a
green percentage. A public page where an honest `0/18 executed` is displayed
*proudly* is both a trust wedge and a forcing function on ourselves (we
already run this play — the promise registry's exact-only counters). But the
sequencing discipline must hold: Observatory before receipts exist is a
dashboard of `not_run`, which is honest but not a wedge. The wedge moment is
when the MVP's own AssuranceSpec shows its first real
CONFIRMED-with-falsifier-receipt next to seventeen honest gaps. Ship the
landing page now (it is in flight), ship Observatory after AS-4. Speculation
flag: I believe "public traces" also becomes a sales artifact for the hosted
tier (customers point at their own trace pages), but we have zero customer
evidence for that yet.

### 4.6 Mutation testing is the oracle-sensitivity engine — promote it

Law 4 says a required oracle must name a falsifier it rejects. Today
falsifiers are hand-named (the dogfood slice uses the existing duplicate-ID
rejection — good, real, and artisanal). At any scale beyond a handful of
obligations, hand-naming falsifiers becomes the bottleneck and the quality
risk: agents will name weak falsifiers that technically satisfy the field.
Mutation testing mechanizes exactly this — a mutation is a machine-generated
falsifier, and "oracle kills the mutant" is a receipt-able sensitivity
observation. The design already gestures here (AS-5 mentions a mutation-test
compiler; `weak_oracle` is a designed diagnostic). My recommendation is to
promote it from AS-5 garnish to *the* AS-4→AS-5 centerpiece: an
`openagents.mutation.v1` adapter that takes an admitted obligation's oracle,
applies a bounded mutation set to the subject code, and emits
`oracle_sensitivity_receipt.v1` evidence. It converts our most philosophical
law into our most automatable product feature, and it is a capability the
generated-test-theater competitors structurally cannot fake.

### 4.7 Behavior contracts and product promises are the same spine — wire, don't duplicate

We now have three registries of committed expectation: behavior contracts
(owner-stated UX expectations, oracle in the sweep), the promise registry
(sole public-claim authority), and AssuranceSpec obligations (reviewed proof
claims). The obligation schema already carries `contract_refs` and
`promise_refs`, which is the right shape — obligations *reference* the other
two spines, never restate them. Two wiring rules I'd make explicit law when
the first cross-referencing spec is admitted: (a) an AssuranceSpec obligation
whose `contract_refs` names a behavior contract must use that contract's
oracle as its oracle ref, not a paraphrase — one oracle, two indexes; (b) a
promise-flip gate may *require* named assurance receipts as inputs, but the
promise registry remains the deciding authority (Law 10 — receipts report).
The long-run picture, speculation-flagged: these three plus Eval Suites
converge into one queryable "what does this company claim and how does it
know" graph, which is also exactly what the company-brain lane wants to
ingest. Do not build the unified graph now; keep the refs clean so it stays
buildable later.

### 4.8 Monetization sequencing

The Observer plan's business line is right ("managed environments, compute,
expertise, and evidence retention — not holding the basic proof contract
hostage") and matches the local-OSS/BYO vs hosted split. My only addition is
sequencing discipline: sell nothing until AS-4, then sell *environments*
first — managed browser/OS/device matrices are the piece with obvious
willingness-to-pay, zero authority-model risk (they execute admitted
manifests; they decide nothing), and natural usage pricing. Evidence
retention second (it compounds with time, so starting the clock early
matters). Expertise/audits last (they don't scale and shouldn't shape the
product). Observatory share pages are marketing surface, not a SKU, until
proven otherwise. Speculation flag: all pricing intuition here is untested;
the honest statement is that the *cost* structure (device labs, retention)
is real and the revenue structure is hypothesis.

### 4.9 What NOT to build

- **No semantic anything inside the deterministic tools** (§4.4). Standing
  answer, worth writing once more because it will be proposed repeatedly.
- **No spec-to-spec AssuranceSpec graph** until multiple AssuranceSpecs exist.
  Upstream's graph is good; ours would be a shelf.
- **No npm publication / public starter kit** until the format survives the
  first dogfood revision (AT-6 gating in AGENT_TOOLING.md). Publishing then
  churning is the upstream failure mode with our name on it.
- **No review-annotation tooling** (the 12-axis grading serialization) before
  a second reviewer exists who would consume it. First admissions will be
  owner-reviewed prose; that is fine.
- **No VS Code extension / editor surface.** Agents are the users; the CLI,
  MCP, and skills are the editor integration. (Upstream also skipped this,
  correctly.)
- **No competing intent-layer features.** Every temptation to "improve"
  ProductSpec's job — richer scope language, our own related-artifact
  vocabulary — is the boundary blur that Law 1 and the naming rule exist to
  prevent. We are the proof layer. Upstream can be great at intent; the more
  portable their layer, the more repos ours can attach to.
- **No second QA taxonomy document.** §4.3. The next thing
  `docs/assurance/` gains should be a receipt.

## 5. Close

The strategic read in one paragraph: upstream ProductSpec is winning the
intent layer's *adoption* race and has left the proof layer entirely
unclaimed; our AssuranceSpec design claims it with more rigor than anyone in
sight; our implementation is honest but a slice; our Desktop workroom is a
real runtime asset nobody else has; and the whole position converts into
something defensible only when the first admitted obligation produces the
first real receipt through the first real bridge. Tooling now (AT-1), parity
next (PSEL-0), vertical slice before taxonomy (AS-MVP), mutation testing as
the sensitivity engine, public traces as the wedge once there is something
true to trace. Receipts over prose — including this prose.

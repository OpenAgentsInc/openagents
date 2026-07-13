# Episode 252 Notes — Preemptive Quality Assurance

## Working title

**Preemptive Quality Assurance**

## One-line pitch

Before we let a ProductSpec drive a fleet of coding agents, commit how we will
prove it first.

The idea is a new companion called **AssuranceSpec**: ProductSpec says what the
product should do and gives evidence stable attachment points. Its workroom
loop tracks the work and receipt references. AssuranceSpec says what proof
would make us believe it. Observer compiles that reviewed agreement into a
verification graph. QA Swarm and the product's native test tools run it.
Receipts say what happened. None of those systems gets to accept its own work.

## Why this comes before multiplayer

Episode 251 deliberately reduced the first OpenAgents Desktop release to a
useful base hit: a signed, local-first, Codex-only, ProductSpec-native workroom.
The multiplayer plan moved to Episode 253.

That is the right order. The multiplayer project board wants to show public
progress against ProductSpec criteria. Before many agents race through those
criteria, we need a shared answer to a more basic question: what evidence would
make a criterion count?

Otherwise the multiplayer system will beautifully visualize issues, pull
requests, traces, and passing test commands without knowing whether any of them
test what the ProductSpec actually promised.

Episode 252 defines the proof discipline. Episode 253 makes it social.

## Continuity from episodes 248–251

- Episode 248 made local Codex history predictable and contract-gated.
- Episode 249 made child agents first-class, named, clickable threads.
- Episode 250 connected those conversations to Fleet, multiple accounts,
  delegation, usage, and evidence-backed identity.
- Episode 251 cut the initial product back to a ProductSpec-native Codex
  workroom and made Acceptance Criteria the durable units of agent work.
- Episode 252 asks what must exist between “the ProductSpec says this” and “the
  agent says it is done.”

This also picks up Episode 246's durable idea: close the gap between what we say
and what we ship. Product promises govern the macro claim. Behavior contracts
govern exact behavior. Tests and Eval Suites act as oracles. QA Swarm drives
and explores. ProductSpec's new Related Artifacts can point back to all of that
evidence, and our Desktop workroom already tracks packets, evidence, independent
verification, and owner disposition. The missing layer is still a reviewed
proof design created early enough to say what should count before the
implementation shapes the answer.

## Working name: Observer

Recommended internal codename: **Observer**.

The StarCraft Observer reveals what would otherwise remain hidden. That is the
job here: inspect the ProductSpec, reveal invisible failure surfaces, and keep
watching as the implementation changes.

The names stay distinct:

- **AssuranceSpec** — the framework-neutral companion standard.
- **Observer** — the OpenAgents planner/compiler/product codename.
- **QA Swarm** — execution, exploration, and regression distillation.
- **Arbiter** — an evidence-graph visualization surface.
- **Observatory** — a possible later multi-project hosted surface.

Other names worth discussing on camera:

- **Defensive Matrix** — excellent metaphor for wrapping the build in
  preemptive protection.
- **Scanner Sweep** — a strong name for one assurance run.
- **Shield Battery** — fun, but sounds more like recovery than verification.
- **Photon Gate** — communicates a release gate, but is an invented noun.
- **Oracle** — perfect testing language and a Protoss unit, but commercially
  confusing.
- **Sentry** — strong metaphor, already inseparable from the monitoring
  company.

Observer is a working codename, not public brand clearance. Use StarCraft's
command-console principles, not Blizzard names or assets, in a public product.

## The idea in one picture

```text
ProductSpec       intent + durable evidence attachment points
  ├─ workroom     plan → packets → evidence refs → verification → disposition
  └─ AssuranceSpec what proof should count
       ↓
     Observer      deterministic verification graph
       ↓
     QA tools      checks, exploration, and exact receipts
       └─────────> workroom + ProductSpec evidence links by reference

Human/policy      what the evidence permits
```

The product-promise registry remains the only authority for public claims.

The full format, versioning, conformance, environment, adapter, admission,
manifest, receipt, and authority design has moved out of these recording notes
to [`../assurance/README.md`](../assurance/README.md).

## What exists and what does not

The ingredients are real:

- ProductSpec parsing, validation, revision/digest binding, and stable
  criterion IDs;
- OpenAgents Desktop's accepted plans, criterion packets, leases, evidence
  receipts, verifier/producer ref checks, owner disposition, and workroom tests;
- the double-gated ProductSpec-native MVP proof driver for an isolated
  two-criterion fixture and real Codex capacity;
- behavior contracts and fixture-first Eval Suites;
- QA Runner, QA Swarm, traces, screenshots, and run receipts;
- seam probes, property/model tests, mutation specs, and bounded formal models;
- release preflights, proof rungs, and product-promise gates.

The composed AssuranceSpec system is not real yet. There is no AssuranceSpec
parser, schema/conformance corpus, admission system, Environment Profile,
adapter lock, deterministic Manifest compiler, normalized Assurance Receipt,
or QA Swarm Manifest consumer today.

There is also a useful ProductSpec catch-up story: the founder's current
`0.19.0` parser has structured `AC-*`/`SM-*` items and Related Artifacts, while
our local parser and MVP revision 6 still use the earlier `CW-AC-*` profile.
The episode can show the boundary without pretending that portable item-level
evidence links already work here.

The episode should show that gap honestly. We are not renaming a pile of tests
and calling the architecture shipped.

The new MVP proof driver is a particularly useful example of the distinction:
it proves a real fixture journey, but its `FX-AC-*` fixture is not the canonical
revision-6 `CW-AC-*` ProductSpec. AssuranceSpec should make that boundary
impossible to blur.

## First dogfood: the current MVP ProductSpec

We should test the idea on the exact product we are building now, not on a toy
spec and not on a hypothetical future framework.

The first subject is the current OpenAgents Desktop Codex Workroom MVP
ProductSpec:

```text
docs/mvp/openagents-codex-workroom-mvp.product-spec.md
ProductSpec format 0.1
spec_revision 6
CW-AC-01 through CW-AC-18
```

The first Assurance Spec will bind all 18 criteria but make only one narrow
slice executable: the stable criterion-identity portion of `CW-AC-04`. We will
reuse the existing test proving the MVP spec exposes the exact unique
`CW-AC-01…18` set, then run the existing duplicate-ID rejection as its
known-bad falsifier.

The other criteria remain visible as uncovered/`needs_design`. They do not
become not-applicable, waived, or green because we have not built their proof
yet.

This is the useful first test of the whole thesis: can we bind real intent,
reuse a real oracle, prove that oracle can fail, compile the plan
deterministically, and tell the truth about everything still missing?

## What we build first

Only the thin vertical slice needed for that first real Assurance Spec:

1. the bounded AssuranceSpec document model, parser, serializer, validator,
   stable diagnostics, and conformance fixtures;
2. exact ProductSpec path/revision/digest/criterion binding;
3. a review annotation and explicit admission artifact;
4. one local Bun Environment Profile and pinned Bun-test adapter;
5. a deterministic immutable Assurance Manifest compiler;
6. candidate and falsifier execution through existing tests;
7. normalized receipts, an honest partial projection, and stale-input proof.

We do not begin with generated tests, a universal adapter taxonomy, browser and
device farms, a hosted Observatory, or a fake “100% covered” dashboard.

The concrete build sequence, exact subject digest, first obligation, tests,
artifacts, and blockers are in
[`../assurance/MVP_FIRST_ASSURANCESPEC.md`](../assurance/MVP_FIRST_ASSURANCESPEC.md).

## Risks to confront on camera

- **Generated-test theater.** Many files and high coverage can still test
  nothing. The test must point to the bug it kills.
- **Specification laundering.** A vague criterion must become `needs_design`,
  not an arbitrary test that makes ambiguity look resolved.
- **Testing the mocks.** A green client and green server can coexist with a
  dead seam.
- **Framework capture.** Effect Native is customer zero, not protocol shape.
- **Formal-method cosplay.** A beautiful model of the wrong boundary proves
  very little.
- **Flake laundering.** Retry-until-green destroys evidence.
- **Exception entropy.** Missing proof remains visible even when an authorized
  exception permits progress.
- **Unsafe production testing.** Production is blocked or read-only unless
  explicitly armed.
- **Customer-data leakage.** Source, prompts, traces, screenshots, and logs are
  private by default.
- **Authority creep.** A compiler, runner, verifier, or dashboard cannot approve
  intent, merge, deploy, accept, spend, or flip a promise.
- **Slow factory over useful product.** One real criterion end to end is more
  valuable than a universal taxonomy that runs nothing.

## Open questions for the recording

- Which parts of Observer are planner, compiler, coordinator, and hosted
  product?
- Who may admit proof design for each risk class?
- What minimum falsifier qualifies for subjective UI and human-evaluation
  obligations?
- Which proof classes belong in every product packet, and which activate only
  at integration or release?
- Should deterministic Manifests be committed or regenerated and
  byte-compared?
- Which evidence can survive a semantically equivalent source refactor, and who
  admits that equivalence?
- What is the first non-Effect reference project after MVP dogfood?
- How should Episode 253's agents propose tests without grading their own work?

## Proposed episode structure

1. **Cold open: move multiplayer back one slot.** “Before we add more agents,
   let us make it much harder for one agent to bullshit us.”
2. **Show the assurance gap.** ProductSpec now indexes evidence and the
   workroom tracks it, but neither precommits a complete verification graph or
   decides which proof is adequate.
3. **Tour the real ingredients.** ProductSpec, contracts, Eval Suites, native
   tests, QA Swarm, seam probes, property/model tests, formal checks, proof
   rungs, and promises.
4. **Tell the seam incident.** Everything passed; the real phone could not
   connect. “Both sides have tests” is not a seam test.
5. **Name the layers.** ProductSpec, Related Artifacts, workroom Evidence Loop,
   AssuranceSpec, Observer, Manifest, QA Swarm, receipts, and human authority.
6. **Use the actual MVP ProductSpec.** Show revision 6 and its 18 criteria.
7. **Write the first obligation.** Bind `CW-AC-04` to the existing exact-ID
   test and duplicate-ID falsifier.
8. **Break it on purpose.** If the known-bad version stays green, the oracle is
   decorative.
9. **Show all the honest gaps.** Seventeen uncovered criteria are the correct
   first result.
10. **Draw the growth path.** Local fixture → Desktop workroom → real seam →
    packaged artifact → release evidence.
11. **Close on sequencing.** Build the MVP, grow its Assurance Spec one real
    obligation at a time, then open the multiplayer lobby.

## Candidate lines for the recording

> Before we let a ProductSpec generate a bunch of code, let it generate a bunch
> of ways the code can be wrong.

> QA usually shows up after the implementation and politely asks the
> implementation what it would like to be tested on. I want the tests waiting
> at the crime scene before the code arrives.

> ProductSpec says what the product should do. AssuranceSpec says how we will
> know. Its Evidence Loop points to what happened. Observer compiles the proof
> agreement. QA Swarm runs it. Arbiter shows the receipts. None of those gets
> to accept its own work.

> A test suite is not impressive because it is large. It is impressive because
> every important test can point to the bug it kills.

> If I delete the fix and your test stays green, your test is decorative.

> The client has a test. The server has a test. The product is still broken.
> The seam is a third thing and it needs its own contract.

> Formal verification is for the small state machine where one bad transition
> ruins your week. It is not a blue checkmark we sprinkle on a screenshot.

> Expected red is honest before the feature exists. Green because we skipped
> the environment is not.

> Effect Native is customer zero, not the only customer.

> Multiplayer needs a shared definition of progress. Observer gives it a proof
> graph instead of a pull-request counter.

## Honest ending

Episode 252 names and plans the missing proof-design layer. It does not claim
that AssuranceSpec, Observer, automatic test planning, universal adapters,
Effect Native conformance, formal-counterexample conversion, hosted device
labs, Observatory, or an Observer product promise are live.

The next action is deliberately small: build enough of the real format and
toolchain to author and admit the first Assurance Spec for the exact current
MVP ProductSpec, make one criterion's oracle and falsifier real, and leave all
other gaps visible. Then widen the graph criterion by criterion.

Build the base hit. Prove one real thing. Then open the multiplayer lobby.

## Further reading

- [Assurance documentation index](../assurance/README.md)
- [AssuranceSpec companion proposal](../assurance/ASSURANCE_SPEC.md)
- [Current-system integration map](../assurance/CURRENT_SYSTEM_MAP.md)
- [Observer product-plan seed](../assurance/OBSERVER_PRODUCT_PLAN.md)
- [First MVP AssuranceSpec dogfood plan](../assurance/MVP_FIRST_ASSURANCESPEC.md)
- [Current MVP package](../mvp/README.md)
- [Episode 253 multiplayer notes](./253-notes.md)

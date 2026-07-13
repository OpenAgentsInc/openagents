# Episode 252 Notes — Preemptive Quality Assurance

## Working title

**Preemptive Quality Assurance**

## One-line pitch

Before we let the ProductSpec drive a fleet of coding agents, make it drive a
fleet of tests.

The first post-MVP ProductSpec should define a framework that turns committed
product intent into a reviewable assurance manifest before implementation:
deterministic unit, component, browser, native, device, seam, property,
model-based, performance, security, accessibility, recovery, and bounded formal
checks; the environments that must run them; the evidence each must produce;
and the deliberately broken case proving each oracle can actually catch a bug.

Then the implementation has somewhere honest to go: from expected red, through
fixture-proven, through real-environment evidence, to a release decision that
can answer, criterion by criterion, “Does the product do what we designed?”

## Why this comes before multiplayer

Episode 251 deliberately reduced the first OpenAgents Desktop release to a
useful base hit: a signed, local-first, Codex-only, ProductSpec-native workroom.
The former Episode 252 multiplayer plan is now Episode 253.

That order is better. The multiplayer project board wants to show public
progress against ProductSpec criteria. Before many agents can race through
those criteria, we need a machine-readable answer to a more basic question:
what evidence would make a criterion count? Otherwise the multiplayer system
will beautifully visualize issues, pull requests, traces, and passing test
commands without knowing whether any of them test the thing the ProductSpec
actually promised.

Episode 252 defines the proof graph. Episode 253 can make that graph social.

## Continuity from episodes 248–251

- Episode 248 made local Codex history predictable and contract-gated.
- Episode 249 made child agents first-class, named, clickable threads.
- Episode 250 connected those conversations to Fleet, multiple accounts,
  delegation, usage, and evidence-backed identity.
- Episode 251 cut the initial product back to a ProductSpec-native Codex
  workroom and made acceptance criteria the durable units of agent work.
- Episode 252 asks what must exist between “the ProductSpec says this” and “the
  agent says it is done.”

This also picks up the most durable idea from Episode 246: close the gap between
what we say and what we ship. Product promises govern the macro claim. Behavior
contracts govern the exact behavior. Eval Suites and deterministic tests act as
oracles. QA Swarm drives the product and records what happened. The missing
piece is the compiler that assembles those parts from a ProductSpec early
enough to shape the build instead of merely grading it afterward.

## Working name: Observer

Recommended internal codename: **Observer**.

The StarCraft Observer is built to reveal what is otherwise hidden. That is
exactly the job here: inspect the ProductSpec, discover the invisible failure
surfaces, and keep watching as the implementation changes. It also fits the
existing vocabulary without stealing it:

- **Observer** compiles intent into assurance obligations.
- **QA Swarm** executes scripted checks, exploration, probes, and regressions.
- **Arbiter** can visualize the evidence graph and verdicts.
- A later **Observatory** can be the hosted dashboard containing many projects
  and runs.

Other names worth discussing on camera:

- **Defensive Matrix** — the most literal episode metaphor: wrap the build in
  preemptive protection. Strong codename, but less clear as the name of a
  compiler or public service.
- **Scanner Sweep** — excellent name for one assurance run that reveals hidden
  failures across the map, rather than the whole system.
- **Observatory** — strongest platform/service name, less clear for the
  compiler itself.
- **Shield Battery** — fun and preemptive, but sounds like recovery rather than
  verification.
- **Photon Gate** — communicates a release gate and is StarCraft-adjacent, but
  is an invented noun.
- **Oracle** — perfect testing terminology and a Protoss unit, but commercially
  confusing beside Oracle Corporation.
- **Sentry** — an excellent defensive metaphor and Protoss unit, but already
  inseparable from the monitoring company.

Use **Observer** as a working codename, not public brand clearance. The repo's
StarCraft design guide says to borrow the command-console principles rather
than Blizzard's names or assets; a public product should therefore undergo
normal naming review and may simply become **OpenAgents Assurance**. The
technical artifact should keep the framework-neutral name **Assurance
Manifest** so the protocol remains understandable even if the product name
changes.

## The thing we are actually designing

Observer is not another test runner and not “ask an LLM to write Playwright.”
It is a specification-to-assurance compiler plus a lifecycle for the artifacts
it creates.

Inputs:

1. an exact ProductSpec path, revision, and digest;
2. a typed environment profile describing the repository, frameworks,
   renderers, targets, capabilities, auth posture, risk class, and existing
   test surfaces;
3. linked behavior contracts, Eval Suites, product promises, invariants, and
   existing receipts;
4. explicit owner policy for cost, production access, mutable actions,
   retention, and required proof rung.

Outputs:

1. an Assurance Manifest with stable obligation IDs;
2. traceability from every acceptance criterion to one or more executable
   oracles, or an explicit blocker/waiver/not-applicable decision;
3. deterministic test and model scaffolds placed in the owning repository's
   normal verification paths;
4. an environment matrix saying where each obligation must run;
5. falsifiers, known-bad fixtures, or mutations proving that each oracle is
   sensitive to the failure it claims to detect;
6. run receipts, coverage and freshness ledgers, counterexamples, traces,
   screenshots, videos, and exact commands;
7. a release projection that reports observed evidence without granting
   deploy, acceptance, promise, or public-claim authority.

The important separation is:

> ProductSpec declares intent. Observer compiles verification obligations.
> Behavior contracts state durable behavior and cite oracle refs. Eval Suites
> and admitted tests provide executable evaluators. QA Swarm executes and
> explores. Receipts record evidence. Maintainers accept. Product promises
> alone govern public claims.

No layer gets to promote itself.

## What “deterministic” means here

The ProductSpec is prose plus bounded structured blocks. Turning prose into a
good test design involves judgment. We should not pretend a model-generated
test file is deterministic merely because it was generated once.

The deterministic contract should be narrower and stronger:

- the same ProductSpec revision/digest plus the same accepted environment
  profile and compiler version produces the same criterion IDs, obligation
  IDs, dependency graph, required evidence classes, and manifest digest;
- bounded ProductSpec parsing and environment decoding use explicit schemas;
- the semantic mapping from an acceptance criterion into assurance domains
  uses one typed selector/planner and produces reviewable structured output,
  never ad hoc keyword rules scattered across adapters;
- agent-authored tests and formal models are proposals until reviewed and
  admitted into the manifest;
- once admitted, every executable check has a pinned command, code digest,
  target profile, seed policy, timeout policy, and expected evidence shape;
- rerunning the same accepted check against the same source/target state gives
  the same verdict, or reports the nondeterministic input and an honest flaky
  disposition;
- an unmappable criterion becomes `needs_design`, never a guessed green.

This lets agents contribute creativity without letting nondeterminism leak into
the meaning of a release gate.

## Existing machinery we are composing

This is a synthesis of systems that already exist in the monorepo, not a blank
sheet.

### ProductSpec

`@openagentsinc/product-spec` already parses and validates ProductSpec v0.1.
The repository convention is explicit: a spec declares intent and never
enforces it. Acceptance criteria are the pre-launch build contract; success
metrics are the post-launch market contract. Behavior contracts and Eval
Suites remain the enforcement layer, receipts remain evidence, and the product-promise
registry remains the sole authority for public claims.

The parser can preserve structured `productspec-ai-evals` declarations, but
there is no generic executor that binds those declarations to the repository's
many harnesses. That missing binding is part of Observer's job, and an eval's
fractional pass threshold must not automatically become whole-criterion
acceptance.

Observer should preserve that boundary. It reads a ProductSpec and produces
linked enforcement artifacts; it must never quietly reinterpret the spec to
match whatever implementation happened to land. Intent changes require a
`spec_revision` bump and reconciliation.

### Behavior contracts

`@openagentsinc/behavior-contracts` already provides the micro-promise shape:
an expectation kept verbatim where possible, provenance, status, enforcement
tier, blocker refs, and one or more oracle refs. An enforced contract requires
a nonempty statement and verification text, at least one oracle, a test-sweep
or nightly enforcement tier, and no blocker. Existing categories include
indicator truthfulness, stated-flow
availability, latency budgets, honest error states, dead controls,
cross-surface consistency, claim safety, accessibility, and money-path
integrity.

The current coverage checker establishes source/scenario presence and contract
ID linkage, including stricter two-sided citations for declared seams. It does
not prove that the cited oracle semantically enforces the statement or rejects
a known-bad implementation. That stronger adequacy check belongs to Observer.

Observer should generate proposed contract entries where the ProductSpec
contains durable user-visible behavior, then require the contract and its
oracle to land together. The ProductSpec links the IDs; it does not copy their
definitions.

### Planned-feature Eval Suites

The QAM-7 mobile work already demonstrates a catalog-first preemptive pattern:
register planned cases, oracle statements, expected fixture refs, and blockers
while features are still planned, with honest blocked/implemented/waived and
red/green summary states. Its current tests validate catalog shape and
completeness; they do not execute every named future fixture.

Observer should generalize that hand-authored catalog into a standard
ProductSpec compilation step and make the accepted declarations executable.

### QA Swarm and `qa-runner`

`@openagentsinc/qa-runner` already supplies the execution substrate:
scripted deterministic sessions, LLM exploration, browser, terminal,
container, a native macOS backend proven on the historical Khala target,
target adapters, public-safe traces, videos,
screenshots, CONFIRMED/REFUTED/INCONCLUSIVE verification, crash-safe artifact
flush, bounded sharding, PR evidence, local swarm-projection/schema
scaffolding, and a distiller that turns discoveries into committed rerunnable
tests. General hosted execution, owned-runner receipts, persistence, arbitrary
`/qa/{runRef}` lookup, and self-serve dispatch remain separate gaps.

Observer should not reimplement those capabilities. It produces the manifest
and corpus that QA Swarm runs. QA Swarm explores beyond the manifest, but a
discovery only becomes standing assurance when it is distilled, reviewed,
linked to an obligation or regression class, and proven deterministic.

### The QA framework work

The former Khala Code framework design contributes several principles that are
still correct even though that client is now historical source material:

- one scenario can run through multiple access modes;
- determinism precedes exploration;
- every phase needs an oracle;
- mode disagreement is itself a finding;
- public safety is executable;
- every run emits a coverage ledger and performance samples;
- formal models inform but never authorize;
- counterexamples become regression tests.

Those principles should move into framework-neutral Observer contracts and be
dogfooded first on the greenfield OpenAgents Desktop and Effect Native stack.

### Seam testing

The 2026-07-06 mobile incident is the warning label for this entire ProductSpec:
client tests and server tests were green while the real client sent a WebSocket
token one way and the server only read it another way. A tiny real-transport
probe found what every mocked layer missed.

Observer cannot count component coverage as system coverage. Its compiler must
identify seams—client/server, renderer/host, process/IPC, auth/transport,
storage/restart, device/backend, third-party API—and assign at least one
wiring-level obligation that imports or drives both real sides, or cite an
end-to-end receipt. An oracle file existing is not proof the seam was crossed.

### Property, model, and formal checks

The repository already has `fast-check` property suites, model-based lifecycle
testing, TLA+ specifications, mutation specifications, public-safe
counterexample seeds, and `specs/run-tlc.sh`. The roadmap calls for TLC and
counterexample conversion in the nightly, but the current QA-nightly matrix
does not wire that step. Observer should bind and generalize these techniques
for ProductSpec obligations and the active apps where they fit:

- property tests for codecs, convergence, redaction, ordering, idempotency,
  serialization, and hostile input;
- model-based tests for bounded lifecycle/state transitions;
- TLA+/TLC for approval, lease, sync, recovery, revocation, exactly-once,
  authority, and concurrency invariants;
- mutation/known-bad variants to prove that the oracle actually rejects a
  forbidden transition.

Formal verification is not prestige garnish. If no bounded state machine or
meaningful invariant can be named, the manifest says `not_applicable` and uses
the appropriate test layer instead.

### Product promises and proof rungs

The current operating model separates code-landed, fixture-proven,
deployed/distributed, live-proven, owner-accepted, and closed. Observer should
attach a required proof rung to each obligation and never collapse them into a
single green checkmark.

A generated test passing in a fake environment may prove a local property. It
does not prove the release artifact, a physical device, a production seam, a
customer outcome, or a public product claim.

## The honest current gap

The ingredients are real; the composed product is not.

- There is no ProductSpec-to-Assurance-Manifest compiler today.
- ProductSpec AI-eval blocks are parsed but are not a generic execution plan.
- Behavior-contract coverage proves registry/oracle linkage more readily than
  it proves semantic adequacy or oracle sensitivity.
- The generic runner, historical Khala harness, active Desktop/mobile apps,
  Effect Native renderers, release preflights, and formal models do not yet
  enroll through one criterion-level manifest.
- The QA Swarm projection and `/qa/{runRef}` surface have working substrate,
  but lookup currently resolves fixed seed/sample aliases; arbitrary hosted-run
  persistence, broad third-party execution, and live browser/native/device
  matrices remain gated or incomplete.
- A complete Effect Native renderer-conformance sweep is a goal, not a current
  cross-platform guarantee.
- Current QA runner, share-surface, and hosted-service promise scopes are not a
  blanket green launch claim.

Episode 252 should make these gaps the starting state of the ProductSpec, not
edit the historical plans until they read as if the combined service shipped.

## The Assurance Manifest

Proposed schema name: `openagents.observer.assurance_manifest.v1`.

The manifest is derived from, but lives separately from, the ProductSpec. It is
versioned, reviewable, and diffable. At minimum it contains:

- ProductSpec path, `spec_revision`, content digest, and criterion refs;
- accepted environment-profile revision and digest;
- compiler/planner version and accepted semantic-plan receipt;
- stable obligation ID, title, rationale, risk, and owning criterion IDs;
- linked behavior-contract, Eval Suite, invariant, and promise IDs;
- assurance domain and test technique;
- target tier: static, fixture, local, preview, staging, release artifact,
  device, or production;
- adapter, command, seed, timeout, retry, isolation, auth, and mutation policy;
- expected oracle and the exact falsifier it must reject;
- required evidence and public-safety classification;
- required proof rung and independence requirements;
- separate state axes: lifecycle/admission (`proposed`, `admitted`,
  `superseded`, `retired`), readiness (`needs_design`, `planned_red`,
  `blocked`, `not_applicable`, `executable`), latest verdict (`not_run`,
  `CONFIRMED`, `REFUTED`, `INCONCLUSIVE`), infrastructure (`ready`, `unarmed`,
  `unavailable`, `failed`), stability (`stable`, `flaky`), freshness
  (`current`, `stale`), and exception/waiver refs;
- latest run, source, artifact, verifier, exception, freshness, and
  supersession refs.

The manifest is a plan and ledger, not a second source of product truth. Its
rows dereference the authoritative spec, contract, test, source, target, and
receipt records.

## Assurance domains: test the hell out of it, systematically

Every criterion does not need every layer. Every criterion does need an
explicit technique selection and a reason. Observer should choose from a
standard catalog and make omissions visible.

### A0 — Intent and static structure

- ProductSpec conformance, unique criterion IDs, revision/digest discipline;
- schema and type checks;
- dependency/architecture boundary scans;
- generated registry/docs parity;
- forbidden capability and public-safety scans;
- route, command, component, variant, and RPC inventory completeness.

### A1 — Unit and component behavior

- pure domain functions and reducers;
- typed error and impossible-state handling;
- component state matrices;
- fake-clock timing and cancellation;
- resource acquisition/release;
- accessible name, focus, keyboard, and state semantics.

### A2 — Property and model-based behavior

- generated hostile inputs and state sequences;
- round-trip, idempotency, convergence, monotonicity, and permutation laws;
- lifecycle models compared to real implementations;
- deterministic failing seeds preserved as replay fixtures.

### A3 — Contracts and real seams

- schema compatibility on the actual transport;
- real client against real server route wiring;
- host/renderer IPC and capability boundaries;
- auth, cookie, bearer, expiry, and revocation paths;
- persistence, restart, replay, and migration seams;
- third-party adapters against sandbox or test accounts.

### A4 — User journeys

- browser journeys through the visible product;
- signed native desktop artifact journeys;
- simulator and physical-device journeys where platform behavior matters;
- visual, layout, accessibility-tree, keyboard, menu, and deep-link checks;
- the critical flow in both success and induced-failure states.

### A5 — Resilience and fault injection

- interruption, timeout, process death, reconnect, duplicate delivery, gap
  repair, stale state, partial degradation, and rollback;
- truthful error surfaces with no infinite spinner or false success;
- crash/interrupt evidence flush and bounded recovery;
- no orphan process, leaked lease, stuck approval, or duplicated mutation.

### A6 — Performance and resource budgets

- named latency budgets on realistic data sizes;
- throughput and concurrency ceilings;
- memory, handle, disk, network, and process-lifetime budgets;
- trend receipts and explicit regression thresholds;
- no invented precision when the target cannot be measured.

### A7 — Security, privacy, and authority

- least-capability boundaries;
- secret and private-data noninterference in projections and artifacts;
- denial, revocation, scope, and confused-deputy checks;
- read-only production posture and explicit arming for mutations;
- proof that evidence, verification, acceptance, deployment, claims, spend,
  and settlement remain separate authorities.

### A8 — Bounded formal verification

- explicit model boundary and production contract;
- safety and, where meaningful, liveness properties;
- bounded TLC checks with pinned config;
- deliberately weakened mutation models that must produce counterexamples;
- counterexample-to-replay-fixture conversion.

### A9 — Exploration and regression distillation

- seeded monkeys covering unvisited state;
- LLM explorers targeting coverage frontiers and seams;
- cross-mode and N-version disagreement hunting;
- every confirmed discovery distilled into a deterministic regression;
- undistillable findings remain INCONCLUSIVE or exploratory evidence, never a
  release green.

## Oracle sensitivity: a test must prove that it can fail

The most important addition to ordinary test generation is a falsifier.

Every required obligation should answer: what minimal wrong implementation,
fixture, response, transition, timing, or visual state must this oracle reject?
The manifest then records one of:

- a known-bad fixture;
- a source or model mutation;
- a fault-injection toggle;
- a wrong server/client combination;
- a deleted auth or validation check;
- a deliberately stale or duplicated event;
- an accessibility or visual defect variant;
- a recorded production regression replay.

If the oracle passes both the correct candidate and the deliberately broken
variant, the result is `oracle_unsound`, not green. This is how we prevent
generated tests from asserting their own fixture setup, mirroring the
implementation, checking only that code ran, or producing a beautiful suite
of tautologies.

For safety-critical bounded models, the mutation spec is the falsifier: the
weakened rule must generate a counterexample. For browser behavior, the seeded
defect must be visible to the same assertion path. For a seam, deleting either
side of the compatibility fix must break the real transport probe.

## Red before code without making the repository useless

Preemptive QA should produce tests before the implementation, but permanently
red trunk is not a useful operating model.

Use staged gates:

1. **Design gate:** every criterion is mapped, deferred, or blocked; test
   scaffolds and falsifiers exist; planned features may be `planned_red` or
   explicitly waived with owner/blocker refs. The meta-gate passes only if the
   red is expected and correctly cataloged.
2. **Implementation gate:** when a work packet claims a criterion, its required
   fixture/static/unit obligations become hard gates. A PR cannot replace or
   weaken them merely to pass.
3. **Integration gate:** seam, browser, native, recovery, and matrix obligations
   become mandatory as their target environments are available.
4. **Release gate:** every required obligation is current and green at its
   declared proof rung, or a separate authority explicitly accepts a narrow
   release exception. The obligation remains waived/unconfirmed, and the
   exception caps the permitted proof rung and public-claim scope.
   `planned_red`, stale, skipped-without-authority, and unarmed are never
   recolored green.
5. **Post-release gate:** live-safe probes and success metrics watch the real
   outcome without retroactively pretending pre-release fixtures proved it.

The QAM-7 planned-feature catalog is the precedent: register red/waived
acceptance intent, expected fixture refs, and blockers before implementation.
Observer's addition is to generate/admit runnable checks and require later
changes to turn those same obligations green.

## Environment-aware without becoming framework-locked

The assurance plan is necessarily environment-specific. A React DOM app, an
Effect Native desktop app, a Rust protocol crate, a Cloudflare Worker, and an
iOS binary do not have the same useful tests. The universal layer should be the
contract, not one test tool.

Proposed adapter boundaries:

- `EnvironmentProfile` — repo graph, languages, frameworks, platforms,
  targets, capabilities, existing commands, risk and data classes;
- `AssurancePlanner` — typed semantic selection of applicable domains and
  obligations;
- `TestAdapter` — scaffolds and runs a framework's unit/component/e2e tests;
- `TargetAdapter` — auth, fresh identity, restart, restrictions, and target
  conversion;
- `OracleAdapter` — translates a domain expectation into executable
  assertions without changing its meaning;
- `FormalAdapter` — bounded model/checker conventions and counterexample
  conversion;
- `EvidenceAdapter` — normalizes public-safe artifacts into receipts;
- `GateAdapter` — attaches obligations to local verify, pre-push, PR, nightly,
  release, or live cadences.

For the OpenAgents repository, gates run locally or on OpenAgents-owned
runners, never GitHub Actions. A PR check or comment is only a projection of
those receipts, not an independent source of green.

Adapters decode typed capabilities. The planner must not infer tool selection
with ad hoc substring or filename matching. Framework detection can propose a
profile, but the accepted environment manifest is explicit and reviewable.

An unsupported stack yields precise adapter gaps and portable manual
obligations. It does not yield fake universal coverage.

## Effect Native as customer zero for the framework adapter

Effect Native is a useful first dogfood target because we are defining the
framework while using it to build OpenAgents Desktop. Observer can make its
quality contract part of the framework rather than an afterthought.

The Effect Native adapter should eventually exploit:

- Schema-decoded component, intent, state, service, and renderer boundaries;
- `Layer` substitution for deterministic service fakes;
- `TestClock` or injectable time for no-sleep concurrency tests;
- `Scope` and structured concurrency for resource-leak/interruption checks;
- one component/intent corpus replayed across the current DOM and React Native
  renderers first, with separately accepted future native, canvas, and terminal
  rows as they actually land;
- renderer conformance: equivalent semantics need not mean identical pixels;
- host-capability tests proving a tokenless renderer cannot acquire filesystem,
  process, credential, raw IPC, or unbounded network authority;
- platform-specific journeys where equivalence is impossible or undesirable.

Today the core, DOM renderer, and React Native renderer have real tests, and
OpenAgents Desktop consumes the DOM renderer. React Native's iOS path includes
bounded renderer-owned SwiftUI lowering, but a full SwiftUI/native renderer,
canvas and terminal renderers, and one enum-driven shared conformance corpus
are future work. Observer must not present the architecture roadmap as a
current cross-renderer guarantee.

The Effect Native adapter is first-party convenience, not the protocol. A
customer using Vitest, Playwright, XCTest, Rust `proptest`, JUnit, Maestro, or a
different formal checker should still produce the same Assurance Manifest and
receipt vocabulary through different adapters.

## The ProductSpec to write during Episode 252

Create the proposed post-MVP spec at:

`specs/qa/openagents-observer.product-spec.md`

Use ProductSpec v0.1 and the same validated conventions as the current Desktop
MVP:

- `artifact_type: prd` only after the owner commits this as the next product
  lane; otherwise begin as `hypothesis` and promote by explicit revision;
- `spec_revision: 1`;
- role author `OpenAgents`;
- linked repo `OpenAgentsInc/openagents`;
- custom Owner Gates, Receipts, and Promise Links sections;
- required Problem, Hypothesis, Scope, Acceptance Criteria, and Success
  Metrics;
- User Experience, Solution, Risks, Open Questions, Pricing, and Rollout;
- unique author-visible criteria such as `OBS-AC-01`;
- structured `in`, `out`, and `cut` scope;
- exact metric IDs, targets, windows, segments, and sources.

Do not expand the Desktop MVP ProductSpec to absorb Observer. The MVP can ship
with the deterministic checks it needs. Observer is the post-MVP product that
systematizes this process for future OpenAgents work and arbitrary customer
stacks.

## ProductSpec seed

### Problem

ProductSpec gives product teams and coding agents one durable statement of
intent before implementation, but it does not turn that intent into executable
proof. Acceptance criteria, behavior contracts, unit tests, browser journeys,
device checks, seam probes, property suites, formal models, release receipts,
and production monitors are authored in separate workflows—often after the
implementation already biases what the tests say. Agents can produce large
passing suites that assert fixtures, mirror implementation details, miss real
integration seams, skip unavailable environments, or mistake activity and
coverage for product correctness. Teams cannot reliably answer, criterion by
criterion, whether the exact release artifact behaves as designed.

### Hypothesis

If OpenAgents compiles an accepted ProductSpec plus a typed environment profile
into a reviewable Assurance Manifest before implementation—mapping every
criterion to risk-appropriate deterministic oracles, real seams, environment
tiers, falsifiers, evidence, and proof rungs—and uses QA Swarm to execute,
explore, distill, and receipt those checks, then OpenAgents and customer teams
will detect specification drift and integration defects earlier, ship fewer
false greens, and produce trustworthy release evidence without being locked to
Effect Native or any single test framework.

### Proposed `in`

- one validator-clean ProductSpec revision with unique stable criterion IDs;
- one accepted, digest-pinned environment profile for the owning repository;
- typed semantic planning into the standard assurance-domain catalog;
- one versioned Assurance Manifest with stable obligations and traceability;
- behavior-contract and Eval Suite proposal/link generation;
- staged planned-red, implementation, integration, release, and post-release
  gates;
- test scaffolds for the environment's supported unit, component, property,
  seam, browser/native/device, resilience, performance, security, and
  accessibility adapters;
- bounded formal-model proposals where state-machine invariants justify them;
- known-bad fixture, fault, or mutation sensitivity proof for every required
  oracle;
- QA Swarm execution, exploration, regression distillation, and public-safe
  evidence integration;
- exact spec, source, command, target, seed, adapter, artifact, and receipt
  digests;
- orthogonal lifecycle/admission, readiness, verdict, infrastructure,
  stability, freshness, and exception axes so no mixed state rounds up to
  green;
- first-party Effect Native/OpenAgents Desktop adapter and at least one
  framework-neutral third-party reference adapter;
- local OSS/BYO path with no OpenAgents account required;
- hosted private run matrix and shareable opt-in report as a later paid tier;
- default read-only external production policy and explicit arming for writes;
- no public claim changes except through the existing promise registry.

### Proposed `out`

- proving that a ProductSpec's product hypothesis or market demand is correct;
- replacing product judgment, design review, code review, security review,
  maintainer acceptance, or owner gates;
- perfect automatic translation of arbitrary prose into correct tests;
- one universal runner or DSL replacing Vitest, Playwright, XCTest, Maestro,
  Rust tests, TLA+, or customer-native tools;
- formal verification of aesthetics, broad human preference, or unbounded
  production systems;
- live mutation of a customer's production environment by default;
- auto-deploy, auto-merge, promise-state promotion, spend, payout, or
  settlement authority;
- public customer specs, prompts, source, traces, or artifacts by default;
- indefinite retention of customer test data;
- multiplayer contribution dispatch, public project management, or bounties;
- claiming the historical Khala Code client as the active dogfood target.

### Proposed `cut`

- acceptance criteria without stable IDs becoming executable by guesswork;
- keyword or filename heuristics as user-facing assurance/tool routing;
- a generated test counting as accepted merely because it compiles or passes;
- an oracle without a known falsifier or sensitivity proof counting as strong
  release evidence;
- mocked client and mocked server tests counting as proof of the real seam;
- executor self-verification or a model grading its own unconstrained output;
- retries, sleeps, quarantine, or broad waivers hiding a persistent failure;
- skipped, unarmed, missing-adapter, stale, or inconclusive results rounding up
  to green;
- test coverage percentage standing in for behavior coverage;
- a formal model granting runtime or release authority;
- weakening a runtime policy, ProductSpec, behavior contract, Eval Suite, or
  oracle to make the implementation pass without explicit revision/approval;
- raw secrets, prompts, credentials, cookies, private paths, customer data, or
  provider payloads in receipts or public projections;
- Observer becoming a second ProductSpec, promise registry, source database,
  test runner, or CI system instead of compiling and projecting existing
  authorities.

## Candidate acceptance criteria

- **OBS-AC-01:** Given one validator-clean ProductSpec with unique criterion
  IDs, Observer binds the exact path, revision, digest, compiler version, and
  accepted environment-profile digest and emits a schema-valid Assurance
  Manifest with stable obligation IDs. Missing/duplicate IDs or a changed
  digest stop compilation with a typed reconciliation state.
- **OBS-AC-02:** Every acceptance criterion is mapped to at least one admitted
  assurance obligation or one explicit `needs_design`, blocked, waived, or
  not-applicable record with rationale and authority. The manifest reports
  unmapped criteria and cannot present full traceability while any remain.
- **OBS-AC-03:** Technique and adapter selection comes from one typed semantic
  planner over the accepted environment profile. Its structured plan is
  reviewable and digest-pinned; unsupported capabilities fail precisely, and
  no ad hoc keyword/file matching silently chooses a test or tool.
- **OBS-AC-04:** The design gate emits planned-red or waived fixture-first
  suites before implementation without making expected-red trunk appear
  release-green. Claiming a criterion activates its required implementation
  gates; release refuses while a required obligation is red, stale, skipped,
  unarmed, flaky, inconclusive, or missing evidence.
- **OBS-AC-05:** Every required oracle declares a falsifier and produces an
  oracle-sensitivity receipt. The correct fixture/candidate passes and the
  known-bad fixture, fault, source mutation, or model mutation fails. An oracle
  that accepts both is `oracle_unsound` and cannot satisfy the criterion.
- **OBS-AC-06:** For each declared cross-process, client/server,
  renderer/host, auth/transport, persistence/restart, device/backend, or
  third-party seam, at least one obligation drives or imports both real sides
  or links a qualifying end-to-end receipt. Unit tests of isolated mocks do not
  satisfy the seam row.
- **OBS-AC-07:** A supported web criterion can generate/admit and run unit or
  component checks plus one real-browser journey with condition waits, failure
  state, accessibility assertion, trace, screenshot/video as applicable, and
  deterministic replay at fixture/local and configured integration tiers.
- **OBS-AC-08:** The Effect Native reference adapter replays one shared typed
  component/intent scenario across at least two applicable renderers, preserves
  semantic behavior and capability boundaries, and records platform-specific
  differences without requiring pixel identity.
- **OBS-AC-09:** One admitted framework-neutral scenario runs through every
  supported access mode for its target—such as RPC, DOM, browser, native, or
  headless—normalizes comparable observations, and treats unexplained mode
  disagreement as a finding rather than choosing whichever result is green.
- **OBS-AC-10:** A lifecycle or concurrency criterion selected for model-based
  testing generates bounded command sequences against the real implementation,
  preserves a failing seed, and turns the minimized divergence into a
  deterministic regression fixture.
- **OBS-AC-11:** A criterion selected for formal verification declares its
  narrow production contract, model boundary, invariants, bounded checker
  config, and mutation model. The intended model passes, the weakened mutation
  yields a counterexample, and that counterexample becomes or links a runtime
  regression test. Model success grants no acceptance authority.
- **OBS-AC-12:** QA Swarm can consume the manifest, shard runnable obligations
  within declared budgets, explore an uncovered frontier, and distill one
  confirmed discovery into a reviewed deterministic regression. An
  undistillable exploration remains INCONCLUSIVE.
- **OBS-AC-13:** The same spec/profile/compiler inputs reproduce the same
  manifest and obligation graph. A run verdict is reproducible only against an
  immutable target snapshot with the same source, target, fixture, toolchain,
  seed, and check digests; dynamic targets bind the observed deployment/state
  or emit typed nondeterminism. Changed inputs never silently reuse prior green.
- **OBS-AC-14:** Every run flushes partial evidence on failure or interruption,
  keeps CONFIRMED/REFUTED/INCONCLUSIVE verdict separate from infrastructure
  failure and flaky/stale state, and records exact command, environment, seed,
  duration, cost/usage when measurable, proof rung, freshness, and artifact
  refs.
- **OBS-AC-15:** External production targets are blocked or read-only by
  default. Mutating tiers require explicit target-owner arming, a fresh test
  identity, bounded actions/budget, revocation, and private artifacts. Public
  projections pass secret/private-data tripwires and disclose only approved
  refs.
- **OBS-AC-16:** A release projection keeps intent, implementation, test
  execution, evidence, independent verification, owner/maintainer acceptance,
  deployment, live proof, and public-promise state as separate axes. Admitted
  verification dereferences and schema-decodes verifier output, binds it to the
  exact spec, obligation/oracle, source, target, and evidence, and proves the
  verifier differs from the evidence producer. No green manifest or run can
  merge, deploy, accept, spend, settle, or promote a public claim.
- **OBS-AC-17:** The OSS local path can compile and execute a supported fixture
  manifest with BYO tools/models and no OpenAgents account. A hosted private
  path can run an explicitly consented browser/native/device matrix and produce
  a shareable report without making hosted services a dependency of the local
  protocol.
- **OBS-AC-18:** One non-Effect-Native reference project implements the same
  manifest through its native test tools and produces conforming receipts,
  proving the protocol is portable rather than an OpenAgents-only wrapper.
- **OBS-AC-19:** Spec revision, environment-profile revision, adapter change,
  source change, target deployment, behavior-contract change, or oracle change
  marks affected evidence stale by dependency, retains history, and requires
  explicit reconciliation rather than silently carrying green forward.
- **OBS-AC-20:** Observer emits separate ledgers for criterion-to-obligation
  traceability, executed obligation/environment coverage, and reachable
  state/action/surface frontier coverage. Each ledger retains current, union,
  and delta views plus replay seeds; required-surface regression blocks, the
  frontier steers exploration, and line/branch coverage remains advisory.

## Proposed receipt vocabulary

These are candidate schemas to define in the ProductSpec, not claims that they
exist today:

- `openagents.observer.assurance_manifest.v1` — compiled obligation graph;
- `openagents.observer.semantic_plan_receipt.v1` — accepted criterion/domain
  mapping;
- `openagents.observer.oracle_sensitivity_receipt.v1` — correct-vs-falsifier
  result;
- `openagents.observer.seam_receipt.v1` — exact real endpoints/artifacts crossed;
- `openagents.observer.formal_check_receipt.v1` — model boundary, checker,
  invariants, mutation, and counterexample refs;
- `openagents.observer.run_receipt.v1` — normalized execution and artifact
  result;
- `openagents.observer.release_projection.v1` — proof-rung-aware aggregate;
- existing QA Runner/QA Swarm, ATIF trace, behavior-contract, Eval Suite, and
  product-promise receipts linked rather than cloned.

Candidate promise link: existing `qa.agentic_qa_runner.v1` for the runner
substrate is currently yellow; hosted-run and share-surface promises remain
planned. Add an Observer-specific promise record only after the first
end-to-end manifest has current evidence. The notes and ProductSpec must not
turn any of these into public launch copy.

## Candidate success metrics

The recording should choose real targets rather than leave these as slogans:

- **Pre-build traceability:** accepted ProductSpec criteria with a reviewed
  obligation/blocker/waiver mapping before the first implementation packet.
  Proposed target: 100% for Observer-backed projects.
- **Oracle sensitivity:** release-required oracles whose known-bad variant is
  rejected. Proposed target: 100%.
- **False-green rate:** confirmed escaped regressions for which the release
  projection was green while a required same-scope oracle already existed and
  would not fail. Proposed target: zero.
- **Seam coverage:** declared critical seams with a current real-wiring receipt
  at the required tier. Proposed target: 100% before release.
- **Time to first useful red:** elapsed time from accepted ProductSpec/profile
  to the first reviewed failing assurance corpus. This should fall over the
  first three dogfood projects.
- **Reproducibility:** reruns against identical immutable snapshots and declared
  digests producing the same verdict; dynamic targets must produce a typed
  nondeterminism or target-drift finding. Proposed target: at least 99% in
  deterministic tiers and 100% explanation of differences.
- **Escape detection latency:** time from an escaped regression reaching a
  monitored target to a REFUTED receipt and deterministic replay fixture.
- **Paid conversion later:** eligible OSS projects opting into a hosted
  browser/native/device matrix after a successful local manifest. Do not set a
  public target until pricing and data handling are owner-approved.

## Product and business shape

The same discipline we use internally can become a product without making the
core protocol proprietary.

### Free and local

- ProductSpec and environment-profile validation;
- Assurance Manifest compiler;
- adapter SDK and reference adapters;
- local deterministic runner composition;
- BYO model for optional semantic planning/exploration;
- local HTML/JSON report;
- no account and no required OpenAgents Cloud.

### Planned hosted and paid

- managed browser/version/viewport matrix;
- macOS/Windows/Linux native artifact runners;
- simulator and physical-device labs;
- private parallel QA Swarm exploration;
- PR, pre-push, nightly, release, and live-safe gates;
- retained encrypted evidence, trends, and team review workflow;
- private or explicitly public share pages;
- customer-specific adapter engineering and assurance audits;
- exact usage and budget controls.

Possible packages remain the earlier QA Swarm shapes—Swarm Audit,
QA-on-every-push, and Swarm Sprint—but Observer makes the deliverable more
durable: not only a run report, but an accepted ProductSpec-to-assurance graph
the customer keeps rerunning.

The paid service sells managed environments, compute, expertise, and evidence
retention. It does not hold the basic test contract hostage.

## Product surface

The local workroom and later hosted Observatory should be criterion-first.

For each ProductSpec criterion, show four separate facts:

1. **Mapped** — has an accepted obligation set.
2. **Executable** — adapters, targets, tests, and falsifiers exist.
3. **Observed** — current runs produced evidence at named tiers.
4. **Accepted** — the authorized reviewer made a disposition.

Expanding a row reveals behavior contracts, Eval Suites, test files, formal
models, environments, known-bad variants, latest receipts, flakes, blockers,
waivers, freshness, and the next required action. The default summary should
not be “87% complete.” It should say exactly which criteria are fixture-proven,
which lack real-seam evidence, which are stale, and which need owner action.

The visual graph can use the existing Arbiter discipline: edges light only when
a real receipt binds them. A ProductSpec criterion connected to a test file by
prose is not the same as a criterion connected to a current sensitivity-proven
run.

This projection becomes a major input to Episode 253's multiplayer public
project page. People can see agents making progress, but the progress states
come from Observer's proof graph rather than issue theater.

## Owner gates to seed in the ProductSpec

- Approve **Observer** as the working codename and whether **Observatory** is the
  hosted surface; require brand/trademark review before public naming.
- Decide whether the first artifact is a committed `prd` or an experimental
  `hypothesis` until one dogfood compilation succeeds.
- Approve which obligation classes are mandatory for the first OpenAgents
  Desktop release versus introduced immediately after the MVP.
- Approve the staged planned-red/release-gate policy and the authority allowed
  to waive an obligation.
- Approve first-party Effect Native and third-party reference adapters.
- Approve hosted customer-data regions, retention, deletion, encryption,
  artifact visibility, model-data policy, and support access.
- Approve live-target mutation policy, test-identity requirements, cost caps,
  and the exact environments OpenAgents may operate for customers.
- Approve OSS license, hosted packaging, pricing experiment, and any public
  promise record only after evidence exists.

## Risks to confront on camera

- **Generated-test theater.** Many files and high coverage can still test
  nothing. Falsifiers and mutation sensitivity are non-negotiable.
- **Specification laundering.** A vague criterion can become an arbitrary test
  that makes the ambiguity look resolved. `needs_design` is the correct result
  until product intent is clarified.
- **Testing the mocks.** Component-level greens can coexist with a dead real
  seam. Every declared seam needs wiring evidence.
- **Framework capture.** Dogfooding on Effect Native could turn the protocol
  into Effect-specific assumptions. The non-Effect reference adapter is a
  release criterion.
- **Formal-method cosplay.** A beautiful TLA+ model of the wrong boundary proves
  very little. State the production contract, model limits, mutation, and
  counterexample path.
- **Flake laundering.** Retry-until-green destroys evidence. Retries remain
  bounded and visible; flaky is not confirmed.
- **Waiver entropy.** A planned-red catalog can become a graveyard. Waivers need
  owner, scope, reason, expiry/review date, proof-rung limit, and visibility.
- **Unsafe production testing.** Customer prod is read-only or blocked by
  default; mutable checks use staging/test identities unless explicitly armed.
- **Customer-data leakage.** Browser traces, screenshots, prompts, source, and
  logs are private by default and separately redacted for sharing.
- **The compiler becomes the authority.** Observer proposes and records; it
  cannot approve intent, merge code, deploy, accept outcomes, or flip promises.
- **Slow factory over useful product.** The first version should support one
  real ProductSpec and a narrow adapter set end to end. A universal taxonomy
  that runs nothing is failure.

## Open questions for Episode 252

- Is **Observer** the product, the compiler, an agent role, or all three?
- Does the Assurance Manifest live beside the ProductSpec under `specs/qa/`,
  under a generated `assurance/` tree, or as a typed projection from source
  registries?
- Which ProductSpec criteria are too subjective to compile without a separate
  rubric or human study?
- Who accepts the semantic mapping from criterion to obligations: owner,
  product lead, QA reviewer, or criterion-specific maintainer?
- What is the minimum falsifier for a UI journey? Is a fixture toggle enough,
  or must critical journeys use a source mutation?
- Which mutation engine should be used per language, and where is a known-bad
  fixture more stable than source mutation?
- Which proof classes belong in every MVP packet, and which activate only at
  integration or release?
- How do we keep planned-red obligations visible without blocking unrelated
  trunk work or normalizing permanent waivers?
- What is the first non-Effect-Native reference project?
- Which hosted device/browser/native environments are worth owning versus
  integrating?
- Should customers share a public proof page, an unlisted report, a private
  workspace, or all three?
- How much of the semantic planner can run with a local/BYO model while
  retaining reproducible accepted output?
- When a ProductSpec revision changes, can unaffected obligations retain
  evidence by dependency analysis, or must the entire manifest go stale?
- Can Episode 253's multiplayer agents propose tests and falsifiers before
  claiming implementation packets, with an independent agent reviewing the
  oracle plan?

## Proposed episode structure

1. **Cold open: move multiplayer back one slot.** “Before we add more agents,
   let us make it much harder for one agent to bullshit us.”
2. **Show the ProductSpec gap.** Acceptance criteria tell agents what to build,
   but not yet the complete executable evidence graph.
3. **Tour what already exists.** ProductSpec, behavior contracts, fixture-first
   Eval Suites, QA Swarm, qa-runner, seam probes, property/model tests, TLC,
   mutation specs, proof rungs, and product promises.
4. **Tell the seam incident.** Everything passed; the real phone could not
   connect. Explain why “both sides have tests” is not a seam test.
5. **Name Observer.** Explain the StarCraft unit, the Assurance Manifest, and
   the relationship to QA Swarm and Arbiter.
6. **Compile one criterion on a whiteboard.** Criterion → behavior contract →
   unit/property/seam/browser obligations → falsifiers → environments → proof
   rungs → receipts.
7. **Break the implementation on purpose.** Make the known-bad variant fail and
   explain why a test that cannot fail is not a test.
8. **Use Effect Native as customer zero.** Tour the current DOM and React
   Native renderer tests and typed host boundary, then design the shared
   cross-renderer component/intent corpus required by OBS-AC-08.
9. **Draw the staged gates.** Planned red before implementation, hard gates as
   packets activate, real-environment evidence before release, live-safe checks
   after deployment.
10. **Create the Observer ProductSpec.** Write Problem, Hypothesis, scope,
    acceptance criteria, metrics, receipts, owner gates, pricing, and rollout.
11. **Close on the sequence.** Ship the MVP, wrap every new ProductSpec in
    Observer, then let multiplayer agents contribute against tests whose
    meaning was agreed before they wrote the code.

## Candidate lines for the recording

> Before we let a ProductSpec generate a bunch of code, let it generate a bunch
> of ways the code can be wrong.

> QA usually shows up after the implementation and politely asks the
> implementation what it would like to be tested on. I want the tests waiting
> at the crime scene before the code arrives.

> ProductSpec is intent. Observer turns intent into obligations. QA Swarm runs
> them. Arbiter shows the receipts. None of those gets to accept its own work.

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

Episode 252 plans and writes the ProductSpec for Observer. It does not claim
that automatic ProductSpec-to-test compilation, universal framework adapters,
the shared Effect Native conformance corpus, nightly TLC/counterexample
conversion, hosted device labs, a public Observatory, or an Observer product
promise are live. The repository already contains many
of the component systems—ProductSpec validation, behavior contracts, Eval
Suites, QA Runner/QA Swarm machinery, seam probes, property/model testing, TLA+
models, mutation specs, and evidence routes—but they are not yet one automatic
post-MVP product.

The honest win is a validated contract for composing them: every criterion
mapped before implementation, every oracle able to demonstrate a real red, and
every green tied to the environment and proof rung it actually exercised.

Build the base hit. Put an Observer over the next spec. Then open the
multiplayer lobby.

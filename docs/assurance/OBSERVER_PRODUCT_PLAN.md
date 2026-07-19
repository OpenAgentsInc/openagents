# Observer product-plan seed

Date: 2026-07-13

Status: proposed post-MVP ProductSpec source material. Not an admitted Product
Spec, implementation claim, release gate, price, or public promise

## Product sentence

Observer helps a team turn an accepted ProductSpec into a reviewed Assurance
Spec, compile that proof design into an immutable verification graph, run it
through native QA tools, and return approved evidence pointers to ProductSpec's
Evidence Loop without pretending that a link is a verdict.

The framework-neutral standard is **AssuranceSpec**. Observer is the OpenAgents
planner/compiler/product codename. The standard design lives in
[`ASSURANCE_SPEC.md`](./ASSURANCE_SPEC.md).

## Problem

The ProductSpec standard commits intent and now provides Related Artifacts for
linking AC/EVAL/SM IDs to external evidence. OpenAgents Desktop also has a real
workroom loop that turns an accepted ProductSpec plan into packets and records
leases, evidence envelopes, unequal verifier/producer refs, and owner packet
disposition. What neither provides is a separately reviewed pre-build proof
design specifying adequate oracles, falsifiers, environments, seams, proof
rungs, freshness, and evidence policy.

Acceptance criteria, behavior contracts, unit tests, browser journeys, device
checks, seam probes, property suites, formal models, release receipts, and
production monitors are still authored in separate workflows—often after the
implementation has biased what the tests say. Related Artifact links can make
that evidence discoverable, but cannot say whether it is the right evidence.

Agents can produce large passing suites that assert fixtures, mirror
implementation details, miss real integration seams, skip unavailable
environments, or mistake activity and code coverage for product correctness.
Teams cannot reliably answer, criterion by criterion, whether the exact release
artifact behaves as designed.

## Hypothesis

If OpenAgents turns an accepted ProductSpec into a separately reviewed and
admitted AssuranceSpec before implementation—mapping every criterion to
risk-appropriate oracles, real seams, environment tiers, falsifiers, evidence,
proof rungs, gates, and authority boundaries—then compiles that proof design
into an immutable Assurance Manifest and uses native adapters and QA Swarm to
execute, explore, distill, and receipt those checks, teams will detect
specification drift and integration defects earlier, ship fewer false greens,
and produce trustworthy release evidence. Approved receipts can then flow by
reference through the existing workroom and ProductSpec Related Artifacts
without being locked to Effect Native or one test framework.

## Proposed scope

### In

- one validator-clean ProductSpec revision with stable criterion IDs.
- upstream-current structured AC/EVAL/SM and Related Artifact compatibility, or
  an explicitly named legacy profile with no portability overclaim.
- one schema-valid, reviewed, admitted AssuranceSpec bound to its exact
  ProductSpec revision, document digest, intent digest, and item IDs.
- digest-pinned Environment Profiles and adapter lock.
- typed semantic planning that produces proposed obligations.
- deterministic compilation to an immutable, byte-stable Assurance Manifest.
- behavior-contract and Eval Suite references/proposals.
- planned-red, implementation, integration, release, and post-release gates.
- existing-test import before generated-test replacement.
- supported unit, component, property, seam, browser, native/device,
  resilience, performance, security, accessibility, and formal adapters.
- an explicit falsifier or sensitivity proof for every required oracle.
- QA Swarm execution, exploration, regression distillation, and public-safe
  evidence integration.
- a stable public-safe Assurance Evidence Index and approved ProductSpec
  Related Artifact projection.
- a typed immutable Assurance Receipt resolver and opaque-ref registration
  through the existing Desktop workroom without replacing its packet/status
  ledger.
- exact spec, source, command, target, seed, adapter, artifact, and receipt
  digests.
- orthogonal admission, readiness, observation, infrastructure, stability,
  freshness, disposition, and exception axes.
- first-party OpenAgents Desktop/Effect Native dogfood plus one non-Effect
  reference project.
- local OSS/BYO execution without an OpenAgents account.
- later hosted private matrices and opt-in shareable reports.
- blocked/read-only external production by default and explicit arming for
  writes.
- no public claim changes outside the product-promise registry.

### Out

- proving that a product hypothesis or market demand is correct.
- replacing product, design, code, security, maintainer, or owner review.
- perfect automatic translation of arbitrary prose into correct tests.
- one universal runner replacing native test tools.
- replacing ProductSpec Related Artifacts or the Desktop workroom loop.
- formal verification of aesthetics or unbounded production systems.
- mutating customer production by default.
- auto-merge, auto-deploy, promise promotion, spend, payout, or settlement.
- public customer source, prompts, traces, or artifacts by default.
- multiplayer contribution dispatch, public project management, or bounties.
- making hosted OpenAgents services a dependency of the local protocol.

### Deliberately cut

- executing unlabeled criteria by fuzzy guesswork.
- keyword/filename heuristics silently selecting tools or permissions.
- treating model-generated proof mappings as deterministic compiler output.
- accepting a generated test merely because it compiles or passes.
- counting an oracle without a falsifier as strong release evidence.
- counting isolated mocks as proof of a real seam.
- executor self-verification or unconstrained model self-grading.
- retry, sleep, quarantine, or broad-waiver laundering.
- skipped, unarmed, stale, flaky, unavailable, or inconclusive results rounding
  up to green.
- line/branch coverage standing in for behavior coverage.
- formal models granting runtime or release authority.
- weakening ProductSpec, runtime policy, contracts, Eval Suites, or oracles to
  make an implementation pass.
- secrets, prompts, credentials, private paths, or customer data in public
  evidence.
- mutable latest-run status inside the generated Assurance Manifest.
- AssuranceSpec becoming a second source of product intent.
- link count, workroom `verified`, or owner packet disposition standing in for
  assurance, release, or promise state.

## Candidate acceptance criteria

- **OBS-AC-01 — Exact subject binding.** Given one validator-clean ProductSpec
  and one admitted AssuranceSpec, Observer binds exact paths, format versions,
  revisions, ProductSpec document and intent digests, Assurance digests,
  review/admission set, Environment Profiles, adapter lock, and compiler
  version. An intent mismatch blocks compilation. An evidence-index-only
  document change has its own typed refresh path only after semantic
  classification proves that no dependency or consumed metadata changed.
- **OBS-AC-02 — Complete criterion disposition.** Every applicable criterion
  has one or more admitted obligations or a reviewed `not_applicable`
  disposition. `needs_design`, blocked, and exceptions remain visible and do
  not count as proof.
- **OBS-AC-03 — Reviewable semantic planning.** One typed semantic planner
  produces a proposed AssuranceSpec. Review annotations and an admission
  receipt bind its exact revision/digest before deterministic compilation.
- **OBS-AC-04 — Honest planned red.** Design-time checks can be planned red
  without making trunk release-red before activation. Once activated, required
  negative or unknown states block the relevant gate.
- **OBS-AC-05 — Oracle sensitivity.** Every required oracle names a falsifier.
  The intended candidate passes and the known-bad candidate is refuted. An
  oracle accepting both is `oracle_unsound`.
- **OBS-AC-06 — Real seams.** Each declared cross-process/client-server/
  renderer-host/device-backend seam drives both real sides or cites a
  qualifying end-to-end receipt. Mock-only rows do not satisfy it.
- **OBS-AC-07 — Web journey.** A supported web obligation can run local checks
  plus one real-browser journey with condition waits, failure state,
  accessibility assertion, trace, and deterministic replay.
- **OBS-AC-08 — Effect Native conformance.** One shared component/intent
  scenario replays across at least two applicable renderers and records
  semantic agreement and typed platform differences without requiring pixel
  identity.
- **OBS-AC-09 — Access-mode comparison.** One scenario runs through every
  supported access mode for its target and treats unexplained disagreement as a
  finding instead of choosing the green result.
- **OBS-AC-10 — Model-based regression.** A lifecycle/concurrency obligation
  generates bounded sequences against the real implementation, preserves a
  failing seed, and distills a minimized divergence into a deterministic
  regression.
- **OBS-AC-11 — Bounded formal proof.** A formal obligation declares its
  production contract, model boundary, invariants, checker config, and
  mutation. The mutation yields a counterexample that links a runtime
  regression. Model success grants no acceptance authority.
- **OBS-AC-12 — Swarm execution.** QA Swarm can shard manifest units within
  declared budgets, explore an uncovered frontier, and propose one distilled
  regression. Undistillable exploration remains `INCONCLUSIVE`.
- **OBS-AC-13 — Compiler determinism.** Identical ProductSpec, AssuranceSpec,
  admission, profile, adapter-lock, and compiler inputs produce byte-identical
  canonical manifest output without clock, random, network, discovery, model,
  timestamp, or absolute-path inputs.
- **OBS-AC-14 — Durable run evidence.** Runs flush partial evidence on failure
  or interruption and keep observation separate from infrastructure, flake,
  and freshness while recording exact inputs and artifacts. In Desktop, a
  typed bridge validates a qualifying `CONFIRMED` Assurance Receipt and issues
  an immutable opaque ref for the ProductSpec workroom. The host verifier ref
  differs from the lease executor, Assurance producer/reviewer policy is
  checked separately, native and Assurance receipts remain intact, and
  non-confirming states cannot become host `passed`.
- **OBS-AC-15 — Safe external targets.** Production is blocked/read-only by
  default. Mutation requires owner arming, fresh test identity, bounded
  actions/budget, revocation, and private evidence.
- **OBS-AC-16 — Authority containment.** Release projections keep intent,
  implementation, execution, evidence, independent verification, acceptance,
  deployment, live proof, and promise state separate. No green manifest/run
  grants merge, deploy, spend, settlement, or public-claim authority.
- **OBS-AC-17 — Local and hosted separation.** The OSS path validates, compiles,
  and runs one fixture obligation with BYO tools and no OpenAgents account. A
  later hosted matrix is optional.
- **OBS-AC-18 — Non-Effect portability.** One non-Effect reference project uses
  native tools and emits conforming receipts without changing AssuranceSpec
  semantics.
- **OBS-AC-19 — Dependency freshness.** Subject intent, profile, adapter,
  source, target, contract, or oracle change stales only dependent evidence and
  requires explicit reconciliation. A typed diff limited to classified
  evidence attachments refreshes the index without masquerading as
  product-intent drift. `product_spec` dependencies and consumed
  `tool_metadata` remain intent-bound.
- **OBS-AC-20 — Separate coverage ledgers.** Criterion traceability,
  obligation/environment execution, and reachable frontier remain separate.
  code coverage is advisory.
- **OBS-AC-21 — Document conformance.** The implementation includes a canonical
  format, schemas, parser/serializer/validator/CLI, stable codes, valid/invalid
  fixtures, round trips, schema/parser parity, version checks, and preserved
  custom sections.
- **OBS-AC-22 — Compiler conformance.** Duplicate/dangling/cycle, capability
  mismatch, subject drift, exact golden bytes, and separate gate-evaluation
  fixtures exist, plus a self-hosting AssuranceSpec for Observer.
- **OBS-AC-23 — Independent revisions.** ProductSpec intent revisions,
  ProductSpec evidence-index updates, AssuranceSpec proof-intent revisions,
  implementation/manifest changes, and run receipts advance independently
  under explicit stale/reconciliation laws.
- **OBS-AC-24 — Immutable manifest.** The manifest contains resolved
  verification units and dependency/gate graphs only. All dynamic status
  remains in receipts and projections.

## Candidate artifact vocabulary

- `*.assurance-spec.md` — authored proof design.
- `*.assurance-environment.json` — reusable environment policy/capabilities.
- `assurance-adapters.lock.json` — adapter versions and content digests.
- `*.assurance-manifest.json` — generated immutable verification graph.
- `*.assurance-review.json` — portable proof-design review.
- `*.assurance-decision-trace.json` — proof-policy change history.
- `*.assurance-evidence-index.json` — public-safe mutable projection behind a
  stable ProductSpec Related Artifact link.
- `openagents.observer.assurance_admission_receipt.v1`.
- `openagents.observer.oracle_sensitivity_receipt.v1`.
- `openagents.observer.seam_receipt.v1`.
- `openagents.observer.formal_check_receipt.v1`.
- `openagents.observer.run_receipt.v1`.
- `openagents.observer.release_projection.v1`.

These names are candidates, not evidence that schemas or implementations exist.

## Candidate success metrics

- **Pre-build traceability:** 100% of accepted criteria have an admitted
  obligation or reviewed not-applicable disposition before implementation.
- **Oracle sensitivity:** 100% of release-required oracles reject their
  declared known-bad candidate.
- **False-green rate:** zero escaped regressions where an existing required
  same-scope oracle remained green.
- **Seam coverage:** 100% of critical seams have current real-wiring evidence at
  the required tier before release.
- **Time to first useful red:** decreases over the first three dogfood projects.
- **Reproducibility:** at least 99% same-verdict reruns in deterministic tiers
  and 100% typed explanations for differences.
- **Escape detection latency:** time from escaped regression to `REFUTED`
  receipt plus deterministic replay.

No paid-conversion target is public until packaging, data handling, and pricing
are owner-approved.

## Product surface

The local workroom and possible Observatory are criterion-first. For each
criterion they show four separate facts:

1. mapped — an admitted obligation set exists.
2. executable — adapters, targets, oracles, and falsifiers exist.
3. observed — current runs produced evidence at named tiers.
4. accepted — authorized review made a disposition.

The default summary says which criteria are fixture-proven, missing real-seam
evidence, stale, blocked, or owner-gated. It does not replace those facts with a
single “percent complete.” ProductSpec Related Artifact links are visible as
evidence locations, never as an additional green state.

## Product and business shape

### Local OSS

- ProductSpec, AssuranceSpec, and Environment Profile validation.
- deterministic Manifest compiler.
- adapter SDK and narrow reference adapters.
- local native test composition and JSON/HTML report.
- optional local/BYO model for proposal and exploration.
- no OpenAgents account.

### Later hosted service

- managed browser/version/viewport matrices.
- macOS/Windows/Linux native artifact runners.
- simulator and physical-device labs.
- private parallel QA Swarm exploration.
- retained encrypted evidence, trends, and review workflow.
- private/unlisted/opt-in public share pages.
- customer-specific adapters and assurance audits.
- exact usage/budget controls.

The paid service sells managed environments, compute, expertise, and evidence
retention. It does not hold the basic proof contract hostage.

## Owner gates

- Approve AssuranceSpec/Observer/Observatory naming and brand review.
- Approve the ProductSpec/AssuranceSpec/Manifest authority boundary.
- Approve reviewer roles and axes that can admit proof design.
- Decide when the future Observer ProductSpec becomes `prd` rather than
  `hypothesis`.
- Approve mandatory first-release obligation classes and staged gate policy.
- Approve Effect Native and non-Effect reference adapters.
- Approve hosted regions, retention, encryption, model-data, support access,
  live-target mutation, identity, cost caps, license, packaging, and pricing.
- Approve any public promise only after exact current evidence exists.

## Risks

- generated-test theater.
- vague-criterion/specification laundering.
- testing mocks instead of seams.
- Effect Native capture of a supposedly portable protocol.
- formal-method cosplay over the wrong boundary.
- retry/flaky laundering.
- exception entropy.
- unsafe production tests.
- customer-data leakage.
- compiler or verifier authority creep.
- building a universal taxonomy before one useful vertical slice.

## Rollout

1. Build the minimum MVP dogfood slice in
   [`MVP_FIRST_ASSURANCESPEC.md`](./MVP_FIRST_ASSURANCESPEC.md).
2. Catch up `@openagentsinc/product-spec` to upstream `0.19.0`, then explicitly
   reconcile the MVP's `CW-AC-*`/semantic metric IDs into portable structured
   items with a linked machine-readable ID map, single-line criterion fixtures,
   and custom preservation of metric `segment`/`source` context before claiming
   item-level Related Artifact interoperability.
3. Expand that admitted AssuranceSpec criterion by criterion, importing
   existing tests before generating new ones.
4. Dogfood Effect Native/OpenAgents Desktop across local, packaged, seam, and
   release tiers.
5. Self-host Observer's own AssuranceSpec and mutation-test the compiler.
6. Prove portability on one non-Effect project.
7. Only then consider a standalone standard repository or hosted service.

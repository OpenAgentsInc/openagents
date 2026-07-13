# Observer product-plan seed

Date: 2026-07-13

Status: proposed post-MVP ProductSpec source material; not an admitted Product
Spec, implementation claim, release gate, price, or public promise

## Product sentence

Observer helps a team turn an accepted ProductSpec into a reviewed Assurance
Spec, compile that proof design into an immutable execution graph, run it
through native QA tools, and inspect exactly what the resulting evidence does
and does not prove.

The framework-neutral standard is **AssuranceSpec**. Observer is the OpenAgents
planner/compiler/product codename. The standard design lives in
[`ASSURANCE_SPEC.md`](./ASSURANCE_SPEC.md).

## Problem

ProductSpec gives product teams and coding agents one durable statement of
intent before implementation, but it does not turn that intent into executable
proof. Acceptance criteria, behavior contracts, unit tests, browser journeys,
device checks, seam probes, property suites, formal models, release receipts,
and production monitors are authored in separate workflows—often after the
implementation has biased what the tests say.

Agents can produce large passing suites that assert fixtures, mirror
implementation details, miss real integration seams, skip unavailable
environments, or mistake activity and code coverage for product correctness.
Teams cannot reliably answer, criterion by criterion, whether the exact release
artifact behaves as designed.

## Hypothesis

If OpenAgents turns an accepted ProductSpec into a separately reviewed and
admitted Assurance Spec before implementation—mapping every criterion to
risk-appropriate oracles, real seams, environment tiers, falsifiers, evidence,
proof rungs, gates, and authority boundaries—then compiles that proof design
into an immutable Assurance Manifest and uses native adapters and QA Swarm to
execute, explore, distill, and receipt those checks, teams will detect
specification drift and integration defects earlier, ship fewer false greens,
and produce trustworthy release evidence without being locked to Effect Native
or one test framework.

## Proposed scope

### In

- one validator-clean ProductSpec revision with stable criterion IDs;
- one schema-valid, reviewed, admitted Assurance Spec bound to its exact
  ProductSpec revision and digest;
- digest-pinned Environment Profiles and adapter lock;
- typed semantic planning that produces proposed obligations;
- deterministic compilation to an immutable, byte-stable Assurance Manifest;
- behavior-contract and Eval Suite references/proposals;
- planned-red, implementation, integration, release, and post-release gates;
- existing-test import before generated-test replacement;
- supported unit, component, property, seam, browser, native/device,
  resilience, performance, security, accessibility, and formal adapters;
- an explicit falsifier or sensitivity proof for every required oracle;
- QA Swarm execution, exploration, regression distillation, and public-safe
  evidence integration;
- exact spec, source, command, target, seed, adapter, artifact, and receipt
  digests;
- orthogonal admission, readiness, observation, infrastructure, stability,
  freshness, disposition, and exception axes;
- first-party OpenAgents Desktop/Effect Native dogfood plus one non-Effect
  reference project;
- local OSS/BYO execution without an OpenAgents account;
- later hosted private matrices and opt-in shareable reports;
- blocked/read-only external production by default and explicit arming for
  writes;
- no public claim changes outside the product-promise registry.

### Out

- proving that a product hypothesis or market demand is correct;
- replacing product, design, code, security, maintainer, or owner review;
- perfect automatic translation of arbitrary prose into correct tests;
- one universal runner replacing native test tools;
- formal verification of aesthetics or unbounded production systems;
- mutating customer production by default;
- auto-merge, auto-deploy, promise promotion, spend, payout, or settlement;
- public customer source, prompts, traces, or artifacts by default;
- multiplayer contribution dispatch, public project management, or bounties;
- making hosted OpenAgents services a dependency of the local protocol.

### Deliberately cut

- executing unlabeled criteria by fuzzy guesswork;
- keyword/filename heuristics silently selecting tools or permissions;
- treating model-generated proof mappings as deterministic compiler output;
- accepting a generated test merely because it compiles or passes;
- counting an oracle without a falsifier as strong release evidence;
- counting isolated mocks as proof of a real seam;
- executor self-verification or unconstrained model self-grading;
- retry, sleep, quarantine, or broad-waiver laundering;
- skipped, unarmed, stale, flaky, unavailable, or inconclusive results rounding
  up to green;
- line/branch coverage standing in for behavior coverage;
- formal models granting runtime or release authority;
- weakening ProductSpec, runtime policy, contracts, Eval Suites, or oracles to
  make an implementation pass;
- secrets, prompts, credentials, private paths, or customer data in public
  evidence;
- mutable latest-run status inside the generated Assurance Manifest;
- AssuranceSpec becoming a second source of product intent.

## Candidate acceptance criteria

- **OBS-AC-01 — Exact subject binding.** Given one validator-clean ProductSpec
  and one admitted Assurance Spec, Observer binds exact paths, format versions,
  revisions, digests, review/admission set, Environment Profiles, adapter lock,
  and compiler version. Any mismatch blocks compilation with a typed result.
- **OBS-AC-02 — Complete criterion disposition.** Every applicable criterion
  has one or more admitted obligations or a reviewed `not_applicable`
  disposition. `needs_design`, blocked, and exceptions remain visible and do
  not count as proof.
- **OBS-AC-03 — Reviewable semantic planning.** One typed semantic planner
  produces a proposed Assurance Spec. Review annotations and an admission
  receipt bind its exact revision/digest before deterministic compilation.
- **OBS-AC-04 — Honest planned red.** Design-time checks can be planned red
  without making trunk release-red before activation. Once activated, required
  negative or unknown states block the relevant gate.
- **OBS-AC-05 — Oracle sensitivity.** Every required oracle names a falsifier.
  The intended candidate passes and the known-bad candidate is refuted; an
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
  regression; model success grants no acceptance authority.
- **OBS-AC-12 — Swarm execution.** QA Swarm can shard manifest units within
  declared budgets, explore an uncovered frontier, and propose one distilled
  regression. Undistillable exploration remains `INCONCLUSIVE`.
- **OBS-AC-13 — Compiler determinism.** Identical ProductSpec, AssuranceSpec,
  admission, profile, adapter-lock, and compiler inputs produce byte-identical
  canonical manifest output without clock, random, network, discovery, model,
  timestamp, or absolute-path inputs.
- **OBS-AC-14 — Durable run evidence.** Runs flush partial evidence on failure
  or interruption and keep observation separate from infrastructure, flake,
  and freshness while recording exact inputs and artifacts.
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
- **OBS-AC-19 — Dependency freshness.** Subject, profile, adapter, source,
  target, contract, or oracle change stales only dependent evidence and
  requires explicit reconciliation.
- **OBS-AC-20 — Separate coverage ledgers.** Criterion traceability,
  obligation/environment execution, and reachable frontier remain separate;
  code coverage is advisory.
- **OBS-AC-21 — Document conformance.** The implementation includes a canonical
  format, schemas, parser/serializer/validator/CLI, stable codes, valid/invalid
  fixtures, round trips, schema/parser parity, version checks, and preserved
  custom sections.
- **OBS-AC-22 — Compiler conformance.** Duplicate/dangling/cycle, capability
  mismatch, subject drift, exact golden bytes, and separate gate-evaluation
  fixtures exist, plus a self-hosting Assurance Spec for Observer.
- **OBS-AC-23 — Independent revisions.** ProductSpec intent revisions,
  AssuranceSpec proof-intent revisions, implementation/manifest changes, and
  run receipts advance independently under explicit stale/reconciliation laws.
- **OBS-AC-24 — Immutable manifest.** The manifest contains resolved execution
  and dependency/gate graphs only; all dynamic status remains in receipts and
  projections.

## Candidate artifact vocabulary

- `*.assurance-spec.md` — authored proof design;
- `*.assurance-environment.json` — reusable environment policy/capabilities;
- `assurance-adapters.lock.json` — adapter versions and content digests;
- `*.assurance-manifest.json` — generated immutable execution graph;
- `*.assurance-review.json` — portable proof-design review;
- `*.assurance-decision-trace.json` — proof-policy change history;
- `openagents.observer.assurance_admission_receipt.v1`;
- `openagents.observer.oracle_sensitivity_receipt.v1`;
- `openagents.observer.seam_receipt.v1`;
- `openagents.observer.formal_check_receipt.v1`;
- `openagents.observer.run_receipt.v1`;
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

1. mapped — an admitted obligation set exists;
2. executable — adapters, targets, oracles, and falsifiers exist;
3. observed — current runs produced evidence at named tiers;
4. accepted — authorized review made a disposition.

The default summary says which criteria are fixture-proven, missing real-seam
evidence, stale, blocked, or owner-gated. It does not replace those facts with a
single “percent complete.”

## Product and business shape

### Local OSS

- ProductSpec, AssuranceSpec, and Environment Profile validation;
- deterministic Manifest compiler;
- adapter SDK and narrow reference adapters;
- local native test composition and JSON/HTML report;
- optional local/BYO model for proposal and exploration;
- no OpenAgents account.

### Later hosted service

- managed browser/version/viewport matrices;
- macOS/Windows/Linux native artifact runners;
- simulator and physical-device labs;
- private parallel QA Swarm exploration;
- retained encrypted evidence, trends, and review workflow;
- private/unlisted/opt-in public share pages;
- customer-specific adapters and assurance audits;
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

- generated-test theater;
- vague-criterion/specification laundering;
- testing mocks instead of seams;
- Effect Native capture of a supposedly portable protocol;
- formal-method cosplay over the wrong boundary;
- retry/flaky laundering;
- exception entropy;
- unsafe production tests;
- customer-data leakage;
- compiler or verifier authority creep;
- building a universal taxonomy before one useful vertical slice.

## Rollout

1. Build the minimum MVP dogfood slice in
   [`MVP_FIRST_ASSURANCESPEC.md`](./MVP_FIRST_ASSURANCESPEC.md).
2. Expand that admitted Assurance Spec criterion by criterion, importing
   existing tests before generating new ones.
3. Dogfood Effect Native/OpenAgents Desktop across local, packaged, seam, and
   release tiers.
4. Self-host Observer's own Assurance Spec and mutation-test the compiler.
5. Prove portability on one non-Effect project.
6. Only then consider a standalone standard repository or hosted service.

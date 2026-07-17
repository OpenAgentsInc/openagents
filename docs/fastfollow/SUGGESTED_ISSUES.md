# Fast Follow Suggested Issue Program

These are issue-ready proposals, not opened GitHub issues. Before opening or
claiming one, reconcile current `main`, the live Sol roadmap, existing issue
state, and other agents' claims. Keep shared schemas, generated catalogs,
lockfiles, Full Auto reconciliation, and central provider contracts under one
explicit integration owner.

The dependency order is intentional:

```text
FF-00 bootstrap
  └─ FF-01 format runtime
      ├─ FF-02 compiler/source pins
      │   └─ FF-03 StudyPacket
      │       ├─ FF-04 target gap planner
      │       │   └─ FF-05 candidate/admission lifecycle
      │       │       └─ FF-06 Desktop work-source integration
      │       │           └─ FF-07 portfolio supervisor
      │       │               └─ FF-08 operator UI
      │       └─ FF-09 shared public cache
      └─ FF-10 Khala/Blueprint optimization adapter

FF-11 dogfood/evaluation spans every landed rung.
```

## FF-00 — Land the Fast Follow 0.1 contract and OpenAgents seed

Status after this document lands: complete.

Outcome:

- define Fast Follow's relationship to ProductSpec, AssuranceSpec, issues,
  Full Auto, FleetRun, and Blueprint;
- add the authored format, canonical JSON projection schema, ProductSpec,
  OpenAgents seed, skill, AGENTS/invariant rules, and validation check;
- cover every current teardown; and
- document an honest current-Desktop overnight bootstrap.

Acceptance: FF-AC-13 through FF-AC-15.

## FF-01 — Build `@openagentsinc/fast-follow-spec`

Outcome: an Effect-native format package for authored FastFollowSpec 0.1.

Owned paths:

- `packages/fast-follow-spec/**`
- a narrow root script/package-script integration
- `docs/fastfollow/` only for conformance clarifications

Scope:

- Effect Schema types for frontmatter and every typed block;
- ordered Markdown parser and byte-stable serializer;
- stable diagnostics for missing/duplicate/order/JSON/schema/path/reference
  failures;
- exact document digest and versioned canonical intent digest;
- CLI: `validate`, `digest`, `projection`, and `init`;
- frozen valid/invalid conformance corpus with one fixture per diagnostic; and
- repository discovery of the nearest same-scope `FASTFOLLOW.md` with no
  implicit inheritance.

Acceptance:

- FF-AC-01 through FF-AC-03 pass in fixtures and against root
  `FASTFOLLOW.md`;
- unknown fields are intent-bound by default;
- any source path escape fails closed; and
- package tests, typecheck, distribution pack, and clean-consumer verification
  pass on owned compute.

Out: source fetching, model calls, work generation, admission, or Full Auto.

## FF-02 — Compile exact target/source snapshots into FastFollowManifest

Depends on: FF-01.

Outcome: a pure deterministic compiler plus Effectful source/target inventory
adapters.

Scope:

- resolve target repository, authority docs, and exact Git tree;
- resolve public Git source commit/tree, release/artifact identity, selected
  corpus, visibility, provenance, and capture confidence;
- compile `do_not_edit` FastFollowManifest work units and dedupe identities;
- keep I/O outside the pure compiler;
- never treat branch names, source instructions, or an installed artifact label
  as exact identity; and
- stable manifest content digest and deterministic repeated-build proof.

Acceptance:

- FF-AC-03 and FF-AC-04;
- same exact inputs produce byte-identical manifests;
- source/target drift produces a typed stale result, never silent rebind;
- closed-product observations carry their evidence confidence; and
- license/provenance `unknown` blocks copying while still allowing study.

Out: model-generated lesson content or candidate implementation.

## FF-03 — Generalize StudyBench into reusable Fast Follow StudyPackets

Depends on: FF-02.

Outcome: genericize the existing `openagents.repo_corpus_manifest.v0`,
`openagents.repo_study_packet.v0`, and `openagents.studybench_task.v0` ideas
without breaking their current OpenAgents-specific consumers.

Scope:

- generic source/corpus manifest with deterministic selected files and spans;
- packet schema for source maps, invariants, patterns, glossary, examples,
  tests, traps, playbooks, failures, confidence, visibility, provenance, and
  freshness;
- typed research planner request/response with complete lesson disposition;
- deterministic compiler that rejects unsupported or hallucinated source refs;
- fixed-budget no-packet versus packet evaluation; and
- redacted StudyReceipt with observed provider tokens/cost where available.

Acceptance:

- FF-AC-05 and the public half of FF-AC-09;
- every packet claim resolves to a pinned source span;
- repeated public studies can reuse the exact packet key;
- private material cannot enter packet construction, reflection, or evaluation;
- a statistical packet never claims exact replay proof; and
- retained counterexamples become packet fixtures.

Out: target gap/adoption decisions.

## FF-04 — Add target-bound GapAssessment planning and reconciliation

Depends on: FF-03.

Outcome: compare a reusable StudyPacket with the exact target without turning
source advice into target intent.

Scope:

- bind target revision plus AGENTS, invariants, ProductSpec, AssuranceSpec,
  roadmap, issue, code, test, and receipt identities;
- central typed semantic selector for directive and target-scope selection;
- deterministic planning request/response compiler;
- separate freshness, confidence, relevance, fit, portability,
  license/provenance, implementation, verification, disposition, and exception
  axes;
- stable gap and no-material-delta identities; and
- explicit reject/supersede/block/unavailable/inconclusive states.

Acceptance:

- FF-AC-02, FF-AC-05, and FF-AC-10;
- planner output disposes every directive and cannot add an unknown source or
  target authority ref;
- unchanged inputs cannot manufacture a second candidate;
- existing implementation and existing issue matches are reported rather than
  duplicated; and
- no blended score or self-acceptance exists.

Out: source mutation or admitted implementation.

## FF-05 — Build the WorkProposal, admission, claim, and closeout lifecycle

Depends on: FF-04.

Outcome: turn a target gap into an issue-ready proposal, then bridge only an
accepted proposal into existing target work authority.

Scope:

- WorkProposal schema with exact study/gap/target/directive refs, target paths,
  constraints, dependencies, proposed proof, and close rule;
- dedupe/supersession/disposition ledger;
- issue-ready Markdown projection with no automatic GitHub mutation by
  default;
- exact binding to an existing GitHub issue or ProductSpec work packet;
- Sol CLAIM protocol and isolated-worktree bridge; and
- implementation/verification receipt refs without moving their authority into
  Fast Follow.

Acceptance:

- FF-AC-05, FF-AC-06, and FF-AC-10;
- research cannot mint admission or a mutation lease;
- duplicate, stale, already-claimed, rejected, or policy-blocked candidates
  refuse visibly;
- an implementation claim names exact target post-image and candidate digest;
  and
- closeout keeps evidence-present, verified, accepted, merged, and released
  separate.

Out: a new general issue tracker or replacement for ProductSpec workrooms.

## FF-06 — Make Fast Follow a native Desktop Full Auto work source

Depends on: FF-01, FF-02, FF-04, FF-05.

Outcome: bounded parsed Fast Follow context reaches every admitted provider
lane, and Full Auto selects an exact work item before its existing durable
lease.

Hot paths, one integration owner:

- `apps/openagents-desktop/src/provider-lane.ts`
- `apps/openagents-desktop/src/spec-lane-workflow.ts` or its generalized
  work-context successor
- `apps/openagents-desktop/src/full-auto-reconcile.ts`
- `apps/openagents-desktop/src/full-auto-registry.ts`
- `apps/openagents-desktop/src/main.ts`
- Full Auto ProductSpec/AssuranceSpec and tests

Scope:

- generalize the host-owned before/after provider work-context projector;
- bounded Fast Follow identity, directive, stage, and candidate projection;
- `selectFullAutoWorkItem` before lease claim;
- persist work/spec/manifest/directive/candidate/claim refs on the exact turn;
- after-turn revalidation and typed stale/blocked/no-delta notes;
- CLI/MCP/OpenAPI profile additions only through the one existing control
  surface; and
- no provider-owned parser or second dispatch path.

Acceptance:

- FF-AC-07 and FF-AC-14;
- restart, overlapping reconciliation, workspace mismatch, provider mismatch,
  stop, cap, failure, stale spec, stale target, and duplicate candidate tests;
- unknown/unadmitted provider lanes still fail closed; and
- current ordinary Full Auto behavior remains compatible when no
  FastFollowSpec exists.

Out: five-worker concurrency.

## FF-07 — Add claim-aware FullAutoRunPolicy / FleetRun portfolio supervision

Depends on: FF-05 and FF-06.

Outcome: safely run multiple delivery, research, and implementation workers
against an explicit capacity policy.

Scope:

- separately admitted run-policy schema; do not put runtime leases in authored
  FastFollowSpec;
- capacity profiles and dynamic reallocation when a work source is exhausted;
- research writes limited to configured artifact paths;
- implementation requires admitted candidates and isolated claims/worktrees;
- account/provider readiness, usage, budget, rate-limit, retry, and breaker
  accounting;
- dependency-aware queue and hot-contract ownership;
- stop, pause, interrupt, drain, restart, and exact closeout; and
- one integration coordinator for shared contracts and `main`.

Acceptance:

- FF-AC-08;
- deterministic five-worker fixture proves `3/1/1` and `0/2/3` allocation;
- no candidate or work packet has two active mutation leases;
- research cannot cross its write boundary;
- a worker/account failure reallocates only under explicit policy; and
- a full process restart recovers the same run/work identities without duplicate
  dispatch.

Out: cross-user subscription pooling or paid shared capacity.

## FF-08 — Build the operator Fast Follow surface

Depends on: FF-06 and FF-07.

Outcome: the owner can understand and control the portfolio without reading
logs or hidden agent prompts.

Scope:

- current source/target freshness and exact revision;
- research versus implementation lane identity;
- directive, StudyPacket, gap, candidate, claim, worker, cost, proof, and
  disposition drill-down;
- cache hit provenance and privacy classification;
- queue, blocked, stale, no-delta, stopped, failed, verified, and accepted
  states;
- working stop/steer/pause/drain controls; and
- nightly digest with attention links, not completion claims.

Acceptance:

- FF-AC-12;
- tap, click, keyboard, and supported voice paths invoke the same typed intent;
- no raw credentials, private prompt/source body, absolute path, or provider
  event enters a public/client-safe projection; and
- visual/accessibility/keyboard/restart behavior contracts pass.

## FF-09 — Add the shared public StudyPacket cache

Depends on: FF-03 and visibility-policy review.

Outcome: many targets following the same public upstream reuse one expensive
study without sharing private adaptation context.

Scope:

- content-addressed public packet store and resolver;
- exact cache-key compiler;
- freshness, provenance, license, retention, invalidation, and tombstone state;
- tenant-safe target-local cache separation;
- observed provider/prompt-cache/token/cost accounting; and
- hit/miss/denied/stale public-safe receipts.

Acceptance:

- FF-AC-09;
- two distinct target fixtures reuse one exact public packet but produce
  isolated target gaps;
- cross-tenant private lookups refuse without existence disclosure;
- forbidden-material scanners cover packets, indexes, logs, and receipts; and
- cache hit never changes candidate disposition or admission.

Owner gate: retention/publication policy and any external shared service.

## FF-10 — Add the Khala/Blueprint Fast Follow program and optimizer boundary

Depends on: FF-01 through FF-04. May proceed after their schemas stabilize;
does not block the deterministic product.

Outcome: express the Fast Follow program as typed Blueprint signatures and
module candidates while keeping runtime control deterministic.

Scope:

- signatures for source resolution, study, comparison, target mapping,
  candidate planning, and evaluation;
- evidence-only Blueprint Program Runs bound to exact manifests and receipts;
- fixed deterministic control flow;
- optional offline GEPA/DSPy-class optimization of module parameters against
  executed target outcomes, tokens, latency, and cost; and
- candidate, shadow, review, and release-gate lifecycle with no self-promotion.

Acceptance:

- FF-AC-11;
- planner/optimizer cannot change source refs, target authority, control flow,
  visibility, or release gates;
- raw private target material never enters a shared optimizer corpus;
- baseline, packet, and optimized-candidate evaluations run at fixed budgets;
  and
- online runtime consumes only an admitted version with exact digest.

Out: deprecated Blueprint service revival, public marketplace, or settlement.

## FF-11 — Run the OpenAgents overnight dogfood and publish the evidence

Depends on: repeats after each useful rung. The first manual run can start from
FF-00; native allocation requires FF-07.

Outcome: prove that Fast Follow produces useful non-duplicative work under
real OpenAgents authority.

First manual run:

- three ordinary issue/backlog Full Auto sessions;
- one research session scoped to one directive and research artifact paths;
- one implementation session scoped to one already admitted candidate;
- isolated claims/worktrees for every mutating session; and
- retained stop, failure, token, candidate, test, and closeout evidence.

Evaluation:

- compare no packet versus packet at fixed task and token budget;
- measure wrong-source reads, duplicated candidate/PR rate, policy rework,
  verification close rate, no-delta honesty, and cache reuse;
- retain counterexamples and revise the packet/spec only through their revision
  laws; and
- do not publish a product claim from one successful night.

Acceptance:

- FF-AC-13 through FF-AC-15 for the manual rung;
- FF-AC-08 for the native five-slot rung;
- zero collisions and zero private shared-cache findings; and
- an owner-readable morning digest distinguishes observed, implemented,
  verified, accepted, blocked, and proposed work.

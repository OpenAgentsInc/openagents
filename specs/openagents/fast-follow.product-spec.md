---
spec_format_version: "0.1"
title: "Fast Follow Learning and Work Generation"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-16T00:00:00Z"
updated_at: "2026-07-16T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_design: "docs/fastfollow/FAST_FOLLOW_SPEC.md"
  openagents_seed: "FASTFOLLOW.md"
  openagents_issue_program: "docs/fastfollow/SUGGESTED_ISSUES.md"
  openagents_assurance_level: "cross-source-learning-authority"
---

## Problem

OpenAgents repeatedly performs excellent point-in-time teardowns of adjacent
products, but the findings live in prose. There is no stable machine-readable
target catalog, no way to declare that one target outcome should combine
lessons from several projects, no freshness or dedupe law, and no typed bridge
from shared upstream research to target-local issue and implementation work.

The result is duplicated inference, stale comparison documents, repeated gap
analysis, lost negative decisions, and autonomous agents that either ignore the
research or over-read it as permission to copy a competitor. When the ordinary
issue backlog is exhausted, Full Auto has no bounded, trustworthy, effectively
unbounded learning well. When capacity is available before the backlog is
empty, there is no explicit way to reserve research and admitted Fast Follow
implementation lanes beside ordinary delivery.

## Hypothesis

If every project can declare an authored FastFollowSpec beside `AGENTS.md`, and
OpenAgents deterministically compiles that learning intent with exact source
and target revisions into content-addressed study, gap, and candidate work,
then one expensive public-upstream study can be reused by many targets while
each target retains its own intent, policy, admission, and proof authority.

If Full Auto and FleetRun consume those candidates through their existing
workspace, provider, claim, lease, and verification paths, then owners can
allocate ordinary delivery, Fast Follow research, and admitted implementation
concurrently without creating a second unsafe autonomy system.

## Scope

```productspec-scope
in:
  - a discoverable nearest-scope `FASTFOLLOW.md` authored format with stable identity, independent format version and semantic revision, lifecycle, exact document digest, and canonical intent digest
  - typed target, source, lesson, directive, work-generation, reuse, guardrail, and authority blocks with stable IDs and complete referential integrity
  - one directive combining any number of lessons from any number of source projects into one bounded target outcome and target scope
  - exact source and target snapshot pinning in a deterministic immutable FastFollowManifest rather than mutable branch names in authored intent
  - public content-addressed StudyPackets, target-bound GapAssessments, evidence-only WorkProposals, explicit dispositions, and separate run receipts
  - independent source freshness, evidence confidence, relevance, target fit, portability, license/provenance, implementation, verification, disposition, and exception axes
  - public-only cross-project StudyPacket reuse with private target analysis isolated by default
  - a deterministic resolve, pin, study, map, evaluate, propose, admit, claim, implement, verify, and closeout program with typed recovery
  - ProductSpec and AssuranceSpec references without moving product intent or proof authority into FastFollowSpec
  - Full Auto work-source projection before provider dispatch and target revalidation afterward through the shared host-owned provider-lane seam
  - work-source selection before the existing durable Full Auto turn lease, with exact FastFollow identities persisted into the work/turn record
  - a separately admitted FullAutoRunPolicy or FleetRun capacity profile supporting delivery, Fast Follow research, and admitted Fast Follow implementation lanes
  - one research lane that may write only configured study, gap, candidate, receipt, and teardown artifacts; and one implementation lane that consumes only admitted target-local work
  - an OpenAgents seed covering every current `docs/teardowns/` document and the owner's five-slot 3/1/1 capacity example
  - Khala/Blueprint-compatible study and outcome artifacts that a future DSPy/GEPA-class optimizer may consume only as proposal evidence
out:
  - FastFollowSpec replacing AGENTS.md, INVARIANTS.md, ProductSpec, AssuranceSpec, roadmap, issue, work-packet, claim, test, review, release, owner, or product-promise authority
  - an upstream repository, teardown, model, skill, prompt, issue, test, cache hit, or optimizer granting mutation, dispatch, merge, release, spend, settlement, or promotion authority
  - implicit parent/child FastFollowSpec merging or inheritance in format 0.1
  - cross-tenant reuse of private target code, prompts, traces, gaps, credentials, customer data, or private holdouts
  - pooling user model subscriptions, credentials, or compute merely because public research artifacts are shared
  - a second Full Auto dispatch path or provider-owned FastFollowSpec parser
  - claiming that current per-session Desktop Full Auto is already a concurrent five-worker scheduler
  - requiring GEPA, DSPy, a deprecated Blueprint runtime, a public marketplace, or payment/settlement for the first implementation
cut:
  - automated public marketplace publication or contributor revenue sharing for StudyPackets
  - cross-machine Fast Follow control beyond the separately scoped Full Auto control program
  - automatic upstream issue or pull-request mutation
  - automatic ProductSpec or AssuranceSpec admission, owner acceptance, promise promotion, release, or deployment
```

## User Experience

A project author creates or edits one `FASTFOLLOW.md` beside the applicable
`AGENTS.md`. They choose sources, name lessons, combine them into directives,
set freshness and sharing policy, and select manual, backlog-fallback, or
continuous work generation. Validation shows stable diagnostics for malformed
blocks, duplicate IDs, dangling refs, unsafe paths, and unsupported versions.

An operator can inspect the exact source snapshots, reusable StudyPacket,
target gap, candidate, current disposition, claim, implementation, evidence,
and freshness state. Research and implementation are visibly different lanes.
A cache hit says “reused study evidence,” never “recommended” or “adopted.”

In Desktop, Fast Follow appears as a work source inside the existing Full Auto
experience rather than as a hidden second agent. A five-slot portfolio can
allocate three ordinary delivery workers, one research worker, and one admitted
implementation worker; the owner can stop, steer, and inspect each. If the
ordinary backlog empties, the run policy may reallocate capacity without
inventing work. `no_material_delta` is a successful honest closeout.

The format-0.1 bootstrap works before the native UI: repository `AGENTS.md` and
the bundled workspace skill direct existing per-session Full Auto to the seed
spec. The UI must label that as manual multi-session allocation, not native
fleet scheduling.

## Solution

The authored file is human-readable Markdown with YAML frontmatter and ordered
strict-JSON blocks. The compiler validates a canonical JSON projection, then
pins exact source and target revisions into an immutable manifest. Models may
propose lesson extraction, gap mapping, and candidate work through typed
outputs. Deterministic compilation checks exact inputs, reference completeness,
authority, cache classification, and dedupe identity.

The artifact split is deliberate:

- `StudyPacket` is public-upstream, digest-addressed, and reusable.
- `GapAssessment` is exact-target-bound and private by default.
- `WorkProposal` is an evidence-only issue/work-packet candidate.
- an ordinary admitted issue or work packet plus claim/lease authorizes
  implementation;
- AssuranceSpec, tests, review, receipts, and owner gates authorize confidence
  and acceptance through their existing paths.

Full Auto integration adds bounded Fast Follow projection to the existing
host-owned provider work-context seam and selects a typed work item before
claiming the existing durable continuation lease. Fleet concurrency belongs in
a claim-aware supervisor, not in the authored learning document.

## Acceptance Criteria

- **FF-AC-01:** A target resolves the nearest applicable `FASTFOLLOW.md` beside
  `AGENTS.md`; missing, escaped, malformed, unsupported, duplicate, or dangling
  input fails with a stable diagnostic and produces no inferred policy.
- **FF-AC-02:** The parser and validator expose stable source, lesson, and
  directive IDs; every `source#lesson` reference resolves exactly once, and one
  directive can deterministically combine lessons from multiple projects for
  multiple bounded target scopes.
- **FF-AC-03:** Exact authored bytes and canonical learning intent have separate
  SHA-256 identities. A source or target revision refresh recompiles a new
  immutable manifest without editing authored intent, while a material learning
  change requires a Fast Follow revision bump.
- **FF-AC-04:** External repository content, docs, prompts, skills, hooks, and
  scripts are decoded as untrusted study material. Research cannot mutate
  product code or an external source and cannot gain network, credential,
  provider, spend, release, deployment, or SCM authority from the spec.
- **FF-AC-05:** Public StudyPackets, target-bound GapAssessments, WorkProposals,
  and receipts have distinct schemas, exact refs, dedupe keys, freshness, and
  dispositions. No artifact collapses evidence, implementation, verification,
  owner acceptance, and exception into one score.
- **FF-AC-06:** Candidate implementation starts only from a current target-owned
  admitted issue, accepted plan, or work packet after authority reconciliation,
  isolated mutation claim/worktree, dependency checks, and target-local proof
  requirements. Research agents cannot self-admit their candidates.
- **FF-AC-07:** Full Auto consumes Fast Follow through the existing host-owned
  provider-lane projection and the existing serialized reconciliation/lease
  path. The bound turn records exact work, spec, manifest, directive, candidate,
  and claim identities; no parallel dispatcher or provider parser exists.
- **FF-AC-08:** A separately admitted run policy can enforce the owner example
  of `3 delivery / 1 research / 1 implementation` while actionable backlog
  exists and `0 / 2 / 3` after it empties, without duplicate claims or mutation
  collisions. Current per-session Desktop behavior remains honestly labeled
  until this supervisor is implemented and proven.
- **FF-AC-09:** A shared StudyPacket cache key binds exact public source bytes,
  lesson/program/schema/planner/tool/evaluator versions, and visibility/license
  policy. Private target analysis is isolated, and a cache hit projects only
  reusable evidence—not relevance, adoption, verification, or permission.
- **FF-AC-10:** Source refresh, gap mapping, candidate proposal, rejection,
  supersession, staleness, unavailability, inconclusive, and
  `no_material_delta` are durable dispositions. Exact dedupe prevents the same
  unchanged candidate from being reopened merely to keep an autonomous loop
  busy.
- **FF-AC-11:** Khala/Blueprint and any DSPy/GEPA-class module consume only
  typed, redacted, visibility-safe artifacts and executed outcome evidence.
  Optimizer output is a candidate behind review and release gates and cannot
  dispatch, mutate, self-promote, pay, settle, or change a public claim.
- **FF-AC-12:** The operator surface distinguishes research, candidate,
  admitted implementation, verification, blocked, stale, and terminal states;
  shows source/target freshness and cache provenance; and retains working stop,
  steer, and bounded-cost controls.
- **FF-AC-13:** The committed OpenAgents seed validates and references every
  non-README Markdown document in `docs/teardowns/`, including cross-target and
  OpenAgents-synthesis documents, with no dangling lesson or directive ref.
- **FF-AC-14:** On current main, an owner can turn on the existing Full Auto
  toggle and explicitly run the seed's research, admitted implementation, or
  backlog-fallback method through repository instructions and the Fast Follow
  skill; docs state the 20-continuation cap and do not claim native portfolio
  scheduling.
- **FF-AC-15:** Format validation, reference validation, teardown coverage,
  authority boundaries, and the two five-slot capacity profiles have committed
  automated checks. Any future breaking validation change bumps the format and
  freezes a conformance corpus.

## Success Metrics

```productspec-success-metrics
- id: fast_follow_source_reuse
  metric: eligible_public_upstream_studies_reused_by_more_than_one_target_without_repeating_source_inference
  target: ">= 60% after three adopting targets"
  window: first 90 days after shared StudyPacket service admission
  segment: exact-source-and-lesson cache eligible studies
  source: fast_follow_study_and_cache_receipts
- id: fast_follow_candidate_quality
  metric: admitted_fast_follow_candidates_that_close_with_target_local_verification_without_policy_rework
  target: ">= 70%"
  window: rolling 30 admitted candidates
  segment: implementation-stage OpenAgents dogfood candidates
  source: candidate_implementation_and_verification_receipts
- id: fast_follow_duplicate_work
  metric: concurrent_or_repeated_implementation_attempts_for_the_same_candidate_digest_without_explicit_supersession
  target: "0"
  window: every Full Auto and FleetRun dogfood
  segment: all Fast Follow mutation lanes
  source: claim_lease_and_candidate_dedupe_ledger
- id: fast_follow_private_cache_exposure
  metric: private_target_code_prompt_trace_credential_or_customer_material_occurrences_in_shared_study_packets_or_cross_tenant_cache
  target: "0"
  window: every artifact publication and cache read
  segment: all shared Fast Follow artifacts
  source: visibility_policy_and_forbidden_material_scans
- id: fast_follow_honest_no_delta
  metric: source_refreshes_with_no_material_target_gap_that_close_without_manufacturing_a_candidate_issue
  target: "100%"
  window: every no-material-delta refresh
  segment: unchanged source and target intent combinations
  source: gap_disposition_and_candidate_dedupe_receipts
```

## Risks

- “Fast follow” can be read as permission to copy product surfaces or authority
  postures. Stances, target constraints, exact provenance, and admission must
  remain first-class.
- A “limitless” well can turn into churn. Content digests, negative
  dispositions, freshness, and `no_material_delta` are safety features, not
  optional reporting polish.
- Shared inference can leak target-private context. Only public upstream study
  is cross-project by default; target adaptation stays isolated.
- Current Full Auto is serialized per Desktop instance and has no retry timer
  after a failed turn unless another trigger fires. Native portfolio work must
  not be designed as prompt text over that limitation.
- A five-worker run can duplicate PRs and collide on hot contracts. Claims,
  isolated worktrees, dependency ordering, and one integration owner are
  mandatory.
- Closed product teardowns can overstate observations. Access, confidence, and
  server-behavior limits must survive distillation.
- License and provenance may block a technically attractive port. A rejection
  is a correct outcome.
- Optimizer language can make proposals sound promoted. Blueprint evidence,
  admission, execution, verification, release, and public claims remain
  separate transitions.

## Owner Gates

- Admission of the native FastFollowSpec parser/compiler and any repository-wide
  enforcement beyond the format-0.1 dogfood contract.
- Any default capacity policy that reserves paid provider or compute slots.
- Any cross-tenant StudyPacket service, retention policy, public artifact
  publication, contributor attribution/revenue, or settlement mechanism.
- Any public claim that Fast Follow autonomously keeps a project at parity,
  reduces cost by a stated amount, or operates a concurrent five-worker fleet.
- Any change that lets an optimizer or research agent admit implementation or
  promotion without a target-owned review path.

## Receipts

- Exact FastFollowSpec document/intent digest and validation report.
- Deterministic manifest receipt binding compiler, target, sources, authority
  docs, directives, and work units.
- StudyPacket source/corpus/license/visibility/freshness receipt.
- GapAssessment target and authority reconciliation receipt.
- Candidate dedupe, disposition, admission, claim, and supersession receipts.
- Implementation post-image plus target-local test, assurance, review, and
  owner disposition refs.
- Shared-cache hit/miss and forbidden-private-material scan receipts.
- Full Auto/FleetRun allocation, provider usage, stop, retry, recovery, and
  collision-free claim receipts.

## Promise Links

No public promise is created or promoted by this ProductSpec. Any future
claim about autonomous parity, cost reduction, shared intelligence, overnight
work, or verified adaptation requires a separately registered product promise,
fresh dereferenceable receipts, and owner sign-off.

---
spec_format_version: "0.1"
title: "Standing Independent-Review Capacity"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-20T00:00:00Z"
updated_at: "2026-07-20T00:00:00Z"
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
  openagents_epic: "9104"
  openagents_lane: "VSE-03 (#9108)"
  openagents_assurance_level: "verification-governance"
  openagents_origin: "docs/fable/2026-07-20-verifiable-software-engine-gap-analysis-and-roadmap.md gap G1"
  openagents_admission_status: "candidate specification; not owner-admitted. Dispatch remains limited to a live issue or accepted plan. This document declares the independent-review function; it grants no admission authority and admits no AssuranceSpec."
  openagents_consumer_admission: "specs/desktop/full-auto.assurance-spec.md (#8978) is the first consumer"
  openagents_independence_law_sources: "AD-AC-10 in specs/openagents/authority-delegation.product-spec.md; QA-5 #8910; packages/assurance-spec admission gate (producer_may_verify hard error)"
---

## Problem

The AssuranceSpec admission gate already refuses producer self-admission. The
compiler rejects any AssuranceSpec whose `independence.producer_may_verify` is
true, admission needs a `recognized_actor_ref` and a `recognized_role`, and
QA-5 (#8910) landed the "no agent accepts its own work" rule. But the gate
assumes a reviewer exists. Every admission so far consumed ad hoc reviewer
capacity, only one AssuranceSpec (the MVP) has ever been admitted, and no spec,
issue, or budget owns the standing function that supplies an independent
reviewer, sets a review cadence, and records honestly when no independent
reviewer is available. The gap analysis names this G1: the independence law
exists as a gate, the capacity to satisfy it does not. Without that capacity,
independent admission becomes the single bottleneck that blocks every surface
AssuranceSpec from reaching `observed`.

## Hypothesis

If a standing independent-review function defines reviewer roles, keeps
reviewer identity provably separate from producer identity, sets an explicit
review cadence and budget, admits community falsification through the existing
Forum-first and strict-bug-form intake as receipted review input, and records a
typed escalation whenever no independent reviewer exists for a domain, then
AssuranceSpec admissions stop depending on improvised capacity, the
producer/verifier boundary the compiler already enforces gains a resourced
supply side, and an unstaffed review lane fails closed and visibly rather than
being self-filled by the producer.

## Scope

```productspec-scope
in:
  - a reviewer role model distinct from the producing role, expressed against the existing `AssuranceAdmission` fields (`recognized_actor_ref`, `recognized_role`, `review_set_digest`) so the admission compiler consumes it unchanged
  - identity-separation rules: the operating identity that authored or executed an obligation's evidence may never be the identity that admits that obligation, verified by distinct actor refs and, where the authority profile requires it, distinct execution identity per AD-AC-10
  - an explicit review cadence and budget: the expected reviewer effort per admission, who allocates it, the acceptable admission-queue depth, and the disposition when the queue exceeds it
  - community falsification as receipted review input: a Forum-first report or a strict-bug-form submission becomes a typed review observation with its own receipt, feeding the same obligation dispositions as a commissioned review
  - a typed escalation path recorded when no independent reviewer exists for a domain, so the gap is a visible honest state, not a producer self-admission
  - a public-safe review record: which obligations an admission covered, the reviewer role and actor ref, the review-set digest, and the disposition, without exposing prompts, private evidence bodies, or reviewer-private notes
out:
  - a paid verification market, bonded outcomes, or settlement (owner-gated VERIFY-PRICED, a later phase)
  - any change to the no-self-admission law, the compiler's digest-exactness gate, or the `producer_may_verify` hard error
  - admission of any specific AssuranceSpec; #8978 and its successors own their own admissions and their own evidence
  - automated adjudication of whether an obligation's evidence is sufficient; the reviewer role is human-or-designated-independent judgment, not a new oracle
cut:
  - self-amplification: a review record, a role assignment, or a cadence budget never grants deploy, spend, release, public-claim, or invariant-bypass authority
  - reviewer identity reuse across the producer boundary to clear a backlog faster
  - treating an unstaffed escalation as a soft pass
```

## User Experience

An admission owner assembles an AssuranceSpec's evidence, then requests review.
The function resolves an eligible reviewer whose identity is provably not the
producer's, records the review-set digest and role, and returns a typed
disposition per obligation. An outside contributor who files a falsification in
the Forum or via the strict-bug form sees it become a receipted review
observation attached to the relevant obligation, not a comment that evaporates.
When no independent reviewer exists for a domain, the owner sees an explicit
`escalation_required` state naming the missing capacity, and the admission
stays blocked rather than passing.

## Acceptance Criteria

- Every admission names a reviewer actor ref that is provably distinct from
  every actor ref that authored or executed the covered obligation's evidence.
  A shared actor ref across the producer boundary is a typed refusal.
- The role model maps onto the existing `AssuranceAdmission` schema without
  changing it: `recognized_actor_ref`, `recognized_role`, and
  `review_set_digest` carry the function's output, and the compiler admits the
  result only when `lifecycle_state === "admitted"` and every digest matches.
- The cadence and budget are stated as concrete values: reviewer effort per
  admission, the allocating authority, and the maximum admission-queue depth
  before the function reports `queue_over_budget`.
- A Forum-first or strict-bug-form falsification becomes a typed review
  observation with a receipt, and that observation can move an obligation
  disposition exactly as a commissioned review can. A falsification that
  survives is recorded against the reviewer track record.
- When no independent reviewer exists for a domain, the function returns a
  typed `escalation_required` state naming the missing capacity, the admission
  stays blocked, and no producer identity may clear it.
- No output of this function grants any authority beyond recording a review
  disposition. A review record cannot deploy, spend, release, flip a promise,
  or admit anything the digest-exact compiler would not already admit.

## Success Metrics

```productspec-success-metrics
- id: producer_verifier_separation
  metric: admissions_with_reviewer_actor_ref_distinct_from_every_producing_actor_ref
  target: "100%"
  window: every admission processed through the function
  segment: all AssuranceSpec admissions
  source: admission_review_records_and_actor_ref_audit
- id: admission_queue_health
  metric: admissions_blocked_only_by_absent_independent_reviewer
  target: "reported honestly as escalation_required, never self-cleared; queue depth within stated budget"
  window: rolling review cadence
  segment: surface AssuranceSpecs awaiting admission
  source: review_queue_receipts
- id: community_falsification_intake
  metric: forum_or_strict_bug_falsifications_converted_to_receipted_review_observations
  target: "100% of in-scope submissions carry a receipt and a typed disposition"
  window: every falsification intake
  segment: outside-contributor falsifications against admitted or pending obligations
  source: falsification_intake_receipts
- id: escalation_honesty
  metric: unstaffed_review_domains_shown_as_escalation_required_rather_than_passed
  target: "100%; zero producer-self-cleared escalations"
  window: every admission attempt against an unstaffed domain
  segment: domains with no eligible independent reviewer
  source: escalation_state_receipts
```

## Risks

- A role model that lives only in prose can be bypassed under backlog
  pressure. Identity separation must be a decoded refusal in the admission
  path, not a convention a rushed operator can skip.
- Community falsification intake can become a denial-of-service or a
  low-signal flood. Intake must be bounded, receipted, and triaged, and a
  surviving falsification must be distinguishable from a rejected one.
- An escalation that reads as a soft warning invites self-clearing. The state
  must block admission and must not be resolvable by a producer identity.
- Independence by role is necessary but not sufficient: correlated reviewers
  that share the generator's priors can miss the same failures. This spec
  supplies the capacity contract, verification diversity remains a separate,
  later obligation.

## Solution

The function is a thin governance layer over the machinery that already exists.
`packages/assurance-spec` owns the admission compiler, the `AssuranceAdmission`
schema, and the `producer_may_verify` hard error. This spec adds the supply
side: a reviewer registry keyed by role and actor ref, an identity-separation
check that runs before an admission is compiled, a cadence-and-budget record
that bounds queue depth, and a receipted intake that turns Forum and
strict-bug-form falsifications into typed review observations. Every output is
a review record that the digest-exact compiler either consumes or rejects, the
function never widens what the compiler would admit. When the registry has no
eligible independent reviewer for a domain, the function emits
`escalation_required` and the admission stays blocked, which is the honest
state the gap analysis asks for.

## Related Artifacts

- Origin gap analysis:
  `docs/fable/2026-07-20-verifiable-software-engine-gap-analysis-and-roadmap.md`
- Independence law and distinct-execution-identity requirement:
  `specs/openagents/authority-delegation.product-spec.md` (AD-AC-10)
- Admission machinery and the producer-self-admission refusal:
  `packages/assurance-spec`
- First consumer: `specs/desktop/full-auto.assurance-spec.md` (#8978)
- Verifiable-software thesis and its independence risk:
  `docs/fable/2026-07-19-verifiable-software.md`

## Owner Gates

- Any public claim that OpenAgents operates a standing independent-review
  function. A landed spec and code alone cannot authorize that statement.
- Naming or resourcing specific human or designated-independent reviewers is
  an owner action, not a producer action.

## Receipts

- Admission review records binding reviewer actor ref, role, review-set
  digest, and per-obligation disposition.
- Falsification-intake receipts for Forum-first and strict-bug-form
  submissions converted to review observations.
- `escalation_required` receipts naming the missing reviewer capacity per
  domain.

## Promise Links

- No promise flips to green from this spec. The verification-governance
  promises stay non-green until the function has processed real admissions
  and produced the receipts above.

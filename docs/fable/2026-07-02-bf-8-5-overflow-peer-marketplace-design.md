# BF-8.5 Overflow/Peer Marketplace Deferred Design

Date: 2026-07-02
Status: deferred design and gate contract for
[`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) BF-8.5 / GitHub issue #8113. This
flips no product-promise state, grants no marketplace dispatch authority,
creates no settlement path, and broadens no public copy.

Source material:

- [`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) BF-8.5: qualified, intake-complete
  matters can eventually be sub-contracted to vetted peers with a platform cut,
  but only after BF-4 is proven.
- [`2026-07-02-business-fulfillment-engine-meditations.md`](./2026-07-02-business-fulfillment-engine-meditations.md):
  the fulfillment engine shape, the BF-4 proof burden, and the
  "client-identifying information never enters this repo" invariant.
- [`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md) AW-0/AW-6: services engagements
  produce demand evidence, case studies, and repeatable delivery receipts
  before broader network routing.
- [`../promises/checks-and-gates.md`](../promises/checks-and-gates.md) and
  [`../promises/registry.md`](../promises/registry.md): marketplace, payout,
  and settlement copy must stay separated from availability, readiness, and
  proof.

## 1. Narrow Claim

The overflow/peer marketplace is a future expansion of the business
fulfillment engine. Once the engine can reliably intake, qualify, promise,
fulfill, review, receipt, and prove customer work in-house, an
intake-complete matter may be offered to a vetted peer when OpenAgents should
not or cannot fulfill it directly.

The marketplace does not sell "generic leads." It routes only scoped,
customer-approved, receipt-planned matters whose private context remains under
the workspace authority boundary. The first design target is a manual,
operator-approved referral of an overflow engagement to one vetted peer, with
an opaque matter ref and a written receipt plan. Automation, self-serve peer
discovery, pooled routing, and settlement-bearing open-market dispatch are
later gates.

Until the section 8 gate passes, safe copy must say this is **planned** or
**deferred**. Unsafe copy includes "marketplace live", "we subcontract your
matter automatically", "earn from customer work today", "peer providers are
available now", or any payout/platform-cut claim without a settlement receipt.

## 2. Entry Gate: What Counts As Intake-Complete

A matter may be considered for peer overflow only when all of these records
exist:

| Record | Requirement |
| --- | --- |
| `intakeSpecRef` | Structured qualification summary confirmed by the customer. |
| `scopeRef` | Bounded deliverable scope, exclusions, acceptance criteria, and due window. |
| `promiseRef` | Per-customer service promise or accepted-outcome contract naming the committed deliverable. |
| `receiptPlanRef` | Planned evidence bundle: artifacts, review decisions, handoff refs, and metric receipts expected at closeout. |
| `privacyTier` | `standard`, `regulated_private`, or stricter; peer eligibility must match or exceed it. |
| `redactionPolicyRef` | Corpus/redaction policy for any context that could leave the customer workspace. |
| `approvalRef` | Explicit customer/operator approval to route the matter to a named external peer. |

If any record is absent, the matter is not overflow-eligible. The system must
surface it as an in-house or operator-blocked engagement, not as marketplace
supply.

## 3. Peer Trust Requirements

A peer is eligible for a matter only when the peer profile has current,
auditable trust evidence:

- **Identity and contracting:** verified legal/contact profile, signed service
  terms, confidentiality terms, tax/payment readiness where applicable, and a
  revocable provider ref.
- **Capability fit:** vertical descriptors, deliverable kinds, rubric scores,
  example receipt refs, and explicit exclusions. No client names or raw work
  samples in public projections.
- **Authority fit:** allowed tools, credential policy, communication channels,
  review obligations, and whether the peer may interact directly with the
  customer. Default is no direct contact without an approval receipt.
- **Privacy fit:** maximum data tier accepted, redaction-before-disclosure
  support, regulated-data restrictions, raw-log retention policy, and incident
  contact.
- **Quality history:** accepted/rejected outcome counts, review-minutes
  burden, rework rate, on-time rate, and dispute refs. Counts are public-safe;
  evidence details are owner/customer-private.

No peer can receive regulated or customer-identifying context merely because
they are generally "vetted." Eligibility is matter-specific and must be
recomputed against the matter's privacy tier, vertical profile, and tool
authority.

## 4. Routing Flow

The first implementation should be a deliberate, low-automation handoff:

1. Operator marks an intake-complete matter as overflow-candidate and chooses
   one vetted peer.
2. The system creates an `overflowOfferRef` with the opaque matter ref, scope
   summary, deliverable kind, due window, budget range, receipt plan, privacy
   tier, and redaction policy.
3. The peer accepts or declines. Acceptance creates an `overflowEngagementRef`
   and a delivery workspace or peer-limited workroom.
4. The peer performs work inside the approved authority boundary and submits
   artifacts plus receipt evidence.
5. OpenAgents review gates run before anything is sent, published, filed, or
   marked accepted for the customer.
6. Customer acceptance, rejection, rework, and closeout are recorded against
   the original promise and the peer engagement lineage.

The customer-facing promise remains owned by OpenAgents until a later contract
explicitly says otherwise. Peer execution is a delivery lane, not an excuse to
weaken the BF-4 approval ladder.

## 5. Receipt And Data Model

The first receipt can be documentation-backed. The implementation target is a
structured receipt with opaque refs:

```yaml
receiptKind: business.overflow_peer_engagement.v1
matterRef: matter.<opaque>
workspaceRef: workspace.<opaque>
overflowOfferRef: overflow_offer.<opaque>
overflowEngagementRef: overflow_engagement.<opaque>
peerRef: peer.<opaque>
verticalProfile: legal | health | agency | commerce | software | other
deliverableKind: document | site | campaign | workflow | software | other
privacyTier: standard | regulated_private
redactionPolicyRef: redaction_policy.<opaque>
approvalRefs: []
receiptPlanRef: receipt_plan.<opaque>
artifactRefs: []
reviewDecisionRefs: []
customerAcceptanceRef:
platformFeePolicyRef:
settlementState: not_applicable | planned | pending | settled | disputed
publicSafety: opaque_refs_only
```

Public projections may expose vertical descriptors, deliverable kinds, counts,
timestamps, and opaque receipt refs. They must not expose customer names,
private matter facts, raw prompts, provider payloads, local paths, peer
contract terms, tax data, wallet data, or settlement secrets.

## 6. Platform Cut And Settlement Design

BF-8.5 needs a platform-cut policy before any implementation because the cut
changes incentives and dispute handling:

- **Customer price:** the customer pays OpenAgents or an approved processor
  path under the existing business receipt model.
- **Peer share:** a percentage or fixed amount is computed from the accepted
  outcome, not merely from offer acceptance.
- **Platform cut:** recorded as a policy ref with versioned terms; public copy
  may mention a platform cut only after the policy and settlement receipt
  exist.
- **Holdback:** a configurable holdback covers review, customer acceptance,
  refund windows, and disputes.
- **Disputes:** rejection, rework, partial acceptance, and breach cases must
  have explicit settlement states and audit refs.

Until the money-moving path is armed, receipts must use
`settlementState: "not_applicable"` or `planned`. Do not synthesize earnings,
payouts, or public counter movement from offer state, peer acceptance, or
review progress.

## 7. Failure Modes To Design Against

- **Lead marketplace drift:** selling unqualified prospects instead of
  intake-complete, receipt-planned matters.
- **Authority laundering:** routing work to a peer to bypass BF-4 review,
  redaction, approval, or professional sign-off.
- **Privacy leak by summary:** exposing enough scope detail in an offer or
  public receipt to identify the customer or matter.
- **Perverse cut incentives:** preferring peer routing because it produces a
  fee, even when in-house delivery would better satisfy the promise.
- **Unpayable acceptance:** accepting peer work without a reconciled customer
  acceptance, platform-fee policy, and settlement state.
- **Quality debt:** counting peer capacity as scale while review-minutes per
  accepted outcome rises.

## 8. Green Gate For #8113 Follow-On Implementation

This design doc is the #8113 deliverable. It does not complete the future
marketplace implementation. A later implementation issue may open only after:

1. BF-4 is proven by at least one real deliverable through the surfaced
   workroom, per-customer promise, approval ladder, and fulfillment receipt.
2. BF-1/BF-2 can produce an intake-complete, paid or payment-planned matter
   with an opaque workspace ref and receipt plan.
3. BF-3 redaction/privacy policy can classify what context may leave the
   customer workspace.
4. Peer vetting records exist with trust, capability, privacy, and quality
   evidence.
5. The operator and customer approval path names the peer and the scope before
   any context is disclosed.
6. The platform-cut policy, holdback/dispute states, and settlement receipt
   shape are reviewed before any payout copy or live money movement.
7. Public-safe projections are tested to ensure they expose only opaque refs,
   vertical descriptors, counts, and receipt states.

## 9. Non-Goals

- No build, schema migration, route, UI, worker, settlement, or public API in
  this issue.
- No client-identifying information, private matter facts, raw prompts, local
  paths, provider payloads, credentials, wallet data, or peer contract terms in
  this repository.
- No public promise flip, pricing copy, payout copy, provider-capacity copy,
  or marketplace launch claim.
- No self-serve peer onboarding, pooled capacity routing, or open-market
  dispatch before the BF-4 and settlement gates are satisfied.

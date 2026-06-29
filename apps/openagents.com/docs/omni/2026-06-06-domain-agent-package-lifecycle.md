# Domain Agent Package Lifecycle

Status: implemented for issue #374 / `OPENAGENTS-LATE-014`.

## Purpose

Domain agents need a lifecycle that can move from package draft to fixture
validation, review, org-private enablement, public projection, runtime
promotion, and marketplace attribution without automatically installing,
promoting, monetizing, or executing anything. This contract records that
lifecycle as a read-only evidence projection.

Implementation:

- `workers/api/src/omni-domain-agent-packages.ts`
- `workers/api/src/omni-domain-agent-packages.test.ts`

## Package Record

The package record carries:

- package and version refs;
- domain kind;
- display name ref;
- context template refs;
- outcome template refs;
- program signature refs;
- fixture records;
- review records;
- enablement records;
- public projection refs;
- promotion records;
- attribution records;
- source refs;
- blocker refs; and
- caveat refs.

Projection timestamps are friendly labels and do not expose raw ISO strings.

Supported domain kinds include Sites, Forum, CRM follow-up, investor ops,
project ops, support, legal safe-hold, Pylon ops, and general packages.

## Lifecycle States

Supported states:

- `draft`;
- `fixture_validated`;
- `review_recorded`;
- `org_private_enabled`;
- `public_projection_ready`;
- `runtime_promotion_requested`;
- `runtime_promoted`;
- `marketplace_attributed`;
- `blocked`; and
- `deprecated`.

Each later state requires the evidence that justifies it. Fixture validation
requires passed fixtures. Review requires approved reviews. Org-private
enablement requires enabled org-private records. Public projection requires
public projection refs and enablement. Runtime promotion request requires a
promotion record. Runtime promotion requires a promoted record. Marketplace
attribution requires recorded attribution. Blocked packages require blocker
refs.

## Fixtures And Review

Fixture records include scenario refs, expected outcome refs, evidence refs,
validation receipt refs, caveats, score in basis points, and state. Passed
fixtures require validation receipts, evidence, and expected outcomes.

Review records include reviewer refs, evidence refs, receipt refs, caveats,
and state. Approved reviews require reviewer, evidence, and receipt refs.

## Enablement

Enablement records separate:

- `org_private`;
- `public_projection`; and
- `runtime`.

Enabled records require approval receipts, receipts, and audience refs. This
keeps org-private enablement distinct from public projection and both distinct
from runtime promotion.

## Runtime Promotion And Rollback

Promotion records include promotion refs, evidence refs, approval receipt refs,
runtime receipt refs, rollback refs, rollback posture, caveats, and state.

Runtime promotion is gated by explicit evidence and approvals. Promoted
records require runtime receipts and `rollback_ready` posture. Rollback-ready
or rolled-back records require rollback refs.

The projection still reports `runtimePromotionAllowed: false`; this contract
does not promote packages by itself.

## Marketplace Attribution

Attribution records include accepted outcome refs, contributor refs, package
version refs, split policy refs, receipt refs, caveats, and state. Recorded
attribution requires accepted outcome, contributor, receipt, and split policy
refs. This is marketplace memory, not a payout or listing mutation.

## Authority Boundaries

Domain agent package lifecycle projections cannot:

- execute fixtures;
- mutate reviews;
- enable org access;
- mutate public projection;
- promote runtime packages;
- create marketplace listings;
- spend or mutate payment state; or
- mutate rollback state.

## Projection Audiences

Supported audiences are:

- `public`;
- `agent`;
- `customer`;
- `team`; and
- `operator`.

Public and agent projections redact private package source, review,
enablement, promotion, attribution, provider, receipt, rollback, source,
split, title, and version refs as appropriate. Operator and team projections
can retain the full safe ref set, but all projections reject private package
source, secrets, customer data, provider credentials, payment/wallet material,
raw fixtures, private repos, raw logs, and raw timestamps.

## Tests

Coverage includes:

- lifecycle projection and friendly times;
- fixture validation, review, enablement, public projection, runtime
  promotion, and marketplace attribution separation;
- promotion gates and rollback posture requirements;
- attribution receipt requirements;
- public redaction; and
- hard false fixture execution, review, enablement, public projection,
  runtime promotion, marketplace listing, payment, and rollback mutation
  authority.

# Market Memory Hooks v1

Issue #218 adds the first Omni market memory hook model.

Market memory hooks are an evidence ledger for learning from accepted and
rejected outcomes. They are not routing authority, payout authority, public
claim authority, or module promotion authority.

## What They Record

Each hook links:

- Workroom ref.
- Lifecycle decision ref.
- Accepted or rejected outcome state.
- Work kind.
- Memory category.
- Memory ref.
- Evidence ref.
- Source ref.
- Optional route scorecard ref.
- Optional economics ref.
- Public caveat ref.

The initial categories are:

- `route_quality`
- `account_reliability`
- `repo_convention`
- `source_quality`
- `module_usefulness`
- `marketplace_attribution`

## Authority Boundary

Every hook is written with `authorityBoundary = evidence_only` and explicit
boolean guardrails:

- `noRoutingMutation`
- `noPayoutMutation`
- `noPublicClaimMutation`
- `noModulePromotion`

This lets future route memory, repo memory, marketplace attribution, and
Blueprint release gates consume evidence while preventing this early ledger
from becoming a hidden control plane.

## Redaction

The model rejects refs and metadata that contain raw provider payloads, run
logs, raw emails, payment material, wallet material, settlement/payout claims,
route overrides, module promotion claims, public-claim publication, or private
customer material.

## Lifecycle Match

Hooks must point at an existing workroom and lifecycle decision. The lifecycle
decision must belong to that workroom and work kind, and the hook's outcome
state must match the decision's accepted or rejected state.

## Next Steps

Later systems can use these hooks to:

- Rank routes after reviewed outcomes.
- Remember repo conventions and flaky command patterns.
- Track source usefulness for Exa/retrieval planning.
- Attribute useful modules and Program Signatures.

Those consumers still need separate reviewed promotion gates before changing
production routing, public proof, payouts, or runtime authority.

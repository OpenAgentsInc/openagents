---
spec_format_version: "0.1"
title: "Sarah Owner Orchestrator"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-18T00:00:00Z"
updated_at: "2026-07-18T00:00:00Z"
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
  openagents_authority: "AUTHORITY.md revision 3 + docs/authority/SARAH_AUTHORITY.md revision 1"
  openagents_source: "docs/transcripts/24Xsarah.md"
  openagents_assurance_level: "owner-private cross-resource orchestration"
---

## Problem

The owner must currently assemble company state by asking different agents,
opening issues, checking release pages, inspecting Full Auto, and remembering
prior decisions. Earlier Sarah work created a separate public app, prospect
CRM state, avatar services, and another relationship model. That duplicated
the product and was retired. The durable need remains: one trusted contact on
the phone who remembers the business, knows what is live, makes decisions, and
can get admitted work done without repeatedly waiting for the owner.

## Hypothesis

If Sarah is an authenticated principal on one stable Khala Sync thread, with
cited durable memory, fresh business projections, an Effect authority service,
and brokered access to existing OpenAgents capabilities, then the owner can run
the company through one mobile conversation without reviving a second app,
leaking credentials, or bypassing assurance and release controls.

## Scope

```productspec-scope
in:
  - one stable owner-private Sarah principal and conversation thread across devices and restarts
  - conversation history as durable cited memory plus refreshed bounded business context
  - current GitHub releases and open issues, Forum-ready context, Full Auto, FleetRun, cloud-health, product-contract, and company-priority projections
  - Effect Schema principal, context, capability, authority-decision, and receipt contracts
  - Sarah pinned inside the existing Effect Native OpenAgents mobile conversation UI
  - hosted Khala inference with Sarah's cited system context
  - brokered repository, Google Cloud, RC release, GitHub, and Forum operations under exact authority
out:
  - a separate Sarah app, public web landing, prospect CRM, transcript database, provider dispatcher, or issue queue
  - avatar, video, ambient voice, GPU rendering, or public sales-persona infrastructure
  - raw database dumps, raw credentials, private paths, customer-private payloads, or unbounded context in model prompts
cut:
  - financial custody or value movement
  - legal, employment, tax, regulatory, or natural-person commitments
  - destructive production customer-data operations
  - stable publication without current owner direction
  - invariant weakening, unsupported public claims, or self-amplification
```

## User Experience

After signing into OpenAgents mobile, the owner lands on Sarah unless an
explicit restored coding session or active Full Auto run takes precedence.
Sarah is pinned in the existing workspace drawer and uses the ordinary typed
composer/transcript. The header says `Sarah — Owner orchestrator` with the
effective authority revision. The owner can ask “what is the latest release?”,
“what is blocking Full Auto?”, “who is saying what?”, or “what should we do
next?” Sarah answers from bounded source refs, marks stale/unavailable state,
and proposes or delegates the narrow next action. She never claims an action
ran until a target receipt exists.

## Acceptance Criteria

- **SARAH-AC-01:** One authenticated owner maps deterministically to one opaque
  Sarah thread; another owner cannot observe or mutate it.
- **SARAH-AC-02:** The same thread and history survive app restart, device
  change, and repeated bootstrap without duplicate identities or conversations.
- **SARAH-AC-03:** Mobile pins Sarah inside the existing conversation UI and
  sends Sarah messages through the hosted Khala runtime; public `/sarah` stays
  404 and no second persona state machine returns.
- **SARAH-AC-04:** Current business claims cite exact bounded sources with
  freshness and owner/private classification; missing sources fail soft and
  remain explicit.
- **SARAH-AC-05:** Model context contains no raw tokens, credentials,
  mnemonics, private filesystem paths, customer-private payloads, or unbounded
  database/tool output.
- **SARAH-AC-06:** Sarah's effective authority is the intersection of the root
  profile, Sarah profile, active program, target policy, and exact capability;
  explicit deny wins and self-amplification is impossible.
- **SARAH-AC-07:** Visibility never implies mutation. Repository, GCP, release,
  GitHub, Forum, and Full Auto actions enter their existing typed adapters and
  emit bounded authority plus target receipts.
- **SARAH-AC-08:** Financial custody, legal/employment, destructive customer
  data, stable release without direction, invariant weakening, unsupported
  claims, and secret export are refused.
- **SARAH-AC-09:** Sarah distinguishes observed fact, inference,
  recommendation, delegated action, succeeded action, refusal, and unavailable
  state in owner-visible language.
- **SARAH-AC-10:** Revocation or supersession stops new actions immediately;
  an in-flight action reaches only its safest bounded checkpoint.

## Success Metrics

```productspec-success-metrics
- id: owner_contact_continuity
  metric: owner_questions_answered_in_the_same_durable_sarah_thread_with_cited_current_context
  target: ">= 95%"
  window: rolling 30 days
  segment: authenticated owner mobile sessions
  source: private_thread_and_context_receipts
- id: orchestration_truth
  metric: unsupported_current_state_or_action_completion_claims
  target: "0"
  window: lifetime
  segment: Sarah owner interactions
  source: citation_and_authority_receipt_audit
- id: delegated_completion
  metric: admitted_sarah_delegations_reaching_terminal_receipted_outcome_without_repeated_owner_permission
  target: ">= 90%"
  window: rolling 30 days
  segment: non-reserved delegated actions
  source: authority_and_target_receipts
```

## Risks

- “Full knowledge” can become an unsafe database dump. Only purpose-built,
  bounded, owner-scoped projections enter context.
- A human name can obscure that Sarah is an AI. The system prompt and product
  copy identify her as AI and prohibit impersonation.
- One point of contact can become one point of failure. Her thread is durable,
  context sources fail independently, and every action stays in existing
  systems with its own receipts and rollback.
- Broad company decision authority can collapse separation of duties. Sarah
  may decide and delegate, but cannot self-verify assurance or self-release
  from her own evidence.

## Owner Gates

The current owner direction admits revision 1 and the mobile OTA. Only a
future expansion into reserved financial, legal/people, destructive-data,
human-identity, over-budget, or stable-release authority requires another
owner direction and profile revision.

## Receipts

- `openagents.authority_decision_receipt.v1` for each action or refusal.
- Stable-principal bootstrap and owner-scope test receipts.
- Cited-context source/freshness ledger without raw source secrets.
- Existing repository, GCP, Full Auto, release, GitHub, and Forum receipts by
  reference rather than duplication.
- Mobile OTA manifest/digest and production route smoke.

## Promise Links

The unreleased Episode 24X transcript is intent evidence, not a public promise.
This implementation makes no public `/sarah` or autonomous-company claim. Any
future public positioning must enter the product-promise registry and cite
production evidence from this contract.

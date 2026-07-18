---
spec_format_version: "0.1"
title: "Sarah Owner Orchestrator"
artifact_type: "prd"
spec_revision: 2
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
  openagents_authority: "AUTHORITY.md revision 4 + docs/authority/SARAH_AUTHORITY.md revision 2"
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
  - Gemma 4 buffered function calling for bounded owner-linked Codex worker dispatch/status and existing Full Auto run read/pause/resume/stop
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
  - remote Full Auto start, raw local workspace selection, MemoHarness private experience-bank reads, during-run adaptation, or harness-candidate promotion
```

## User Experience

After signing into OpenAgents mobile, the owner lands on Sarah unless an
explicit restored coding session or active Full Auto run takes precedence.
Sarah is pinned in the existing workspace drawer and uses the ordinary typed
composer/transcript. The conversation header says only `Sarah`; authority and
runtime details remain available through receipts and explicit status requests
instead of consuming the chat header. The owner can ask “what is the latest release?”,
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
- **SARAH-AC-11:** Gemma 4 function calls are decoded through the normalized
  inference contract, bounded to six tool rounds, and replay assistant calls
  plus tool results without exposing thought text, raw credentials, or
  unbounded output.
- **SARAH-AC-12:** Sarah can read owner-linked coding capacity and dispatch at
  most eight Codex workers through the existing Khala/Pylon broker. Every real
  dispatch pins the exact current public `OpenAgentsInc/openagents` commit and
  returns actual assignment refs; no linked capacity yields an honest blocker.
- **SARAH-AC-13:** Sarah can read the owner's public-safe Full Auto projection
  and dispatch only pause, resume, or stop for an exact existing run. The
  server result remains `pending` until Desktop applies or rejects it; pending
  is never described as an applied transition.
- **SARAH-AC-14:** Every tool call emits ordered private runtime activity and
  an exact Sarah authority receipt. The final assistant answer follows those
  events and distinguishes partial, pending, refused, failed, and completed
  target outcomes.
- **SARAH-AC-15:** Sarah receives no tool for remote Full Auto start, raw local
  workspace selection, MemoHarness private-bank retrieval, during-run harness
  adaptation, candidate self-promotion, AssuranceSpec admission, or authority
  expansion. The new FA-AC-69–76 lifecycle remains governed by its own
  unimplemented admission gates.

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

The current owner direction admits revision 2 and the server runtime deploy.
The mobile UI already consumes ordered runtime events, so no new mobile OTA is
required for this server-side tool rollout. Only a
future expansion into reserved financial, legal/people, destructive-data,
human-identity, over-budget, or stable-release authority requires another
owner direction and profile revision.

## Receipts

- `openagents.authority_decision_receipt.v1` for each action or refusal.
- Stable-principal bootstrap and owner-scope test receipts.
- Cited-context source/freshness ledger without raw source secrets.
- Existing repository, GCP, Full Auto, release, GitHub, and Forum receipts by
  reference rather than duplication.
- Production route, hosted-turn, tool-activity, authority-receipt, and target-
  broker smoke receipts. No mobile artifact receipt is required when the
  already-installed client consumes the unchanged runtime-event contract.

## Promise Links

The unreleased Episode 24X transcript is intent evidence, not a public promise.
This implementation makes no public `/sarah` or autonomous-company claim. Any
future public positioning must enter the product-promise registry and cite
production evidence from this contract.

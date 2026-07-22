---
spec_format_version: "0.1"
title: "Sarah Owner Orchestrator"
artifact_type: "prd"
spec_revision: 5
author: "OpenAgents"
created_at: "2026-07-18T00:00:00Z"
updated_at: "2026-07-22T00:00:00Z"
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
  openagents_authority: "AUTHORITY.md revision 6 + Sarah runtime authority revision 4"
  openagents_source: "docs/transcripts/24Xsarah.md"
  openagents_assurance_level: "owner-private cross-resource orchestration"
  openagents_managed_sandbox_epic: "9023"
  openagents_assurance_spec_status: "revision 3 exactly rebinds this rev-4 intent after SBX-00 authority admission; SARAH-AC-21 through SARAH-AC-23 have complete proposed proof design while SARAH-AC-01 through SARAH-AC-20 remain needs_design"
  openagents_revision_4_note: "Rev 4 adds the owner-directed managed-sandbox outcome without granting a generic cloud shell or pretending the broker is already live. Sarah may create, list, inspect, dispatch into, interrupt, stop, resume, and delete exact owner-scoped OpenAgents-managed GCP sandboxes only after the managed-sandbox ProductSpec, SBX-00 authority revision, target broker, and live GCP proof land. Every action binds program/work-unit/target/profile/TTL/budget/capability refs and returns authority plus target receipts. It remains distinct from remote Full Auto start, raw topology, broad cloud credentials, and generic container administration. Adds SARAH-AC-21 through SARAH-AC-23."
  openagents_revision_5_note: "Rev 5 records the Episode 260 company-command direction of 2026-07-22 (AUTHORITY.md revision 7 + Sarah authority revision 5). Sarah runs the company during the owner's parental leave and commands the coding fleet, Full Auto, releases across all channels, communications, the blog, and the documents. Stable release is admitted through the same release broker under an independent reviewer and the standing Episode 260 direction, never waiving verification, rollback, monotonic-update, or evidence gates. Web communications are admitted: blog and document drafts deliver through repository delivery now; outward timeline and animated-spoken publication refuse with a receipt until the owner supplies the animation and speech interfaces and the web-communications broker and guardrails are deployed and healthy. Sales operations are admitted as intent only and refuse with a receipt until a bounded sales broker and its guardrails are separately deployed. No customer-data, financial-custody, legal, secret, or authority-expansion reach is admitted. Adds SARAH-AC-24 through SARAH-AC-27."
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
  - after exact authority and broker admission, managed-sandbox create/list/inspect/dispatch/interrupt/stop/resume/delete for the authenticated owner's Google Cloud capacity through `specs/openagents/managed-agent-sandboxes.product-spec.md`
  - one released content-addressed six-dimension Sarah conversational harness bound immutably before each provider turn
  - owner-private terminal-thread experience compilation, bounded candidate production, held-out evaluation, and an independent Blueprint release gate for next-turn activation
  - stable release publication and promotion through the existing release broker under independent verification and the standing Episode 260 owner direction
  - blog, document, and Forum drafting now, with blog and document drafts delivered through repository delivery
  - a self-introduction as a repository-hosted communication
out:
  - a separate Sarah app, public web landing, prospect CRM, transcript database, provider dispatcher, or issue queue
  - avatar, video, ambient voice, GPU rendering, or public sales-persona infrastructure
  - raw database dumps, raw credentials, private paths, customer-private payloads, or unbounded context in model prompts
  - outward publication to the public timeline or animated-spoken publication before the owner supplies the animation and speech interfaces and the web-communications broker and guardrails are deployed and healthy
  - any sales action, customer-data access, or financial reach before a bounded sales broker and its guardrails are separately admitted and deployed
cut:
  - financial custody or value movement
  - legal, employment, tax, regulatory, or natural-person commitments
  - destructive production customer-data operations
  - stable publication without a current or standing owner direction, or without independent verification
  - invariant weakening, unsupported public claims, or self-amplification
  - remote Full Auto start, raw local workspace selection, raw gcloud or shell access, generic container administration, broad project/topology/credential access, Full Auto MemoHarness private-bank access or mutation, current-turn learning, during-turn adaptation, Sarah/optimizer candidate self-promotion, or assurance self-admission
```

## User Experience

After signing into OpenAgents mobile, the owner lands on Sarah unless an
explicit restored coding session or active Full Auto run takes precedence.
Sarah is pinned in the existing workspace drawer and uses the ordinary typed
composer/transcript. The conversation header says only `Sarah`. Authority and
runtime details remain available through receipts and explicit status requests
instead of consuming the chat header. The owner can ask “what is the latest release?”,
“what is blocking Full Auto?”, “who is saying what?”, or “what should we do
next?” Sarah answers from bounded source refs, marks stale/unavailable state,
and proposes or delegates the narrow next action. She never claims an action
ran until a target receipt exists.

## Acceptance Criteria

- **SARAH-AC-01:** One authenticated owner maps deterministically to one opaque
  Sarah thread. Another owner cannot observe or mutate it.
- **SARAH-AC-02:** The same thread and history survive app restart, device
  change, and repeated bootstrap without duplicate identities or conversations.
- **SARAH-AC-03:** Mobile pins Sarah inside the existing conversation UI and
  sends Sarah messages through the hosted Khala runtime. Public `/sarah` stays
  404 and no second persona state machine returns.
- **SARAH-AC-04:** Current business claims cite exact bounded sources with
  freshness and owner/private classification. Missing sources fail soft and
  remain explicit.
- **SARAH-AC-05:** Model context contains no raw tokens, credentials,
  mnemonics, private filesystem paths, customer-private payloads, or unbounded
  database/tool output.
- **SARAH-AC-06:** Sarah's effective authority is the intersection of the root
  profile, Sarah profile, active program, target policy, and exact capability.
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
- **SARAH-AC-10:** Revocation or supersession stops new actions immediately.
  an in-flight action reaches only its safest bounded checkpoint.
- **SARAH-AC-11:** Gemma 4 function calls are decoded through the normalized
  inference contract, bounded to six tool rounds, and replay assistant calls
  plus tool results without exposing thought text, raw credentials, or
  unbounded output.
- **SARAH-AC-12:** Sarah can read owner-linked coding capacity and dispatch at
  most eight Codex workers through the existing Khala/Pylon broker. Every real
  dispatch pins the exact current public `OpenAgentsInc/openagents` commit and
  returns actual assignment refs. No linked capacity yields an honest blocker.
- **SARAH-AC-13:** Sarah can read the owner's public-safe Full Auto projection
  and dispatch only pause, resume, or stop for an exact existing run. The
  server result remains `pending` until Desktop applies or rejects it. Pending
  is never described as an applied transition.
- **SARAH-AC-14:** Every tool call emits ordered private runtime activity and
  an exact Sarah authority receipt. The final assistant answer follows those
  events and distinguishes partial, pending, refused, failed, and completed
  target outcomes.
- **SARAH-AC-15:** Sarah receives no tool for remote Full Auto start, raw local
  workspace selection, Full Auto harness mutation, current-turn learning,
  candidate self-promotion, AssuranceSpec admission, or authority expansion.
  The broader FA-AC-69–76 Full Auto lifecycle remains governed by its own
  admission gates.
- **SARAH-AC-16:** Before provider inference, every Sarah turn resolves exactly
  one released content-addressed policy and durably binds its digest and six
  dimension refs. A review or activation during the turn cannot change that
  binding. A conflict fails closed.
- **SARAH-AC-17:** Sarah can request a review of only terminal turns from the
  authenticated owner's exact Sarah thread. The separate Effect compiler
  creates append-only owner-private experiences with source refs/digests and
  bounded outcome facts. The running turn, deleted rows, and other owners are
  ineligible.
- **SARAH-AC-18:** Harness optimization and evaluation are separate Gemma 4
  invocations over disjoint training and held-out experience snapshots. The
  candidate schema can alter only 1–8 bounded conversational instructions and
  a 40–240 word default ceiling. Six dimension identities and every authority-
  bearing field remain immutable and unexpressible.
- **SARAH-AC-19:** Sarah and the optimizer cannot evaluate, release, or activate
  their candidate. A separate Blueprint gate requires held-out quality and
  regression scores of at least 0.75, privacy and safety scores of at least
  0.90, exact dimension compatibility, and deterministic secret/provenance
  fencing before compare-and-swap activation. A concurrent base change fails
  closed.
- **SARAH-AC-20:** Harness bank rows, optimizer/evaluator prompts, raw thread
  content, and private scores have no public or mobile projection. A released
  improvement affects only subsequent ordinary Sarah replies and exposes only
  bounded private activity/receipt refs when the owner explicitly asks.
- **SARAH-AC-21:** After SBX-00 admits the exact Sarah authority and managed-
  sandbox broker, Sarah can create, list, inspect, stop, resume, and delete
  only the authenticated owner's OpenAgents-managed sandboxes. Every request
  binds exact program/work-unit/target/image-profile/TTL/budget/capability and
  idempotency refs, and every actual outcome is supported by both an authority
  receipt and the sandbox lifecycle receipt.
- **SARAH-AC-22:** Sarah can dispatch one bounded long-running work unit into
  an exact ready owner sandbox, follow ordered structural runtime activity,
  and interrupt that exact turn. Quiet output is never called idle or
  completed, and a model response, SDK status, pending operation, or sandbox
  state cannot substitute for the native terminal and cleanup receipts.
- **SARAH-AC-23:** Sarah receives no raw `gcloud`, shell, database, topology,
  guest-address, service-account, provider-credential, filesystem-path, or
  generic container-admin tool. Budget, capacity, authority, broker, guest,
  revoke, and cleanup failures remain explicit. A failed or recovery-required
  teardown is never described as successful. This capability does not grant
  remote Full Auto start or cross-machine `FullAutoRun` admission.
- **SARAH-AC-24:** Sarah can publish or promote a stable release only through
  the existing release broker, only after an independent reviewer with a
  distinct execution identity reproduces the release evidence, and only under
  the standing Episode 260 owner direction. The producer cannot verify or
  release from its own evidence. Every publication records the exact trigger,
  release actor, authority revision and grant, source revision, distribution
  boundary, and a receipt. Rollback, monotonic-update, and evidence gates are
  never waived. Absent a current or standing owner direction, stable
  publication refuses.
- **SARAH-AC-25:** Sarah can draft blog, document, and Forum content and
  deliver blog and document drafts through repository delivery under normal
  review. Drafted content carries no raw secrets, private paths, or
  customer-private payloads, states only what current bounded evidence
  supports, and marks stale or unavailable state honestly.
- **SARAH-AC-26:** Outward publication to the public timeline and
  animated-spoken publication refuse with a receipt until the owner supplies
  the animation and speech interfaces and the web-communications broker and
  channel guardrails are deployed, healthy, and receipt-capable. Broker text or
  a provider object never substitutes for runtime availability.
- **SARAH-AC-27:** Any sales action, customer-data access, or financial reach
  refuses with a receipt until a bounded sales broker, its customer-data
  boundary, and its financial reserve rules are separately admitted and
  deployed. The company-command scope grants no customer-data or financial
  reach on its own.

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
- id: managed_sandbox_orchestration_truth
  metric: sarah_managed_sandbox_actions_with_exact_authority_target_and_cleanup_outcomes
  target: "100%; zero generic-cloud actions and zero false completion"
  window: every managed-sandbox release candidate and rolling 30-day owner dogfood
  segment: authenticated owner Sarah thread
  source: sarah_authority_and_managed_sandbox_receipts
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
- Self-improvement can become evaluation leakage or authority amplification.
  Terminal-only snapshots, disjoint held-out turns, a non-authority candidate
  schema, a distinct evaluator, and a compare-and-swap release gate keep the
  producer out of verification and activation.
- A managed-sandbox tool can become generic cloud-admin authority. The broker
  is a closed lifecycle and work-unit API with exact budgets and capability
  refs. Raw cloud, shell, database, topology, and credential surfaces remain
  unrepresentable.

## Related Artifacts

- Managed sandbox intent and issue program:
  `specs/openagents/managed-agent-sandboxes.product-spec.md`,
  `docs/sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md`, and epic
  [#9023](https://github.com/OpenAgentsInc/openagents/issues/9023).
- Source architecture evidence:
  `docs/teardowns/2026-07-19-ascii-box-optibox-openagents-gcp-analysis.md`.

## Owner Gates

The current owner direction admits revision 4 as product and roadmap intent,
alongside the already-live pre-managed-sandbox Sarah runtime, TestFlight build 119, and server
deploy. Sarah may request her own bounded terminal-history
review, but the independent evaluator and Blueprint gate own release and
activation. SBX-00 admits the managed-sandbox contract and exact root/Sarah
action vocabulary. Managed-sandbox mutation stays unavailable until the exact broker lands and SBX-09
records live GCP and cleanup proof. Only a
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
  broker smoke receipts.
- TestFlight build 119 Apple processing receipt plus source, archive,
  fingerprint, and IPA digest evidence.
- Sarah harness bundle, terminal-experience snapshot, held-out evaluation,
  Blueprint release, activation, and next-turn binding receipts.
- Managed-sandbox authority, lifecycle, runtime-turn, event, budget/cost,
  interrupt, stop/resume/delete, and zero-residue cleanup receipts.

## Promise Links

The unreleased Episode 24X transcript is intent evidence, not a public promise.
This implementation makes no public `/sarah` or autonomous-company claim. Any
future public positioning must enter the product-promise registry and cite
production evidence from this contract.

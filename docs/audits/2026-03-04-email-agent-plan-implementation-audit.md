# 2026-03-04 Email Automation Plan Implementation Audit

Date: 2026-03-04  
Author: Codex  
Status: Complete (repository audit pass)

## Scope

- Plan source reviewed: external `dump.md` source document provided for this audit.
- This report intentionally omits client identifiers and business-specific details from that source.
- Repository audited: `openagents` (pruned MVP working set).
- Product/scope authorities used during audit:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`

## Normalized Plan Requirements

The source plan normalizes to the following implementation requirements:

1. Automate repetitive inbound/outbound communication workflows.
2. Connect Google Gmail accounts.
3. Ingest and analyze a large historical communications corpus (quoted as ~10 GB) for response learning.
4. Generate response drafts for incoming messages, grounded in organization knowledge.
5. Support follow-up email workflows and optional programmatic send paths.
6. Preserve human-like response style quality.
7. Support isolated, non-public deployment boundaries for customer workloads.
8. Create a repeatable onboarding and delivery motion for similar deployments.

## Audit Method

- Read current MVP/spec boundaries in `docs/MVP.md` and ownership boundaries in `docs/OWNERSHIP.md`.
- Inspected active app/state/surface code in `apps/autopilot-desktop`.
- Ran targeted repository scans for Gmail/email integration signals and related dependencies.
- Cross-checked reusable foundations (chat lane, credentials, MCP/OAuth, skill system).

Targeted scan snapshots (captured 2026-03-04):

```text
gmail_refs=0
smtp_refs=0
imap_refs=0
email_agent_refs=0
google_oauth_credential_templates=0
skill_paths_with_email=0
skill_markdown_mentions_email=0
```

Filesystem snapshot of retained app surfaces:

```text
apps/autopilot-desktop
```

## Executive Assessment

The requested email-automation plan is **not implemented** in the current retained codebase.

What exists today is a **general Autopilot desktop foundation** (chat, command/pane system, generic MCP OAuth surface, credentials storage, skill discovery) that can be used as a base. The plan’s email-specific capabilities (Gmail integration, corpus ingestion/learning, drafting/sending workflows, quality controls, and isolated per-customer deployment lane) are absent.

## Requirement Status Matrix

| Requirement | Status | Evidence | Notes |
| --- | --- | --- | --- |
| 1. Repetitive communications automation | Partial foundation only | `apps/autopilot-desktop/src/pane_registry.rs:95`, `apps/autopilot-desktop/src/input/actions.rs:3` | A local Autopilot chat/execution lane exists, but no communication-domain workflow implementation. |
| 2. Gmail account connection | Not implemented | `apps/autopilot-desktop/src/credentials.rs:24`, scan snapshot above | No Gmail/Google credential templates or Gmail connector implementation in retained code. |
| 3. Historical corpus ingestion (~10 GB) | Not implemented | `apps/autopilot-desktop/src/skills_registry.rs:30`, scan snapshot above | No ingestion/indexing pipeline for email/chat history; skill discovery exists but is not corpus ingestion. |
| 4. Draft generation from inbound messages + knowledge context | Not implemented | `apps/autopilot-desktop/src/app_state.rs:54`, `docs/PANES.md:7` | No email inbox/draft pane models; existing inbox/history surfaces are NIP-90/provider job surfaces, not email. |
| 5. Follow-up workflows and programmatic sends | Not implemented | scan snapshot above, Cargo scan for mail deps returned none | No SMTP/IMAP/mail transport or send orchestration in code/deps. |
| 6. Human-style quality controls for responses | Not implemented | `apps/autopilot-desktop/src/input/actions.rs:3` | General chat exists; no style-learning/evaluation lane tied to communication outputs. |
| 7. Isolated non-public deployment boundaries | Not implemented in retained repo | filesystem snapshot above; `docs/MVP.md:109` | Current retained repo is desktop-first MVP and does not include a customer-isolated email service control plane. |
| 8. Repeatable onboarding motion for this plan type | Not implemented | `apps/autopilot-desktop/src/app_state.rs:1869`, `docs/PANES.md:58` | Generic settings/credentials UX exists, but no onboarding workflow for email-agent deployments. |

## What Is Implemented and Reusable

1. Desktop agent shell and local task lane.
- `Autopilot Chat` surface is implemented as a first-class pane and command target.
- Evidence: `apps/autopilot-desktop/src/pane_registry.rs:95`, `apps/autopilot-desktop/src/pane_registry.rs:102`, `apps/autopilot-desktop/src/input/actions.rs:131`.

2. Generic MCP/OAuth integration surface.
- MCP server list and OAuth lifecycle notifications are integrated in desktop state reducers.
- Evidence: `apps/autopilot-desktop/src/pane_registry.rs:160`, `apps/autopilot-desktop/src/input/reducers/codex.rs:584`, `apps/autopilot-desktop/src/input/reducers/codex.rs:617`.

3. Credentials storage and scoped environment injection.
- Credential repository + scoped templates + secure read/write path exist.
- Evidence: `apps/autopilot-desktop/src/credentials.rs:24`, `apps/autopilot-desktop/src/credentials.rs:98`, `apps/autopilot-desktop/src/credentials.rs:228`.

4. Skill discovery/registry scaffolding.
- Local skill discovery and manifest derivation are implemented.
- Evidence: `apps/autopilot-desktop/src/skills_registry.rs:30`, `apps/autopilot-desktop/src/skills_registry.rs:34`, `apps/autopilot-desktop/src/skills_registry.rs:139`.

## Not Implemented (Direct Gaps)

1. Gmail connector implementation.
- No Gmail API integration, OAuth token management for Gmail, mailbox sync worker, or Gmail schema domain models.

2. Communication corpus ingestion + retrieval.
- No ETL/indexing/chunking/retrieval pipeline for historical communication data.

3. Email-domain workflow surfaces.
- No panes or domain state for message queues, draft queues, approval queues, send logs, or deliverability telemetry.

4. Draft-generation application logic.
- No deterministic pipeline that maps inbound message + retrieved context -> draft artifact with traceability.

5. Follow-up/send automation.
- No scheduled follow-up engine, send policy controls, or send execution transport.

6. Human-style quality controls.
- No measurable quality rubric, regression tests, or escalation workflow for communication quality fidelity.

7. Isolated per-customer infrastructure lane in retained repo.
- No retained deployment templates/workflows dedicated to this communication-agent architecture.

## MVP Context Alignment Note

Current repo authority (`docs/MVP.md`) is centered on desktop Autopilot + NIP-90 provider/wallet loop and deterministic sync continuity, not communication-agent productization.

- Desktop Autopilot + compute-provider scope: `docs/MVP.md:109`, `docs/MVP.md:110`, `docs/MVP.md:147`.
- This explains why reusable foundations exist while the email-agent plan remains unimplemented.

## Overall Completion Estimate for This Plan

- Implemented directly: ~0-10% (no email-specific core lanes).
- Foundation that can accelerate build-out: ~25-35% (chat shell, OAuth-capable MCP surface, credentials, skill scaffolding).
- Remaining work: ~65-75% (all communication-domain, data, workflow, quality, and deployment lanes).

## Suggested Build Sequence (Execution-Oriented)

1. Define a dedicated communication-domain crate and desktop pane set (inbox, drafts, approvals, send log).
2. Implement Gmail connector + OAuth token lifecycle and mailbox sync workers.
3. Implement corpus ingestion/indexing + retrieval pipeline with deterministic audit metadata.
4. Implement draft-generation pipeline with explicit prompt/context trace and approval workflow.
5. Add send/follow-up engine with policy guardrails and full outcome telemetry.
6. Add quality evaluation harness for style fidelity and safety regression.
7. Add deployment isolation workflow for customer-specific runtime boundaries.

## GitHub Issues Required for Full Implementation

The following issue set is the minimum end-to-end backlog required to fully implement the audited plan.

### 1. `[email-agent] Define architecture, scope, and contracts`
Description:
- Establish the target system design, crate/app boundaries, runtime topology, and API contracts before implementation.

Supporting details:
- Align with `docs/OWNERSHIP.md` so product workflows remain in `apps/autopilot-desktop` and reusable primitives live in crates.
- Define event and state contracts for: mailbox item, draft artifact, approval decision, send attempt, and follow-up job.
- Produce one source-of-truth design doc in `docs/` and list non-goals.

Done when:
- Approved architecture doc is merged.
- All downstream issues reference stable contract/types.

### 2. `[email-agent] Add Gmail OAuth credentials and secure token lifecycle`
Description:
- Add Gmail/Google OAuth credential templates and secure local storage/refresh handling.

Supporting details:
- Extend credential templates in `apps/autopilot-desktop/src/credentials.rs`.
- Support client id/secret + access/refresh token flow with explicit scopes and expiration handling.
- Include rotation and revoke paths; never log secrets.

Done when:
- OAuth credentials can be stored, loaded, refreshed, and revoked from desktop flows.

### 3. `[email-agent] Implement Gmail mailbox connector (initial backfill)`
Description:
- Build Gmail API connector and first-pass mailbox importer for historical message ingestion.

Supporting details:
- Add typed Gmail integration layer (API calls, paging, retry/backoff, rate-limit handling).
- Import message metadata, thread relationships, body excerpts/full text, and timestamps.
- Make backfill resumable with checkpoints.

Done when:
- A mailbox can be connected and backfilled deterministically without duplicate records.

### 4. `[email-agent] Implement incremental mailbox sync and change tracking`
Description:
- Keep mailbox state up to date after backfill.

Supporting details:
- Use Gmail history/watch semantics to fetch deltas.
- Handle missed cursors, rebootstrap, and duplicate suppression.
- Emit deterministic sync events to activity/audit surfaces.

Done when:
- New inbound/outbound mailbox changes are reflected in local state with replay-safe behavior.

### 5. `[email-agent] Build communication corpus ingestion + normalization pipeline`
Description:
- Transform raw message data into normalized records for retrieval and modeling.

Supporting details:
- Normalize subjects, threads, participants, quoted content, signatures, timestamps, and labels.
- Add PII-safe handling rules and truncation strategies.
- Store immutable source pointers for traceability.

Done when:
- Raw imported messages become normalized corpus artifacts with deterministic IDs.

### 6. `[email-agent] Build retrieval index for historical communication context`
Description:
- Implement retrieval layer used during draft generation.

Supporting details:
- Support semantic + lexical retrieval by thread/contact/topic/time.
- Include relevance scoring and context window controls.
- Expose query API returning snippets plus source references.

Done when:
- Draft pipeline can fetch top-k relevant historical examples with source trace.

### 7. `[email-agent] Implement style profile learning from historical responses`
Description:
- Derive a controllable response-style profile from historical sent messages.

Supporting details:
- Extract tone, brevity, structure, greeting/sign-off norms, and escalation patterns.
- Store versioned style profile artifacts.
- Allow profile refresh/retrain triggers.

Done when:
- Draft pipeline can select and apply a concrete style profile version.

### 8. `[email-agent] Implement knowledge-base ingestion and grounding`
Description:
- Add knowledge sources used to ground draft content beyond prior email examples.

Supporting details:
- Support file/document ingestion with chunking and retrieval tags.
- Maintain source provenance for each grounded claim/snippet.
- Enforce confidence thresholds and fallback behavior.

Done when:
- Drafts can cite grounded knowledge sources and link them in audit metadata.

### 9. `[email-agent] Implement draft generation pipeline (inbound -> candidate draft)`
Description:
- Build the core deterministic draft-generation workflow.

Supporting details:
- Inputs: inbound message + retrieved context + style profile + policy constraints.
- Outputs: draft text + explanation metadata + source pointers + confidence score.
- Record full decision trace for each generated draft.

Done when:
- Connected inbound messages produce reproducible candidate drafts with traceability.

### 10. `[email-agent] Add desktop panes for inbox, draft queue, and send log`
Description:
- Introduce first-class UI surfaces for communication operations.

Supporting details:
- Add panes and commands for: Inbox, Draft Queue, Approval Queue, Send Log, Follow-up Queue.
- Show per-item status machines with explicit failure states.
- Keep pane behavior deterministic and replay-safe.

Done when:
- Operators can review inbound items, inspect drafts, approve/reject, and view send outcomes in UI.

### 11. `[email-agent] Add approval workflow and manual override controls`
Description:
- Require explicit approval controls before sends, with optional policy-based auto-approve modes.

Supporting details:
- Include per-draft approve/reject/edit actions.
- Record decision actor/time/reason.
- Support emergency kill switch and queue pause/resume.

Done when:
- No draft is sent without an auditable policy path or operator decision.

### 12. `[email-agent] Implement send execution with idempotency and retries`
Description:
- Send approved drafts through Gmail with safe retry behavior.

Supporting details:
- Guarantee idempotent send semantics (dedupe keys, request fingerprints).
- Classify transient vs permanent failures and retry accordingly.
- Persist provider message IDs and final delivery state.

Done when:
- Approved drafts are sent exactly once (or deterministically failed) with full audit records.

### 13. `[email-agent] Implement follow-up scheduler and policy engine`
Description:
- Automate follow-up creation based on explicit rules and outcomes.

Supporting details:
- Rule examples: no reply after N days, unanswered critical thread, reminder cadence.
- Include quiet hours/business-hour constraints and per-recipient limits.
- Emit upcoming and executed follow-up events.

Done when:
- Follow-up jobs are scheduled and executed according to visible policy rules.

### 14. `[email-agent] Add quality scoring and regression test harness`
Description:
- Build quality measurement for “human-like” output and prevent regressions.

Supporting details:
- Define rubric: tone match, factual grounding, clarity, actionability, and safety.
- Add golden-set evaluation corpus and score thresholds.
- Gate releases on minimum quality scores.

Done when:
- CI/validation can fail builds when response quality drops below agreed thresholds.

### 15. `[email-agent] Add observability and audit trail`
Description:
- Make the full pipeline inspectable end to end.

Supporting details:
- Track lifecycle events: ingest, retrieve, draft, approve, send, follow-up.
- Add correlation IDs across all stages.
- Expose diagnostics in UI and logs without leaking secrets.

Done when:
- Every sent message can be traced back to inputs, decisions, and generation context.

### 16. `[email-agent] Implement tenant-isolated deployment lane`
Description:
- Add isolated deployment topology for customer-specific runtimes.

Supporting details:
- Separate config, storage, credentials, and runtime identities per tenant.
- Define network/security boundaries and secret scopes.
- Document provisioning, rotation, and teardown.

Done when:
- New tenant environments can be provisioned and operated with hard isolation guarantees.

### 17. `[email-agent] Add security/privacy controls and retention policies`
Description:
- Implement operational controls for data minimization and lifecycle management.

Supporting details:
- Add retention windows, deletion workflows, export controls, and access auditing.
- Define redaction policy for logs and debug traces.
- Add compliance-ready operational runbook.

Done when:
- Data lifecycle and privacy controls are enforceable and test-covered.

### 18. `[email-agent] Build end-to-end test harness and release gates`
Description:
- Add deterministic integration tests that cover the entire communication loop.

Supporting details:
- Scenario: connect mailbox -> backfill -> receive new message -> draft -> approve -> send -> follow-up.
- Include failure-injection cases (token expiry, API rate limit, send failure, sync cursor stale).
- Add lint/test gate script for this lane similar to existing repo gates.

Done when:
- A single command can validate full pipeline behavior before release.

### 19. `[email-agent] Create onboarding/runbook and operator playbooks`
Description:
- Document repeatable onboarding and day-2 operations for this product lane.

Supporting details:
- Include setup checklist, credential setup, sync verification, quality checks, and incident runbooks.
- Add troubleshooting matrix for common failures.
- Define SLA/SLO metrics and escalation paths.

Done when:
- New deployments can be brought online and operated from docs without tribal knowledge.

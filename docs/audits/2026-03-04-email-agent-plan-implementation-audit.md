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

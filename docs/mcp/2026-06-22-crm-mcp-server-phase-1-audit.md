# CRM as the First OpenAgents MCP Server — Status + Build Audit

**Date:** 2026-06-22
**Author:** Raynor (agent)
**Trigger:** Owner wants the **next phase of MCP work** to expose our **CRM** as the **first OpenAgents function served over MCP** — so coding/operator agents (Claude Code, Codex, ChatGPT, our own Autopilot) can read and steer the CRM through a typed, policy-bound MCP server.

> **Decision (re-sequencing):** The existing roadmap (`2026-06-21-openagents-overarching-mcp-roadmap.md`) leads Phase 1 with a *local Pylon stdio* server and puts the **Worker MCP facade at Surface D / Phase 5**. This audit **re-sequences the Worker facade to first**, scoped to **CRM only**, because the CRM is the cleanest, highest-value, already-shipped, bounded surface to prove the whole MCP authority model end-to-end. Nothing about the principles changes — CRM MCP is a *projection* of the CRM HTTP routes that already enforce auth, tenant scope, suppression, dry-run, and approval. Pylon stdio remains the next server after this.

---

## 0. TL;DR

- **Phase 0 is done.** `@openagentsinc/mcp-contract` ships the full contract: authority classes, tool/resource/prompt descriptors, JSON-RPC envelopes, tagged errors + HTTP-status map, receipts/progress/elicitation, output-safety/redaction, transport + lifecycle schemas, naming + URI rules. The Worker already imports it (`workers/api/src/mcp-contract-import.ts`, declared intent: `operator_read`, output safety `operator`, reserved transport `streamable_http`).
- **No live MCP transport exists anywhere yet.** The Pylon `tas/mcp-server.ts` + `mcp-client.ts` are a pure protocol core (registry + envelope builders), not a server process.
- **The CRM is fully built and merged** (epic #5980, PR #5989): 23 `/api/operator/crm/*` endpoints (13 read, 10 mutating), all admin-gated, tenant-scoped, with a shared suppression gate, dry-run-by-default batch, and an approval-gated `send_email` command. This is an ideal first MCP surface.
- **What's needed:** a stateless **Streamable-HTTP JSON-RPC handler** (`POST /api/mcp`) in the existing Effect omni-route cascade that (1) speaks `initialize`/`tools/list`/`tools/call`/`resources/list`/`resources/read`, (2) registers a **CRM tool catalog** built from `@openagentsinc/mcp-contract` descriptors, (3) dispatches each tool to the existing CRM store/route functions, (4) projects every result through the contract's output-safety helpers, and (5) starts **read-only** then adds **propose/approve** writes — never bypassing the CRM's existing gates.
- **Agent-initiated sends go through the approval-gated command** (#5986): an MCP agent **proposes** a send; a human **approves**. MCP never gets a direct un-gated "blast" button.

---

## 1. Current MCP status (review)

### 1.1 Phase 0 contract — complete
`packages/mcp-contract` (`@openagentsinc/mcp-contract`) exports everything a server needs:

- **Authority classes** `OpenAgentsMcpAuthorityClass`: `public_read`, `operator_read`, `private_account_read`, `workspace_read`, `workspace_write`, `local_node_control`, `coding_session_control`, `approval_resolution`, `payment_read`, `payment_receive`, `payment_spend`, `deployment`, `admin`. High-risk subset + `isOpenAgentsMcpHighRiskAuthority`.
- **Descriptors**: `OpenAgentsMcpToolDescriptor` (name, title, description, `requiredAuthorities`, `riskClass`, `inputSchemaRef`, `outputSchemaRef`, `receiptBehavior`, `progressBehavior`, `publicSummary`, `sourceRefs`); `OpenAgentsMcpResourceDescriptor` (uri, namespace incl. `worker`, staleness, `publicProjectionSafe`); `OpenAgentsMcpPromptDescriptor`.
- **Errors**: tags `denied|missing_grant|needs_auth|blocked_by_policy|validation_failed|transport_failed|target_unavailable|unsafe_output_omitted` + an HTTP-status map (403/401/423/400/503/206).
- **Receipts / progress / elicitation** schemas.
- **Output safety**: `detectOpenAgentsMcpUnsafeMaterial`, `redactOpenAgentsMcpUnsafeText`, `projectOpenAgentsMcpOutput` (redact + truncate + persistence policy); safety classes `public|operator|private_account|local_only|workspace_private|secret_bearing|omitted`.
- **Transport**: kinds incl. `streamable_http`; `OpenAgentsMcpServerConfig`, lifecycle statuses.
- **Naming**: `assertValidOpenAgentsMcpName` (`^[a-z][a-z0-9]*(\.[a-z][a-z0-9]*)+$`); `parseOpenAgentsMcpResourceUri` (`mcp://openagents/{namespace}/{path}`).

### 1.2 Pylon protocol core — reusable, not a server
- `apps/pylon/src/tas/mcp-server.ts`: `createMcpToolRegistry`, `registerTool`, `handleToolsList`, `dispatchToolCall` (name → route descriptor, or `unknown_tool`).
- `apps/pylon/src/tas/mcp-client.ts`: `buildToolsListRequest`, `buildToolCallRequest`, `parseToolResult`, `isResponseForRequest`, `buildRequestId`; envelope types for `initialize`/`tools/list`/`tools/call`.
- Reusable: the registry + envelope shapes. Pylon-specific: its `handlerKind` set. The CRM server will use the same envelope/registry idea but its own tool catalog + dispatch.

### 1.3 Worker — where MCP mounts
- `routeOmniRequest` is a stateless Effect cascade of `(request, env, ctx) => Effect.Effect<Response> | undefined` handlers (the 7 CRM handlers are already chained here). A new `mcpRoutes.routeMcpRequest` slots in identically.
- Auth today: `requireAdminApiToken` (Bearer, timing-safe, from central config). **No agent-bearer / scoped MCP token yet** (agent tokens `oa_agent_` exist for sessions but aren't wired to an authority-grant model).
- Discovery surfaces exist (`/api/openapi.json`, `/.well-known/openagents.json`) but CRM routes aren't in them and there's **no `/api/mcp` or `/.well-known/openagents-mcp.json`** yet.
- **No Cloudflare Agents `McpAgent`/Durable Object** in the repo. The Worker is custom Effect. → use a **stateless Streamable-HTTP JSON-RPC handler**, not a DO.

### 1.4 The CRM surface (the thing we're exposing)
23 endpoints under `/api/operator/crm/*`, all admin-gated + tenant-scoped (`tenant` param, default `tenant.openagents`):

- **Read (13):** contacts list/get, contact activities, contact engagement, accounts list/get, lists, opportunities list/get, import-runs, templates list, contact emails, gmail-queue, commands list.
- **Mutating (10):** CSV import; template upsert; gmail-writeback; resend-send (armed-flag, dry-run if off); unified send; command propose/approve/reject; batch send (dry-run default).
- Already enforces: suppression/unsubscribe gate (`readEmailSendEligibility`), dry-run defaults, approval-gated command flow, ledger receipts. **MCP must reuse these, not re-implement them.**

---

## 2. Why CRM is the right first MCP server

1. **Bounded + shipped.** 23 endpoints, one Worker, one auth mode, one tenant model — small enough to do completely and correctly.
2. **High agent value.** "List contacts who opened but didn't reply", "draft a follow-up to this segment", "propose a send for approval" are exactly what coding/operator agents want — and what makes OpenAgents' CRM a *product* customers reach via MCP, not just a UI.
3. **Proves the whole authority model end-to-end** on a real surface: read tools (`operator_read`), propose (no send), approval-gated execute (`approval_resolution`), output redaction, receipts, tenant isolation — every contract concept exercised once.
4. **Safe by construction.** Every CRM mutation already has a gate; MCP becomes a thin, policy-respecting projection. Agent-initiated sends use the existing **propose → human approve** flow, so MCP never holds an un-gated send.
5. **Multi-tenant from day one.** The CRM is tenant-scoped, so a CRM MCP server is also the template for *customers* exposing their own CRM over MCP — same infra we use ourselves.

---

## 3. Target architecture

**A stateless Streamable-HTTP JSON-RPC MCP endpoint in the existing Worker, scoped to CRM.**

```
MCP client (Claude Code / Codex / ChatGPT / Autopilot)
        │  JSON-RPC over Streamable HTTP
        ▼
POST /api/mcp        ── mcpRoutes.routeMcpRequest (new, in the omni cascade)
        │  initialize | tools/list | tools/call | resources/list | resources/read
        ▼
CRM MCP tool catalog (crm-mcp.ts)  ── descriptors from @openagentsinc/mcp-contract
        │  grant filter (authority class ⊆ caller grant) → dispatch
        ▼
existing CRM store/route fns (crm-store / crm-email / crm-send / crm-command / crm-batch)
        │  (same suppression gate, dry-run, approval, tenant scope, ledger)
        ▼
projectOpenAgentsMcpOutput(result)  ── redact + truncate + safety class → JSON-RPC result
```

Design rules:
- **One new module pair:** `crm-mcp.ts` (tool catalog + dispatch over the CRM fns) + `crm-mcp-routes.ts` (the JSON-RPC transport handler). Mounted in `routeOmniRequest` next to the CRM routes.
- **Reuse, don't re-route:** each tool calls the same store/route function the HTTP endpoint calls. No new business logic, no new authority.
- **Auth, staged:** start with `requireAdminApiToken` (Bearer) → caller gets the full CRM grant for `tenant.openagents`. Then add a **scoped MCP token / agent-bearer** that carries an explicit authority-class grant + a bound `tenantRef`, so `tools/list` filters by grant and customers get tenant-isolated access. (This is the one genuinely new auth primitive.)
- **Tenant scoping:** the bound tenant comes from the token/grant, not client input; default `tenant.openagents` for the admin token.
- **Output safety:** every result through `projectOpenAgentsMcpOutput`; CRM data is `operator` safety class (emails/names are operator data, not public).
- **Errors:** map failures to the contract's tagged errors + HTTP-status map (e.g. suppression → `blocked_by_policy` 423; missing grant → `missing_grant` 403; bad args → `validation_failed` 400).
- **Schema gap to close:** descriptors carry `inputSchemaRef`/`outputSchemaRef` (string refs), but the MCP wire protocol's `tools/list` needs **actual JSON Schema** for each tool's `inputSchema`. So we add a small **CRM tool schema catalog** (JSON Schemas keyed by ref) and a resolver that emits them in `tools/list`. (Flagged as gap G4.)

Why not McpAgent/Durable Object: CRM MCP is request→response (list/get/dispatch). The Effect cascade is already the right tool; DOs are for stateful streaming sessions (a later Pylon/Verse concern).

---

## 4. The CRM MCP tool + resource map

Names follow the contract's dot rule. Authorities use real contract classes. Each tool wraps an existing CRM function — no new authority is created.

### Wave 1 — read-only (`operator_read`, `riskClass: read_only`, `receiptBehavior: noop`)
| MCP tool | Wraps | Notes |
|---|---|---|
| `crm.contacts.list` | `listCrmContacts` | `?search`, `?limit`, tenant from grant |
| `crm.contact.get` | `getCrmContactById` | 404 → `target_unavailable` |
| `crm.contact.activities.list` | `listCrmActivitiesForContact` | timeline |
| `crm.contact.engagement.get` | `getCrmEngagementSnapshot` | |
| `crm.contact.emails.list` | `listCrmEmailMessagesForContact` | send ledger |
| `crm.accounts.list` / `crm.account.get` | `listCrmAccounts` / `getCrmAccountById` | |
| `crm.lists.list` | `listCrmContactLists` | segments |
| `crm.opportunities.list` / `crm.opportunity.get` | `listCrmOpportunities` / `getCrmOpportunityById` | |
| `crm.import_runs.list` | `listCrmSourceImportRuns` | import audit |
| `crm.templates.list` | `listCrmEmailTemplates` | |
| `crm.contact.render` | `composeCrmEmailForContact` + `readEmailSendEligibility` | **read-only**: returns the personalized preview + eligibility; sends nothing |
| `crm.gmail_queue.list` | `listCrmQueuedGmailMessages` | queued local sends |
| `crm.commands.list` | `listCrmCommands` | approval queue |

**Resources** (`namespace: worker`, `publicProjectionSafe: false`, `operator` staleness):
`mcp://openagents/worker/crm/contacts`, `.../contact/{id}`, `.../contact/{id}/activities`, `.../accounts`, `.../lists`, `.../opportunities`, `.../import-runs`, `.../commands`.

### Wave 2 — propose (no send) (`operator_read`, `riskClass: low`, `receiptBehavior: mutation`)
| MCP tool | Wraps | Notes |
|---|---|---|
| `crm.send.command.propose` | `proposeCrmSendCommand` | agent proposes a `send_email{channel}`; records a `pending_approval` command. **Sends nothing.** This is the agent's send entry point. |
| `crm.template.upsert` | `upsertCrmEmailTemplate` | author/update a template |

### Wave 3 — gated execution (`approval_resolution` / `workspace_write`, `riskClass: medium`, `receiptBehavior: mutation/approval`)
| MCP tool | Wraps | Authority | Notes |
|---|---|---|---|
| `crm.send.command.approve` | `approveAndExecuteCrmSendCommand` | `approval_resolution` | human/operator-grant only; executes the unified send; suppression gate still held |
| `crm.send.command.reject` | `rejectCrmCommand` | `approval_resolution` | |
| `crm.import.run` | `importCrmContactsFromCsv` | `workspace_write` | CSV import; records an audited import-run |
| `crm.batch.send` (dry-run only over MCP) | `runCrmBatch` (`dryRun:true`) | `operator_read` | MCP gets the **plan** (would_send/suppressed/failed counts); a live batch (`dryRun:false`) stays operator-only, not in the default MCP grant |

### Deliberately NOT exposed in the first CRM MCP server
- Direct un-gated `crm.send.dispatch` (the raw unified `POST .../send`) and live `crm.batch.send`: agents must go through **propose → approve**. Live blast authority is operator-only.
- Anything touching wallet/payment/deploy/admin (not CRM).

---

## 5. Gaps — what's needed to ship a CRM MCP server

| # | Gap | Severity |
|---|---|---|
| G1 | No JSON-RPC transport handler in the Worker (`initialize`/`tools/list`/`tools/call`/`resources/list`/`resources/read` over Streamable HTTP at `POST /api/mcp`) | **High** (blocks everything) |
| G2 | No CRM tool catalog: descriptors (from `@openagentsinc/mcp-contract`) + dispatch table mapping each tool → CRM fn | **High** |
| G3 | No grant model for MCP: today only the admin token; need a scoped MCP token (or agent-bearer) carrying authority classes + a bound `tenantRef`, and a `tools/list` filter that hides ungranted tools | **High** for non-admin / customer use |
| G4 | Descriptor schema refs ≠ wire JSON Schema: need a CRM tool input/output **JSON Schema catalog** + resolver so `tools/list` emits real `inputSchema` | **Medium** |
| G5 | No discovery doc (`/.well-known/openagents-mcp.json`) advertising the server, transport, version, and public-safe tool refs | **Medium** |
| G6 | Output projection not yet applied at an MCP boundary (the helper exists; nothing calls it for MCP results) | **Medium** (safety) |
| G7 | No receipts emitted in MCP envelopes for mutations (CRM has ledger rows; need to surface `OpenAgentsMcpReceipt` refs in tool results) | **Medium** |
| G8 | No MCP client smoke (a real client — MCP Inspector / Claude Code / Codex — initializes, lists granted tools, calls `crm.contacts.list`, proposes a send, and cannot call an ungranted/un-gated send) | **High** (proof) |
| G9 | Web MCP projection lanes + `docs/mcp/README.md` don't yet show a live CRM MCP server ref | Low |

---

## 6. Proposed next-phase issue ladder (CRM MCP epic)

Sequential, each landing with tests + `check:deploy` green, on a worktree branch, merged at epic end (same flow as the CRM epic).

1. **Epic: CRM as the first OpenAgents MCP server** (umbrella).
2. **MCP JSON-RPC transport handler** — `crm-mcp-routes.ts`: `POST /api/mcp` with `initialize` (protocolVersion + capabilities + serverInfo), method dispatch, tagged-error → HTTP-status mapping, admin-token auth first. Mounted in `routeOmniRequest`. (G1)
3. **CRM tool catalog + dispatch (Wave 1, read-only)** — `crm-mcp.ts`: descriptors from `@openagentsinc/mcp-contract` for the 13 read tools, the input/output JSON Schema catalog (G4), and dispatch to the CRM store fns; `tools/list`/`tools/call` return real, schema-bound, output-projected results. (G2, G4, G6)
4. **CRM resources** — `resources/list` + `resources/read` for the `mcp://openagents/worker/crm/*` URIs, reusing the same reads + output safety. (G2, G6)
5. **Scoped MCP grant + tenant binding** — a scoped MCP token (or agent-bearer) carrying an authority-class set + bound `tenantRef`; `tools/list` filters by grant; ungranted tools are **absent**, not disabled; admin token = full grant on the default tenant. (G3)
6. **Wave 2 propose + template tools** — `crm.send.command.propose`, `crm.template.upsert`, with `OpenAgentsMcpReceipt` refs in results. (G7)
7. **Wave 3 gated execution tools** — `crm.send.command.approve`/`reject` (`approval_resolution`), `crm.import.run` (`workspace_write`), `crm.batch.send` dry-run-only; prove suppression/approval gates hold through MCP. (G7)
8. **Discovery + projection** — `/.well-known/openagents-mcp.json`, update the web MCP server-export/capability-catalog lanes to show the live CRM MCP ref, update `docs/mcp/README.md`. (G5, G9)
9. **MCP client compatibility smoke** — a real-client smoke (MCP Inspector + at least one of Claude Code/Codex): initialize, list granted tools, `crm.contacts.list`, propose a send, confirm an ungranted/un-gated send is absent + a suppressed address is blocked. (G8)

(Pylon stdio server — the previously-planned Phase 1 — becomes the **next** MCP server after this CRM epic, reusing the same transport + grant patterns proven here.)

---

## 7. Test plan

**Unit**
- descriptor generation for every CRM tool; names pass `assertValidOpenAgentsMcpName`.
- `tools/list` returns only granted tools; ungranted absent.
- read-only tools never call a mutating CRM fn.
- invalid args rejected (`validation_failed`) before dispatch.
- `projectOpenAgentsMcpOutput` applied: no raw tokens/paths; operator safety class; truncation honored.
- suppression → `blocked_by_policy`; missing grant → `missing_grant`; unknown tool → stable error.
- propose records a `pending_approval` command and **sends nothing**; approve executes and still honors the suppression gate; batch over MCP is dry-run.

**Integration**
- `POST /api/mcp` `initialize` → capabilities.
- `tools/call crm.contacts.list` → shaped contacts for the bound tenant only (tenant isolation).
- `crm.contact.render` returns a personalized preview + eligibility, no ledger write.
- `crm.send.command.propose` → command id; `crm.commands.list` shows it; `approve` (operator grant) → executed/queued; a low-grant client cannot call `approve`.

**Client smoke (before claiming readiness)**
- MCP Inspector / Claude Code connects to `/api/mcp`, lists tools, reads contacts, proposes a send; a second lower-grant client sees fewer tools; revoking the grant removes access.

---

## 8. Invariants / safety

- MCP is a **projection** of the CRM HTTP routes; it creates no authority the routes don't already grant.
- Every CRM gate is preserved through MCP: suppression/unsubscribe, dry-run defaults, the approval-gated command, tenant isolation, ledger receipts.
- Agent-initiated sends are **propose-only**; a human approves. No un-gated blast over MCP.
- All MCP output passes the contract's redaction/output-safety; CRM data is `operator` class.
- No Laravel/Convex; no new business logic; Bun/Effect/Effect Schema; reuses `@openagentsinc/mcp-contract`.
- This audit changes no runtime behavior and exposes no transport by itself.

---

## 9. References

- Contract: `packages/mcp-contract/src/index.ts` (`@openagentsinc/mcp-contract`); Worker marker `apps/openagents.com/workers/api/src/mcp-contract-import.ts`.
- Protocol core: `apps/pylon/src/tas/mcp-server.ts`, `mcp-client.ts`.
- Worker routing/auth: `apps/openagents.com/workers/api/src/index.ts` (`routeOmniRequest`, `requireAdminApiToken`).
- CRM surface: `apps/openagents.com/workers/api/src/crm-*.ts` (epic #5980, PR #5989) + `docs/crm/*`.
- Prior MCP audits: `docs/mcp/2026-06-21-openagents-monorepo-mcp-infrastructure-audit.md`, `docs/mcp/2026-06-21-openagents-overarching-mcp-roadmap.md`.
</content>

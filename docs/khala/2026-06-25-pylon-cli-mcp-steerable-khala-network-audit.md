# Audit + post-#6273 roadmap: portable Pylon CLI Khala-issuer + MCP server

> Status: **audit + build roadmap for the NEXT phase, AFTER the open #6273 epic
> (children #6274–#6281) and #6271/#6272 land.** This doc does NOT implement
> code; it audits the current systems and plans the work that turns the #6273
> foundation into a **portable, agent-loadable** capability.
>
> **Scope of this phase (owner intent).** The #6273 foundation links a user's
> OpenAuth account ↔ many Pylons + many keys and routes coding workflows
> **Khala → the user's own coding agents** (own-capacity-only). This phase builds
> the **inverse direction**: let any caller — including a bare coding agent —
> **issue a request INTO the Khala/pylon network that draws on the user's own
> capacity, from anywhere**, authenticated only by the user's token.
>
> 1. **Pylon CLI as a portable Khala request-issuer.** A new CLI verb issues a
>    Khala request that uses the user's own capacity. It must work **anywhere**:
>    a Pylon running **remotely** (GCE) authed with the user's token can draw on
>    the user's capacity wherever it lives. Capacity is location-independent; the
>    user's token is the key.
> 2. **Expose that path as an MCP server (+ shareable MCP config).** Wrap the CLI
>    request path as an MCP server so **any coding agent** (Claude Code, Codex,
>    …) loads one MCP config line and becomes a Khala-network client: from inside
>    a bare session it sends a request the Pylon routes through the network →
>    Khala → the user's capacity.
> 3. **Then fold it into Autopilot Verse desktop.** Once the CLI + MCP path is
>    solid, the **Autopilot Verse** desktop app consumes the same path. Later
>    step.
>
> **Not a product promise, not public-claim copy.** Labels used below: **NOW** =
> firm scope for this phase; **FUTURE** = explicitly deferred / gated;
> **DEFAULT-ON** = armed and live by default (no owner flip). Every existing
> invariant holds: **own-capacity-only** (a caller only ever uses its OWN linked
> capacity), **all-orchestrated-tokens-count** (source-agnostic public counter),
> **semantic-not-keyword** routing, **resumable-SSE via Durable Streams**, one
> model (`openagents/khala`, no variants), no-resale (subscription-scoped only).
>
> **Direction note — "eventually others" (pooling) is FUTURE / gated.** Routing
> an issued request beyond the user's own linked Pylons into a shared pool is
> explicitly out of scope here; it is named so the data model does not foreclose
> it, but no pooling, marketplace gate, settlement, or cross-user routing is
> designed in this phase.
>
> Companion reading: the #6273 foundation
> `docs/khala/2026-06-25-pylon-linked-coding-capacity-routing-spec.md`; the Pylon
> CLI audit `apps/pylon/docs/2026-06-15-pylon-cli-only-agent-steerable-audit.md`;
> the CRM MCP server phase-1 audit
> `apps/openagents.com/docs/mcp/2026-06-22-crm-mcp-server-phase-1-audit.md`
> (referenced as `SOURCE` across the CRM MCP files); the MCP contract package
> `packages/mcp-contract/src/index.ts`; the Pylon presence auth contract in
> `apps/openagents.com/AGENTS.md`; the Durable Streams primitive
> `packages/durable-stream/` and the inference resume route
> `apps/openagents.com/workers/api/src/inference/durable-inference-read-routes.ts`.

---

## 0. Executive summary

The same surprise as the #6273 audit holds here: **most of the substrate already
exists, scattered across surfaces that have never been wired into a single
"issue-into-the-network" path.** Specifically:

- A **remote Pylon authed with the user's token** already runs today. The GCE
  deploy (`apps/pylon/deploy/gcloud/README.md` + `setup-pylon.sh`) provisions a
  VM, copies an env file carrying `OPENAGENTS_AGENT_TOKEN` +
  `PYLON_OPENAGENTS_BASE_URL=https://openagents.com`, and runs the Pylon against
  production. Token-authed-anywhere is **already real** at the deploy layer.
- The **token → account chain is already location-independent.** An `oa_agent_`
  bearer resolves to `accountRef = agent:<user_id>` at the inference auth edge
  (`chat-completions-routes.ts` `authenticate` →
  `authenticateProgrammaticAgent`), exactly the same way regardless of where the
  request originates. A remote GCE Pylon and a local desktop Pylon present the
  *same* identity if they carry the same token.
- A **Khala request path already exists** end to end: the OpenAI-compatible
  `/v1/chat/completions` gateway, the `openagents/khala` model, and the
  **resumable Durable Streams** read route
  (`durable-inference-read-routes.ts`, `GET /v1/chat/completions/durable/{id}`).
  The Verse desktop already calls it via its `khalaTurn` RPC
  (`apps/autopilot-desktop/src/shared/rpc.ts`).
- **MCP server infrastructure already exists in this repo** — and is the single
  biggest reuse win. There are TWO working MCP servers plus a shared contract:
  - `packages/mcp-contract/` — a typed Effect-Schema MCP contract (authority
    classes, grants, descriptors, output projection, receipts).
  - `apps/openagents.com/workers/api/src/crm-mcp*.ts` — a **Streamable-HTTP
    JSON-RPC MCP server** mounted at `POST /api/mcp`, with a public discovery
    doc at `GET /.well-known/openagents-mcp.json` and a token/grant auth model.
    This is the canonical "expose a Worker capability as a remote MCP server"
    pattern.
  - `apps/pylon/src/tas/mcp-server.ts` + `mcp-client.ts` — a **pure MCP
    dispatch core** (tool registry, `tools/list`, `tools/call`) already living
    *inside the Pylon process*. This is the canonical "Pylon has an MCP surface"
    seed.

So this phase, like #6273, is **mostly wiring** — but with two genuinely net-new
pieces and one honest hard gap:

1. **There is NO Pylon CLI verb that issues a Khala request drawing on the
   user's capacity.** (Net-new. Details in §1.1.)
2. **There is NO Pylon-side MCP server that exposes that request path** to a
   loaded coding agent. (Net-new, but built from the §1.5 reuse set.)
3. **Remote-token capacity authorization is partial.** Token-authed-anywhere is
   real, but the server cannot yet answer "for this token's account, which
   capacity is available and where" (that answer is #6276's job, NOT YET
   LANDED), and there is no path that routes an *issued* request from one
   (remote) Pylon across the account's *other* linked Pylons (that router is
   #6278's job, NOT YET LANDED). This phase **depends on those #627x issues**
   and adds the issue-into-the-network layer on top.

### Target architecture (text diagram)

```
  bare coding agent (Claude Code / Codex / any MCP client)
        │  loads one MCP config line (stdio: `pylon mcp` | remote: https://openagents.com/api/mcp + token)
        ▼
  Pylon MCP server (NEW; built on tas/mcp-server.ts core + mcp-contract)
        │  tool: khala.request({ prompt|objective, model:"openagents/khala", workflow?, stream:true })
        ▼
  Pylon CLI Khala-issuer verb (NEW; e.g. `pylon khala request`)
        │  auth: OPENAGENTS_AGENT_TOKEN (the user's token) — SAME token a remote GCE Pylon carries
        ▼
  /v1/chat/completions  (EXISTS)  ──auth edge──▶  accountRef = agent:<user_id>  (location-independent)
        │
        ▼
  caller-coding-capacity resolver (#6276, NOT YET LANDED)
        │  "for this accountRef + its OpenAuth-linked set, what capacity, where, available?"
        ▼
  pylon-network router (#6278, NOT YET LANDED)
        │  route to one of the user's OWN linked Pylons (local / remote / cloud)   [pooling=FUTURE/gated]
        ▼
  Khala orchestrator → user's capacity (local desktop Pylon | remote GCE Pylon | cloud)
        │  executes (Codex first; Claude next — executors already exist on the Pylon)
        ▼
  resumable Durable Stream (packages/durable-stream/)  ──▶  result back to the bare agent
        │  client disconnect? resume from GET /v1/chat/completions/durable/{id}?offset=…  (never meters)
        ▼
  tokens orchestrated ──▶ COUNT on public khala-tokens-served (source-agnostic) + internal demand_kind tag
```

The own-capacity-only invariant is enforced at the same single point as #6273:
the resolver enumerates ONLY Pylons whose `ownerAgentUserId` equals the issuing
token's agent-user, and the router can only target that set. A remote issuer
cannot widen scope — the token *is* the scope. "Eventually others" (pooling)
would be a new gated lane above this, not a relaxation of it.

---

## 1. System-by-system audit (exists / partial / missing, with file evidence)

### 1.1 A Pylon CLI Khala-issuer verb — **MISSING (net-new)**

`apps/pylon/src/index.ts` (4,367 lines) dispatches ~24 top-level commands
(catalogued in `cli-catalog.ts` and the 2026-06-15 CLI audit). The closest
existing surfaces, and why none of them is the verb this phase needs:

- **`pylon work submit "<objective>" --adapter codex|claude_agent|fable …`**
  (`index.ts` ≈L3457, `work-requester.ts` `submitPylonAutopilotWork` ≈L312)
  POSTs to **`/api/autopilot/work`** (a Forge work-order intake that returns
  `402`), NOT to Khala/`/v1/chat/completions`. It creates a *paid work order*,
  not a Khala completion that draws on the caller's own capacity. Auth is
  `OPENAGENTS_AGENT_TOKEN` (good — the right token), endpoint is wrong for this
  phase.
- **`pylon work request / offers / accept`** (`index.ts` ≈L3494,
  `work-requester.ts` `createPylonWorkRequest`) POSTs to
  **`/api/forum/work-requests`** — the public NIP-90-style labor market, also
  not Khala-into-own-capacity.
- **`resolveModelAdapter(env)`** (`agent-surface.ts` ≈L122) can call a **local**
  OpenAI-compatible endpoint (`PYLON_LOCAL_MODEL_URL`) or the user's **Gemini**
  key directly. It never targets the openagents.com Khala gateway and is not a
  CLI verb; it is an internal helper for `ask-artanis`.

**Honest gap:** there is **no `pylon khala request` (or equivalent) verb** that
issues a request to `/v1/chat/completions` (model `openagents/khala`,
`stream:true`) authenticated by the user's token and intended to be routed
through the network to the user's own capacity. This verb is the **keystone of
this phase** (P1) and must reuse the existing `TipsNetworkOptions` auth pattern
(`agentToken` from `OPENAGENTS_AGENT_TOKEN`, `baseUrl` from
`PYLON_OPENAGENTS_BASE_URL`) that every other network verb already uses.

### 1.2 Token-authed-anywhere (remote/GCE) — **EXISTS at deploy + auth edge; capacity-side is partial**

- **Deploy layer EXISTS.** `apps/pylon/deploy/gcloud/setup-pylon.sh` +
  `README.md`: creates/starts a GCE VM, copies an env file
  (`OPENAGENTS_AGENT_TOKEN`, optional `ANTHROPIC_API_KEY`) over IAP/SSH, sets
  `PYLON_OPENAGENTS_BASE_URL=https://openagents.com`, and runs
  `apps/pylon/scripts/install-cloud-node.sh`. It deliberately keeps tokens OUT
  of instance metadata/startup scripts. The `#6089` GPU serving lane is already
  documented here. A remote Pylon authed with the user's token against
  production is a **shipped capability**, aligned with "our cloud = OpenAgents
  GCE" (`oa-codex-control` + GCE).
- **Auth edge is location-independent EXISTS.** `chat-completions-routes.ts`
  resolves the bearer via `authenticate` → `authenticateProgrammaticAgent`
  (`agent-registration.ts`) → `accountRef = agent:<user_id>`. Same code path,
  same result, regardless of origin IP. `authenticateProgrammaticAgent` is the
  shared resolver used across `pylon-api-routes.ts`, `agent-*-routes.ts`, etc.
- **Capacity-side is PARTIAL.** Token-authed-anywhere proves *identity*
  anywhere; it does NOT yet prove *capacity* anywhere. The "for this token's
  account, what capacity exists and where" answer is #6276 (capacity discovery,
  NOT YET LANDED), and routing an issued request to that capacity is #6278
  (router, NOT YET LANDED). The per-service heartbeat capacity reporting (#6273
  §1.6 / P2) is also still coarse today
  (`apps/pylon/src/presence.ts` sends one static
  `capacityRefs: ["capacity.public.pylon_cli.available"]`).

**Honest gap:** the *remote issuer → user's capacity wherever it lives* link
depends on #6276 + #6278 existing. This phase wires the issuer to that machinery;
it does not re-build discovery/routing.

### 1.3 MCP infrastructure to reuse — **EXISTS (three reusable pieces)**

This is the strongest reuse story in the repo. Three layers, all present:

1. **`packages/mcp-contract/`** (`src/index.ts`) — typed Effect-Schema MCP
   contract. Defines `OpenAgentsMcpSchemaVersion` (`openagents.mcp.phase0.v1`),
   `OpenAgentsMcpAuthorityClass` (13 classes incl.
   `coding_session_control`, `local_node_control`, `payment_spend`,
   `workspace_write`), high-risk authority set, `OpenAgentsMcpGrant`,
   tool descriptors, output projection, and receipt kinds. `phase` is
   `phase_0_contract_groundwork`, `runtimeTransportExposed: false` — i.e. the
   contract is built and waiting for a runtime transport, exactly what this
   phase adds.
2. **`apps/openagents.com/workers/api/src/crm-mcp*.ts`** — a working
   **Streamable-HTTP JSON-RPC MCP server** (the canonical remote-MCP pattern):
   - `crm-mcp-routes.ts` — `POST /api/mcp` stateless JSON-RPC transport
     (`initialize`, `ping`, `tools/list`, `tools/call`, `resources/*`),
     protocol version `2025-06-18`, server info, `McpPrincipal`
     (`subjectRef` + `tenantRef` + `grants`), tool failures returned as
     `isError` results (not transport errors). **Auth is at the transport
     boundary** (admin token = full grant; a scoped MCP grant token = its
     declared authorities + bound tenant). Client-supplied tenant is never
     trusted.
   - `crm-mcp.ts` — the tool catalog (delegates to existing store/read
     functions; "MCP is a projection of existing routes — no new authority").
   - `crm-mcp-discovery-routes.ts` — `GET /.well-known/openagents-mcp.json`
     advertising schema/protocol version, transport (`streamable_http`),
     endpoint, auth model (`admin_token_or_scoped_grant`,
     `X-OpenAgents-Tenant`), and a PUBLIC-SAFE tool/resource catalog (names,
     required authorities, risk class — no data, no secrets).
3. **`apps/pylon/src/tas/mcp-server.ts`** + **`mcp-client.ts`** — a **pure MCP
   dispatch core inside the Pylon process**: `createMcpToolRegistry`,
   `registerTool`, `handleToolsList`, `dispatchToolCall`,
   `McpToolContract { name, handlerKind, readOnly }`. `mcp-client.ts` defines
   the full JSON-RPC envelope types (`McpRequestEnvelope`,
   `McpResponseEnvelope`, `McpInitializeRequest`, server/client capabilities).
   Tests in `apps/pylon/tests/tas-mcp-server.test.ts`.

There is also a user-facing **Forge MCP server *export*** view
(`apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/mcp-server-export.ts`)
that *describes/projects* an MCP server config to the user — a precedent for
surfacing an MCP config in the product UI, though it is a projection, not a live
transport.

**Honest gap:** none of these exposes the **Khala request path**. The CRM MCP
exposes CRM reads; the TAS MCP core has no Khala tool registered. This phase
registers a `khala.request` tool on a Pylon-hosted MCP server (P3), choosing
stdio-for-CLI-loaded-agent vs remote-HTTP (see §3 design).

### 1.4 Auth / token portability — **EXISTS, with a known #6273 linkage upgrade in flight**

The token model is already portable and already the right shape:

- `oa_agent_<…>` → SHA-256 → `agent_credentials.token_hash` →
  `credential.user_id` (an *agent* `users` row) → `accountRef = agent:<user_id>`
  (`agent-registration.ts`, `inference-free-tier-key.ts`). Same chain at every
  edge.
- A Pylon registration is bound to `ownerAgentUserId` (= the same agent-user)
  via `requireAgent()` in `pylon-api-routes.ts`; every Pylon route re-checks
  `registration.ownerAgentUserId !== session.user.id` and rejects cross-agent
  access. **This is the own-capacity-only enforcement point**, and it is
  origin-agnostic — a remote GCE issuer presenting account A's token can only
  ever resolve and target A's own Pylons.
- The **OpenAuth account ↔ many keys + many Pylons** link model is #6273's P1
  (NOT YET LANDED). This phase's issuer/MCP path **reads the caller's linked set
  through that model** once it exists; until then it is scoped to the single
  agent-user the token resolves to.

**Own-capacity-only when the issuer is remote (the central worry):** enforced
NOT by trusting the issuer's location but by scoping both the capacity *read*
(#6276 resolver) and any assignment/route *write* (#6278) to the token's
`ownerAgentUserId`. A remote issuer is just another holder of account A's token;
it inherits A's scope and nothing more. The presence-auth contract
(`apps/openagents.com/AGENTS.md`) already establishes that the bearer token, not
a self-held key, is the only accepted authority — so a remote node cannot forge
broader presence/authority.

### 1.5 Pylon network routing of an issued request — **DEPENDS ON #6278 (NOT YET LANDED)**

Today there is no path that takes a request from one (possibly remote) Pylon and
routes it across the account's *other* linked Pylons. The pieces that exist:

- The **assignment-lease pull** (`apps/pylon/src/assignment.ts`,
  `pollAssignments()` → `GET /api/pylons/<ref>/assignments`) is how a Pylon
  receives server-originated work today (#6273 §1.1/§1.7). The router-originated
  coding path in #6273 (P4) reuses this — the router *creates* an assignment and
  the caller's Pylon picks it up.
- `cloud-control-client.ts` + `openagents-cloud-provider.ts` +
  `scripts/qa-async-gce-trigger.ts` prove server→cloud placement
  (`oa-codex-control`, `openagents.codex_placement_assignment.v1`).

**Honest gap:** "route an issued request to the user's pylon network" is the job
of #6276 (capacity discovery) + #6278 (router), both of which are **open #6273
children, NOT YET LANDED**. This phase consumes them; it must not re-implement
discovery/routing. The "eventually others" pooling lane is **FUTURE/gated** and
is explicitly NOT designed here (it would need a marketplace gate + settlement +
a fresh review, per #6273 §2 invariants).

### 1.6 Khala router + Durable Streams (the ride the issued request takes) — **EXISTS**

- `/v1/chat/completions` is the OpenAI-compatible gateway
  (`chat-completions-routes.ts`); `model-router.ts` `selectAdapterPlan(model)`
  picks lanes; #6273 P3 adds the caller-capacity branch (NOT YET LANDED).
- **Durable Streams EXISTS.** `packages/durable-stream/` (offset-addressed
  replay, `Stream-Next-Offset` / `Stream-Closed`, exactly-once writes,
  CDN-friendly fan-out: `core.ts`, `durable-object.ts`, `http.ts`,
  `offset.ts`, `store.ts`, conformance + live tests). The resume route
  `inference/durable-inference-read-routes.ts` serves
  `GET /v1/chat/completions/durable/{requestId}?offset=<last-offset>` and
  **NEVER meters** (replays stored bytes only).
- The issued request **rides this model**: the issuer (CLI/MCP) gets a resumable
  SSE; a dropped client (a bare agent whose session ended) resumes from its last
  offset. This is the same contract #6273 P4 mandates.

### 1.7 Autopilot Verse desktop consumption — **EXISTS (the later step's target)**

`apps/autopilot-desktop/src/shared/rpc.ts` already has the consumption surface
this phase eventually folds into (note: **Autopilot Verse**, not "versus"):

- **`khalaTurn` RPC** (≈L1018) — submits to `/v1/chat/completions`, model
  `openagents/khala`, `stream:true`, projecting back answer text + the khala
  token path; the durable handle rides on the response, attached on stream close
  (≈L1276–1281). The deprecated `khala-mini`/`khala-code` ids are noted as
  gateway-rewritten to `openagents/khala` (consistent with one-model).
- **`spawnSession` RPC** (≈L1153) — `adapter: "codex" | "claude_agent"`,
  `accountRef?`, with `lane: "auto" | "local" | "cloud-gcp" | "cloud-shc"` where
  `auto` already means "own Pylon first, then cloud."
- `accountRef` per-session picker (≈L36) + managed-account registry RPCs.

**Honest gap:** the Verse desktop talks to Khala directly via `khalaTurn`; it
does NOT yet route through the Pylon CLI/MCP issuer path. P5 folds it in so the
desktop consumes the same portable path (so the desktop, a CLI, and a bare agent
all use one issuer contract).

---

## 2. The MCP surface design

### What the MCP server exposes (tools)

A small, typed tool set, declared as `OpenAgentsMcpToolDescriptor`s from
`packages/mcp-contract/`, dispatched through the `tas/mcp-server.ts` registry
core, each mapping 1:1 to a Pylon CLI verb (no new authority beyond the CLI):

| Tool | Authority class | Maps to | Notes |
| --- | --- | --- | --- |
| `khala.request` | `coding_session_control` (or `private_account_read` for plain chat) | `pylon khala request` (P1) | issue a Khala request drawing on the user's capacity; `stream:true`, returns the durable handle |
| `khala.resume` | `private_account_read` | durable read route | resume a dropped stream from an offset (`GET /v1/chat/completions/durable/{id}`) — never meters |
| `khala.capacity` | `private_account_read` | `listCallerCodingCapacity` (#6276) | "what own capacity is available, where" (read-only) |
| `khala.status` | `private_account_read` | `pylon work status` analogue | poll an in-flight request/assignment |

`khala.request` is the keystone; `khala.capacity`/`khala.status`/`khala.resume`
make a bare agent self-sufficient (discover capacity, poll, recover). All four
are **projections of the CLI/existing routes — no new authority is minted at the
MCP layer** (the CRM MCP discipline).

### The config a user adds to Claude Code / Codex

Two transport shapes, mirroring the CRM MCP precedent. The user drops ONE entry:

**stdio (preferred for a CLI-loaded agent on the same box / a Pylon host):**

```jsonc
// .mcp.json / claude_desktop_config / codex mcp config
{
  "mcpServers": {
    "khala": {
      "command": "pylon",
      "args": ["mcp"],
      "env": {
        "OPENAGENTS_AGENT_TOKEN": "oa_agent_…",        // the user's token = the key
        "PYLON_OPENAGENTS_BASE_URL": "https://openagents.com"
      }
    }
  }
}
```

`pylon mcp` (NEW, P3) boots the `tas/mcp-server.ts` registry over stdio JSON-RPC,
registers the `khala.*` tools, and authenticates each underlying call with the
user's `OPENAGENTS_AGENT_TOKEN` (the exact pattern every other network verb uses
via `TipsNetworkOptions`).

**remote Streamable-HTTP (for an agent on a different box, or no local Pylon):**

```jsonc
{
  "mcpServers": {
    "khala": {
      "type": "http",
      "url": "https://openagents.com/api/mcp",          // reuse the CRM MCP transport pattern
      "headers": { "Authorization": "Bearer oa_agent_…" }
    }
  }
}
```

The remote shape reuses the **exact** `crm-mcp-routes.ts` Streamable-HTTP
transport + `/.well-known/openagents-mcp.json` discovery, adding the `khala.*`
tools to a catalog gated by the bearer's resolved `accountRef`. This is the
"bare Codex/Claude + one config line → Khala-network client" path.

### stdio vs remote — when to use which

- **stdio (`pylon mcp`)** when a Pylon is present (local desktop, or the GCE box
  the agent runs on): the agent talks to a *local* Pylon process, which is also
  the thing that can BE the user's capacity. Lowest trust surface (token never
  leaves the host), and it is the inverse-of-#6273 sweet spot — the same Pylon
  that receives Khala→agent assignments can also issue agent→Khala requests.
- **remote HTTP (`/api/mcp`)** when there is no local Pylon (a bare cloud agent,
  a CI agent): the agent authenticates directly to the Worker MCP transport. The
  Worker resolves `accountRef` from the bearer and routes through #6276/#6278 to
  the user's capacity wherever it lives. This is the purest "token is the key,
  capacity is location-independent" realization.

### How it authenticates with the user's token

Identical to the rest of the system: the `oa_agent_` bearer →
`authenticateProgrammaticAgent` → `accountRef = agent:<user_id>`. stdio passes
the token via env to the local `pylon mcp` process; remote passes it in the
`Authorization` header to `/api/mcp`. Either way the MCP layer mints **no new
authority** — it is bounded by the token's existing scope and the
own-capacity-only enforcement at the resolver/router.

---

## 3. The portability / auth model (token-authed-anywhere; own-capacity-only; pooling = FUTURE)

- **Token is the key; capacity is location-independent (NOW).** Identity
  resolution is already origin-agnostic (§1.2, §1.4). A request issued from a
  remote GCE Pylon, a CI box, or a bare laptop agent all resolve to the same
  `accountRef` if they carry the same token, and all are routed to the same
  owned capacity set.
- **Own-capacity-only when the issuer is remote (NOW, firm).** Enforced by
  scoping the capacity read (#6276) and any route/assignment write (#6278) to the
  token's `ownerAgentUserId`, reusing the existing
  `registration.ownerAgentUserId !== session.user.id` guard. A remote issuer
  inherits exactly the token's scope — it cannot reach another account's
  capacity. Property test required (P6).
- **All-orchestrated-tokens-count (NOW).** Requests issued through the CLI/MCP
  path that Khala orchestrates count on the public `khala-tokens-served` scalar,
  source-agnostic, with an internal `demand_kind`/`demand_source` tag for honest
  analytics (consistent with #6273 §1.8 / P5a).
- **Resumable-SSE via Durable Streams (NOW).** Every issued request returns an
  interruptible, resumable SSE; a bare agent whose session dropped can resume
  from its last offset (§1.6). Long work never blocks the turn.
- **Semantic-not-keyword (NOW).** Any workflow classification on the issued
  request goes through the typed/semantic classifier #6273 introduces (P3a),
  never ad-hoc keyword matching on prose.
- **Pooling / "eventually others" (FUTURE / gated).** Routing an issued request
  beyond the user's own linked Pylons into a shared pool is explicitly out of
  scope. The data model is left open to it (the OpenAuth link model in #6273 P1
  already aggregates many Pylons), but no pooling, marketplace gate, settlement,
  or cross-user routing is designed here. It would require a fresh dated review.

---

## 4. Dependencies on the open issues

This phase builds AFTER the #6273 epic (children #6274–#6281) and #6271/#6272.
Per-epic dependencies below (issue numbers map to the #6273 children by role;
verify exact numbers against the live epic before starting):

| This-phase epic (§5) | Hard dependency from the open #627x set | Why |
| --- | --- | --- |
| P1 (CLI Khala-issuer verb) | #6273 P3 router branch + `/v1/chat/completions` (exists) | the verb issues into the router; full own-capacity routing needs the branch |
| P2 (remote-token capacity authorization) | #6276 capacity discovery; #6273 P1 OpenAuth link model | "what capacity, where, for this token" comes from #6276 + the link model |
| P3 (Pylon MCP server + config) | P1 (this phase) | the MCP server wraps the P1 CLI path |
| P4 (network routing of an issued request) | #6278 router; #6276 discovery | issued request routes across the account's linked Pylons |
| P5 (Verse desktop integration) | P1–P4 (this phase) | desktop consumes the settled CLI/MCP path |
| P6 (accounting, invariants, tests) | #6273 P5a/P5b accounting + property-test scaffolding | extends the same counter + own-capacity property test to the issuer path |

**Do not start P2/P4 until #6276 and #6278 have landed on `main`.** P1 and the
stdio half of P3 can begin as soon as #6273 P3 (router branch) is on `main`,
because they only need the existing `/v1/chat/completions` + token auth.

---

## 5. Phased roadmap (starts AFTER the open #6273 issues land)

Each task: scope · seam it touches · acceptance. No code is written here.

### P1 — Pylon CLI Khala-issuer verb (NOW, keystone)

- **P1.1 `pylon khala request` verb.**
  - Seam: `apps/pylon/src/index.ts` command dispatch; a new
    `apps/pylon/src/khala-requester.ts` mirroring `work-requester.ts`'s
    `TipsNetworkOptions` auth pattern (`agentToken` from
    `OPENAGENTS_AGENT_TOKEN`, `baseUrl` from `PYLON_OPENAGENTS_BASE_URL`);
    `cli-catalog.ts` entry.
  - Scope: issue a request to `POST /v1/chat/completions` (model
    `openagents/khala`, `stream:true`) authenticated by the user's token,
    intended to be routed to the user's own capacity. Accept
    `--prompt`/`--objective`, optional typed `--workflow` (feeds #6273 P3a
    classifier), `--json`, and a `--resume <durable-id> --offset <n>` form for
    the resume path. Money/spend discipline: no seed/offer material; public-safe
    args only (reuse `assertPublicSafe` from `work-requester.ts`).
  - Acceptance: `pylon khala request --prompt … --json` returns a durable
    request id + initial SSE handle against a fixture gateway; auth uses the
    user's token; absent token errors cleanly; output is `--json` and
    catalogued in `pylon help --json`.
- **P1.2 Resume / status sub-verbs.**
  - Seam: same requester module; the durable read route
    `durable-inference-read-routes.ts`.
  - Scope: `pylon khala resume <id> --offset <n>` replays the suffix (never
    meters); `pylon khala status <id>` polls in-flight state.
  - Acceptance: a dropped-stream fixture resumes from the last offset and
    receives the terminal frames; resume path proven to not meter.

### P2 — Remote-pylon token auth / anywhere-capacity authorization (NOW; needs #6276 + #6273 P1)

- **P2.1 Capacity authorization for an issued request.**
  - Seam: the #6276 `listCallerCodingCapacity(ownerAgentUserId)` resolver and the
    #6273 P1 OpenAuth link model; consumed by the issuer route.
  - Scope: when an issued request arrives (from any origin), resolve the token →
    `accountRef` → the OpenAuth-linked own capacity set, and authorize drawing on
    it ONLY within that set. No new auth — reuse `authenticateProgrammaticAgent`
    + the `ownerAgentUserId` guard.
  - Acceptance: a request issued with account A's token (from a remote origin)
    authorizes only A's own linked capacity; an attempt to target B's capacity is
    rejected; a fixture proves origin-independence (same result local vs remote).
- **P2.2 Remote-Pylon issuer smoke (GCE).**
  - Seam: `apps/pylon/deploy/gcloud/` (existing); a new doc smoke entry.
  - Scope: document + fixture-prove that a GCE Pylon authed with the user's token
    can issue `pylon khala request` against production and have it authorize on
    the user's capacity. No new deploy code — the deploy path already carries the
    token.
  - Acceptance: a runbook + a smoke that a remote-authed issuer reaches the
    gateway and is scoped to the owner's capacity.

### P3 — Pylon MCP server + config (NOW; needs P1)

- **P3.1 `pylon mcp` stdio server.**
  - Seam: a new `apps/pylon/src/khala-mcp.ts` building on
    `apps/pylon/src/tas/mcp-server.ts` (registry/dispatch core) +
    `tas/mcp-client.ts` (JSON-RPC envelopes) + `packages/mcp-contract/`
    descriptors; a `pylon mcp` command in `index.ts`.
  - Scope: boot a stdio JSON-RPC MCP server registering `khala.request`,
    `khala.resume`, `khala.capacity`, `khala.status`, each dispatching to the P1
    CLI/requester path, authenticated by the env `OPENAGENTS_AGENT_TOKEN`. No new
    authority (CRM MCP discipline).
  - Acceptance: `tools/list` returns the four tools; `tools/call khala.request`
    issues a request and returns the durable handle; tool failures surface as
    `isError` results, not transport errors; a fixture proves a bare MCP client
    can drive the full path.
- **P3.2 Remote Streamable-HTTP MCP surface for Khala.**
  - Seam: extend the existing `crm-mcp-routes.ts` transport + the discovery doc
    `crm-mcp-discovery-routes.ts` (or a sibling `/api/mcp` catalog), adding the
    `khala.*` tools gated by the bearer's resolved `accountRef`.
  - Scope: expose `khala.*` over `POST /api/mcp` with bearer auth → `accountRef`,
    advertised in `/.well-known/openagents-mcp.json`. Reuse `McpPrincipal`,
    grant filtering, and `2025-06-18` protocol version.
  - Acceptance: a remote MCP client with the user's bearer lists + calls the
    `khala.*` tools; discovery doc advertises them public-safe (no data); another
    account's bearer is scoped to its own capacity.
- **P3.3 Shareable MCP config artifacts.**
  - Seam: a `pylon mcp config --json` emitter + (optionally) the product UI
    precedent `autopilot-work/mcp-server-export.ts`.
  - Scope: emit the exact stdio and remote `mcpServers` config blocks (§2) a user
    drops into Claude Code / Codex. Token is referenced, never embedded in
    committed material.
  - Acceptance: `pylon mcp config --json` yields valid stdio + remote configs; a
    doc shows a bare Claude Code / Codex picking them up.

### P4 — Bare-agent-via-MCP end-to-end through the pylon network (NOW; needs #6278 + P2 + P3)

- **P4.1 Route an issued request across the account's linked Pylons.**
  - Seam: the #6278 router; the assignment-lease pull
    (`apps/pylon/src/assignment.ts` `pollAssignments`); the durable stream.
  - Scope: an MCP-issued `khala.request` is authorized (P2), classified (#6273
    P3a), routed (#6278) to one of the user's OWN linked Pylons (local / remote /
    cloud) — Codex first, Claude next (executors already exist) — and the result
    returns over a resumable durable stream. Pooling stays FUTURE/gated.
  - Acceptance: an end-to-end fixture (no live spend): bare agent (MCP) → issued
    request → authorized + routed to a fixture owned Pylon → executes → durable
    frames → bare agent receives result; a dropped agent resumes via the durable
    route; a second account cannot see/resume the stream.
- **P4.2 Bare Codex / Claude Code acceptance smoke.**
  - Seam: a doc smoke + fixture harness.
  - Scope: prove "bare Codex/Claude + one MCP config line → Khala-network client"
    against fixtures.
  - Acceptance: a runbook + smoke showing a vanilla agent session driving the
    full path with only the MCP config + token.

### P5 — Autopilot Verse desktop integration (later; needs P1–P4)

- **P5.1 Verse consumes the issuer/MCP path.**
  - Seam: `apps/autopilot-desktop/src/shared/rpc.ts` (`khalaTurn`,
    `spawnSession`, `accountRef`).
  - Scope: route the Verse chat/khalaTurn path through the same Pylon CLI/MCP
    issuer contract so the desktop, a CLI, and a bare agent share one path. The
    desktop already holds `accountRef` + a local Pylon; prefer the stdio MCP /
    local issuer when a Pylon is present, remote `/api/mcp` otherwise.
  - Acceptance: a Verse turn issues through the unified path; resumable handle
    surfaces in the UI; own-capacity-only holds; no regression to the existing
    `khalaTurn` behavior when the unified path is off.

### P6 — Accounting, invariants, tests (NOW)

- **P6.1 Count issued-path tokens on the public counter (source-agnostic).**
  - Seam: `token-usage-ledger.ts`, `served-tokens-recorder.ts`,
    `public-khala-tokens-served-routes.ts` (extends #6273 P5a).
  - Acceptance: an issued-path completion moves the public scalar; its
    `demand_kind`/`demand_source` tag lets a private read break it out.
- **P6.2 Own-capacity-only property test for the remote issuer.**
  - Seam: model the bounded state (callers × tokens × origins × linked Pylons ×
    assignments/streams); assert no issued request from any origin is ever
    authorized/routed/resumable outside the token's own
    `ownerAgentUserId`-scoped set, across distinct OpenAuth accounts. Convert any
    counterexample to a regression test (workspace invariant discipline).
  - Acceptance: the property holds; a remote origin cannot widen scope.
- **P6.3 No-resale + semantic-not-keyword + no-new-MCP-authority coverage.**
  - Seam: `inference-resale-authorization.ts`; the #6273 P3a classifier;
    `mcp-contract` authority filtering.
  - Acceptance: the issuer path never triggers `subscription_capacity_resale`;
    classification has no raw keyword/string match on prose; the MCP tools mint
    no authority beyond their declared classes / the CLI's existing scope.

---

## 6. Open questions

1. **stdio-first vs remote-first default.** Should `pylon mcp` (stdio) be the
   recommended default for users who have a Pylon, with `/api/mcp` (remote) as
   the no-Pylon fallback? (Leaning stdio-first: lowest token-exposure surface and
   the cleanest inverse of #6273.)
2. **Grant granularity for the MCP tools.** Do `khala.request` etc. ride the
   user's full `oa_agent_` bearer, or should P3 mint a *scoped MCP grant*
   (`coding_session_control` only, via `mcp-contract`'s grant model) the user can
   revoke independently of their main key? (Scoped grant is more honest;
   full-bearer is simpler for v1.)
3. **Workflow signal on the issued request.** Reuse #6273 P3a's typed/semantic
   classifier as-is, or let the CLI/MCP `--workflow` field always take
   precedence (deterministic, allowed only after the semantic route is chosen)?
4. **Remote-HTTP MCP endpoint name.** Reuse `/api/mcp` (CRM transport, add a
   catalog) or mount a sibling `/api/khala/mcp`? (Reuse is less code; sibling is
   cleaner separation of catalogs.)
5. **"Eventually others" trigger.** What is the first concrete gate that would
   open the FUTURE pooling lane (own idle Pylon serving another user)? Out of
   scope to build, but worth naming the gate (marketplace gate + settlement +
   review) so the link model in #6273 P1 does not foreclose it.

---

## 7. Sharpest findings (TL;DR)

1. **There is NO Pylon CLI Khala-issuer verb today — this is the real net-new
   work.** `pylon work submit` POSTs to Forge (`/api/autopilot/work`, returns
   402); `pylon work request` POSTs to the public labor market
   (`/api/forum/work-requests`); `agent-surface.ts resolveModelAdapter` only
   hits a LOCAL OpenAI endpoint or Gemini. Nothing issues a request to
   `/v1/chat/completions` (model `openagents/khala`) drawing on the user's own
   capacity. P1 builds it, reusing the existing `OPENAGENTS_AGENT_TOKEN` /
   `TipsNetworkOptions` auth pattern every other network verb already uses.
2. **MCP-server infra to reuse is strong and already in-repo — the MCP piece is
   wiring, not greenfield.** Three layers exist: the typed contract
   `packages/mcp-contract/` (authority classes incl. `coding_session_control`,
   grants, descriptors; `runtimeTransportExposed:false` — waiting for exactly
   this); a working remote Streamable-HTTP JSON-RPC MCP server at
   `POST /api/mcp` with discovery at `/.well-known/openagents-mcp.json`
   (`crm-mcp*.ts`); and a pure MCP dispatch core ALREADY inside the Pylon
   (`apps/pylon/src/tas/mcp-server.ts` + `mcp-client.ts`). P3 registers a
   `khala.request` tool on these — no new MCP framework needed.
3. **Remote-token capacity authorization is the honest gap, and it depends on
   unlanded #6273 children.** Token-authed-anywhere is ALREADY real
   (GCE deploy carries `OPENAGENTS_AGENT_TOKEN`; the auth edge resolves
   `accountRef` origin-independently). But "for this token's account, what
   capacity is available and where" is #6276 (NOT YET LANDED) and routing an
   issued request to it is #6278 (NOT YET LANDED). This phase must consume those,
   not rebuild them; P2/P4 are blocked until #6276/#6278 are on `main`.
   Own-capacity-only when the issuer is remote is enforced the same way as #6273
   — scope both read and write to the token's `ownerAgentUserId`; the token IS
   the scope, so a remote origin cannot widen it.
4. **Durable Streams + the Verse `khalaTurn` RPC mean the result path and the
   later desktop fold-in are already built.** The resumable-SSE contract
   (`packages/durable-stream/` + `durable-inference-read-routes.ts`, which never
   meters) is the ride for every issued request, and
   `apps/autopilot-desktop/src/shared/rpc.ts` already talks to
   `openagents/khala` via `khalaTurn` (durable handle attached on stream close)
   and `spawnSession` (`lane:"auto"` = own Pylon first). P5 routes that existing
   surface through the unified issuer/MCP path rather than building a new one.

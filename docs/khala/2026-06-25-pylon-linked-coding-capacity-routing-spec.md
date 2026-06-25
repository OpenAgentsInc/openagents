# Audit + roadmap: Pylon-linked coding capacity routing

> Status: **audit + build roadmap, OWNER DECISIONS APPLIED 2026-06-25.** Goal:
> route our own (and any user's) day-to-day coding work **off the bare Codex /
> bare Claude Code CLIs and onto Khala**, by linking a user's **Pylon** (which
> already steers a local Codex / Claude Code install) to that user's
> openagents.com account, so the router can delegate coding workflows to that
> user's OWN coding agents — orchestrated **through Khala**.
>
> **Khala is a model/agent ORCHESTRATOR** (Episode 232; see
> `docs/transcripts/232.md` — "electrons in, orchestration, tokens out", and the
> "accepted outcomes per kilowatt hour" thesis). The orchestrator framing changes
> two earlier recommendations in this doc, now reversed by owner decision:
>
> 1. **The public `khala-tokens-served` counter is SOURCE-AGNOSTIC.** Every token
>    Khala orchestrates counts — local, crowd, swarm, cloud, subscription, "beamed
>    down from space." It does not matter that our own inference engine did not
>    serve them; if they flow through the Khala system they hit the counter. So
>    **own-capacity Codex/Claude coding tokens orchestrated through Khala DO
>    increment the public counter.** (See §1.8 / §2 / P5a — this REVERSES the
>    earlier "keep them out of the counter" guidance.)
> 2. **Account linking is built NOW, on the multi-link model.** Execution stays
>    single-user / own-capacity (firm invariant), but the **OpenAuth web-login
>    account is the one account that links MULTIPLE Pylons AND multiple agent/API
>    keys.** (See §1.2 / §1.3 / P1 — this upgrades the earlier "OpenAuth-many-keys
>    is FUTURE" to NOW.)
>
> **Not a product promise, not public-claim copy.** This is an audit of what
> exists and a phased plan. Every other existing invariant holds: evidence-only,
> no keyword/string intent routing, no self-promotion, no exactness inflation,
> identity guard, one model (`openagents/khala`, no variants). Labels used below:
> **NOW** = firm scope for this phase; **FUTURE** = explicitly deferred /
> speculative; **DEFAULT-ON** = armed and live by default (no owner flip needed).
>
> Owner decisions applied (override all prior recommendations in this doc):
> (1) counter counts ALL Khala-orchestrated tokens, source-agnostic, with an
> internal `demand_kind`/source tag for honest analytics; (2) build OpenAuth
> account ↔ many-Pylons + many-keys linking NOW; (3) coding-workflow classifier:
> full typed/semantic version (no keyword routing); (4) execution rides our
> **Durable Streams** resumable-SSE model (every request returns an interruptible,
> resumable SSE); (5) capacity reporting includes ALL dimensions (Codex×N /
> Claude×M / busy / available / queued); (6) routing is **default-ON, armed now**
> (not owner-gated-off); (7) ship **Codex first**, Claude Code flagged as the
> next-sophistication step (both executors already exist).
>
> Companion reading: Episode 232 transcript `docs/transcripts/232.md` (Khala as
> orchestrator); the Pylon presence auth contract in
> `apps/openagents.com/AGENTS.md`; the no-resale gate in
> `apps/openagents.com/INVARIANTS.md` (Provider Capacity Marketplace Gate,
> lines ~981–1043); `apps/pylon/docs/2026-06-15-pylon-cli-only-agent-steerable-audit.md`;
> `docs/autopilot-coder/2026-06-13-cloud-remote-execution-commercial-plan.md`;
> the Durable Streams primitive `packages/durable-stream/` and the inference
> resume route `apps/openagents.com/workers/api/src/inference/durable-inference-read-routes.ts`.

---

## 0. Executive summary

The surprising headline of this audit: **most of the pipeline already exists.** A
Pylon already discovers, probes, and runs a local Codex (`@openai/codex-sdk`) and
local Claude Code (`@anthropic-ai/claude-agent-sdk`) install; it already advertises
those as capabilities in its heartbeat; the server already has a typed
**assignment-lease** mechanism that carries a `codingAssignment` payload down to a
Pylon, and the Pylon already has executors (`executeCodexAgentAssignment`,
`executeClaudeAgentAssignment`) that run the local agent in a bounded workspace and
report a closeout. Ownership is already bound to `ownerAgentUserId`, which is the
**same agent-user identity an `oa_agent_` API key resolves to**.

What is missing is the **bridge between the Khala inference router and that
assignment pipeline**, plus three smaller gaps:

1. **The router never looks at the caller.** `selectAdapterPlan(model)` in
   `model-router.ts` is pure on model id; `accountRef` is resolved at the request
   edge but is used only for credits/quota/gates — never for lane selection. There
   is no branch that asks "does this caller own coding capacity?"
2. **There is no typed coding-workflow classifier.** The only intent seam,
   `intentToAcceptanceSpec()`, is an explicitly-flagged keyword *placeholder*
   awaiting "a broader semantic selector." Per the workspace rule we must build a
   bounded, typed, deterministic classifier — not keyword matching.
3. **Capacity reporting is coarse.** A Pylon reports
   `capacityRefs: ["capacity.public.pylon_cli.available"]` (static) plus capability
   refs (`capability.pylon.local_codex`, `capability.pylon.local_claude_agent`).
   There is no per-service count (Codex×N / Claude×M) and no busy/available load.
4. **Assignment dispatch is operator-only, and there is no human account that ties
   a user's many Pylons + many keys together.** Today only
   `/api/operator/pylons/assignments` creates an assignment, and the registry binds
   `ownerAgentUserId` to the *agent-user*, not to a human **OpenAuth** account. So
   "a user's own Pylon" works token-to-token, but a human who logs into
   openagents.com via OpenAuth cannot see all of their linked Pylons and keys in
   one place. **Owner decision: build the OpenAuth-account ↔ many-Pylons +
   many-keys linking model NOW** (P1), anchored on the OpenAuth account, designed
   from the start for multi-Pylon / multi-key aggregation in the web UI.
   **Execution stays single-user / own-capacity** (a caller only ever uses their
   OWN linked capacity — firm invariant); the OpenAuth account is purely the
   *linking and aggregation* anchor, not a pooling mechanism.

So this is less "build a system" and more "wire two existing systems together
behind a typed, identity-scoped gate, anchored on the OpenAuth account."

### Target architecture (text diagram)

```
  OpenAuth human account  ──links (P1, NOW)──▶  many oa_agent_ keys + many Pylons
        │                                            (aggregation anchor; web UI = FUTURE/P1b)
        ▼
  oa_agent_ API key  ──auth edge (chat-completions-routes.ts)──▶  accountRef = agent:<user_id>
        │                                                                  │
        │                                                                  ▼
        │                                          ┌──────────────────────────────────────┐
        │                                          │  Caller coding-capacity resolver (NEW) │
        │                                          │  "for this accountRef (+ its OpenAuth  │
        │                                          │   account's linked set), what coding   │
        │                                          │   services, available? all dims:       │
        │                                          │   Codex×N / Claude×M / busy/avail/queued│
        │                                          └──────────────────────────────────────┘
        │                                                  │ reads
        ▼                                                  ▼
  Khala router (model-router.ts)              pylon registry + heartbeat capacity
        │  selectAdapterPlan(model, callerCapacityHint?)   (capabilityRefs + NEW per-service capacityRefs)
        │                                                  │
        ▼                                                  │
  typed/semantic coding-workflow classifier (NEW)  ◀── request ─────┘
  CodingWorkflowClass = pr | fix | refactor | none   (bounded enum; typed/semantic — NOT keyword intent)
        │
        ▼  DEFAULT-ON: if workflowClass != none AND caller owns available coding capacity:
  return resumable SSE (Durable Streams) + delegate ──▶ create assignment-lease (codingAssignment: {codex})
        │       bound to caller's OWN ownerAgentUserId (own-capacity-only invariant); Codex first, Claude next
        ▼
  caller's OWN Pylon polls /assignments ──▶ executeCodexAgentAssignment  (Claude executor exists, ships next)
        │                                       (runs local Codex SDK in bounded workspace)
        ▼
  closeout + artifacts ──▶ appended to durable stream; client resumes/polls handle
        │
        ▼
  tokens orchestrated ──▶ COUNT on public khala-tokens-served (source-agnostic) + internal demand_kind tag
```

The "own-capacity-only" invariant is enforced at one point: the assignment is built
with `ownerAgentUserId = caller's own agent-user id`, and a Pylon only ever polls
its own assignments (it authenticates with its own bearer token bound to the same
`ownerAgentUserId`). A caller can never address another caller's Pylon.

---

## 1. System-by-system audit

### 1.1 Pylon local-agent steering — **mostly EXISTS**

Pylon drives the local agents as **lazy-imported SDK libraries, not CLI spawn**:

- Codex: `apps/pylon/src/codex-agent-executor.ts` `runWithCodexSdk()` imports
  `@openai/codex-sdk` and calls
  `new sdk.Codex({env}).startThread({workingDirectory, sandboxMode, approvalPolicy:"never", skipGitRepoCheck:true, networkAccessEnabled:false, model?}).runStreamed(instructions, {signal})`.
  Package constant `CODEX_AGENT_SDK_PACKAGE = "@openai/codex-sdk"`.
- Claude: `apps/pylon/src/claude-agent-executor.ts` `runWithClaudeAgentSdk()` imports
  `@anthropic-ai/claude-agent-sdk` and calls `sdk.query({prompt, options})` with a
  `PreToolUse` workspace-escape guard and `PostToolUse` edit/command counters.
  Package constant `CLAUDE_AGENT_SDK_PACKAGE = "@anthropic-ai/claude-agent-sdk"`,
  default tool allowlist `["Read","Edit","Write","Bash","Glob","Grep"]`.
- Readiness probes (no interactive CLI): `codex-agent.ts probeCodexAgentReadiness()`
  checks SDK importability + `~/.codex/auth.json`; `claude-agent.ts
  probeClaudeAgentReadiness()` checks the SDK + `~/.claude/.credentials.json` or a
  one-shot macOS Keychain query (`security find-generic-password -s "Claude Code-credentials"`).

**Assignment intake already exists.** `apps/pylon/src/assignment.ts` defines
`PylonAssignmentLease` (`schema: "openagents.pylon.assignment_lease.v0.3"`) carrying
an optional `codingAssignment` field; `pollAssignments()` GETs
`/api/pylons/<ref>/assignments`. The two executors dispatch on the payload shape:
`codexAgentTaskFrom()` keys on `codingAssignment.codex` (schema
`openagents.pylon.codex_agent_task.v0.3`, agentKind `codex_sdk`);
`claudeAgentTaskFrom()` keys on `codingAssignment.claudeAgent` (schema
`openagents.pylon.claude_agent_task.v0.3`, agentKind `claude_agent_sdk`).

**Gap:** capacity reporting is coarse — see §1.6.

### 1.2 Pylon linking + identity — **agent-bearer-token, NOT OpenAuth**

A Pylon is associated with an identity **only** through an `oa_agent_` bearer token.
`apps/openagents.com/workers/api/src/pylon-api-routes.ts`:

- `routeRegister()` calls `requireAgent()` → `authenticateProgrammaticAgent()` →
  binds `ownerAgentUserId = session.user.id` on the registration.
- Every Pylon route re-checks `registration.ownerAgentUserId !== session.user.id`
  and rejects cross-agent access (this is the natural enforcement point for
  own-capacity-only).
- The presence-auth contract is token-only and deliberate (see
  `apps/openagents.com/AGENTS.md` "Pylon Presence Auth Contract"): a node's
  self-held Nostr key is NOT accepted; NIP-98 presence returns a typed 401.

The **Claim-Your-Agent / X(Twitter) verification** flow lives in
`agent-owner-claim-routes.ts` (`StartXOwnerClaimRequest`, `VerifyXOwnerClaimRequest`,
`AgentOwnerClaimRecord`). Critically: claim approval requires a **browser session**
(`requireBrowserSession`, which *is* OpenAuth-backed) and stores `ownerUserId` (the
human) on the claim record — **but the resulting agent credential and all later
Pylon operations use only the agent bearer token bound to `ownerAgentUserId`**. The
human OpenAuth identity is recorded at claim time and then **dropped from the
operational path.**

**Honest answer to the central question:** Pylon-linking is **X-verified +
browser/OpenAuth-approved at claim time, but bearer-token-only at runtime.** It is
*not* continuously "on OpenAuth." There is no column linking an `oa_agent_`
credential back to an OpenAuth human account in the operational tables.

**Owner decision (NOW): change this.** The linking process must be **anchored on
the OpenAuth account**, not just the agent bearer. We build a real link table from
the OpenAuth human account to its many `agent_credentials` (keys) and its many
Pylon registrations, so that when a user logs into openagents.com they can
(eventually) see ALL their linked Pylons, all activity, all balances, all token
spend, what is running, and links to traces — aggregated across every linked key
and Pylon. The web UI for that aggregation is **FUTURE/incremental** (P1b), but
the **link model + linking flow is NOW** (P1) and must be designed for this
multi-Pylon / multi-key aggregation from the start. The own-capacity-only
*execution* invariant is unchanged: linking many capacities under one human
account does not let any caller use another caller's capacity.

### 1.3 OpenAuth account model + API-key mapping — **EXISTS, but layered/indirect**

- `migrations/0003_openauth_storage.sql` is a generic K-V store
  (`openauth_storage(key, value_json, expires_at, updated_at)`) for the OpenAuth
  library. There is **no relational `accounts` table and no `sessions` table.**
- The real identity tables are custom (`migrations/0002_auth_identity_and_agent_registration.sql`):
  - `users(id, kind CHECK in ('human','agent'), ...)`. Human ids are namespaced
    (`github:<id>`, `email:<addr>`); agent ids are `user_<uuid>`.
  - `auth_identities(id, user_id, provider, provider_subject, ...)`,
    `UNIQUE(provider, provider_subject)`. Humans can have multiple (github + email);
    agents get one `provider='agent_programmatic'`.
  - `agent_credentials(id, user_id, token_hash UNIQUE, token_prefix, status, ...)`.
  - `agent_profiles(user_id, slug, metadata_json)`.
- **API-key → account chain:** `oa_agent_<...>` → SHA-256 → `agent_credentials.token_hash`
  → `credential.user_id` (an *agent* `users` row). At the request edge
  (`index.ts` `authenticate`), this resolves to `accountRef = agent:<user_id>`.
  Free-tier quota is keyed on that same `account_ref` (`inference-free-tier-key.ts`,
  `inference_free_tier_keys`/`inference_free_tier_usage`).

**The crux:** an API key maps to an **agent-user**, and a Pylon registration is also
bound to an **agent-user** (`ownerAgentUserId`). So an API key and a Pylon can
already share one identity as long as they share the same agent-user. What was
"missing convenience" is the human **OpenAuth** account that ties together
*multiple* keys and *multiple* Pylons — and **owner decision upgrades this from
FUTURE to NOW.** P1 builds an `openauth_user_id` linkage on `agent_credentials`
(and the registry join through them to Pylon registrations), so one human OpenAuth
account aggregates many keys and many Pylons. Own-capacity-only execution is still
scoped to the agent-user `ownerAgentUserId` at dispatch time; the OpenAuth account
is the *aggregation/linking* layer above it, not a cross-user pool.

### 1.4 The Khala router — **EXISTS; caller-blind**

`apps/openagents.com/workers/api/src/inference/model-router.ts`:

- `selectAdapterPlan(model)` (≈L288–314) returns an ordered adapter-id list. Special
  Khala/Hydralisk ids route to explicit plans; everything else goes through
  `classifyModel()` → `LANE_PLAN_BY_CLASS` (`claude`/`gemini`/`open`/`unknown`).
- `classifyModel()` (≈L191–207) is a **bounded provider-family enum** — the only
  allowed deterministic classification (model id → provider family). Comment at the
  top of the file explicitly forbids ad-hoc string match on user intent.
- `dispatchWithOverflowWithMetadata()` (≈L415–501) tries lanes in order with
  backoff/overflow.

`chat-completions-routes.ts`:

- Auth edge (≈L1712) resolves `accountRef` then uses it for credit balance,
  premium gate, fair-share, spend-cap, free-allowance, free-tier, cache-affinity
  pinning — **never lane selection.** `planFor(requestedModel)` (≈L1914–1920) takes
  only the model. `decideCacheAwareRouting()` (≈L1937) reorders for warm-cache, also
  caller-agnostic for lane choice.

`model-serving-policy.ts`: `resolveSupplyLaneArming()` (≈L366–392) reads only
credential-presence env flags; no caller field.

**Where the new branch slots in:** extend `selectAdapterPlan(model, callerCapacityHint?)`
or post-process in `decideCacheAwareRouting()`. The cleanest seam is: at
`chat-completions-routes.ts` after `accountRef` is known and after the typed
workflow classifier runs, branch *before* the normal plan when (workflowClass is a
coding workflow) AND (caller owns available coding capacity). Account resolution
needs no change — `accountRef` is already threaded everywhere.

### 1.5 Coding-workflow detection — **MISSING; must be typed, not keyword**

The only intent seam is `inference/acceptance-spec.ts` `intentToAcceptanceSpec()`
(≈L171–184). Its own comment (≈L103–115) flags it as a *bounded keyword placeholder*
for acceptance-runner lane selection "until a broader semantic selector replaces
this." It currently defaults every khala-code request to the crossy-road spec. It is
**downstream** of the router and is not a routing decision.

There is **no** typed classifier for "is this a coding workflow," no embedding/cosine
selector, and no coordinator/planner seam for it. Per the workspace
semantic-routing rule, we must NOT add keyword matching on prose. The compliant
shape mirrors `classifyModel()` and `intentToAcceptanceSpec()`:

```
export type CodingWorkflowClass = 'pull_request' | 'bug_fix' | 'refactor' | 'none'
export const classifyCodingWorkflow = (request: CodingWorkflowIntentRequest): CodingWorkflowClass
```

Two compliant implementation routes (pick in P3, both allowed by the rule because
the *route* is selected by a typed/semantic mechanism, not prose keywords):

1. **Structured field (preferred, deterministic).** The OpenAI-compatible request
   carries an explicit typed signal — an `openagents.workflow` extension field or a
   header — that the caller / client sets. Deterministic parse of a bounded enum is
   explicitly allowed *after* the semantic route is chosen.
2. **Central typed semantic selector / embedding search** for free-form chat where
   no field is set — cosine-similarity against a small labeled exemplar set, behind
   a typed `classifyCodingWorkflow` boundary. This is the workspace-sanctioned path
   for genuinely free-form intent; it must be one central selector, not scattered
   string checks.

### 1.6 Capacity discovery + own-capacity-only — **partial; coarse capacity**

A Pylon heartbeat (`apps/pylon/src/presence.ts`, `PylonHeartbeatRequest` schema
`openagents.pylon.heartbeat.v0.3`) sends:

- `capacityRefs: ["capacity.public.pylon_cli.available"]` — **static, single ref.**
- `loadRefs: ["load.public.pylon_cli.low"]` — **static.**
- `capabilityRefs` — **dynamic**, includes `capability.pylon.local_codex` and/or
  `capability.pylon.local_claude_agent` when those probes are ready (assembled in
  `apps/pylon/src/index.ts` via `withCodexAgentCapability` / `withClaudeAgentCapability`).

So the server can already learn *presence* of Codex/Claude on a given Pylon, but
**not** "3 Codexes and 1 Claude" or busy-vs-available. The richer
`oa-node-managed-machine.ts` record models exactly the missing fields —
`supportedRuntimes` (`codex`/`opencode`/`probe`/…), `workloadClasses` (includes
`coding`), `availability` (`available`/`busy`/`draining`/`offline`/`unknown`) — but
that record is **projection/display only today** and is not fed by per-service Pylon
load.

**Own-capacity-only enforcement (NOW, firm):** the existing
`registration.ownerAgentUserId !== session.user.id` checks already isolate Pylons
per agent-user. The new caller-capacity resolver must (a) only enumerate Pylons whose
`ownerAgentUserId` equals the caller's `accountRef` agent-user, and (b) the dispatched
assignment must be built with that same `ownerAgentUserId` (matching the existing
operator path at `pylon-api-routes.ts` ≈L891). No cross-user pooling is even
expressible if both reads and the write are scoped to the caller's agent-user.

### 1.7 Local-agent execution + steering infra to reuse — **EXISTS (two paths)**

The on-Pylon executors (§1.1) are the primary reuse target. Supporting infra:

- `scripts/codex-fleet/` — a working fleet runner that pulls promises and runs
  `codex exec "<brief>" -m gpt-5.5 ... --json` in isolated worktrees, fetching Codex
  OAuth centrally via `fetch-codex-auth.mjs` (lease → grant → resolve). It is a
  **pull** model (workers select work) — not server-initiated delegation, but its
  auth-lease and per-task `CODEX_HOME` isolation patterns are directly reusable.
- `apps/autopilot-desktop/src/shared/rpc.ts` — `spawnSession({adapter:"codex"|"claude_agent", objective, lane:"auto"|"local"|"cloud-gcp"|"cloud-shc", accountRef, repoRef, verify})`,
  `shellControl` (headless autopilotctl), `resolveApproval`, managed-account
  registry RPCs. Lane `auto` already means "own Pylon first, then cloud."
- `apps/pylon/src/cloud-control-client.ts` + `openagents-cloud-provider.ts` +
  `scripts/qa-async-gce-trigger.ts` — the `oa-codex-control` placement contract
  (`openagents.codex_placement_assignment.v1`) and an existing trigger proving
  server→cloud placement works. The cloud daemon itself lives in `cloud/`, not here.

**Gap for true delegation:** all current intake is pull (Pylon polls) or
human/operator-initiated. The router-originated path reuses the **assignment-lease
pull** (the Pylon already polls `/assignments`), so no new server→node push channel
is strictly required for P1–P4 — the router just *creates* the assignment and the
caller's Pylon picks it up. Results return via the existing closeout/artifacts path.

### 1.8 Accounting + invariants — **own-capacity is allowed; counter counts ALL orchestrated tokens**

- **No-resale (INVARIANTS.md, Provider Capacity Marketplace Gate ≈L981–1043).**
  Exact scope: *"The no-resale rule stays scoped to consumer SUBSCRIPTION accounts
  only — API-inference gateway resale on an API-key account is NOT over-blocked."*
  `subscription_capacity_resale` is blocked unconditionally and non-waivably
  (enforced in `inference-resale-authorization.ts`, gated before money moves in
  `firmup-bitcoin-settlement.ts`). Connected provider keys are **user-scoped**:
  *"lease selection uses provider-tagged candidates from the requesting user's own
  connected accounts and never pools credentials across customers"* (≈L1039–1043).
  A user running **their own** Codex/Claude capacity for **their own** work is
  `agentic_work` on owned capacity — explicitly allowed, not resale. **Confirmed: this
  feature does not touch the no-resale prohibition.**
- **Public Khala counter — counts ALL orchestrated tokens (owner decision,
  REVERSES the prior recommendation).** `public-khala-tokens-served-routes.ts`
  exposes `GET /api/public/khala-tokens-served` =
  `SUM(input_tokens)+SUM(output_tokens)` over `token_usage_events` with **no
  filter** (`token-usage-ledger.ts`). The ledger carries a `demand_kind` column
  (`internal`/`external`/`unlabeled`) and a `demand_source` column; private reads
  can filter on them. **Khala is an orchestrator** (Episode 232): the headline rule
  is that **every token Khala orchestrates counts on the public counter,
  source-agnostic** — local, crowd, swarm, cloud, subscription, "beamed down from
  space." It does not matter that our own inference engine did not serve the
  completion bytes; if the work flows through the Khala system, it hits the
  counter. **Therefore own-capacity Codex/Claude coding tokens orchestrated through
  Khala DO increment the public `khala-tokens-served` scalar.** This is consistent
  with the existing design, which already includes internal dog-food in the public
  scalar. We still record an honest internal tag — `demand_kind` (own-capacity vs
  external vs hosted) and `demand_source` — for analytics, so we can always break
  the number down by source; but the tokens **count**. The prior
  "do-NOT-add-them-to-the-counter" recommendation is **withdrawn.**

---

## 2. Invariant section

- **Own-capacity-only (NOW, firm).** A caller's coding capacity is available *only*
  to that caller. Enforced by scoping both the capacity read and the assignment write
  to the caller's `ownerAgentUserId` (= the agent-user their API key resolves to),
  reusing the existing `registration.ownerAgentUserId !== session.user.id` guard. No
  cross-user routing is expressible.
- **Pooling / sharing (FUTURE).** Letting one user's idle Pylon serve another user,
  or pooling capacity into a shared lane, is explicitly out of scope here. It would
  require: a human OpenAuth account model linking keys+Pylons, an explicit
  marketplace gate, settlement, and a fresh review. Do not build toward it in P1–P5.
- **No-resale reconciliation.** Own-capacity coding work is `agentic_work` on owned
  capacity, which the no-resale rule explicitly permits (resale is scoped to
  *subscription-account* resale only). This feature must never let a third party
  consume the owner's subscription capacity — which the own-capacity-only invariant
  already guarantees.
- **Semantic-not-keyword routing.** Coding-workflow detection must go through one
  central typed classifier (`classifyCodingWorkflow`) using either an explicit typed
  request field/header or a central semantic/embedding selector — never ad-hoc string
  matching on prose. Deterministic enum parsing is allowed only after the typed route
  is chosen.
- **All-orchestrated-tokens-count (owner decision).** Khala is an orchestrator;
  every token it orchestrates counts on the public `khala-tokens-served` scalar,
  **source-agnostic** (local / crowd / swarm / cloud / subscription / own-capacity).
  Own-capacity coding tokens orchestrated through Khala therefore DO increment the
  public counter. Keep an internal `demand_kind` (own-capacity / external / hosted)
  and `demand_source` tag for honest analytics, but never *exclude* orchestrated
  tokens from the public count. (Reverses the earlier public-counter-honesty
  recommendation.)
- **Default-on routing (owner decision).** The coding-workflow routing is **armed
  and live by default** — not owner-gated-off. No flag flip is required to enable
  it; it is on. (If a kill switch is wired at all, it is an off-by-default *disable*
  switch, never an off-by-default *enable* gate.)
- **Codex-first execution (owner decision).** Ship **Codex** as the delegate agent
  for now. Both on-Pylon executors already exist (`executeCodexAgentAssignment`,
  `executeClaudeAgentAssignment`); **Claude Code is flagged as the
  next-sophistication step** and must be called out in code comments and the
  roadmap, but Codex ships first.

---

## 3. Phased roadmap

Each task lists scope, the seam it touches, and acceptance. Code is NOT written here
— this is the plan.

### P1 — OpenAuth account ↔ many Pylons + many keys (NOW, keystone)

**Owner decision: build this today, on the multi-link model.** The linking process
is anchored on the **OpenAuth account**, not just the agent bearer. Execution stays
single-user / own-capacity (firm invariant); the OpenAuth account is the
aggregation/linking anchor designed from the start for multi-Pylon / multi-key.

- **P1.1 OpenAuth-anchored link model (data model).**
  - Seam: `migrations/*` (new migration), `agent_credentials`, the Pylon registry
    in `pylon-api.ts`, `migrations/0002_auth_identity_and_agent_registration.sql`
    (`users`, `auth_identities`, `agent_credentials`).
  - Scope: add an `openauth_user_id` linkage so one human OpenAuth account ties
    together its many `agent_credentials` (keys) AND its many Pylon registrations
    (through `ownerAgentUserId`). Design for aggregation: one human → many keys →
    many agent-users → many Pylons. No cross-user pooling; the link is purely the
    human's own set.
  - Acceptance: a fixture proves one OpenAuth account resolves to its full set of
    linked keys and Pylons, and another account's keys/Pylons are never returned.
- **P1.2 Linking flow (endpoints), anchored on the OpenAuth account.**
  - Seam: a browser/OpenAuth-session-authenticated linking route (reuse
    `requireBrowserSession`, the OpenAuth-backed path already used at claim time in
    `agent-owner-claim-routes.ts`), plus the existing `pylon-api-routes.ts`
    registration path.
  - Scope: let a logged-in human link/unlink keys and Pylons to their OpenAuth
    account, building the link rows from P1.1. Linking is authorized by the OpenAuth
    browser session, not just the agent bearer.
  - Acceptance: linking endpoints create/read/revoke link rows under the OpenAuth
    session; a non-owner session cannot link or read another account's set.
- **P1.3 Own-capacity-only execution invariant, enforced.**
  - Seam: existing `registration.ownerAgentUserId !== session.user.id` guard in
    `pylon-api-routes.ts`; new dispatch path (P4).
  - Scope: linking many capacities under one human account must NOT let any caller
    use another caller's capacity. Dispatch stays scoped to the caller's own
    `ownerAgentUserId`; the OpenAuth aggregation never widens execution scope.
  - Acceptance: a test proves a request authenticated as account A can only ever
    target A's own linked Pylons, never B's, even when both are linked under
    different OpenAuth accounts.

### P1b — Unified web UI (linked Pylons, activity, balances, spend, traces) (FUTURE / incremental)

- **Note:** the full web UI is FUTURE/incremental, but the P1 link model MUST
  support it. When a user logs into openagents.com via OpenAuth they must
  (eventually) see ALL their linked Pylons, all activity, balances across all
  linked accounts, all token spend, what is running, and links to traces.
  - Seam: `apps/web/` Foldkit surfaces; reads over the P1 link model + P2 capacity
    projection + the token ledger; trace links into existing receipt/closeout
    surfaces.
  - Scope: incremental panels (linked Pylons list, per-Pylon capacity/availability,
    aggregated balances/spend, running assignments, trace links). Build behind the
    P1 link model so the aggregation is real, not faked.
  - Acceptance (when built): a logged-in human sees the aggregated set across all
    linked keys/Pylons; never another account's data.

### P2 — Pylon capacity discovery / reporting (NOW)

- **P2.1 Per-service capacity in heartbeat.**
  - Seam: `apps/pylon/src/presence.ts` (`PylonHeartbeatRequest`,
    `sendHeartbeat`), assembled in `apps/pylon/src/index.ts`.
  - Scope (owner decision: include ALL capacity dimensions): replace the static
    `capacityRefs` with per-service, count- and load-aware refs derived from the
    readiness probes and the concurrency the Pylon actually allows. Carry the full
    dimension set — **Codex×N / Claude×M, and busy / available / queued (and
    draining/offline where known)** — e.g.
    `capacity.coding.codex.ready=<n>`, `capacity.coding.claude.ready=<m>`,
    `load.coding.codex.busy=<k>`, `load.coding.codex.queued=<q>`,
    `load.coding.codex.available=<a>`. Bump heartbeat schema minor (`v0.4`).
  - Acceptance: heartbeat round-trips per-service ready/busy/available/queued
    counts for both Codex and Claude; absent service omitted; tests cover 0/1/N
    Codex and Claude and the busy/available/queued states.
- **P2.2 Server-side capacity projection.**
  - Seam: `pylon-api.ts` registry read; `oa-node-managed-machine.ts` projection.
  - Scope: persist/parse the new capacity refs so the registry can answer "for
    owner X, which Pylons expose which coding services, available now?" Feed the
    managed-machine `availability`/`supportedRuntimes`/`workloadClasses` from real
    heartbeat data instead of leaving it display-only.
  - Acceptance: a typed `listCallerCodingCapacity(ownerAgentUserId)` returns a
    bounded summary carrying all dimensions
    `{ codexReady, codexBusy, codexAvailable, codexQueued, claudeReady,
    claudeBusy, claudeAvailable, claudeQueued, pylonRefs }`, scoped to the caller's
    own agent-user (and, via P1, aggregatable across the caller's OpenAuth account).

### P3 — Router: resolve caller → check capacity → typed classify → decide (NOW / DEFAULT-ON)

- **P3a — Typed/semantic coding-workflow classifier (full version, owner decision).**
  - Seam: new `inference/coding-workflow-intent.ts`, mirroring
    `acceptance-spec.ts` shape; do NOT extend the keyword placeholder
    (`intentToAcceptanceSpec` is a self-described keyword placeholder — replace,
    don't grow it).
  - Scope: `classifyCodingWorkflow(request): CodingWorkflowClass` (bounded enum:
    `pull_request | bug_fix | refactor | none`). Owner decision: **do the full
    version if not too difficult** — a typed/semantic classifier per the workspace
    no-keyword rule. Explicit typed request field/header takes precedence (fully
    deterministic, allowed only *after* the semantic route is chosen); for free-form
    chat, use the central typed semantic / cosine-similarity embedding selector
    against a small labeled exemplar set, behind the one typed boundary. **No
    ad-hoc keyword/string matching on prose** anywhere.
  - Acceptance: unit tests; explicit-field cases deterministic; semantic-path cases
    behind the typed boundary with exemplar coverage; defaults to `none` when
    unparseable; a guard test asserts no raw keyword/string match on prose exists in
    the classifier.
- **P3b — Caller-capacity resolver + router branch (DEFAULT-ON).**
  - Seam: `model-router.ts selectAdapterPlan(model, callerCapacityHint?)` (or a
    post-plan branch in `chat-completions-routes.ts` after `accountRef` is known);
    `model-serving-policy.ts`.
  - Scope (owner decision: **default-ON, armed now** — not owner-gated-off): when
    `classifyCodingWorkflow != none` AND `listCallerCodingCapacity(accountRef)`
    shows an available own coding service, select the delegate branch instead of the
    normal model lane. The routing is live by default; any switch wired is an
    off-by-default *disable* (kill switch), never an off-by-default *enable* gate.
    Plain chat (`none`, or no own capacity) falls through to today's behavior
    unchanged (no regression).
  - Acceptance: with a coding workflow + available own capacity, the request takes
    the delegate branch **by default** (no flag flip); with no coding workflow or no
    capacity, routing is byte-identical to today; another user's capacity is never
    selectable; the caller resolves its capacity via P1's OpenAuth-account
    aggregation (own set only).

### P4 — Resumable-SSE coding-workflow execution via Durable Streams (NOW, Codex-first)

**Owner decision: execution rides our Durable Streams resumable-SSE model.** Every
coding-workflow request **starts by returning an interruptible, resumable SSE** —
if the client disconnects it does not matter; they can resume / come back later and
replay the suffix. This is the same model already implemented for durable
inference: the Durable Streams primitive `packages/durable-stream/`
(offset-addressed replay, `Stream-Next-Offset` / `Stream-Closed` resumability,
exactly-once writes, CDN-friendly fan-out) and the inference resume route
`apps/openagents.com/workers/api/src/inference/durable-inference-read-routes.ts`
(`GET /v1/chat/completions/durable/{requestId}?offset=<last-offset>`, which NEVER
meters — it replays stored bytes only). The coding delegation must produce a
durable, resumable stream of the same shape.

- **P4.1 Router-originated coding assignment over a durable stream.**
  - Seam: reuse `pylon-api.ts buildPylonApiAssignmentRecord` / `createAssignment`
    and the `codingAssignment` payload; today only `/api/operator/pylons/assignments`
    creates assignments — add an internal, server-side create path callable from the
    inference route, bound to the caller's own `ownerAgentUserId` and gated by the
    existing dispatch gate (`controlledPylonAssignmentDispatchGate`). Open a durable
    stream (`packages/durable-stream/`) for the assignment so progress/closeout
    frames are persisted to an offset log and the caller's initial SSE is
    interruptible/resumable from `Stream-Next-Offset`.
  - Scope (owner decision: **Codex first**): translate the classified coding request
    into a `codingAssignment` `codex` task payload (existing
    `openagents.pylon.codex_agent_task.v0.3` schema) targeting the caller's available
    Pylon. The Pylon picks it up via its existing `pollAssignments()` loop and runs
    `executeCodexAgentAssignment`. **Claude Code is the next-sophistication step:**
    `executeClaudeAgentAssignment` already exists; wire a `claudeAgent` payload path
    next, and flag this in code comments at the dispatch + executor seams so the
    Codex-first / Claude-next ordering is explicit. Reuse `codex-fleet` auth-lease
    and per-task `CODEX_HOME` / workspace-isolation patterns where the executor needs
    central auth.
  - Acceptance: an end-to-end fixture (no live spend) shows: coding request → initial
    resumable SSE returned → assignment created with caller's `ownerAgentUserId` →
    fixture Pylon executes Codex → closeout frames appended to the durable stream;
    a dropped client reconnects via the durable read route and replays the suffix;
    a second account cannot see, poll, or resume that assignment/stream.
- **P4.2 Results back through Khala (resumable, never blocks the turn).**
  - Seam: the durable stream + the chat-completions response / `OpenAgentsReceipt`
    + the durable read route.
  - Scope: the initial turn returns immediately with a resumable SSE handle (the
    durable `requestId` / assignment ref) — long-running PR work never blocks the
    HTTP turn. Progress and the terminal closeout (status, artifact refs, e.g. PR
    url) are appended to the durable stream and projected into a typed receipt block,
    so the caller can resume the stream or poll the handle and see the delegated
    result without leaving the Khala API.
  - Acceptance: the response/receipt carries the durable `requestId` + assignment
    ref + status; a client that disconnected mid-run resumes from its last offset and
    receives the terminal closeout; the durable read replay path never meters.

### P5 — Accounting, invariant enforcement, tests (NOW)

- **P5a — Count ALL Khala-orchestrated tokens on the public counter (owner
  decision, source-agnostic).** Record own-capacity coding tokens in
  `token_usage_events` with an honest internal `demand_kind` (own-capacity vs
  external vs hosted) and `demand_source` tag, **and include them in the public
  `khala-tokens-served` scalar** — they are orchestrated through Khala, so they
  count. Seam: `token-usage-ledger.ts`, `served-tokens-recorder.ts`,
  `public-khala-tokens-served-routes.ts`, `khala-tokens-served-sync.ts`.
  Acceptance: a test that an own-capacity coding completion **does** move the public
  counter, AND that its `demand_kind`/`demand_source` tag lets a private analytics
  read break it out by source. (Reverses the prior "counter does not move" test.)
- **P5b — Invariant enforcement + tests.**
  - **Own-capacity-only property test.** Model the bounded state (callers × Pylons ×
    assignments × OpenAuth links) and assert no assignment/stream is ever
    readable / pollable / executable / resumable by a non-owner, even across
    different OpenAuth accounts that each link many Pylons; convert any
    counterexample into a regression test (per workspace invariant discipline).
  - **No-resale reconciliation.** Add a test asserting the own-capacity path routes
    only to the caller's own `ownerAgentUserId` and never triggers
    `subscription_capacity_resale` authorization (reuse
    `inference-resale-authorization.ts`).
  - **Semantic-not-keyword coverage.** Assert the classifier (P3a) has no raw
    keyword/string match on prose and routes only through the typed/semantic
    boundary.
  - **Default-on coverage.** Assert the coding-workflow routing is live by default
    (no enable flag flip required), and that any wired switch is an off-by-default
    *disable* only.

---

## 4. Open questions for the owner — RESOLVED (2026-06-25)

1. **Identity granularity for NOW. RESOLVED → build OpenAuth linking NOW, multi-link
   model.** Not deferred. The **OpenAuth web-login account links MULTIPLE Pylons AND
   multiple agent/API keys**, anchored on the OpenAuth account, designed for
   multi-Pylon / multi-key aggregation from the start (P1). **Execution stays
   single-user / own-capacity** (a caller only uses their OWN linked capacity — firm
   invariant). The web UI for aggregation is FUTURE/incremental (P1b), but the link
   model + flow ship today.
2. **Workflow signal source. RESOLVED → full version.** Do the **full
   typed/semantic classifier** (per the workspace no-keyword rule) if not too
   difficult: explicit typed field/header takes precedence (deterministic), with a
   central semantic / cosine-embedding selector for free-form chat behind the one
   typed boundary. No keyword matching on prose (P3a).
3. **Sync vs async result contract. RESOLVED → resumable SSE via Durable Streams.**
   Every request starts by returning an **interruptible, resumable SSE** (our
   Durable Streams model, `packages/durable-stream/` +
   `durable-inference-read-routes.ts`). The turn never blocks: a dropped client
   resumes from its last offset and replays the suffix; long-running PR work returns
   a durable handle immediately (P4).
4. **Public counter policy. RESOLVED → counter counts ALL orchestrated tokens.**
   Khala is an orchestrator; every token it orchestrates counts on the public
   `khala-tokens-served` scalar, **source-agnostic** (local / crowd / swarm / cloud
   / subscription / own-capacity). Own-capacity coding tokens **DO** increment the
   public counter; keep an internal `demand_kind`/`demand_source` tag for honest
   analytics (P5a). (This reverses the earlier "keep them out" recommendation.)
5. **Arming default. RESOLVED → default-ON, armed now.** The coding-workflow routing
   is **live by default**, not owner-gated-off. Any switch wired is an
   off-by-default *disable* (kill switch) only, never an off-by-default *enable* gate
   (P3b).
6. **Codex vs Claude preference. RESOLVED → Codex for now.** Ship **Codex first**;
   **Claude Code is the next-sophistication step** (both executors exist —
   `executeCodexAgentAssignment` / `executeClaudeAgentAssignment` — flag the
   Codex-first / Claude-next ordering in code comments and ship Codex first) (P4).
7. **Capacity dimensions. RESOLVED → include ALL of them.** Heartbeat + projection
   carry Codex×N / Claude×M and busy / available / queued (+ draining/offline where
   known) (P2).

---

## 5. Sharpest findings (TL;DR)

1. **The execution pipeline already exists end to end.** Pylon already runs local
   Codex (`@openai/codex-sdk`) and Claude Code (`@anthropic-ai/claude-agent-sdk`)
   via assignment-leases carrying a `codingAssignment`, with executors and closeout.
   The server already builds/dispatches those assignments (operator-only today). This
   is a wiring job, not a greenfield build.
2. **Pylon↔identity is bearer-token today; owner decision builds OpenAuth multi-link
   NOW.** Claim-Your-Agent is X-verified and OpenAuth-approved *at claim time*, but
   the human OpenAuth account is dropped afterward; all Pylon ops use the agent
   bearer token bound to `ownerAgentUserId`. **Owner decision (P1): anchor linking on
   the OpenAuth account NOW** — one human OpenAuth account links MANY keys and MANY
   Pylons, designed for web-UI aggregation (linked Pylons, activity, balances, spend,
   traces; UI itself is FUTURE/incremental, P1b). **Execution stays single-user /
   own-capacity** (firm invariant): aggregation never widens execution scope; a
   caller only ever uses their own linked capacity, scoped at dispatch to their own
   `ownerAgentUserId`.
3. **The router is caller-blind and has no coding-workflow classifier.** Two real
   gaps: (a) `selectAdapterPlan(model)` never sees `accountRef`; (b) the only intent
   seam is a self-described keyword *placeholder* (`intentToAcceptanceSpec`). Both
   must be built; the classifier must be typed/semantic per the workspace rule.
4. **Capacity is reported as presence, not quantity/availability.** Heartbeat sends
   one static `capacityRefs` ref + capability presence; there is no Codex×N / Claude×M
   or busy/available signal, and the richer `oa-node-managed-machine` record is
   display-only. P2 (per-service capacity heartbeat + server projection) is the real
   net-new work, alongside P3's router branch and classifier.

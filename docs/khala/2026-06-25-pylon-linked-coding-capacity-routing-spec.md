# Audit + roadmap: Pylon-linked coding capacity routing

> Status: **audit + build roadmap**, 2026-06-25. Goal: route our own (and, later,
> any user's) day-to-day coding work **off the bare Codex / bare Claude Code CLIs
> and onto Khala**, by linking a user's **Pylon** (which already steers a local
> Codex / Claude Code install) to the **same API key** that user uses to hit the
> Khala API, so the router can delegate coding workflows to that user's OWN coding
> agents.
>
> **Not a product promise, not public-claim copy, not a direction to ship.** This
> is an audit of what exists and a phased plan. Every existing invariant holds:
> evidence-only, no keyword/string intent routing, no self-promotion, no exactness
> inflation, identity guard, INERT/OWNER-GATED settlement, one model
> (`openagents/khala`, no variants). Labels used below:
> **NOW** = firm scope for this phase; **FUTURE** = explicitly deferred /
> speculative; **OWNER-GATED** = needs owner arming before it can affect
> production behavior.
>
> Companion reading: the Pylon presence auth contract in
> `apps/openagents.com/AGENTS.md`; the no-resale gate in
> `apps/openagents.com/INVARIANTS.md` (Provider Capacity Marketplace Gate,
> lines ~981–1043); `apps/pylon/docs/2026-06-15-pylon-cli-only-agent-steerable-audit.md`;
> `docs/autopilot-coder/2026-06-13-cloud-remote-execution-commercial-plan.md`.

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
4. **Assignment dispatch is operator-only and identity-coarse.** Today only
   `/api/operator/pylons/assignments` creates an assignment, and the registry binds
   `ownerAgentUserId` to the *agent-user*, not to a human **OpenAuth** account. So
   "a user's own Pylon" works token-to-token, but there is no OpenAuth account that
   ties a human's many Pylons + many keys together. (This is fine for **NOW**:
   own-capacity-only can be enforced purely on the agent-user identity.)

So this is less "build a system" and more "wire two existing systems together
behind a typed, identity-scoped gate."

### Target architecture (text diagram)

```
  oa_agent_ API key  ──auth edge (chat-completions-routes.ts)──▶  accountRef = agent:<user_id>
        │                                                                  │
        │                                                                  ▼
        │                                          ┌──────────────────────────────────────┐
        │                                          │  Caller coding-capacity resolver (NEW) │
        │                                          │  "for this accountRef, what linked     │
        │                                          │   Pylons exist and what coding         │
        │                                          │   services do they expose, available?" │
        │                                          └──────────────────────────────────────┘
        │                                                  │ reads
        ▼                                                  ▼
  Khala router (model-router.ts)              pylon registry + heartbeat capacity
        │  selectAdapterPlan(model, callerCapacityHint?)   (capabilityRefs + NEW per-service capacityRefs)
        │                                                  │
        ▼                                                  │
  typed coding-workflow classifier (NEW)  ◀── request ─────┘
  CodingWorkflowClass = pr | fix | refactor | none   (bounded enum, deterministic parse — NOT keyword intent)
        │
        ▼  if workflowClass != none AND caller owns available coding capacity:
  delegate ──▶ create assignment-lease (codingAssignment: {codex|claudeAgent})
        │       bound to caller's OWN ownerAgentUserId (own-capacity-only invariant)
        ▼
  caller's OWN Pylon polls /assignments ──▶ executeCodexAgentAssignment / executeClaudeAgentAssignment
        │                                       (runs local Codex / Claude Code SDK in bounded workspace)
        ▼
  closeout + artifacts ──▶ results surface back through Khala response / receipt
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
bound to an **agent-user** (`ownerAgentUserId`). So **for NOW, an API key and a
Pylon can already share one identity** as long as they share the same agent-user —
i.e., the same bearer token (or two credentials minted for the same agent-user). The
missing convenience is a human **OpenAuth** account that ties together *multiple*
keys and *multiple* Pylons. That is **FUTURE**; not required for own-capacity-only.

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

### 1.8 Accounting + invariants — **own-capacity is allowed; counter is honest-but-coarse**

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
- **Public Khala counter.** `public-khala-tokens-served-routes.ts` exposes
  `GET /api/public/khala-tokens-served` = `SUM(input_tokens)+SUM(output_tokens)` over
  `token_usage_events` with **no filter** (`token-usage-ledger.ts`). The ledger DOES
  carry a `demand_kind` column (`internal`/`external`/`unlabeled`) and private reads
  can filter on it, but the public scalar intentionally includes internal dog-food.
  **Honest position for this feature:** coding work executed on the *caller's own*
  Codex/Claude is the caller's own capacity, not Khala-served inference. It should be
  recorded with `demand_kind='internal'` (or a new `own_capacity` label) so we never
  inflate the public "tokens served" number with capacity Khala did not actually
  serve. The completion bytes here come from the user's own provider account, not
  from a Khala supply lane — counting them as "Khala tokens served" would overstate
  external traction. **Recommendation: do NOT add own-capacity coding tokens to the
  public counter; record them in the ledger with an internal/own-capacity
  demand_kind only.**

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
- **Public-counter honesty.** Own-capacity coding tokens are not Khala-served
  inference; they must not increment the public `khala-tokens-served` scalar. Record
  them internally with an internal/own-capacity `demand_kind`.

---

## 3. Phased roadmap

Each task lists scope, the seam it touches, and acceptance. Code is NOT written here
— this is the plan.

### P1 — Link a Pylon to an API key via one identity (NOW)

- **P1.1 Confirm/establish single-agent-user identity for key + Pylon.**
  - Seam: `agent_credentials`, `pylon-api-routes.ts requireAgent`,
    `inference-free-tier-key.ts` account_ref.
  - Scope: document and test that an `oa_agent_` key used at
    `/api/v1/chat/completions` and a Pylon registered with the *same* token (or a
    second credential minted for the same agent-user) resolve to the same
    `accountRef`/`ownerAgentUserId`. No schema change needed for NOW.
  - Acceptance: a fixture proves `authenticate()` → `agent:<user_id>` equals the
    `ownerAgentUserId` on that user's Pylon registration.
- **P1.2 (FUTURE, note only) OpenAuth account → many keys/Pylons.** Spec a
  `openauth_user_id` column on `agent_credentials` + a registry join so one human
  account ties multiple keys and Pylons. Deferred; record as an open item.

### P2 — Pylon capacity discovery / reporting (NOW)

- **P2.1 Per-service capacity in heartbeat.**
  - Seam: `apps/pylon/src/presence.ts` (`PylonHeartbeatRequest`,
    `sendHeartbeat`), assembled in `apps/pylon/src/index.ts`.
  - Scope: replace the static `capacityRefs` with per-service, count- and
    load-aware refs derived from the readiness probes and the concurrency the Pylon
    actually allows, e.g.
    `capacity.coding.codex.ready=<n>`, `capacity.coding.claude.ready=<m>`,
    `load.coding.codex.busy=<k>`. Bump heartbeat schema minor (`v0.4`).
  - Acceptance: heartbeat round-trips per-service ready/busy counts; absent service
    omitted; tests cover 0/1/N Codex and Claude.
- **P2.2 Server-side capacity projection.**
  - Seam: `pylon-api.ts` registry read; `oa-node-managed-machine.ts` projection.
  - Scope: persist/parse the new capacity refs so the registry can answer "for
    owner X, which Pylons expose which coding services, available now?" Feed the
    managed-machine `availability`/`supportedRuntimes`/`workloadClasses` from real
    heartbeat data instead of leaving it display-only.
  - Acceptance: a typed `listCallerCodingCapacity(ownerAgentUserId)` returns a
    bounded summary `{ codexAvailable, claudeAvailable, pylonRefs }`.

### P3 — Router: resolve caller → check capacity → typed classify → decide (NOW / OWNER-GATED)

- **P3.1 Typed coding-workflow classifier.**
  - Seam: new `inference/coding-workflow-intent.ts`, mirroring
    `acceptance-spec.ts` shape; do NOT extend the keyword placeholder.
  - Scope: `classifyCodingWorkflow(request): CodingWorkflowClass` (bounded enum).
    Prefer an explicit typed request field/header; fall back to the central semantic
    selector for free-form. No keyword matching on prose.
  - Acceptance: unit tests; explicit-field cases deterministic; semantic-path cases
    behind the typed boundary; defaults to `none` when unparseable.
- **P3.2 Caller-capacity resolver + router branch.**
  - Seam: `model-router.ts selectAdapterPlan(model, callerCapacityHint?)` (or a
    post-plan branch in `chat-completions-routes.ts` after `accountRef` is known);
    `model-serving-policy.ts` arming flag `CALLER_OWNED_CODING_CAPACITY_ENABLED`
    (OWNER-GATED).
  - Scope: when arming flag is on AND `classifyCodingWorkflow != none` AND
    `listCallerCodingCapacity(accountRef)` shows an available own coding service,
    select the delegate branch instead of the normal model lane. Otherwise fall
    through to today's behavior unchanged (no regression for plain chat).
  - Acceptance: with flag off, byte-identical routing to today; with flag on and
    capacity present, the request takes the delegate branch; with flag on and no
    capacity, normal fallback; another user's capacity is never selectable.

### P4 — Execution path: delegate to the caller's own agent, return results (NOW)

- **P4.1 Router-originated coding assignment.**
  - Seam: reuse `pylon-api.ts buildPylonApiAssignmentRecord` / `createAssignment`
    and the `codingAssignment` payload; today only `/api/operator/pylons/assignments`
    creates assignments — add an internal, server-side create path callable from the
    inference route, bound to the caller's own `ownerAgentUserId` and gated by the
    existing dispatch gate (`controlledPylonAssignmentDispatchGate`).
  - Scope: translate the classified coding request into a `codingAssignment`
    (`codex` or `claudeAgent` task payload, existing v0.3 schemas) targeting the
    caller's available Pylon. The Pylon picks it up via its existing
    `pollAssignments()` loop and runs `executeCodexAgentAssignment` /
    `executeClaudeAgentAssignment`. Reuse `codex-fleet` auth-lease and per-task
    `CODEX_HOME`/workspace-isolation patterns where the executor needs central auth.
  - Acceptance: an end-to-end fixture (no live spend) shows: coding request →
    assignment created with caller's `ownerAgentUserId` → fixture Pylon executes →
    closeout returns; a second account cannot see or poll that assignment.
- **P4.2 Results back through Khala.**
  - Seam: assignment closeout/artifacts path + the chat-completions response /
    `OpenAgentsReceipt`.
  - Scope: project the closeout (status, artifact refs, e.g. PR url) back into the
    Khala response or a typed receipt block so the caller sees the delegated result
    without leaving the Khala API. For long-running PR work, define an async handle
    (the assignment ref) rather than blocking the HTTP turn.
  - Acceptance: response/receipt carries the assignment ref + status; polling
    surfaces terminal closeout.

### P5 — Accounting, invariant enforcement, tests (NOW)

- **P5.1 Demand-kind labeling.** Record own-capacity coding tokens in
  `token_usage_events` with an internal/own-capacity `demand_kind`; assert they are
  excluded from the public `khala-tokens-served` scalar. Acceptance: a test that the
  public counter does not move for an own-capacity coding completion.
- **P5.2 No-resale guard alignment.** Add a test asserting the own-capacity path
  routes only to the caller's own `ownerAgentUserId` and never triggers
  `subscription_capacity_resale` authorization (reuse
  `inference-resale-authorization.ts`).
- **P5.3 Own-capacity-only property test.** Model the bounded state (callers ×
  Pylons × assignments) and assert no assignment is ever readable/pollable/executable
  by a non-owner; convert any counterexample into a regression test (per workspace
  invariant discipline).

---

## 4. Open questions for the owner

1. **Identity granularity for NOW.** Are we content that "a user's own Pylon" means
   *same agent-user / same key (or sibling credentials of one agent-user)* for this
   phase, deferring the OpenAuth-account-ties-many-keys work (P1.2) to FUTURE? Or do
   you want the OpenAuth linking built first?
2. **Workflow signal source.** For P3.1, do you want coding-workflow detection driven
   by an **explicit typed field/header** the client sets (cleanest, fully
   deterministic), the **central semantic/embedding selector** for free-form chat, or
   both with the field taking precedence?
3. **Sync vs async result contract.** A "do this PR" task is long-running. Should the
   Khala turn return immediately with an assignment handle (async) by default, or do
   you want a bounded synchronous wait for short tasks with async fallback?
4. **Public counter policy.** Confirm own-capacity coding tokens stay **out** of the
   public `khala-tokens-served` scalar (recorded internal-only). This is the
   honest-accounting recommendation; please confirm before P5.1.
5. **Arming default.** P3.2 is OWNER-GATED behind `CALLER_OWNED_CODING_CAPACITY_ENABLED`.
   Ship it default-off (flag flip arms it) — confirm.
6. **Codex vs Claude preference.** When a caller exposes both, what selects between
   them — a caller preference field, model id, round-robin, or least-busy?

---

## 5. Sharpest findings (TL;DR)

1. **The execution pipeline already exists end to end.** Pylon already runs local
   Codex (`@openai/codex-sdk`) and Claude Code (`@anthropic-ai/claude-agent-sdk`)
   via assignment-leases carrying a `codingAssignment`, with executors and closeout.
   The server already builds/dispatches those assignments (operator-only today). This
   is a wiring job, not a greenfield build.
2. **Pylon↔identity is bearer-token, not OpenAuth, at runtime.** Claim-Your-Agent is
   X-verified and OpenAuth-approved *at claim time*, but the human OpenAuth account is
   dropped afterward; all Pylon ops use the agent bearer token bound to
   `ownerAgentUserId`. Good news: an `oa_agent_` API key resolves to the **same**
   agent-user, so key↔Pylon linking already works on one identity — own-capacity-only
   is enforceable today with zero schema change.
3. **The router is caller-blind and has no coding-workflow classifier.** Two real
   gaps: (a) `selectAdapterPlan(model)` never sees `accountRef`; (b) the only intent
   seam is a self-described keyword *placeholder* (`intentToAcceptanceSpec`). Both
   must be built; the classifier must be typed/semantic per the workspace rule.
4. **Capacity is reported as presence, not quantity/availability.** Heartbeat sends
   one static `capacityRefs` ref + capability presence; there is no Codex×N / Claude×M
   or busy/available signal, and the richer `oa-node-managed-machine` record is
   display-only. P2 (per-service capacity heartbeat + server projection) is the real
   net-new work, alongside P3's router branch and classifier.

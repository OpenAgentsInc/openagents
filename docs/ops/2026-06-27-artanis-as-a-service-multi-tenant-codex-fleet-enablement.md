# Artanis as a Service — Multi-Tenant Codex + Claude Fleet Enablement

**Date:** 2026-06-27
**Scope:** Product + ops architecture to let OTHER Khala users connect their own
Codex and Claude Agent accounts and have a per-user Artanis orchestrate backlog burndown
across their own fleet — across every Khala surface (CLI, desktop/mobile app,
website, REST/OpenAI-compatible API).
**Status:** Gap analysis / proposal. Nothing here ships state; it enumerates what
EXISTS today vs what is NEEDED, verified against the repo on 2026-06-27.

## Vision

We proved internally that one operator can stand up N isolated Codex logins, point
a single standing Pylon and an auto-scaling supervisor at them, and let Artanis
decide the task pool — burning own-capacity Codex tokens against a real backlog at
multi-million-tokens/min with no spend and no resale. The same Pylon delegation
shape now admits the Claude Agent lane for work that benefits from a multi-file
refactor/debugging executor. "Artanis as a Service" is that loop, generalized to
any signed-in Khala user: **bring your own Codex and Claude accounts, link your
own Pylons, point a per-user Artanis at your own repos/issues, and let it route
backlog burndown across your fleet.** The user pays OpenAI/Anthropic directly for
their own capacity; OpenAgents monetizes the *orchestration* layer (the
supervisor + Artanis + the fleet dashboard), never the resale of provider
capacity. Everything is strictly per-tenant isolated.

## The proven baseline (owner-only, working today)

The owner flow runs at ~2.3M tokens/min across 6 Codex accounts, own-capacity /
no-spend. Full operational detail is in the burn runbook
[`2026-06-27-khala-codex-own-capacity-burn-runbook.md`](./2026-06-27-khala-codex-own-capacity-burn-runbook.md);
the protocol/proof contract is the **"Khala -> Pylon -> Codex Coding Delegation
Runbook"** in [`../../CLAUDE.md`](../../CLAUDE.md) and the invariant ledger
`apps/openagents.com/INVARIANTS.md` ("Khala Coding Delegation Through Pylons").

1. `pylon auth codex --account codex-N` connects N Codex accounts in isolated
   per-account homes (`<pylon home>/accounts/codex/<ref>`) — **never `~/.codex`**.
   Claude Agent accounts use the same account-registry/provider split
   (`provider: "claude_agent"`) with isolated Claude homes and public-safe
   `account.pylon.claude_agent.<hash>` refs.
2. A standing Pylon advertises Codex and/or Claude Agent capacity
   (`provider go-online`) and runs leased assignments (`assignment run-no-spend`) via launchd
   `com.openagents.pylon.fable`.
3. The codex-supervisor (`apps/pylon/scripts/codex-supervisor/{launch.sh,codex-supervisor.sh}`)
   fires `khala request --workflow codex_agent_task` round-robin across accounts,
   auto-scaling to `min(SUP_MAX_SLOTS, ready × SUP_PER_ACCOUNT)`, self-throttling
   on 409/429, with an owner-session tripwire.
4. Claude Agent requests use the same typed Khala request seam with
   `--workflow claude_agent_task`; the dispatch gate requires
   `capability.pylon.local_claude_agent`, Claude capacity refs, and a
   `claude_agent` account hash when pinned.
5. Artanis (the owner's operator agent) decides the task pool / division of labor;
   reachable at `POST /api/operator/artanis/chat`, currently OWNER-gated.

## At a glance — EXISTS vs NEEDED

| # | Capability | EXISTS today (verified) | NEEDED for AaaS | Owning surface |
|---|---|---|---|---|
| 1 | Per-user identity & Pylon linking | Owner/caller-owned scope: token → OpenAuth account → linked Pylons; gate is "caller-owned pylon" | Generalize "caller-owned" to ANY signed-in user; remove owner-hardcoded admits | Worker API (`coding-workflow-delegation.ts`, `artanis-owner-authority.ts`, `index.ts:isOpenAgentsAdminEmail`) |
| 2 | Connect-your-fleet UX | `pylon auth codex --account`, Claude `claude_agent` account registry support, `pylon accounts list/usage`; khala CLI reads pylon account homes | Productized list/add/remove/readiness across CLI + app + web for Codex and Claude Agent; on-device security copy | `apps/pylon` CLI, `clients/khala-cli`, web/app |
| 3 | Per-account + per-tenant dispatch | Dispatch gate is caller-owned and service-aware for Codex/Claude; gate-fix #6354 and Claude lane #6388/#6391/#6421 pieces landed | Per-account gate keying for each service at scale; strict per-tenant isolation | Worker API (`inference/coding-workflow-delegation.ts`, `khala-pylon-admission.ts`) |
| 4 | Turnkey supervisor/runner | Owner Codex shell scripts w/ hardcoded `SUP_PYLON_REF`/`SUP_REPO`/`SUP_ISSUES` defaults | First-class `khala fleet` command set (or managed runner): user's token, auto-resolved pylon ref, user's backlog, Codex/Claude division-of-labor policy | `clients/khala-cli` (+ optional managed runner) |
| 5 | Per-user Artanis | Single owner-promoted identity; memory/awareness already keyed `owner:<userId>` | Per-tenant instance/scoping, per-user memory + awareness + approval gates + authority bounds | Worker API (`artanis-*.ts`) |
| 6 | Khala surfaces | CLI `/artanis` owner channel; `POST /api/operator/artanis/chat`; mobile `KhalaArtanis.swift`; `/api/v1/chat/completions` | `khala fleet`, web fleet dashboard, per-user operator/fleet API generalizing the chat route | CLI + web/app + Worker API |
| 7 | Metering & business model | `token_usage_events` exact rows keyed `actor_user_id`; own-capacity/no-spend | Per-tenant metering of orchestration; subscription/metered billing decision | Worker API + billing (owner-gated) |
| 8 | Security & isolation | Isolated per-account homes; caller-owned scope; owner-session tripwire | Generalize tripwire per-user; enforce per-tenant scope on every read/dispatch/trace | All surfaces (hard invariants) |
| 9 | Reliability gaps | Known + documented in burn runbook §5/§9; empty-reply fallback (#6359) | Design these in for tenants, not rediscover | CLI + Worker + ops |
| 10 | Phased rollout | — | Phase 1 CLI BYO → Phase 2 web + per-user Artanis → Phase 3 self-serve + billing | Product |
| 11 | Open decisions | — | Pricing, tenant autonomy default, support burden, ToS/abuse, additional providers beyond Codex/Claude Agent | Owner |

---

## 1. Per-user identity & Pylon linking

**EXISTS.** The authorization boundary today is *token-resolved owner scope*, not
"any user". `inference/coding-workflow-delegation.ts` admits work only for the
**caller-owned** pylon: a firing token must resolve to the OpenAuth account that
owns/links the target pylon, else a typed `403`
(`evidence.khala_coding.target_pylon_ref.not_linked`, "requested Pylon is not
linked to this OpenAuth account"). This is already a *per-account* scope model —
the CLAUDE.md runbook calls it out explicitly ("The authorization boundary is the
token-resolved owner scope. A remote issuer must only read and target Pylons
linked to that same owner scope").

The single-tenant hardcoding lives in two places:

- `apps/openagents.com/workers/api/src/index.ts:1790` —
  `isOpenAgentsAdminEmail()` checks against `OPENAGENTS_ADMIN_EMAILS`.
- `apps/openagents.com/workers/api/src/artanis-owner-authority.ts` — the owner
  promotion: `ARTANIS_OWNER_OPENAUTH_USER_ID = user_ed6d486e-…`,
  `OPENAGENTS_OWNER_AGENT_OPENAUTH_USER_IDS`, and the **standing owner approval**
  for `pylon_job_dispatch` (so the gated `dispatch_codex_task` tool executes for
  the owner without an armed approval-gate row).

**NEEDED.** The own-capacity coding seam is *already* per-owner-scope safe; the
gap is the **admit set** and the **standing-approval grant**, which are
owner-only. For AaaS:

- Generalize "caller-owned pylon" admission so any authenticated user's token can
  link and target THEIR pylons (the gate logic is reusable; the owner-email /
  owner-agent allowlist is what must not be the precondition for *self-service*
  dispatch).
- Replace the single standing owner approval with a **per-tenant** default
  approval policy for the tenant's own `pylon_job_dispatch` (own-capacity,
  no-spend, own pylons only). The never-waivable bounds in
  `artanis-owner-authority.ts` (no `wallet_spend`/`settlement`/`l402_redemption`
  self-approval, no resale on SUBSCRIPTION, no credential leak, public-safe
  claims) must carry over to every tenant unchanged.

> Could not verify: no in-repo "per-user dispatch admit" generalization exists on
> `origin/main` today — the admit set is still owner/admin-keyed. This is net-new.

## 2. Connect-your-fleet UX

**EXISTS.** The multi-account device-login flow is real:

- `pylon auth codex --account codex-N` (isolated per-account home), with usage
  strings in `apps/pylon/src/index.ts` (`pylon accounts connect codex --account
  <ref> [--home <path>] …`, `pylon accounts list|usage|connect …`,
  `pylon accounts usage --account <ref> --refresh --json`).
- Claude Agent accounts are represented by the same account registry using
  `provider: "claude_agent"` and public-safe hashes shaped
  `account.pylon.claude_agent.<hex>`; request dispatch and assignment execution
  select the Claude provider when the workflow is `claude_agent_task`.
- The khala CLI (`clients/khala-cli`, package `@openagentsinc/khala`) already
  discovers pylon Codex account homes (`codex.ts:pylonCodexAccountHomes()`,
  reading `dev.accounts` from pylon config) and has `auth codex` / `codex`
  subcommands (`cli.ts`).
- Credentials stay on the user's device: per-account auth.json under
  `<pylon home>/accounts/<provider>/<ref>`; the Pylon is local. The
  **NEVER `codex login` against `~/.codex`** invariant remains documented in
  CLAUDE.md and burn-runbook §9, and the same "device credentials stay local"
  boundary applies to Claude homes/tokens.

**NEEDED.** A first-class, productized "connect your fleet" experience:

- CLI/app: `list` (with readiness state + `capability.pylon.local_codex` and/or
  `capability.pylon.local_claude_agent`), `add`, `remove`, and `status` —
  surfaced as a fleet view, not raw pylon plumbing.
- A clear on-device security model in the UX copy: "your provider credentials never
  leave your machine; OpenAgents orchestrates, your local Pylon executes."
- Web/app parity: a connected-accounts panel that reflects the same local pylon
  inventory (web cannot hold device creds, so this reads the local pylon's
  public-safe account list, not the secrets).

## 3. Per-account + per-tenant dispatch

**EXISTS.** The dispatch-gate capability bug is fixed (gate-fix #6354):
- `982c33f521` — heartbeat refreshes pylon capability refs so a just-linked Codex
  dispatches (server).
- `1cc0e9ba03` — pylon runtime state load must not strip the dynamically-probed
  codex/claude capability (`apps/pylon/src/state.ts` `loadOrCreateRuntimeState`).

The dispatch gate in `inference/coding-workflow-delegation.ts` admits only
caller-owned Pylons that advertise the requested service capability/capacity
(`codex_agent_task`/`cloud_coding_session` for Codex,
`claude_agent_task` for Claude Agent).

**NEEDED — two distinct things:**

1. **Per-account division of labor.** The gate is **pylon-level, not
   just service-level** when an account hash is supplied: a Codex hash must only
   consume Codex capacity for that account, and a Claude hash must only consume
   Claude capacity for that account. Keep this as a release gate while scaling
   supervisors, because unpinned requests still fall back to pooled service
   capacity.
2. **Strict per-tenant isolation.** One tenant's accounts/pylons/assignments/
   traces must never be visible or usable by another. The caller-owned scope
   (§1) gives the read/dispatch boundary; AaaS must additionally guarantee no
   cross-tenant *capacity borrowing* — a tenant's advertised slots serve only
   that tenant's requests.

## 4. The supervisor as a per-user managed/turnkey runner

**EXISTS.** `apps/pylon/scripts/codex-supervisor/{launch.sh,codex-supervisor.sh}`
(now committed on `origin/main`). It auto-scales, heartbeats, round-robins across
`--account-ref`, self-throttles, and has the owner-session tripwire. But it is
**owner shell tooling with single-tenant defaults** (verified in
`codex-supervisor.sh`):

- `SUP_PYLON_REF` defaults to the stale `pylon.33afd48282a649047e3a` (must be
  overridden to the live ref).
- `SUP_REPO` defaults to `OpenAgentsInc/openagents`; `SUP_ISSUES` defaults to
  OpenAgents' own backlog (`6310 6311 6320 6354 6355 6358`).
- Requires `OPENAGENTS_AGENT_TOKEN` (FATAL if unset) — the owner-linked Artanis
  token.

**NEEDED.** A first-class `khala fleet` command set (or a managed runner) any user
runs with THEIR token, THEIR pylon ref (auto-resolved via `provider go-online
--json`, not a hardcoded default), and THEIR backlog (their repos/issues — never
OpenAgents'):

- `khala fleet connect` (link/verify Codex and Claude Agent accounts — §2), `khala fleet status`
  (live concurrency, per-account readiness, burn), `khala fleet run` (start the
  per-user auto-scaling pool against the tenant's backlog).
- Per-user auto-scale + tripwire: the same `desired = min(MAX, ready ×
  per_account)` math and the same "reauthenticate → GLOBAL-PAUSE + NEEDS-OWNER"
  behavior, but scoped to the tenant and writing to a tenant-visible notice, not
  the owner's `NEEDS_OWNER.md`.
- Optionally a **managed runner** (OpenAgents-hosted control loop firing against
  the tenant's local pylon) so users who don't want a long-lived local script
  still get 24/7 burndown.

## 5. Per-user Artanis

**EXISTS (single-tenant).** Artanis is one owner-promoted identity:

- `artanis-owner-memory.ts` + migration `0245_artanis_owner_memory.sql` — owner
  memory. The chat route already keys it `owner:<session.user.userId>`
  (`artanis-operator-chat-routes.ts:ownerIdForSession`) and loads via
  `loadArtanisMemory(store, ownerId, …)`.
- `artanis-situational-awareness.ts` — `buildArtanisSituationalAwareness(ownerId,
  readers)`, already owner-scoped by argument.
- `artanis-approval-gates.ts` — risky-action approval gates.
- `artanis-owner-authority.ts` — owner promotion + standing dispatch approval
  (§1).
- Empty-reply robustness: `ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK` +
  the `#6359` "never return an empty reply" guard in `artanis-operator.ts`
  (~line 878). Carry this into every tenant instance.

**NEEDED for multi-tenant Artanis:**

- Per-user instance/scoping. The good news: memory and awareness are *already*
  parameterized on `ownerId` — the abstraction exists. The blocker is the
  **admit gate** (`requireAdminSession` in `artanis-operator-chat-routes.ts`
  admits admin API token → hardcoded `chris@openagents.com`, owner-agent bearer,
  or admin-email browser session). For AaaS, any authenticated user must reach
  THEIR Artanis, scoped to `owner:<their userId>`.
- Per-user memory + situational awareness over THEIR fleet/backlog (the readers
  passed to `buildArtanisSituationalAwareness` must be tenant-scoped).
- Per-user approval gates + authority bounds: a tenant Artanis self-approves only
  its own own-capacity no-spend dispatch; the never-waivable money-movement gates
  stay gated for every tenant.
- The operator chat surface per-user across CLI/app/web/API (§6).

## 6. Khala surfaces

- **CLI** (`clients/khala-cli`): EXISTS — an `/artanis` owner-only operator
  channel (`#6363`, epic `#6359`) that talks to `POST /api/operator/artanis/chat`,
  plus `auth codex` / `codex` / `tokens` / `workers`. NEEDED —
  `khala fleet connect|status|run` (§4) and a non-owner-gated `khala artanis`
  chat once §5 lands.
- **Website/app**: NEEDED — a fleet dashboard (connected accounts, live
  concurrency, token burn, task pool, Artanis chat). Mobile already has
  `clients/mobile/Khala/Khala/Net/KhalaArtanis.swift` as an Artanis client seam.
- **REST/OpenAI-compatible API**: EXISTS — `POST /api/v1/chat/completions`
  (OpenAI-compatible, model `openagents/khala`, Bearer key auth, daily-quota /
  balance gate; see `index.ts:6876`, `config.ts`). NEEDED — a per-user
  operator/fleet API generalizing `/api/operator/artanis/chat` (today admin-gated)
  so tenants can drive fleet + Artanis programmatically with their own key.

## 7. Metering & business model

This is **BYO-Codex own-capacity**: the user pays OpenAI directly, so we do **not**
resell their Codex. The monetized layer is **orchestration** (supervisor +
Artanis + dashboard) — subscription and/or metered orchestration.

- EXISTS: `token_usage_events` records exact rows for the coding seam
  (`provider='pylon-codex-own-capacity'`, `model='openagents/pylon-codex'`,
  `usage_truth='exact'`, `demand_kind='own_capacity'`,
  `demand_source='khala_coding_delegation'`), keyed `actor_user_id` to the linked
  OpenAuth account (CLAUDE.md §6). That is the natural per-tenant meter.
- NEEDED: a per-tenant orchestration meter/plan (e.g. priced on orchestrated
  tokens or active fleet-hours), distinct from the own-capacity token rows which
  remain own-capacity/no-spend/no-payout.
- Policy reconciliation: the **no-resale-on-SUBSCRIPTION** invariant
  (`artanis-owner-authority.ts`) is satisfied — we charge for orchestration, never
  resell the tenant's Codex. (API-inference resale is a separate allowed path and
  not what AaaS does.)

> Owner-gated: the actual pricing/billing model is an owner decision (see §11).

## 8. Security & isolation invariants (hard requirements)

These are non-negotiable for every tenant:

- **Credentials stay on the user's device.** Codex auth.json and Claude account
  tokens live only in the local pylon's per-account homes; the Worker never
  receives raw provider credentials.
- **Never `codex login` / `pylon auth codex` against `~/.codex`** — it wipes a
  live session. Always isolated per-account homes (CLAUDE.md, burn-runbook §9).
- **Strict per-tenant scope on every read, dispatch, and trace.** Built on the
  caller-owned token scope (§1); extended so no tenant sees another's pylons,
  accounts, assignments, `token_usage_events`, or owner-only `agent_traces`.
- **No cross-tenant capacity borrowing** — advertised slots serve only their
  owner's requests.
- **Owner-session tripwire generalized to per-user** — a broken local provider
  session GLOBAL-PAUSEs that tenant's pool and notifies that tenant; it must
  never run provider login flows and never affect other tenants.
- Public-safe claims only; never-waivable money-movement gates stay gated for
  every tenant.

## 9. Reliability gaps to design in (not rediscover)

From the burn runbook §5/§9 — bake these into the tenant runner/UX so users never
hit them blind:

- **`go-online` vs bare `presence heartbeat`** — older code's bare heartbeat
  advertises `codex available=0`; the runner must use `go-online` or current-code
  heartbeat (post-#6354).
- **Heartbeat-wedge** — `presence heartbeat` can hang and stall a loop; any
  heartbeat loop must background + `timeout`.
- **Stale `SUP_PYLON_REF` default** — auto-resolve the live ref at launch
  (`provider go-online --json`); never trust the hardcoded default.
- **Cloudflare urllib-UA edge block** — `Python-urllib/*` is hard-blocked at the
  edge for `/api/v1/*` (looks like "fleet down"); the tenant SDK/client must set
  a non-urllib User-Agent. (Full WAF carve-out is owner-gated.)
- **Over-spawn 409-thrash** — right-size requesters to advertised concurrency;
  the supervisor self-throttles via backoff, hand-rolled loops do not.
- **GLM tool-calling broken / paid-fallback** — see
  `docs/inference/2026-06-25-glm-fleet-max-throughput-stress-and-artanis-overseer.md`;
  Artanis's own reasoning/tool model choice must not silently fall back to a paid
  path for tenants.
- **Empty-reply** — keep the `#6359` empty-reply fallback in every tenant Artanis.

## 10. Phased rollout proposal

**Phase 1 — CLI-only BYO Codex/Claude fleet for invited power users.**
- Prereqs: §1 generalize caller-owned dispatch to non-owner authenticated users +
  per-tenant default dispatch approval; §2 `khala fleet connect`; §4 `khala fleet
  run` with auto-resolved pylon ref and tenant-supplied repo/issues; §8
  per-tenant scope + per-user tripwire; §9 reliability defaults baked in.
- Owner-reviewed: invite-gated, manual onboarding, owner watches the first
  tenants' burn/traces.

**Phase 2 — Web fleet dashboard + per-user Artanis chat.**
- Prereqs: §5 per-user Artanis admit (lift the admin-only `requireAdminSession`
  gate on the chat route to authenticated-user-scoped); §6 web dashboard
  (accounts, concurrency, burn, task pool, chat); per-user operator/fleet API.

**Phase 3 — Full self-serve managed service + billing.**
- Prereqs: §3 per-account gate proof for real fan-out; §4 optional managed
  runner; §7 per-tenant orchestration meter + chosen billing model; §11 owner
  decisions resolved (pricing, autonomy default, ToS/abuse).

## 11. Open decisions for the owner

- **Pricing/billing model** — subscription vs metered orchestration vs hybrid;
  what unit (orchestrated tokens, fleet-hours, seats).
- **Default tenant Artanis autonomy** — how much a tenant's Artanis decides/acts
  before requiring tenant approval (the owner gets standing approval today; what's
  the tenant default?).
- **Support burden of users' local Pylons** — managed runner vs pure self-host;
  how much we debug a tenant's local environment.
- **ToS / abuse** — guard against someone connecting provider accounts purely to
  farm tokens or violate provider terms; rate/oversight policy.
- **Additional providers in scope?** — Codex and Claude Agent are the first two
  local coding lanes; decide whether day-one AaaS includes any other adapters.

---

## Appendix — verification status

Verified against `origin/main` on 2026-06-27:

- Caller-owned dispatch + typed 403/409 refusals:
  `apps/openagents.com/workers/api/src/inference/coding-workflow-delegation.ts`
  (`hasAvailableCodexCapacity` ~line 299), admission refs in
  `inference/khala-pylon-admission.ts`.
- Owner hardcoding: `index.ts:1790` `isOpenAgentsAdminEmail`;
  `artanis-owner-authority.ts` (owner agent ids + standing `pylon_job_dispatch`
  approval + never-waivable bounds).
- Artanis chat route admit gate + owner-scoped memory/awareness:
  `artanis-operator-chat-routes.ts` (`requireAdminSession`, `ownerIdForSession`,
  `buildArtanisSituationalAwareness`, `loadArtanisMemory`).
- Per-user Artanis substrate already `ownerId`-keyed: `artanis-owner-memory.ts`
  (migration `0245`), `artanis-situational-awareness.ts`,
  `artanis-approval-gates.ts`; empty-reply guard in `artanis-operator.ts`
  (`ARTANIS_OPERATOR_EMPTY_REPLY_FALLBACK`, ~line 878, #6359).
- Supervisor single-tenant defaults: `apps/pylon/scripts/codex-supervisor/`
  (now committed) — `SUP_PYLON_REF=pylon.33afd48282a649047e3a`,
  `SUP_REPO=OpenAgentsInc/openagents`, `SUP_ISSUES=6310 6311 6320 6354 6355 6358`,
  FATAL-if-unset `OPENAGENTS_AGENT_TOKEN`.
- CLI: `clients/khala-cli` (`@openagentsinc/khala`) `/artanis` owner channel
  (#6363), `auth codex` / `codex` subcommands, `codex.ts:pylonCodexAccountHomes`.
- Pylon CLI: `apps/pylon/src/index.ts` `accounts connect codex --account`,
  `accounts list|usage`, and `khala request --workflow claude_agent_task`.
- OpenAI-compatible API: `/api/v1/chat/completions` (`index.ts:6876`,
  `config.ts`).
- Mobile Artanis seam: `clients/mobile/Khala/Khala/Net/KhalaArtanis.swift`.
- Gate-fix commits `982c33f521` + `1cc0e9ba03` (#6354).

Could **not** verify (flagged inline):

- No per-user/non-owner dispatch admit generalization exists on `origin/main` —
  net-new (§1).
- Pricing/billing and tenant-autonomy defaults are owner decisions, not in repo
  (§7, §11).

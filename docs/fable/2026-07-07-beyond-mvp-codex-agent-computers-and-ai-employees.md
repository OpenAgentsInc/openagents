# Beyond the Mobile MVP — Codex on Agent Computers, Multi-Agent Management from the Phone, and the Road to AI Employees

Date: 2026-07-07
Status: owner-directed post-MVP direction doc. The mobile-only MVP
(epic #8467, `docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md`)
and the Agent Computers strategy
(`docs/khala-code/2026-07-06-agent-computers-strategy.md`) are the base layer
this document builds on top of; nothing here changes MVP scope or blocks the
remaining MVP proof (#8503, #8477). This doc records the owner's stated next
directions — (1) connect one or more ChatGPT/Codex accounts and delegate
cloud turns to them inside Agent Computers, (2) manage multiple cloud agents
from the phone, (3) grow those agents into business-system-integrated "AI
Employees" — grounds each in what already exists in the tree (with file refs),
names the real gaps, and proposes the build sequence. Grounded in three
parallel code/doc explorations run 2026-07-07 (Codex fleet infrastructure,
Agent Computer runtime seams, business/background-agent strategy corpus).

> **Update (later 2026-07-07):** the lane numbering proposed in §6 (CX/AE/BI)
> is reconciled with the Agentic Society integration roadmap's AE/CB phases
> and the blitz program in the overarching strategic roadmap:
> `docs/fable/2026-07-07-overarching-roadmap-khala-code-agent-computers-ai-employees.md`.
> That doc supersedes both numbering schemes for new issue filing; the
> content of this doc's lanes stands unchanged. Execution sequencing now
> lives in `docs/fable/MASTER_ROADMAP.md` (rev 2: CX-1..5 = Phase P2,
> after the store-submitted MVP (P0) and Sarah (P1)).

## 0. The owner direction (2026-07-07, recorded in essence)

Plan beyond the MVP. First: **Codex support** — connect one or more
ChatGPT/Codex accounts (much of this machinery already exists in Khala Code
Desktop and Pylon) and have that Codex run **inside the Agent Computer**, so
a user can delegate to their own Codex instead of (or alongside) the default
Gemini lane. More generally: **expand from "one coding agent in a thread" to
users managing multiple cloud agents from their phone**, agents that
increasingly tie into business systems (CRM and other third-party
integrations) — **real AI Employees**.

## 1. One-paragraph verdict

This is less new architecture than it looks, for the same reason the mobile
pivot was: the seams were built general. The placement contract is literally
named `openagents.codex_placement_assignment.v1`; the public cloud-coding
adapter enum already carries `codex` as its **default**; the runtime
enforcement layer already dispatches real Codex turns for the
`codex_app_server` lane; server-side encrypted custody of ChatGPT/Codex auth
material already exists (`provider_account_token_custody`, migration 0283,
with device-login and auth-material re-prime routes); and the "AI employee"
record — `openagents.agent_definition.v1` with harness choice, typed
toolsets, cron/webhook/inbox triggers, budgets, and escalation — is fully
landed with a production consumer (the lead-gen definition). The three real
gaps are: (1) a **provider-credential broker seam into the microVM** (the
isolation posture currently forbids raw OAuth on the agent computer
outright — correctly — so Codex-in-the-VM needs a scoped, short-lived,
owner-only credential injection path analogous to the #8475 SCM broker, plus
the invariant rewrite that legalizes exactly that and nothing more); (2)
**unifying the two cloud lanes** (background-agent definitions'
`cloud_workroom` lane was parked while Agent Computers built the Firecracker
lane for mobile — they must become the same thing); and (3) the **mobile
Agents panel** (the cockpit UI that turns "a thread with a repo" into "a
staff roster"). Everything else — billing, receipts, isolation, admission,
push — carries over unchanged.

## 2. Direction 1: Codex on Agent Computers

### 2.1 What the user gets

From the phone: **Settings → Agents → Connect Codex account**. The app runs
the standard `codex` device-auth flow (short code + browser — inherently
phone-friendly; no long-string pasting, same UX contract as
`khala fleet connect` in `clients/khala-cli/src/fleet.ts`). Once connected,
the account appears in the user's harness roster with live readiness/quota
state, and any thread (or any agent definition, §3) can target it: the model
picker grows a harness dimension — *Khala cloud (Gemini default)* vs *your
Codex* vs (same pattern later) *your Claude*. Turns dispatched to "your
Codex" run inside the user's Agent Computer exactly like Gemini turns do —
same microVM, same isolation, same receipts — but the model tokens are drawn
from the user's own ChatGPT subscription instead of our metered inference.

Why users want this: they already pay for Codex. Subscription capacity is
prepaid and largely stranded (the wedge analysis in
`docs/fable/2026-07-02-khala-code-business-opportunity-and-openagents-analysis.md`;
~90% of the public served-token counter is already Pylon-Codex own-capacity).
Letting them point that capacity at their phone-dispatched work makes the
marginal Khala Code turn nearly free for them and nearly costless for us.

Why we want this: **BYO-subscription turns collapse our COGS to compute
time.** The Agent Computer strategy (§3 of the strategy doc) already meters
model tokens and agent-computer time as separate rails against one credit
balance. A Codex-backed turn burns zero org model tokens — we charge only
the compute-time meter (plus, later, an orchestration margin). That is a
priceable, honest, high-margin lane that no harness vendor can offer,
because it requires being the orchestration layer above the harness.

### 2.2 What already exists (verified, with paths)

**Local account machinery (complete).** `pylon accounts connect codex` /
`khala fleet connect` implement the whole lifecycle:
isolated homes at `<pylon home>/accounts/codex/<ref>`
(`apps/pylon/src/account-connect.ts` ~199), forced file credential store,
device login always with `CODEX_HOME=<isolated home>` (never `~/.codex`),
auth-validity probing via a bounded `codex exec` ping, registry entries in
Pylon config (`apps/pylon/src/account-registry.ts`), public-safe hashed
account identity (`hashPylonAccountRef` →
`account.pylon.codex.<sha256[:24]>`), health/quota ledgers
(`account-quota-ledger.ts`, `codex-account-health-ledger.ts`, schema
`openagents.pylon.codex_account_health.v0.1`), typed failure classes
(`account_exhausted`, `account_rate_limited` in `session-error-class.ts`),
and round-robin rotation across ready accounts.

**The runtime lane (complete, locally).**
`apps/pylon/src/orchestration/runtime-intent-enforcement.ts` already
dispatches real Codex turns when a `khala_runtime_control_intent.v1` targets
`lane: codex_app_server` — Codex SDK `startThread`/`resumeThread`, streamed
events back into the thread scope, per-thread account pinning for session
resume, fallback rotation on unhealthy accounts. The mobile wire contract
already carries the lane field; **the phone can already ask for Codex** — the
question is only who consumes it and where the credential lives.

**Cloud receipts (already defined).** `usageReceiptProviderForLane` maps
codex → provider/backendProfile `pylon-codex-org-capacity`, model
`openagents/pylon-codex` — the org-cloud receipt identity for Codex turns
was minted in #8473 before any Codex ever ran in the cloud. The ingest path
(`POST /api/khala/cloud/runtime-turn-usage`,
`openagents.khala_cloud_runtime_turn_usage.v1`) is landed and proven on
staging.

**The placement layer (already Codex-shaped).** The placement contract the
Agent Computer path rides is `openagents.codex_placement_assignment.v1`
(`cloud-coding-session-routes.ts` ~1087, `cloud-control-client.ts` ~31) and
carries an `auth_grant_ref` field (`openagents.codex_auth_grant.v1` in the
private `cloud/` contract docs) that was designed for exactly this and is
currently unused by the Gemini lane. `CloudCodingAdapter` is
`['codex', 'claude_agent']` with default `codex`
(`cloud-coding-session-routes.ts` ~91). The naming is not vestigial — Codex
delegation was the original design point the Agent Computer generalized from.

**Server-side credential custody (already exists).** This is the decisive
recent fact: the background-agents build-out (BA-D1, epic #8187) landed
`provider_account_token_custody` (+ audit table), migration
`0283_provider_account_token_custody.sql`, AES-GCM encrypted refresh-token
storage, with device-login routes
(`/api/pylon/provider-accounts/chatgpt-codex/device-login[/start]`) and an
auth-material re-prime endpoint
(`/api/pylon/provider-accounts/chatgpt-codex/auth-material`)
(`apps/openagents.com/workers/api/src/provider-account-token-custody.ts`,
`provider-account-routes.ts`). The custody rail for "the server holds a
user's Codex auth material and re-primes an executor" is **built**. What
does not exist is the delivery of that material into a Firecracker microVM.

### 2.3 The gaps (honest)

1. **No provider-credential path into the microVM — by current invariant,
   forbidden.** The isolation posture
   (`docs/khala-code/2026-07-06-agent-computer-isolation-posture.md`,
   `openagents.agent_computer_isolation_policy.v1`) says: SCM-broker repo
   tokens only; *no raw user OAuth tokens, no provider master keys, ever*.
   That rule was written when the only provider credential in scope was
   ours. Running the user's Codex inside their own agent computer requires a
   deliberate, narrow amendment (§2.4). This is an INVARIANTS change and
   must be treated as one: policy text, placement-contract flags, tests, and
   the credential scanner updated in the same change.
2. **`SUPPORTED_DISPATCH_LANES` in the org-cloud supervisor path defaults to
   `hosted_khala`.** The Codex lane is implemented but the org-cloud
   supervisor only wires Gemini for model inference today; arming
   `codex_app_server` in the cloud means the image carries the `codex`
   binary and the account home materializes inside the VM.
3. **Session-resume continuity vs ephemeral microVMs.** Codex threads resume
   against a `CODEX_HOME`; the per-thread account pin is in-memory and the
   microVM is destroyed on reclaim. Continuity needs either (a) re-priming
   auth + accepting fresh Codex threads per provision (simplest; thread
   context is re-established from Khala Sync history), or (b) the strategy
   doc's §7 per-user persistent volume as the "your agent computer
   remembers" paid tier. Start with (a).
4. **Policy invariant not yet written for this exact shape.** The resale
   law is favorable and unambiguous
   (`apps/openagents.com/INVARIANTS.md` Provider Capacity Marketplace Gate:
   `subscription_capacity_resale` blocked unconditionally; own-capacity
   `agentic_work` explicitly allowed — see also
   `docs/khala/2026-06-25-pylon-linked-coding-capacity-routing-spec.md`
   §365–411). A user's own subscription doing the user's own work on an
   OpenAgents-owned machine is the allowed side of the line. But nobody has
   written the invariant that says so *for cloud custody + injection*, and
   the non-negotiable corollary: **a custodied subscription credential is
   keyed to one owner and may only ever be injected into that owner's own
   work contexts — never pooled, never routed to another user's turn, never
   used to serve org demand.** That sentence must become enforced law with
   tests before the first cloud Codex turn.

### 2.4 The design: a Provider Account Broker, symmetric with the SCM broker

The #8475 SCM broker is the template and the precedent: the executor
authenticates, the Worker derives the user's stored credential from the
authenticated owner, verifies scope, and returns a short-lived, bounded
credential. Do the same for provider accounts:

- **Connect (phone):** mobile drives the Codex device-auth flow; the
  resulting auth material lands in `provider_account_token_custody`
  (existing rail), keyed to the OpenAuth user, with the audit trail the
  table already carries. The mobile UI reuses the readiness/quota projection
  shapes that already exist
  (`openagents.pylon.operator_account_status.v0.1`).
- **Placement:** when an admitted work context's harness selection resolves
  to a connected Codex account, the placement request populates the
  already-present `auth_grant_ref` with a broker grant reference — never the
  material itself. The isolation policy contract gains
  `provider_credential_policy: broker_only` alongside the existing
  `scm_broker_only`.
- **Injection:** inside the microVM, at turn start, the runtime redeems the
  grant against the broker (authenticated executor → Worker → decrypt from
  custody → short-TTL material), materializes the isolated
  `CODEX_HOME` on the scratch disk, and runs the turn. Reclaim already
  requires scratch-wipe + microVM-destroy receipts
  (`agent_computer_reclaim_evidence_missing` fails closed) — the credential
  dies with the VM by construction. The workspace credential scanner
  (`scanLongLivedScmCredentials`, `apps/pylon/src/workspace-materializer.ts`)
  extends to provider auth files so nothing long-lived leaks into writeback.
- **Rotation/health:** quota and auth-health ledger records post back
  through the runtime's authenticated connection, so the phone shows the
  same `account_exhausted` / `account_rate_limited` truth the desktop fleet
  shows today, and multi-account users get automatic rotation in the cloud
  exactly as locally.
- **Billing:** the turn's usage receipt posts with provider
  `pylon-codex-org-capacity` (already defined) and
  `tokenChargeMetered: false` semantics — the user is not charged model
  tokens for their own subscription's work; the compute-time meter (§3 of
  the strategy doc) is the charge. The single-charge invariant from #8503's
  money gates carries over unchanged.

**Claude follows for free.** The account registry already models
`claude_agent` as the second provider, the runtime lane `claude_pylon` is
implemented, and custody generalizes (`CLAUDE_CODE_OAUTH_TOKEN` env
delivery is even simpler than a Codex home). Design the broker
provider-generic from day one; ship Codex first because the fleet machinery
and product positioning (`docs/khala-code/2026-07-01-codex-required-product-positioning.md`)
are Codex-first.

### 2.5 Model/harness selection UX

#8484 landed the per-user model preference store
(`apps/openagents.com/workers/api/src/inference/model-preference-store.ts`)
with typed fallbacks and no silent substitution. Extend the same store shape
from "model id" to "execution target": `gemini` (default) | `khala` hosted
lanes | `codex:<accountRefHash>` | later `claude:<accountRefHash>`. The
episode-245 two-axis model
(`docs/fable/2026-07-01-episode-245-completion-and-multi-harness-orchestration.md`)
applies directly: Axis A = what harness backs this thread's turns; Axis B =
what workers a delegation fans out to; `auto` remains a routing *parameter*
(quota-aware: prefer the user's connected accounts while healthy, fall back
to metered Gemini with an honest event when exhausted — never a silent
substitution). Quota-aware auto-routing across a user's own accounts is a
genuinely new capability no single-vendor harness can offer.

## 3. Direction 2: managing multiple cloud agents from the phone

### 3.1 The reframe

The MVP ships "a thread with a repo that does work." The next product is
**a roster**: several named agents, each with a goal, a harness, a toolset,
triggers, and a budget — visible and steerable from the phone. The record
for that already exists and is not a sketch: it is
`openagents.agent_definition.v1`, fully landed (epic #8187, BA-1..H,
issues #8188–#8214 closed): schema authority in
`packages/agent-runtime-schema`, D1 `agent_definitions` + CRUD
(`/v1/agent-definitions`), dispatch + run history
(`/v1/agent-definitions/:id/runs`, `run-now`), harness adapter contract
(`openagents.agent_harness_adapter.v1`,
`apps/pylon/src/agent-harness-adapter.ts` — Codex and Claude adapters both
exist; one unchanged definition proven on both), compiled deny-precedence
toolsets, cron/webhook (GitHub, Slack) triggers with a scheduler DO,
budgets with auto-pause, and the owner-scoped event ledger / unified inbox
(`event_ledger.v1`). There is a production consumer: the standing lead-gen
definition `agent_definition.autopilot.lead_gen.v1`
(`apps/openagents.com/workers/api/src/autopilot-lead-gen-agent-definition.ts`)
with a drafting-only toolset and send authority denied.

**An "AI employee" is an agent definition + a work context + receipts.** We
do not need a new noun in the schema. We need the two halves connected and a
cockpit.

### 3.2 The unification this direction forces (the one real architecture decision)

Today there are two cloud stories that grew in parallel and must become one:

- Background-agent definitions can target `lane: cloud_workroom`, but that
  lane was explicitly **parked** (`ROADMAP_BACKGROUND_AGENTS.md` §6) in
  favor of own-Pylon execution while the control-plane question was open.
- The mobile MVP built the answer to that question: **Agent Computers** —
  admission, placement, isolation policy, lifecycle receipts, compute
  metering, reclaim.

**Decision this doc proposes: the agent-definition cloud lane IS the Agent
Computer.** A triggered definition run resolves to an admitted work context
and executes inside a Firecracker microVM through the same
`cloud-coding-session-routes.ts` → `oa-codex-control` seam as a mobile
thread turn — same admission gate (positive balance, typed refusals), same
isolation policy, same two-meter billing, same Aiur ops view. One cloud
execution substrate; the definition record is just a second *dispatch
source* alongside the mobile composer. This retires the parked-lane
ambiguity (flagged as contradiction #4 in the strategy-corpus review) and
means every hour invested in #8503 pays into the AI-employee product
directly.

Concretely this needs: a work-context kind for definition runs
(`definition + trigger` rather than `thread + repoBinding` — the placement
contract's `work_context_ref` is already opaque enough), the definition
dispatcher calling the admission gate instead of the own-Pylon assignment
gate when `lane: cloud`, and the compiled toolset policy enforced inside the
microVM runtime (the ADR-0012 compilation already exists; the runtime just
receives it in the placement payload like the isolation policy does).

### 3.3 The phone cockpit

The design already exists as BA-G4 (#8211, unshipped; behavior contract
`background_agents.agents_panel.run_status_indicators_truthful.v1` is
PENDING and must land with it): an **Agents panel** — name, goal, harness
badge, lane, last run, next trigger, per-agent run history — plus the
**inbox** view over the event ledger where `ask`-policy escalations land.
Build it mobile-first now (the MVP made mobile the primary surface; desktop
inherits via ONE-UI later):

- **Roster screen:** the user's agent definitions with live state (idle /
  running / waiting-on-you / paused-over-budget) driven by the same
  `runtime_event` sync scopes the thread view already renders — no new
  transport. Each agent's Agent Computer state (provisioning / active /
  idle / reclaimed) surfaces from the lifecycle receipts Aiur (#8501)
  already projects.
- **One-tap approvals:** escalations arrive as push (the #8485/#8486 rail)
  deep-linking into the inbox; approve/deny writes back as a typed mutation.
  The harness-agnostic audit's warning stands: *approvals must be one
  keystroke or people will widen allowlists.* This is the single most
  important interaction in the product — an employee you can't cheaply
  supervise from a phone is an employee you'll either over-trust or turn
  off.
- **Hiring flow:** "New agent" = pick a template (definition templates are
  configs — the lead-gen definition proves per-customer state is config,
  never a fork), name it, connect what it needs (repo, Codex account,
  connector grants), set budget. The $10-credit signup already funds the
  first payroll.
- **Payroll = credits.** Per-agent spend rollups fall out of receipts
  (every run settles exact accounting keyed by `definitionRef`). The mobile
  balance UI (#8480) grows a per-agent breakdown. Budgets with auto-pause
  are already enforced server-side (BA-B4, enforced contract).

### 3.4 Authority scoping (the Artanis lesson)

The Artanis audit
(`docs/fable/2026-07-01-artanis-fleet-administrator-audit.md`) flagged that
"manage a fleet" means two opposite things — the owner administering the
shared org fleet vs a customer administering their own isolated roster — and
recommended a first-class authority scope
(`owner_self | shared_fleet | owner_operator`). Adopt that before the
cockpit ships: every definition, work context, and credential grant carries
the owner scope; the org-cloud lane's existing invariant (never route
through another user's machine or capacity) extends naturally to *never
inject another owner's credential and never let one owner's definition see
another's event ledger*. One codebase, N isolated per-user fleets, one
shared-fleet admin — typed, not implied.

## 4. Direction 3: business-system integrations — real AI Employees

### 4.1 What "employee" adds over "coding agent"

A coding agent's work context is a repo. An employee's work context is a
**business surface**: a CRM pipeline, an inbox, a content calendar, a
support queue. The strategy corpus already commits to the shape
(`docs/fable/2026-07-02-business-fulfillment-engine-meditations.md`:
*every vertical is a config — connectors + grounding corpus + verification
rubric — never a fork*), and the connector lane is specified as BF-6 in
`docs/fable/ROADMAP_BIZ.md`:

- **BF-6.1** connector sidecar with source-verified events (GitHub first —
  signed webhooks, dedupe, bounded issue/PR-scoped agents, bound writeback
  tools); the sidecar never owns membership/payment/email authority.
- **BF-6.2** shared-channel (Slack Connect) connector; **BF-6.3** social
  publishing (X first), approval-gated.
- **BF-6.4** client-owned payment accounts (Stripe-Connect-style seam,
  explicitly "we do not have" yet).
- **BF-6.5** the connector authority invariant: no provider credentials or
  raw webhook bodies in model context, app-owned idempotency, typed
  per-connector toolsets, short-TTL brokered tokens only (same law as
  BA-D2/D3).
- **BF-3.1** scoped read-only ingestion connectors (drive/docs, then
  mail/calendar) into a per-workspace grounding corpus with provenance;
  redaction-before-inference (BF-3.2) as the trust gate.

The precedent integration is live: **Apollo.io via MCP** (OAuth,
owner-connected 2026-07-03) — prospecting, enrichment, sequences, tasks,
with CRM state treated as a *mirror* of our own pipeline queue, and all
send/activation tools **denied** in the lead-gen definition's toolset. That
is the pattern to generalize, not a one-off: the CRM is the customer's
system of record; the employee reads it through a bounded toolset, drafts
into it, and *never* holds send/spend authority without an approval receipt
(`lead_gen_agent.no_send_without_approval_receipt.v1` is already an enforced
contract).

### 4.2 MCP as the integration substrate

Don't build N bespoke connector clients. The agent-definition toolset schema
already models tool allow/deny/ask lists; extend `toolset` to reference
**per-owner MCP connector grants**: a connector = an MCP server (first-party
sidecar like BF-6.1's GitHub, or third-party like Apollo) + an owner-scoped
credential in the same custody rail as §2.4 + a typed toolset filter
compiled the same way ADR-0012 policies already are. The agent-computer
image hosts the MCP client; the broker injects the connector grant at turn
start with the same lifecycle (short-TTL, dies with the VM). We already run
MCP in anger in three places (the `khala_fleet` local MCP server for
delegation, Apollo, and the public discovery endpoint `/.well-known/mcp.json`)
— this makes the fourth use load-bearing product surface.

The semantic-routing rule from the workspace contract applies here: intent
routing between an employee's tools must go through the typed
selector/planner path, not keyword matching — the toolset compiler and the
definition's classifier-hint triggers (`inboxMatch(classifierRef)`) are the
sanctioned homes for that.

### 4.3 The two-sided story: self-serve employees and the services engine

The business docs rank the revenue engines: services now, tool near, network
later (`2026-07-02-agents-that-work-business-services-analysis.md`). AI
Employees are where the tool lane and the services lane converge on one
substrate:

- **Self-serve (Khala Code mobile):** a user hires a lead-gen/content/triage
  employee from a template, connects their own CRM and their own Codex,
  funds it with credits. The straight line stays straight: connect GitHub →
  do cool shit → *hire something that keeps doing it while you sleep* → pay.
- **Services (AW-0 / BF engine):** fulfillment agents servicing per-customer
  promises (BF-5) are *the same definitions* run by our operators with
  customer-scoped configs (`lead_gen_config.openagents.customer_001.v1` is
  the shipped precedent). The operator-minutes-per-engagement metric
  (BF-9.4) falls as the cockpit and behavior contracts mature — the services
  motion continuously specs the self-serve product, which is exactly the
  "come for the tool, stay for the network" absorption path
  (`2026-07-02-come-for-the-tool-stay-for-the-network.md`).

Pricing composes from meters that all exist or are one owner decision away:
model tokens (metered, or zero on BYO subscription) + agent-computer time
(rate is the standing NEEDS_OWNER from #8479) + per-connector/orchestration
margin (new, owner-priced). The employee makes agent-computer time *legible*
— "your employee's machine" — which the strategy doc §3 already argued is
the reason to bill compute as its own rail.

### 4.4 Trust posture (what makes this sellable to businesses)

Everything in §2–§3 compounds here, and it is the honest differentiator:

- **Isolation:** each employee's turns run in a per-work-context microVM
  with the blast-radius sentence already enforced by contract
  (`openagents.agent_computer_isolation_policy.v1`).
- **Receipts:** every run, token, compute-minute, connector action, and
  approval is an exact receipt; behavior contracts make stated expectations
  executable (`packages/behavior-contracts` — the Ep 246 insight that
  business intake converts customer vibes into behavior contracts is the
  qualifying artifact for an employee's job description).
- **Bounded authority:** deny-precedence toolsets compiled and enforced,
  never prompted; send/spend behind approval receipts; credentials brokered
  and short-lived, never in model context.
- **Verifiability:** promise records and verification rubrics per vertical
  (BF-4.2 per-customer service promises) — an employee whose work is
  *accepted*, not just emitted.

No incumbent coding-agent vendor is positioned to say those four sentences.

## 5. What this explicitly does not change

- **MVP scope and gates.** #8503's DoD (real mobile turn in a microVM with
  the receipt bundle), #8477 writeback, Aiur #8500/#8501, and the launch
  promises remain exactly as scoped. Everything here is post-MVP and rides
  on those proofs landing.
- **The resale line.** `subscription_capacity_resale` stays blocked
  unconditionally and non-waivable. Connected subscription accounts serve
  their owner's work only. API-inference resale on our own commercial
  accounts remains the separately-authorized Model-2 path.
- **Exact-only accounting, receipt-first billing, fail-closed arming,
  owner-gated green flips, public-safe projections.** All invariants from
  the MVP epic carry forward verbatim.
- **The org-cloud boundary.** Agent Computers are OpenAgents-owned capacity;
  nothing here widens access to any user-owned machine. The desktop
  Pylon/fleet lane remains postponed per the reopen ledger (launch audit
  §8) — §2 deliberately does *not* resurrect desktop pairing; it moves the
  *account*, not the machine, to the cloud.

## 6. Proposed build sequence (post-MVP lanes)

Dependency spine: CX-1/CX-2 unblock CX-3..5; AE-1 unblocks AE-2..4; BI-*
parallelizes after AE-1. Nothing starts before #8503's proof bundle exists
(the microVM path everything below rides on).

**CX — Codex on Agent Computers**

- **CX-1 Provider-credential invariant + broker contract.** Write the law
  first: amend the isolation policy/INVARIANTS for owner-scoped provider
  credential injection (`provider_credential_policy: broker_only`), the
  never-pooled/never-cross-owner rule, scanner coverage, fail-closed tests.
- **CX-2 Mobile Codex connect.** Device-auth flow from the phone into the
  existing `provider_account_token_custody` rail; accounts list UI with
  readiness/quota projections; disconnect + revocation.
- **CX-3 Injection + cloud Codex turn.** Broker redemption inside the
  microVM, isolated `CODEX_HOME` on scratch, `codex_app_server` armed in the
  org-cloud supervisor's lane set, image layer carrying the codex binary;
  DoD mirrors #8503: one real mobile-dispatched turn on the user's own
  Codex inside Firecracker, receipt bundle with `tokenChargeMetered: false`
  model rows + compute-time receipts, reclaim wipes the credential.
- **CX-4 Harness/target selection UX.** Model-preference store → execution
  targets; per-thread harness pill; quota-aware `auto` with typed fallback
  events; multi-account rotation surfaced honestly.
- **CX-5 Claude account parity.** Same broker, `claude_pylon` lane,
  `CLAUDE_CODE_OAUTH_TOKEN` delivery.

**AE — Agents (AI Employees) on the phone**

- **AE-1 Lane unification.** `agent_definition.v1` cloud dispatch resolves
  to Agent Computer admission/placement; definition-run work-context kind;
  compiled toolset policy delivered in the placement payload; retire the
  parked `cloud_workroom` framing.
- **AE-2 Mobile Agents panel + inbox.** Roster, run history, live state from
  sync scopes, event-ledger inbox, one-tap approvals via push deep links;
  land the pending `agents_panel.run_status_indicators_truthful.v1` and
  `definitions.harness_swap.v1` contracts with it.
- **AE-3 Templates + hiring flow.** Definition templates as configs;
  create/edit from mobile; per-agent budgets and spend rollups in the
  balance UI.
- **AE-4 Authority scopes.** `owner_self | shared_fleet | owner_operator` as
  typed, tested scope on definitions/contexts/grants (the Artanis
  recommendation, made law).

**BI — Business integrations**

- **BI-1 Connector grants on the custody rail.** Owner-scoped MCP connector
  credentials, brokered into the microVM like CX-3; BF-6.5 authority
  invariant enforced (no raw creds/webhook bodies in model context).
- **BI-2 First-party GitHub connector sidecar** (BF-6.1) as the reference
  connector; Slack (BF-6.2) second.
- **BI-3 CRM lane.** Generalize the Apollo pattern: CRM-as-mirror toolsets,
  drafting-only defaults, approval-receipt send gates; first non-coding
  employee template (lead-gen is the shipped precedent — productize its
  config surface for outside owners).
- **BI-4 Ingestion + grounding** (BF-3.1/3.2): scoped read-only corpus
  connectors with redaction-before-inference, per-employee grounding.
- **BI-5 Employee pricing rail.** Owner-priced connector/orchestration
  margin as a third labeled receipt kind on the same Pool B ledger; itemized
  per-agent in mobile + Aiur.

**Later (not committed):** standing/persistent agent computers as the
premium "your employee's desk" tier (strategy doc §7's persistent volumes);
warm pools for instant trigger response; agent computers as a directly
rentable primitive; post-to-earn (#8494) as employee-referral growth;
BF-6.4 client-owned payment accounts; the overflow/peer marketplace once
authority scopes and settlement primitives mature.

## 7. Open questions (flagged, not resolved here)

1. **Codex ToS posture for custodied credentials.** Our resale law is clean;
   the remaining diligence is OpenAI-side terms on where a user's Codex
   session may execute. The device-auth flow the user performs is identical
   to authorizing any machine they control; document the position explicitly
   in CX-1 rather than assuming silently.
2. **Concurrency semantics per connected account.** Locally, one account =
   one supervisor slot with rotation. In the cloud, does a user's single
   Codex account serve multiple simultaneous employees' turns (queue) or
   does the roster need per-account serialization? Propose: serialize per
   account with typed queueing events; more accounts = more concurrency
   (same law as the fleet).
3. **Definition-run threads.** Employee runs need a place to render on
   mobile — a thread per run, or a per-agent activity feed backed by the
   run-history API plus the event ledger? Propose feed-first (threads stay
   human-initiated), but the sync-scope shape should be decided with AE-2's
   design.
4. **Where the cockpit's desktop twin lands** — ONE-UI (#8339) makes the
   React panel portable, but desktop remains postponed; do not let desktop
   parity gate any AE lane.
5. **Naming.** "Agents" (panel), "AI Employees" (marketing), "agent
   definitions" (schema), "agent computers" (substrate) — one glossary entry
   in the promises/copy pass before any public copy ships, gated through
   `docs/promises/` as usual.

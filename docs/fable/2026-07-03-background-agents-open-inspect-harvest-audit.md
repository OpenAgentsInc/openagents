# Open-Inspect (background-agents) — Harvest Audit For The OpenAgents Cloud/Forge Stack

Date: 2026-07-03
Status: reference audit + harvest plan. Examines the local read-only clone
`projects/repos/background-agents` (Open-Inspect, the open-source background
coding-agent system inspired by Ramp's Inspect) against our own stack — the
private `cloud/` repo (`oa-codex-control`, `oa-node`, `oa-workroomd`), the
private `forge/` repo, Pylon's cloud seams in this monorepo, and the plans in
`docs/fable/` — and says concretely what we should harvest, what we should
ignore, and where each harvested pattern lands. Documentation-only; flips no
promise state, changes no runtime authority, broadens no public copy.

Companions:
[`2026-07-02-harness-agnostic-background-agent-definitions-audit.md`](./2026-07-02-harness-agnostic-background-agent-definitions-audit.md)
(the `agent_definition.v1` / triggers / inbox plan this audit feeds),
[`ROADMAP.md`](./ROADMAP.md) (WS-2 orchestration store, WS-10 status spine),
`docs/khala/2026-06-25-pylon-linked-coding-capacity-routing-spec.md`, and the
`cloud/` contracts (`openagents.codex_placement_assignment.v1`,
`resource_usage_receipt.v1`).

Discipline: patterns-only. `projects/repos/background-agents` stays read-only
reference; ideas port, code does not; no product-surface naming.

## 1. What Open-Inspect Is

A mature (~115k LoC TS + ~11k LoC Python, 208 TS test files + 39 pytest
files, CI on every push, Terraform-managed) three-tier background coding
system:

- **Control plane** — Cloudflare Workers + Durable Objects. One `SessionDO`
  per session with its own SQLite database (30 numbered in-DO migrations), a
  hibernatable WebSocket hub (client mapping persisted for recovery), an
  event pipeline with cursor pagination, and a single multiplexed DO alarm
  (execution timeout, inactivity snapshot, heartbeat-stale checks). D1 holds
  shared state (session index, repo metadata, AES-256-GCM-encrypted secrets,
  automations, model preferences, user SCM tokens); R2 holds media
  artifacts. No Cloudflare Queues — queuing is a per-session messages table
  plus alarms, and a singleton `SchedulerDO` runs automations off Worker
  cron.
- **Data plane** — provider sandboxes (Modal, Daytona, Vercel Sandbox,
  OpenComputer) running a Python supervisor + **OpenCode** as the single
  in-sandbox harness, with an `AgentBridge` WebSocket back to the control
  plane (30s heartbeats, reconnection with exponential backoff, and an
  ack-id event buffer so critical events survive reconnects).
- **Clients** — Next.js web, plus Slack/GitHub/Linear bots (all CF Workers +
  Hono) that follow one uniform pattern: external event → normalize →
  `createSession` + `sendPrompt` → callback/poll → post result back.

Signature mechanisms: brokered short-lived SCM credentials via a git
credential helper (no long-lived tokens in sandboxes), three warming layers
(post-prompt filesystem snapshots, 30-minute-cron prebuilt repo images,
warm-on-typing), centralized ChatGPT/Codex OAuth refresh so subscription
tokens survive ephemeral sandboxes, agent-spawned child sessions with
`parent_session_id`/`spawn_depth`, multiplayer presence with per-prompt
commit attribution, and an automations engine (cron / inbound webhook /
Sentry / Slack triggers, auto-pause after consecutive failures).

Security model: **explicitly single-tenant**. One shared GitHub App; no
per-user repo access validation; PRs attributed via the prompting user's
OAuth token when present. This is a trusted-org tool, not a marketplace.

## 2. What We Have (The Comparison Baseline)

From the current repos (verified 2026-07-03):

- **`cloud/`** (private, Rust, active — last commit 2026-06-24): the real
  execution control plane is `oa-codex-control` (~8.2k LoC): HTTP routes
  `/v1/placement`, `/v1/codex-runs` (+continue/steer/cancel), `/v1/queue`,
  `/v1/cloud-vm/sessions`, `/v1/workrooms/codex/start`; live GCE per-session
  VM leases; a cross-OS Firecracker provisioner (opt-in); cost-driven
  placement (GCE primary, SHC held); durable run queue (off by default);
  refs-and-limits-only `resource_usage_receipt.v1` with measured
  `vm_seconds` and a shared cost model. `oa-node` and `oa-workroomd` are
  well-tested CLI-shaped scaffolds (their own README says "placeholders...
  without claiming production readiness") that define the workroom sidecar
  vocabulary: link-local gateways, per-workroom `CODEX_HOME`, sandbox
  profiles with `secret_policy: brokered_no_raw_secrets`, artifact closeout,
  receipts. **Zero Cloudflare usage** — this repo is a GCE/Firecracker/SHC
  data plane.
- **`forge/`** (private, Rust axum+SQLite, ~28.7k LoC, dormant since
  2026-05-08): the lifecycle authority vocabulary — Work Orders, Runs,
  Workspaces, Controller Leases (TTL/renewal/stale-recovery), Evidence
  Bundles, Verification Reports, Delivery Receipts, signed callback outbox.
  Real REST surface, localhost SQLite, no runtime execution of its own
  (delegated to Probe, which never grew the worker CLI).
- **This monorepo**: Pylon already speaks the cloud placement contract
  (`apps/pylon/src/cloud-control-client.ts` →
  `openagents.codex_placement_assignment.v1`, polls
  `codex_workroom_event.v1`, maps lane-transparently into the `SessionEvent`
  stream); the orchestration store (`apps/pylon/src/orchestration/` — WS-2)
  holds FleetRun/claims/work-planner/merge-policy; the `openagents.com`
  Worker is already Cloudflare (D1, DO, exact `token_usage_events`,
  owner-only traces, Durable Streams).
- **The plan of record**: the harness-agnostic background-agent definitions
  audit (2026-07-02) already concluded we have the **execution** and
  **enforcement** layers but are missing the **definition**, **trigger**,
  and **inbox** layers, and sketched `agent_definition.v1`, dispatch lanes
  (`own_pylon | cloud_workroom`), and a Cloudflare-primitives event ledger.

So the comparison is not "should we build what Open-Inspect built" — most of
its skeleton exists here in stronger (multi-tenant, receipt-bearing,
claim-guarded) form. The comparison is: **Open-Inspect has production-shaped
implementations of exactly the connective tissue our plans name but have not
built.**

## 3. Structural Mapping

| Open-Inspect concept | Our counterpart | State of ours |
| --- | --- | --- |
| `SessionDO` (SQLite-in-DO, WS hub, event stream) | Durable Streams + `pylon_api_*` D1 rows in the openagents.com Worker; workroom event ingest callbacks | Ours is flatter (D1 tables + resumable SSE), no per-session DO, no client-facing WS hub |
| `SchedulerDO` + automations (cron/webhook/Sentry/Slack) | Nothing (named gap: definitions-audit "triggers" layer) | Missing |
| Sandbox provider abstraction (capability flags, typed transient/permanent errors, circuit breaker) | `oa-codex-control` placement lanes (GCE / Firecracker / SHC), Pylon `CloudComputeLane` | Ours is lane-enum-shaped, not a capability interface |
| Warming: snapshots + prebuilt repo images + warm-on-typing | Per-run ephemeral GCE VMs; no snapshot/restore, no prebuild, no proactive warm | Missing (cold starts are our tax) |
| Git credential helper + brokered short-lived SCM tokens | `secret_policy: brokered_no_raw_secrets` in cloud sandbox profiles (declared, not implemented); owner-local Pylon uses the user's own git | Declared invariant without an implementation shape |
| Centralized ChatGPT/Codex OAuth refresh (rotating refresh tokens persisted in D1, re-primed into sandboxes) | Per-workroom `CODEX_HOME` session auth; identity/token footguns documented in the burn runbook | Missing centralized rotation; known footgun class |
| Bridge ack-id event buffer surviving reconnects | Pylon lifecycle NDJSON + fail-soft event-chunk posting | Ours is fail-soft (drop) rather than ack/replay |
| Child sessions (`spawn_depth`, parent polling, cancel) | `codex_spawn` / FleetRun / claims / refill | Ours is stronger (claims prevent duplicates; theirs has none) |
| PR delivery + commit attribution per prompting user | Assignment closeouts + PR conventions; neutral commit metadata is our explicit rule | Different by policy, not by gap |
| Session index + metrics in D1 | `sessionCatalog` (desktop), `pylon_api_assignments` | Comparable |
| Model registry + per-session/message effort | workerKind + (planned) model-role registry per the oh-my-pi audit | Comparable direction |
| Single-tenant shared GitHub App | Owner-scoped dispatch gate, token-resolved authorization | Ours is categorically stronger; theirs is a non-goal |

## 4. What To Harvest (Ranked)

Each item names the pattern, why it earns its place, and the owning repo.
"Harvest" means port the design and invariants, write it in our idiom
(Effect/Bun on the Worker, Rust in `cloud/`), never vendor code.

### H1 — The brokered SCM credential helper (highest leverage, `cloud/` + Pylon)

Their git credential helper (`oi-git-credentials`) means **no long-lived
tokens ever live in a sandbox**: every git operation asks the control plane
for a fresh short-lived installation token, scoped by protocol and host,
cached ~1h, with an env fallback only for legacy snapshots. This is a proven
implementation shape for the `secret_policy: brokered_no_raw_secrets`
invariant that our cloud sandbox profiles *declare but do not implement*.
Harvest into `oa-workroomd` (workroom gateway mints/serves the credential;
helper installed in the VM image) and reuse the same helper shape for any
Pylon-materialized workspace that ever runs with non-owner credentials.
This also generalizes: the same broker pattern serves model API keys and
webhook secrets through the workroom gateways.

### H2 — Centralized Codex/ChatGPT OAuth refresh (`cloud/` + openagents.com Worker)

Their sandbox-side plugin delegates token refresh to the control plane so
**rotating refresh tokens are persisted centrally** (encrypted D1) instead
of dying with ephemeral sandboxes; sandboxes re-prime tokens over the
bridge with a 5-minute buffer. Our per-workroom `CODEX_HOME` session auth
has exactly the failure mode this kills (the burn runbook's identity/token
footguns; expired embedded credentials on VM resume). Harvest: a token
custody + refresh service on our control plane (owner-scoped, encrypted,
audit-logged), with workrooms pulling short-lived access tokens through the
`oa-workroomd` gateway rather than holding refresh tokens on disk. This
directly hardens the subscription-rail economics the oh-my-pi audit banked
on.

### H3 — Warming: snapshots, prebuilt repo images, warm-on-typing (`cloud/`)

Their three layers turn cold starts into seconds: (a) filesystem snapshot
after each completed prompt + before inactivity shutdown, restored on
resume; (b) repo images rebuilt every 30 minutes when commits land (setup
script pre-run, dependencies installed); (c) sandbox warming starts when the
user starts *typing*. Our per-run ephemeral GCE VMs pay full clone + setup
every time. Harvest the **decision structure** (snapshot triggers/reasons,
image-staleness planner, warm-on-intent signal from the desktop composer)
into `oa-codex-control`'s lease lifecycle — GCE disk snapshots/machine
images and Firecracker memory snapshots are the native equivalents. This is
the single biggest UX delta between "fire and forget in 90 seconds" and
"fire and wait".

### H4 — The automations/trigger engine shape (openagents.com Worker)

Their `SchedulerDO` + trigger registry is the **triggers layer the
definitions audit says nothing owns today**: trigger rows carry
`trigger_type` (cron with tz / inbound webhook with JSONPath conditions /
Sentry / Slack), `next_run_at`, and `consecutive_failures` with auto-pause
at a threshold; a singleton DO processes overdue automations per cron tick
with per-tick backpressure; webhook ingress normalizes per-source events in
a shared package. Harvest this whole shape as the trigger half of
`agent_definition.v1` §4.3 — on our stack it is a Worker cron + DO on the
openagents.com Worker (satisfying the Cloudflare-primitives preference),
dispatching through the existing `own_pylon` / `cloud_workroom` lanes
instead of spawning their sessions. Auto-pause-on-consecutive-failures is a
budget-protection invariant we should copy verbatim (a buggy background
watcher is a money pump).

### H5 — Provider abstraction with capability flags + error taxonomy (`cloud/`)

`SandboxProvider` with `supportsSnapshots/Restore/Warm/PersistentResume/
ExplicitStop` capability flags and typed transient/permanent errors feeding
a circuit breaker is a cleaner seam than our lane enum (`cloud-gcp`,
`cloud-shc`, firecracker opt-in env). Harvest the interface shape into
`oa-codex-control` placement so GCE, Firecracker, and SHC become providers
with declared capabilities, the cost model picks among *capable* providers,
and provider failures degrade predictably (circuit breaker per provider)
instead of failing runs. This also keeps the door open to grafting a
third-party burst lane later without touching call sites — while our
execution remains our own GCE/Firecracker per the standing policy.

### H6 — DO-per-run session state + hibernatable WS hub (openagents.com Worker)

Their thin-DO decomposition (the 1,846-line `SessionDO` is a transport
shell; all logic lives in injected, unit-testable services; in-DO numbered
SQLite migrations; WS client mapping persisted so hibernation recovery
works) is the strongest *implementation discipline* in the repo. We should
not replace Durable Streams — resumable SSE already serves Pylon and
mobile-later well — but the **background-run surface** the definitions
audit plans (`/v1/agent-definitions/:id/runs`, run history, live
multi-client watching, steer/stop) fits a per-run DO with exactly their
decomposition. Harvest the pattern (thin DO, injected services, in-DO
migrations, persisted WS mapping, single multiplexed alarm) when WS-10's
status spine grows a client-facing live channel.

### H7 — Ack-buffered bridge events (Pylon + `oa-workroomd`)

Their bridge keeps critical events (snapshot-ready, push-complete/error) in
an ack-id-keyed buffer and re-sends until the control plane acknowledges —
events survive WebSocket drops and sandbox network flaps. Our raw
event-chunk posting is deliberately fail-soft (drop, diagnostic), which is
correct for bulk raw events but wrong for the *small set of load-bearing
lifecycle events* (closeout, push, receipt). Harvest the split: bulk stream
stays fail-soft; a named critical-event class gets ack/replay semantics in
the Pylon lifecycle poster and the future `oa-workroomd` callback path.

### H8 — The bot integration template (openagents.com Worker / Forum)

One uniform pattern across their Slack/GitHub/Linear bots: external event →
verify signature → normalize → create session + prompt → completion
callback posts the result back to the source surface. We will want exactly
this for Forum-triggered runs, GitHub @mention runs, and (post-WS-11)
mobile push. Harvest the template (and their completion-callback service
seam), not the bots.

## 5. What NOT To Harvest

- **The single-tenant trust model.** Shared GitHub App for all users, no
  per-user repo authorization, any user reaches any installed repo — the
  exact opposite of our token-resolved owner-scope dispatch gate and
  marketplace posture. Nothing from their auth *authorization* layer ports;
  only the credential *mechanics* (H1/H2) do.
- **Third-party sandbox vendors as the data plane.** Modal/Daytona/Vercel/
  OpenComputer conflict with the standing policy that unattended execution
  runs on **our** GCE (plus Firecracker/SHC lanes). We harvest the provider
  *interface*, not the providers.
- **OpenCode as the harness.** Their bridge is OpenCode-specific end to
  end. Our multi-harness position (Codex app-server + Claude SDK behind
  typed adapters) is the product differentiator; single-harness coupling
  would be a regression.
- **Informal cost tracking.** `session.total_cost` accumulated from harness
  frames has no receipt, no exact-row reconciliation, no settlement seam.
  Our exact-only `token_usage_events` + `resource_usage_receipt.v1`
  invariants are categorically stronger; nothing to take.
- **No-claims parallelism.** Their child sessions have no duplicate-work
  protection and no verification gate before PR creation; FleetRun claims +
  verify-command authority stay ours.
- **Their commit attribution.** Commits authored as the prompting user
  conflicts with our neutral-commit-metadata rule; we attribute via PR/
  closeout records instead.

## 6. Where This Leaves Forge

Open-Inspect independently validates the split Forge bet on — a control
plane that **never executes code** above a disposable data plane — but it
also shows the control plane wants to live where the clients and state
already are. Our live reality: the lifecycle-authority vocabulary Forge owns
(Work Orders → Runs → Workspaces → Evidence → Verification → Delivery) is
increasingly implemented, in lighter form, by the Pylon orchestration store
(FleetRun, claims, work planner, merge policy) on the public side and
`oa-codex-control` on the private side — while Forge itself has been dormant
since May with no worker runtime ever attached. Recommendation: do **not**
revive Forge to chase this audit. Treat Forge as the vocabulary/receipts
reference (its Evidence/Verification/Delivery object model remains the best
thought-out piece), and let the definitions-audit lanes land on the
openagents.com Worker + `cloud/` + Pylon store where the momentum is. If a
future consolidation folds Forge's object model into the orchestration
store's typed records, that is a documentation-and-schema exercise, not a
service revival.

## 7. Sequencing (Feeds The Definitions-Audit Plan)

Ordered by risk-reduction per unit work, aligned with the 2026-07-02
sequencing (definition record → second adapter → triggers/ledger → cloud
lane → panel):

1. **H4 triggers engine** with the definitions-audit CRUD — it is the
   missing layer, it is Worker-native, and auto-pause budgets come with it.
2. **H2 OAuth custody/refresh** — hardens both the cloud workroom lane and
   the fleet's subscription rails; kills a documented footgun class.
3. **H1 credential broker** in `oa-workroomd` — turns a declared invariant
   into an implementation; prerequisite for any less-trusted execution.
4. **H3 warming** in `oa-codex-control` — the UX unlock for the
   `cloud_workroom` lane actually feeling background-fast.
5. **H5 provider interface + H7 ack-buffer** — refactors that pay rent when
   the durable queue turns on and lanes multiply.
6. **H6 per-run DO surface + H8 bot template** — when WS-10/WS-11-era
   client surfaces need live multi-client watching and source-surface
   callbacks.

## 8. Source Index

Upstream (`projects/repos/background-agents`, read-only, explored at HEAD
`e130401`):

- `README.md`, `docs/HOW_IT_WORKS.md`, `docs/AUTOMATIONS.md`,
  `docs/IMAGE_PREBUILD.md`, `docs/SECRETS.md`, ADRs 0001/0002
- `packages/control-plane/src/session/durable-object.ts` (+ schema,
  websocket-manager, event pipeline, alarm handler, message-queue,
  pull-request-service, scm-credentials-service)
- `packages/control-plane/src/sandbox/provider.ts`,
  `sandbox/lifecycle/manager.ts` + `decisions.ts`, provider implementations
- `packages/control-plane/src/scheduler/durable-object.ts`,
  `src/db/automation-store.ts`, `packages/shared/src/triggers/**`
- `packages/sandbox-runtime/src/sandbox_runtime/{entrypoint,bridge}.py`,
  `credentials/git_credential_helper.py`, `plugins/codex-auth-plugin.js`
- `packages/control-plane/src/auth/{github-app,openai,jwt,crypto}.ts`,
  `src/session/openai-token-refresh-service.ts`

Ours:

- `cloud/` — README/AGENTS/INVARIANTS, `crates/oa-codex-control`
  (`gce_capacity.rs`, `cloud_vm.rs`), `crates/oa-workroomd`,
  `crates/oa-node`, `docs/contracts/*.v1.md`
- `forge/` — README, `apps/forge-server`, `crates/forge-storage`
- `apps/pylon/src/cloud-control-client.ts`,
  `apps/pylon/src/orchestration/`
- `docs/fable/2026-07-02-harness-agnostic-background-agent-definitions-audit.md`,
  `docs/fable/ROADMAP.md` (WS-2/WS-10),
  `docs/ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md`,
  `products/forge.md` (workspace root)

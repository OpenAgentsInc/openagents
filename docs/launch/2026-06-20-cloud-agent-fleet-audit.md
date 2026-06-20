# Cloud Agent Fleet Feasibility Audit — "~80 agents, one per non-green promise"

Date: 2026-06-20
Author: research + docs only (no fleet implemented by this audit)
Owner question: *"Are there promises about cloud agents? Why can't I spin up ~80
cloud agents (Probe / Gemini / API keys / Codex via our hosted ChatGPT auth) —
one per non-green promise — to parallelize the weekend promise assault?"*

Public-safe: this document describes mechanisms, file paths, and env-var **names**
only. It contains **no secret values, tokens, keys, mnemonics, project IDs, or
credentials**.

---

## TL;DR (honest verdict)

You **can** spin up a real one-agent-per-promise fleet today, but **not at 80×
with one button**, and **not honestly off a single hosted ChatGPT auth.** The
hard blockers are not "we have no infra" — we have a lot. They are:

1. **Orchestration breadth.** The live fan-out loop (Autopilot Coder coordinator
   + `multi-session-run.ts`) is real and proven at ~6 concurrent agents on **one
   local machine**. It is not wired to assign "1 promise → 1 cloud agent" across
   80 promises, and it has **no PR-per-agent / branch-per-agent** automation —
   merge is still manual.
2. **The cloud-coding lane is `red`.** `autopilot.cloud_coding_sessions.v1` —
   the exact "spawn coding sessions on OpenAgents Cloud (GCE)" promise — is
   **red** because the desktop→GCE dispatch loop is not demonstrable and live
   GCE provisioning is a fake-default, ADC-gated stub. We can provision/destroy a
   VM (proven once), but the end-to-end "agent does the work on GCE and the
   result round-trips back" is not green.
3. **Quota caps.** Cloud's own contract caps concurrency at **4 active sessions /
   2 remote leases per owner** and **20 / 10 per org**. 80 parallel agents
   violates our own hardened limits before we hit any provider.
4. **ToS reality (the big one).** Driving ~80 parallel automated agents off a
   **hosted ChatGPT (Codex) subscription auth** is **outside the spirit and very
   likely the letter of OpenAI's consumer ToS** (no sharing/automated fan-out of
   a single human subscription seat; usage caps are per human, not per fleet).
   This is a flag-it-plainly concern, not a "probably fine." API-key lanes
   (Gemini, OpenAI API, Anthropic API) are the honest path for fleet scale.

The realistic answer to "why can't I": the **fan-out loop exists but is
single-machine and code-task-shaped**, the **cloud lane is red**, our **own
quota caps say 4–20 not 80**, and the **auth you named (hosted ChatGPT) is the
one auth that does not legitimately scale to a fleet.** The minimal honest first
build is to take the existing local coordinator and point it at **N API-keyed
agents (Gemini/OpenAI/Anthropic API), one per promise, each in its own
worktree**, on **our GCE via `oa-codex-control`** (which already provisions and
tears down per-run VMs), with quota caps lifted deliberately and PR-per-agent
added.

---

## 1. Promises about cloud agents

Registry file:
`apps/openagents.com/workers/api/src/product-promises.ts`
(version `2026-06-20.3`, schema `openagents.product_promises.v1`).

Each promise has: `promiseId`, `productArea`, `audience[]`, `state`, `claim`,
`safeCopy`, `unsafeCopy`, `evidenceRefs[]`, `blockerRefs[]`, `verification`,
`authorityBoundary`.

`state` ∈ `green | yellow | red | degraded | planned | withdrawn`.

**Counts (verified directly, 2026-06-20):**

| state | count |
|---|---|
| green | 24 |
| yellow | 27 |
| red | 20 |
| planned | 27 |
| degraded | 0 |
| withdrawn | 0 |
| **total** | **108** |

**Non-green = 84** (27 yellow + 20 red + 27 planned). So "one agent per non-green
promise" is ~84, not 80 — same order of magnitude, and the point stands.

State transitions are receipt-gated, not auto-computed:
`apps/openagents.com/workers/api/src/promise-transition-receipt-routes.ts`
(`evaluatePromiseTransition()` requires `blockerRefs.length === 0` + evidence +
named verification before a `green` flip; receipts dereferenceable at
`GET /api/public/product-promises/transitions`). **This audit flips nothing.**

### Cloud / agent-spawn / fan-out / parallel-execution promises

| promiseId | state | what it claims |
|---|---|---|
| `autopilot.cloud_coding_sessions.v1` | **red** | Owners run coding sessions on OpenAgents Cloud (GCE first, SHC second) and admin them from desktop/Expo — spawn, watch, approve, accept — so work continues while away. Blocked: live GCE provisioning is a **fake-default stub**; `cloud.gce.*` event kinds + `resource_usage_receipt` don't round-trip to desktop (#5005); Pylon remote bridge transport missing. *(This is the exact promise your question is about.)* |
| `autopilot.control_center_fanout_marketplace.v1` | yellow | Control center / Autopilot can **fan out work to many agents** + plugin marketplace. A single operator-staged order was proven fanned to the open labor market (2026-06-14, #4783); **self-serve customer fanout** and plugin-marketplace beyond code_task remain blocked. |
| `mobile.autopilot_remote_control.v1` | planned | Expo app pairs with a Pylon node to watch/steer Autopilot sessions (local or cloud) — spawn, observe, approve, cancel. Pylon bridge transport unbuilt; no TestFlight. |
| `cloud.agent_cloud_one_stop_revshare.v1` | planned | OpenAgents Cloud is the one-stop Agent Cloud (inference, fine-tuning, training, sandboxes, agentic compute, tasks, data) from one credit balance. Vision capstone; not shipped. |
| `cloud.primitives_suite.v1` | planned | Full Cloud primitive set (inference, fine-tuning, training, agentic tasks, sandbox compute, web services). Primitives at different stages; no single buyable suite. |
| `autopilot.decision_queue.v1` | planned | Decision queue (continue / steer / rerun / retry-with-another-account / stop / accept / follow-up) spanning desktop/web/Expo. `remote-decision-queue.ts` built (20 tests pass); gated on Pylon remote bridge (#5000). |
| `autopilot.all_in_one_business_system.v1` | planned | Autopilot is the all-in-one business system composed of Cloud primitives + open markets + referral revenue. Vision capstone; no single composed product. |
| `autopilot.builtin_compute_agent.v1` | yellow | Desktop ships a built-in agent on **OpenAgents-provided compute** (no key needed). Source wired; not in the installer; needs signed recut + live-from-install smoke. |
| `pylon.local_claude_agent_bridge.v1` | **green** | Pylon drives a local Claude Agent SDK in a sandboxed dir with tool allowlist (`claude-agent.ts`, `claude-agent-executor.ts`). Live-proven locally. |
| `autopilot.codex_probe_pylon_successor.v1` | **green** | Codex executor lane built + live-proven (`codex-agent.ts`, `codex-agent-executor.ts`, CX1–CX5). |
| `pylon.agent_steerable_cli.v1` | **green** | Pylon is headless CLI-only, fully steerable by an agent (28 first-class `--json` commands, `cli-catalog.ts`, `node/control-cli.ts`). |
| `provider.compliant_usage_labor.v1` | yellow | Contributors connect their **own** provider accounts / prepaid API budgets and earn; **OpenAgents never resells provider access.** *(This is the boundary the hosted-ChatGPT-fleet idea would cross.)* |

**Read this carefully:** the building blocks (Pylon CLI steerable, Codex
executor, Claude-agent bridge) are **green at the single-agent level**. The
things that turn that into a *cloud fleet* — `cloud_coding_sessions`,
`control_center_fanout_marketplace` (self-serve), `decision_queue`,
`agent_cloud_one_stop` — are **red or planned**. That gap *is* the answer to "why
can't I."

---

## 2. Existing infra to spin up cloud agents (what actually exists)

### 2a. `oa-codex-control` + GCE — **EXISTS and is DEPLOYED**

This is the real "our cloud = OpenAgents GCE" unattended-execution control path.

- Source: `cloud/crates/oa-codex-control/src/main.rs` (~5.6k lines) and
  `cloud/crates/oa-codex-control/src/gce_capacity.rs` (~1.3k lines).
- Deploy script: `cloud/scripts/gcp-codex-control-deploy.sh`.
- It is a Rust HTTP control daemon. Deployed today as a persistent GCE container
  (`oa-codex-control-1`, Container-Optimized OS) on our GCP project, using
  metadata-server ADC (no key files), behind an IAP-restricted firewall.

**What it does:**
- Accepts async run requests, returns `202`, spawns a background worker:
  `POST /v1/codex-runs(/start)`, `POST /v1/queue(/start)`,
  `POST /v1/placement(/start)`, plus training / artanis / workroom variants.
- Per-run **ephemeral GCE VM** lifecycle (live path,
  `gce_capacity.rs`): `gcloud compute instances create oa-codex-sess-<digest>`
  (`e2-small` default), session-scoped firewall (IAP SSH only), labels for
  reconciliation, health-probe to RUNNING, then **idempotent delete** of VM +
  firewall on any terminal status, verified to zero leftover VMs.
- Launches the agent via `oa-workroomd codex run` with
  `agentRuntime` selector (default `opencode_codex`, fallback `codex`).
- Durable queue with a tick worker and a **global in-flight counter**
  (`QUEUE_IN_FLIGHT`) enforcing `OA_CODEX_QUEUE_MAX_CONCURRENCY` (**default 1**).
- Emits `openagents.resource_usage_receipt.v1` (VM-seconds, cost-plus basis) +
  cleanup receipts; refs-only, no raw project IDs/IPs retained.

**Auth (mechanisms / env-var names only — no values):**
- Control API bearer: `OA_CODEX_CONTROL_TOKEN`.
- Codex/ChatGPT account material resolved per-run via a **grant resolver**
  (`OA_CODEX_GRANT_RESOLVE_URL` + `OA_CODEX_RUNNER_GRANT_TOKEN`, Vortex
  fallbacks), materialized session-locally under `OA_CODEX_AUTH_JSON_ROOT/<ref>/
  auth.json`, **scrubbed after closeout**. Local dev bypass:
  `OA_CODEX_CONTROL_ALLOW_LOCAL_AUTH_ONLY=true`.
- GCE: ADC via metadata-server (`OA_CODEX_GCE_USE_METADATA_ADC`) or
  `GOOGLE_APPLICATION_CREDENTIALS`; project id via env, never retained.

**Honest limitations (NEEDS-OWNER, from the source/docs):**
- The deployed image is **provisioner-capable**; the run-capable image (Codex/
  OpenCode CLI bundled) and the grant resolver are **not fully wired** on the
  live node (local-auth bypass used in proofs).
- Durable queue is **off by default** (`OA_CODEX_QUEUE_ENABLED=false`) and caps
  concurrency at **1**.
- GitHub writeback requires a separate write-grant resolver
  (`OA_CODEX_GITHUB_WRITE_GRANT_RESOLVE_URL`).
- This is exactly why `autopilot.cloud_coding_sessions.v1` is **red**: the
  primitive (provision/teardown a VM) is proven once, but the *fleet loop*
  (N agents, queue on, run-capable image, results round-tripping) is not.

### 2b. `cloud/` managed nodes — `oa-node` / `oa-workroomd`

- `oa-node` (`cloud/crates/oa-node/src/main.rs`, ~5.1k lines): managed daemon —
  identity, capability advertisement, Forge assignment intake (typed receipts),
  Probe/Psionic worker attachment, update/rollback/quarantine, settlement modes,
  capability-broker redaction. MVP Forge intake currently only accepts
  `workroom.sidecar.scaffold`.
- `oa-workroomd` (`cloud/crates/oa-workroomd/src/main.rs`, ~3.5k lines):
  per-workroom sidecar — lifecycle state machine (created→running→exposed→
  closed_out→archived→destroyed), **per-session ephemeral workspace** with a
  no-wallet `AGENTS.md` boundary, session-scoped `CODEX_HOME` (scrubbed),
  link-local capability gateways, the actual `codex exec` / `opencode run`
  runner, artifact capture + closeout, optional git writeback.
- **Isolation today:** GCE lane = per-session VM; SHC lane = process sandbox
  (bubblewrap where available, `danger_full_access` fallback). Firecracker
  microVM lane is **deferred/planned**, not live.
- **Quota caps (contract `openagents.compute_quota_routing.v1`, CND-065):**
  per-owner **4 active sessions / 2 remote leases**; per-org **20 / 10**;
  session TTL 8h, idle timeout 30m, lease TTL 12h.
- `oa-workroomd` can run Probe workers (`openagents.probe_worker_attachment.v1`,
  one workroom = one workspace, scoped, no raw secrets).

**Implemented vs stub:** node bootstrap, workroom lifecycle, Codex/OpenCode
runner, per-session GCE provisioning, cost-driven placement, quota caps, TTL/idle,
resource receipts, Probe attachment, git writeback are **live**. Warm-pool
density, Firecracker lane, live GCP Billing export, full Nexus sync loop, and full
gateway broker logic are **deferred/scaffold**.

### 2c. `probe/` runtime + `packages/probe`

- `probe/` is mid-reset (old Rust deprecated; target = one unified Probe surface
  over local / swarm / API-key / Codex backends). Current runtime:
  `probe/packages/runtime/src/` — per-run assignments keyed by `runnerSessionId`
  + `assignmentId` + signed proof; **concurrent sessions supported** (isolated
  proofs, per-run auth materialization scrubbed after closeout).
- Backends today: **Gemini API** (`backends/gemini/`), **ChatGPT/Codex** (Omega
  device-code + one-time grants), **Apple FM** (local, no-auth). **No Anthropic
  backend yet** in Probe.
- Startup is **CLI / Effect library, no built-in HTTP server**
  (`probe/packages/runtime/src/cli.ts`).
- OpenAgents consumes it as a private monorepo module
  `@openagentsinc/probe-runtime` under `openagents/packages/probe/`; Pylon
  references it.

**Auth mechanisms (names only):**
- Gemini: `GOOGLE_GENERATIVE_AI_API_KEY` / `GEMINI_API_KEY`, or Omega broker
  (`PROBE_OMEGA_BASE_URL` + `PROBE_OMEGA_BEARER_TOKEN`); header `x-goog-api-key`.
  Omega grant kind `probe_gemini_api_key`, materialized per-run, scrubbed.
- ChatGPT/Codex (provider `chatgpt_codex`): **OAuth device-code**, not an API
  key. `probe auth add chatgpt` → `/api/provider-accounts/chatgpt-codex/
  device-login/...`; grants resolved per session via
  `/api/provider-accounts/chatgpt-codex/grants/resolve`; materialized to
  `PROBE_CHATGPT_AUTH_CONTENT` or a per-run file, scrubbed. Grants are
  one-time-use(-per-session). **This is the "hosted ChatGPT auth" you named.**
- Per-run isolation via `ProbeSecretBroker` (no cross-runner secret sharing).
  Probe **does not mint new keys**; it brokers existing Omega-managed material.

### 2d. Existing fan-out / multi-agent orchestration

- **Autopilot Coder coordinator — LIVE, proven ~6 concurrent agents
  (single machine):** `openagents/apps/pylon/src/coordinator/coordinator-runtime.ts`
  + `planner.ts` (`planIntent()` fans a markdown checklist into N
  `MultiSessionPlanEntry`, one agent per part) + runner
  `openagents/apps/pylon/scripts/multi-session-run.ts` (spawns up to
  `maxFanout` (default 4) child processes, **one isolated git worktree per
  session**, account-pool failover). Worktree manager is live + hardened
  (TTL cleanup; `openagents/docs/autopilot-coder/2026-06-11-autopilot-worktree-support-audit.md`).
- **AFK loop — LIVE:** the `/loop` "CONSTANT MOTION" autonomous driver
  (`openagents/docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md`).
- **Forge — LIVE control plane, but 1-work-order → 1-run** (`forge/README.md`):
  many workers attach/heartbeat/claim-next; each Run is singular with a typed
  Workspace. **Not** a fan-out-per-work-order design.
- **NOT implemented anywhere:** automatic **branch-per-agent / PR-per-agent**.
  Worktrees isolate execution; merge is manual after verify-from-main.
- **Aspirational:** agent-hires-agent payment / "swarm compute" (NIP-90 rails
  exist; subcontract payment flow is roadmap, not live).

**Net:** the "1 task → N agents in isolated worktrees, in parallel, with account
failover, with live visibility" pattern **already exists and runs autonomously**
— but it is **on one local machine, code-task-shaped, and stops short of
PR-per-agent and of running on GCE at fleet width.**

---

## 3. Gap analysis — what's needed to run ~84 agents, one per non-green promise

| Dimension | Today | Gap to 84-agent fleet |
|---|---|---|
| **Assignment (1 promise → 1 agent)** | Coordinator fans a checklist into N agents; promises live in `product-promises.ts` with structured `blockerRefs`. | No bridge that reads non-green promises and emits one assignment per promise (objective = clear its blockers / produce its evidence). Need a promise→intent generator. |
| **Orchestration scale** | `multi-session-run.ts` default fanout 4; coordinator proven ~6 on one machine. `oa-codex-control` queue caps at 1 (off by default). | Need fanout ≫ 4 (or many machines), queue enabled, `OA_CODEX_QUEUE_MAX_CONCURRENCY` raised deliberately. Batching 84 across waves is more realistic than 84 truly-simultaneous. |
| **Auth provisioning at scale** | Per-run materialization + scrub is solid for *one* credential per run. Gemini = API key; Codex = OAuth device-code grant. | Need a **pool of API keys** (Gemini/OpenAI API/Anthropic API), one assigned per agent, with rate-limit-aware rotation. Probe has no Anthropic backend yet. Account-pool failover exists in `multi-session-run.ts` but not at 84-key scale. |
| **Isolation** | git worktree per session (local); per-session VM (GCE) / process sandbox (SHC). Firecracker deferred. | GCE per-session VM works but is cold-start and quota-capped (4/owner). For 84 you need warm pools or many VMs, and the quota contract intentionally forbids 84. |
| **Branch/PR-per-agent + merge integration** | Worktrees isolate; **no auto branch/PR**; merge is manual verify-from-main. | This is the **single biggest missing piece** for a real fleet: each agent must land on its own branch, open a PR, and a verifier (e.g., Trigger) must gate merge. Without it, 84 agents produce 84 uncommitted worktrees a human must reconcile. |
| **Cost** | `e2-small` per session; resource receipts measure VM-seconds at cost-plus-10%. | 84 × VMs × hours is real money; mostly bounded by provider API spend (84 agents hammering Gemini/OpenAI/Anthropic), not VM cost. Needs a spend cap per wave. |
| **Rate limits** | Single-account fan-out hits provider RPM/TPM ceilings fast; account-pool failover partially mitigates. | 84 agents on a handful of keys = throttling. Need enough distinct API keys / org-tier limits, or stagger waves. |
| **ToS / policy honesty** | Codex runs off **hosted ChatGPT subscription** auth. Promise `provider.compliant_usage_labor.v1` explicitly says **"OpenAgents never resells provider access."** | **Powering ~84 parallel automated agents off one hosted ChatGPT (consumer/Codex subscription) auth is outside OpenAI's consumer ToS** (a subscription seat is per-human; automated fan-out / sharing of one seat across a fleet is not a sanctioned use, and usage limits are per-person). **Do not build the fleet on hosted ChatGPT auth.** Use **API keys** (OpenAI API, Gemini API, Anthropic API) — metered, per-call billed, and ToS-clean for programmatic fan-out. The Anthropic Claude-agent bridge (`pylon.local_claude_agent_bridge.v1`, green) and Gemini API are the cleanest lanes; OpenAI **API** (not ChatGPT) is fine too. |

### ToS verdict (state plainly)

- **Hosted ChatGPT / Codex subscription auth → fleet of ~84 agents: NOT OK.**
  Consumer ChatGPT/Codex subscriptions are licensed per human user with per-human
  usage limits; running dozens of concurrent automated agents off one such seat
  is account-sharing / automated-abuse territory and risks suspension. Flag it,
  don't gloss it.
- **API keys (OpenAI API, Google Gemini API, Anthropic API) → fleet: OK**, within
  each provider's rate limits and (for OpenAI) usage-tier concurrency. This is the
  honest, supported way to run many programmatic agents.
- It is also consistent with our own promise `provider.compliant_usage_labor.v1`
  ("never resells provider access") and the no-resale memory note (no-resale is
  scoped to SUBSCRIPTION accounts; API-inference is allowed).

---

## 4. Recommended path — minimal concrete build, phased

**Reuse (don't rebuild):**
- The coordinator + `planIntent()` fan-out (`openagents/apps/pylon/src/coordinator/`).
- The worktree-per-session materializer + `multi-session-run.ts` account-pool
  failover (`openagents/apps/pylon/`).
- `oa-codex-control` for per-run GCE VM provision/teardown + queue + receipts
  (`cloud/crates/oa-codex-control/`).
- `oa-workroomd` for the isolated workspace + runner + closeout
  (`cloud/crates/oa-workroomd/`).
- Probe's Gemini backend + per-run auth materialization (`probe/packages/runtime/`).
- The green Claude-agent bridge (`pylon.local_claude_agent_bridge.v1`).

**Build (the actual gaps), with owned-repo routing:**

1. **Promise→assignment generator** — owner: `openagents` (the Worker owns
   `product-promises.ts`). A read-only job that lists non-green promises and emits
   one structured intent per promise (objective = clear listed `blockerRefs` /
   produce named evidence). No state flips.
2. **PR-per-agent + verifier-gated merge** — owner: `openagents` (coordinator/
   Pylon) for the branch/PR creation; verification gate via Trigger. This is the
   **highest-leverage missing piece** and should come first after #1.
3. **API-key pool + rate-aware rotation** — owner: `probe` (backend/auth layer)
   for Gemini/OpenAI-API/Anthropic-API keys, one per agent, ToS-clean. Add an
   Anthropic backend to Probe (currently missing). **Explicitly do not** route
   the fleet through hosted ChatGPT auth.
4. **Lift quota caps deliberately for an owner-run fleet** — owner: `cloud`
   (`openagents.compute_quota_routing.v1`). Raise per-owner/org concurrency under
   explicit owner authorization, with cost + spend caps per wave; treat as a
   policy change (update the contract + INVARIANTS, per workspace discipline).
5. **Turn on the GCE run loop** — owner: `cloud`. Bundle a run-capable image
   (Codex/OpenCode/Probe), wire the grant resolver, enable
   `OA_CODEX_QUEUE_ENABLED` with a sane `OA_CODEX_QUEUE_MAX_CONCURRENCY`. This is
   what finally moves `autopilot.cloud_coding_sessions.v1` off red.

**Phased plan:**

- **Phase 0 (today, no new infra):** Run the *existing* local coordinator with
  fanout raised to ~6–8, fed by a hand-written batch of the highest-value
  non-green promises, on **API keys** (Gemini + Anthropic). Manual merge. This is
  literally "Raynor + a handful of subagents" widened a bit — honest and works
  now.
- **Phase 1 (PR-per-agent):** Build #1 + #2. Now each agent lands its own branch
  + PR; Trigger gates merge. You can run waves of ~6–8 unattended and wake to a
  PR queue, not a worktree mess. This alone gets ~80% of the value.
- **Phase 2 (cloud width):** Build #3 + #5; lift caps (#4) under owner sign-off.
  Move waves onto GCE via `oa-codex-control`. Flip
  `autopilot.cloud_coding_sessions.v1` toward green with real round-trip
  evidence. Now you can run dozens in parallel across VMs.
- **Phase 3 (one-per-promise fleet):** Wire #1's generator to emit all ~84
  assignments, batched into waves sized to provider rate limits + spend caps,
  each agent API-keyed, isolated, PR-gated. This is the real "one agent per
  non-green promise" fleet — built on API keys, on our GCE, never on hosted
  ChatGPT auth.

**First concrete build step:** implement **PR-per-agent + verifier-gated merge**
in the existing coordinator (Phase 1, item #2) — it converts the already-live
local fan-out from "manual merge bottleneck" into an unattended fleet, and it's
prerequisite to everything at scale. Pair it with the promise→assignment
generator (#1) so the very first unattended wave is literally "one agent per
non-green promise" on API keys.

---

## Appendix — key file/path citations

- Promises registry: `apps/openagents.com/workers/api/src/product-promises.ts`
- Promise transitions: `apps/openagents.com/workers/api/src/promise-transition-receipt-routes.ts`
- GCE control daemon: `cloud/crates/oa-codex-control/src/main.rs`,
  `cloud/crates/oa-codex-control/src/gce_capacity.rs`,
  `cloud/scripts/gcp-codex-control-deploy.sh`
- Managed node + sidecar: `cloud/crates/oa-node/src/main.rs`,
  `cloud/crates/oa-workroomd/src/main.rs`
- Cloud quota contract: `cloud/docs/contracts/openagents.compute_quota_routing.v1.md`
- Cloud GCE/receipt contracts: `cloud/docs/contracts/openagents.gce_capacity_class.v1.md`,
  `cloud/docs/contracts/openagents.resource_usage_receipt.v1.md`
- Probe runtime + auth: `probe/packages/runtime/src/` (`cli.ts`,
  `backends/gemini/auth.ts`, `omega/grant-client.ts`, `omega/account-client.ts`,
  `runner/identity.ts`, `auth/materializer.ts`)
- OpenAgents Probe module: `openagents/packages/probe/`
- Fan-out coordinator: `openagents/apps/pylon/src/coordinator/coordinator-runtime.ts`,
  `openagents/apps/pylon/src/coordinator/planner.ts`,
  `openagents/apps/pylon/scripts/multi-session-run.ts`
- Worktree support: `openagents/docs/autopilot-coder/2026-06-11-autopilot-worktree-support-audit.md`
- AFK loop: `openagents/docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md`
- Forge control plane: `forge/README.md`

*No secret values, tokens, keys, project IDs, or credentials appear in this
document by design.*

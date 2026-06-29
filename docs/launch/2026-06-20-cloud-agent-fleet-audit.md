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

*Sections 1–4 and this appendix contain no secret values, tokens, keys, project
IDs, or credentials by design. The two sections below (the Claude-on-Vertex
resolution + 2026-06-20 spike) intentionally name the GCP project
(`openagentsgemini`) and model IDs because that is the concrete setup that
works — but they still contain **no access tokens, OAuth tokens, SA keys, or
secret values** (all redacted).*

---

## Resolution: Claude-on-Vertex is the ToS-clean fleet path

Section 3's "ToS verdict" said: do **not** build the fleet on hosted ChatGPT
(consumer Codex subscription) auth, and **API keys / provider APIs are the honest
path**. The cleanest concrete instance of that honest path is **Anthropic Claude
served through Google Vertex AI** in our `openagentsgemini` GCP project. It
resolves the ToS wall directly:

- **It is a provider *API*, billed per token through Google Cloud — not a
  subscription seat.** There is no "one human's seat shared across a fleet."
  Every request is a metered, pay-as-you-go Vertex `rawPredict` call. Running N
  parallel automated agents against it is exactly what the API is for. This is
  the opposite of the hosted-ChatGPT problem, and it is consistent with our own
  `provider.compliant_usage_labor.v1` promise ("OpenAgents never resells provider
  access") and the no-resale memory note (no-resale is scoped to SUBSCRIPTION
  accounts; API-inference is allowed).
- **It gives the fleet the strongest single coding model (Claude) without
  needing an Anthropic-direct key per agent.** Probe today has Gemini + Codex
  backends but **no Anthropic backend** (Section 2c). Vertex is a way to put
  Claude into the fleet through one already-authed GCP project, rather than
  distributing N Anthropic API keys.
- **One project, one quota surface, central billing.** All instances authenticate
  as the same GCP project and share that project's per-model Vertex quota
  (RPM/TPM). That is a feature for an owner-run fleet: one place to read spend,
  one place to raise limits, one place to cut it off.

### The env / auth / model setup that works

Authentication for Claude Code's Vertex path is **Application Default
Credentials (ADC)** — the `google-auth` token chain, *not* a Bearer flag. Our
own server adapter
(`apps/openagents.com/workers/api/src/inference/vertex-anthropic-adapter.ts`)
instead uses a Bearer GCP token / `VERTEX_SA_KEY`; for an unattended GCE fleet
the right auth is the **GCE metadata-server ADC of a service account** (no key
files on disk), which is also how `oa-codex-control` already authenticates to GCP
(`OA_CODEX_GCE_USE_METADATA_ADC`).

Verified-working setup (project `openagentsgemini`, project number
`157437760789`, region `global`, `anthropic_version: vertex-2023-10-16`):

```bash
# One-time per machine/account (interactive — establishes ADC):
gcloud auth application-default login
gcloud auth application-default set-quota-project openagentsgemini

# Claude Code on Vertex (per agent / per shell):
env CLAUDE_CODE_USE_VERTEX=1 \
    ANTHROPIC_VERTEX_PROJECT_ID=openagentsgemini \
    CLOUD_ML_REGION=global \
    ANTHROPIC_MODEL=claude-haiku-4-5 \
    ANTHROPIC_SMALL_FAST_MODEL=claude-haiku-4-5 \
    claude -p "..."
```

Required IAM on the principal/SA (verified granted for `chris@openagents.com`):
`roles/aiplatform.user` (`aiplatform.endpoints.predict`) and
`roles/serviceusage.serviceUsageConsumer` (`serviceusage.services.use`). Models
are enabled on `locations/global` (haiku/sonnet/opus all 200-OK per the Vertex
runbook). The small/fast model is a **separate** env var
(`ANTHROPIC_SMALL_FAST_MODEL`) and must also be a Vertex-enabled model or
claude-code's background calls fail.

---

## Test implementation — obstacles found (2026-06-20)

> **UPDATE 2026-06-20 (RESOLVED — `claude -p` ran the full agent loop on Vertex,
> on Opus, on `locations/global`).** The earlier "NO — blocked by ADC reauth"
> verdict below was a *point-in-time* result on an expired-RAPT machine. After the
> owner ran `gcloud auth application-default login`, the end-to-end test
> **passed**: `claude --bare -p` drove a real multi-turn agent loop (including the
> `Read` tool) against Vertex, on **`claude-opus-4-8`** at `CLOUD_ML_REGION=global`,
> produced a correct result, and billed **~$0.023** for a tiny task. Haiku and
> Sonnet on `global` were also confirmed working via `claude --bare -p`
> (`VERTEX_OK` / `PONG`, exit 0, `total_cost_usd` reported in the JSON output).
>
> **Two specific corrections to the obstacle list below:**
> 1. **The ADC-reauth blocker is resolved.** Locally it was a one-command
>    interactive `gcloud auth application-default login` (done). For an unattended
>    fleet it is *avoided entirely* by the GCE service-account metadata-server ADC
>    (no RAPT, no interactive reauth) — same as `oa-codex-control`.
> 2. **"Opus = 0 RPM / effectively unavailable" was WRONG for our actual call
>    path.** That `0 RPM granted` figure is the **US-multi-region** quota
>    *preference*, not the path we use. Our calls go to **`locations/global`**,
>    where `claude-opus-4-8` serves fine — verified by a full `claude -p` agent
>    loop on Opus (above) and by the runbook's `global` probe (haiku / sonnet /
>    opus all 200-OK on `global`,
>    `docs/2026-06-13-vertex-ai-anthropic-claude-runbook.md`). You **can** run a
>    fleet on Opus on `global` today; the US-multiregion grant is irrelevant to it.
>    See the corrected quota item (#4) and the Opus-quota note inline below.
>
> A minimal PR-per-agent fleet runner built on this verified path now lives at
> `scripts/vertex-fleet/` (`assign.mjs` / `worker.sh` / `run.sh` / `README.md`).

**Did `claude -p` run on Vertex? — As of the original spike, NO** (point-in-time;
**now YES, see the RESOLVED update above**). At spike time it was blocked by one
expired-ADC reauth, with the underlying Vertex wire path *proven correct*: the raw
Vertex API worked (200 OK, real completion) and Claude Code dispatched to the
correct model and region but could not mint a Vertex token because that machine's
ADC needed an interactive re-login. That was a **one-command, needs-owner**
blocker, not a model/region/quota/IAM problem — and it has since been cleared.

### What proved correct (raw Vertex sanity, bypassing claude-code)

`rawPredict` against `claude-haiku-4-5` at `locations/global` with a valid GCP
token + `x-goog-user-project: openagentsgemini` returned **HTTP 200** with a real
completion (token redacted):

```bash
curl -sS -X POST \
  -H "Authorization: Bearer [REDACTED]" \
  -H "x-goog-user-project: openagentsgemini" \
  -H "Content-Type: application/json" \
  "https://aiplatform.googleapis.com/v1/projects/openagentsgemini/locations/global/publishers/anthropic/models/claude-haiku-4-5:rawPredict" \
  -d '{"anthropic_version":"vertex-2023-10-16","max_tokens":8,
       "messages":[{"role":"user","content":[{"type":"text","text":"Reply with exactly: OK"}]}]}'
# → HTTP 200
# {"model":"claude-haiku-4-5-20251001", ... "content":[{"type":"text","text":"OK"}],
#  "stop_reason":"end_turn","usage":{"input_tokens":12,"output_tokens":4}}
```

So model enablement, IAM (`aiplatform.endpoints.predict` + `serviceusage.
services.use` both granted), `aiplatform.googleapis.com` (ENABLED), the wire
contract, and the `global` region are all good.

### What claude-code did (the real test)

```bash
env CLAUDE_CODE_USE_VERTEX=1 ANTHROPIC_VERTEX_PROJECT_ID=openagentsgemini \
    CLOUD_ML_REGION=global ANTHROPIC_MODEL=claude-haiku-4-5 \
    ANTHROPIC_SMALL_FAST_MODEL=claude-haiku-4-5 \
    claude --debug -p "reply with exactly: VERTEX_OK"
```

Result: **no stdout, hung ~60s, killed.** claude-code 2.1.183 debug log
(`~/.claude/debug/latest`) shows it *correctly* routed to Vertex and then failed
auth on all 11 retries (redacted):

```text
[API:timing] dispatching to vertex model=claude-haiku-4-5
[ERROR] API error (attempt 1/11): {"error":"invalid_grant",
  "error_description":"reauth related error (invalid_rapt)",
  "error_subtype":"invalid_rapt"}
... (attempts 2/11 … 11/11, exponential backoff) ...
```

`dispatching to vertex model=claude-haiku-4-5` is the proof the model-id +
`CLOUD_ML_REGION=global` mapping is correct. The failure is purely the ADC token
mint. The "hang" was simply claude-code grinding through 11 exponential-backoff
retries; it never reaches the model.

### What claude-code did after the reauth (the passing test, 2026-06-20)

After `gcloud auth application-default login`, the same env block **completed**:

```bash
# tiny smoke (haiku / sonnet on global) — exit 0, correct output
env CLAUDE_CODE_USE_VERTEX=1 ANTHROPIC_VERTEX_PROJECT_ID=openagentsgemini \
    CLOUD_ML_REGION=global ANTHROPIC_MODEL=claude-haiku-4-5 \
    ANTHROPIC_SMALL_FAST_MODEL=claude-haiku-4-5 \
    claude --bare -p "Reply with exactly the token: VERTEX_OK"
# → VERTEX_OK   (exit 0)
# with --output-format json the result also reports total_cost_usd (e.g. ~0.0023)

# full agent loop on OPUS (Read tool, multi-turn), result correct, ~$0.023
env CLAUDE_CODE_USE_VERTEX=1 ANTHROPIC_VERTEX_PROJECT_ID=openagentsgemini \
    CLOUD_ML_REGION=global ANTHROPIC_MODEL=claude-opus-4-8 \
    ANTHROPIC_SMALL_FAST_MODEL=claude-haiku-4-5 \
    claude --bare -p "<tiny multi-step task>"
# → correct result; total_cost_usd ≈ 0.023
```

This confirms the **complete** path end-to-end: model-id + `global` mapping,
auth (post-reauth ADC), the agent tool loop, and per-call cost reporting all
work — on Opus, Sonnet, and Haiku alike. Auth is the only thing that was ever
broken, and it is now resolved (locally via reauth; on the fleet via SA metadata
ADC).

### Obstacle list (symptom → root cause → fix → status)

1. **ADC reauth (`invalid_rapt`) — THE blocker that stopped the test.**
   - *Symptom:* `claude -p` on Vertex produces no output and hangs ~60s; debug log
     shows `invalid_grant / reauth related error (invalid_rapt)` on every retry.
     `gcloud auth application-default print-access-token` errors with
     "Reauthentication failed. cannot prompt during non-interactive execution."
   - *Root cause:* the ADC refresh token's **Reauth Proof Token (RAPT) has
     expired** — a Google account/org security policy requiring periodic
     *interactive* re-login. claude-code's Vertex path uses ADC and cannot
     refresh non-interactively. (The separate `gcloud auth print-access-token`
     **user** token is still valid, which is why the raw curl probe worked — but
     claude-code does not use that token.)
   - *Fix:* owner runs once, interactively:
     `gcloud auth application-default login` then
     `gcloud auth application-default set-quota-project openagentsgemini`. For an
     unattended fleet, avoid this class entirely by using a **service-account
     identity via GCE metadata-server ADC** (no RAPT, no interactive reauth) —
     exactly what `oa-codex-control` already does.
   - *Status:* **RESOLVED.** The owner ran `gcloud auth application-default login`
     and `claude -p` then drove a full agent loop on Vertex (verified on Opus,
     Sonnet, Haiku at `global`). For the unattended fleet this class is avoided
     entirely by the GCE SA metadata ADC (no RAPT) — same as `oa-codex-control`.
     Not a code bug; auth-only, and now cleared.

2. **Model-id + region mapping.**
   - *Symptom:* (none observed — this worked.)
   - *Root cause / fix:* claude-code's Vertex IDs match ours exactly
     (`claude-haiku-4-5`, `claude-sonnet-4-6`, `claude-opus-4-8`) and our models
     live on `CLOUD_ML_REGION=global`. The debug log confirmed correct dispatch.
     If `global` ever fails to carry a model, fall back to `us-east5`
     (sonnet/haiku) or the `us` multi-region (opus) per the Vertex runbook.
   - *Status:* **resolved-in-test** (mapping correct; could not complete only
     because of #1).

3. **Small/fast model needs a Vertex model too.**
   - *Symptom:* would surface as background/title/quota-probe calls failing while
     the main `-p` call looks configured.
   - *Root cause:* claude-code makes auxiliary calls on `ANTHROPIC_SMALL_FAST_MODEL`;
     under `CLAUDE_CODE_USE_VERTEX=1` that must also be a Vertex-enabled model.
   - *Fix:* set `ANTHROPIC_SMALL_FAST_MODEL=claude-haiku-4-5` (done in the test
     command).
   - *Status:* **resolved-in-test** (pre-empted).

4. **Quota — shared, per-model, per-project (the concurrency reality).**
   - *Symptom:* at fleet width, `RESOURCE_EXHAUSTED` (429) on Vertex.
   - *Root cause:* **all instances authenticating as project `openagentsgemini`
     share that project's per-model Vertex quota.** N agents do not get N
     independent budgets — they contend for one RPM/TPM pool per model per
     endpoint class. Live numbers pulled via the Cloud Quotas API on 2026-06-20
     (`cloudquotas.googleapis.com/.../157437760789/locations/global/quotaPreferences`):
     - **US multi-region quota *preferences*:** haiku-4-5 = **120 RPM / 5M in-TPM
       / 500k out-TPM**; sonnet-4-6 = **60 RPM / 2M in-TPM / 200k out-TPM**;
       opus-4-8 preference = **0 RPM granted** (preferred 30, denied/reconciling);
       a legacy `anthropic-claude-opus` bucket shows 3500 RPM granted.
       **IMPORTANT (correction):** this `opus-4-8 = 0 RPM` figure is the
       **US-multi-region** quota *preference* — **not the path we use, and it does
       NOT mean Opus is unavailable.** The earlier reading of this as "Opus is
       effectively unavailable" was wrong. Our calls go to **`locations/global`**.
     - **Global endpoint (the path we actually use):** **no explicit `Global*`
       quota preference is set**, so the `global` path runs on Google's default
       per-model quota. Opus serves fine here: a full `claude -p` agent loop ran on
       `claude-opus-4-8` at `CLOUD_ML_REGION=global` on 2026-06-20 (~$0.023), and
       the runbook's `global` probe returns 200-OK for haiku **and sonnet and
       opus** (`docs/2026-06-13-vertex-ai-anthropic-claude-runbook.md`). So **Opus
       on `global` is usable today**; the US-multiregion `0 RPM` preference is
       irrelevant to it. Only set/raise a `Global*` preference deliberately if you
       want a *higher-than-default* ceiling for fleet width.
   - *Fix / how to raise:* create a Cloud Quotas preference per
     `quotaId` × `base_model` (global IDs:
     `GlobalOnlinePredictionRequestsPerMinutePerProjectPerBaseModel`,
     `…InputTokensPerMinutePerBaseModel`, `…OutputTokensPerMinutePerBaseModel`;
     US equivalents `UsOnlinePrediction…`), per the Vertex runbook's quota
     section, or via the console:
     `console.cloud.google.com/iam-admin/quotas?project=openagentsgemini`
     (filter service `aiplatform.googleapis.com`, metric "online prediction
     requests per minute per base model"). *Concurrency math for a "few-instance"
     start:* haiku's granted 120 RPM comfortably covers ~5–10 agents at a sane
     request rate; sonnet's 60 RPM is the tighter bound (those US-multiregion
     numbers are a *floor* reference — the `global` path runs on Google's default
     per-model quota). **Opus is NOT a hard cap on `global`:** the `0 RPM` figure
     applies only to the unused US-multiregion preference, and a real Opus agent
     loop ran on `global` (above). Plan Opus waves on `global`; raise a `Global*`
     preference only if you need width beyond the default ceiling. (Note: the
     repo path `apps/openagents.com/docs/cloud/quotas/2026-06-19-vertex-ai-anthropic-opus-quota-request.md`
     referenced in planning **does not yet exist on `origin/main`** — the live
     quota state above and the console are the authority.)
   - *Status:* **needs-infra** to raise for scale; **resolved-for-a-few** —
     current haiku/sonnet grants already support a small (5–8 agent) start on
     those two models.

5. **Cost — pay-per-token, not free.**
   - *Symptom:* Vertex usage shows up on the GCP bill.
   - *Root cause:* Claude-on-Vertex is metered per input/output token (Google
     Cloud billing, partner-model pricing). N agents = N× token spend; this is
     real money, unlike the (mis)assumption of "free off a subscription seat."
   - *Fix:* prefer haiku for breadth, reserve sonnet/opus for hard tasks; add a
     per-wave spend cap; reuse `oa-codex-control`'s
     `openagents.resource_usage_receipt.v1` accounting so each agent's spend is
     attributable.
   - *Status:* **needs-owner** (accept the cost model; set a spend cap).

6. **Minimal "few-instance" runner — what it actually needs.**
   - *Reuse:* `oa-codex-control` (already deployed) for per-run ephemeral GCE VM
     provision/teardown + receipts; have it launch `claude` with the
     `CLAUDE_CODE_USE_VERTEX=1` env block above instead of (or alongside) the
     Codex runner. **Auth via the GCE SA metadata-server ADC** — this *also*
     eliminates obstacle #1 (no RAPT/interactive reauth on a service account).
   - *Key/SA distribution:* no per-agent key files needed — every VM uses the
     same project SA's metadata ADC. Just grant the runner SA
     `roles/aiplatform.user` + `roles/serviceusage.serviceUsageConsumer` on
     `openagentsgemini`.
   - *Still missing for a real fleet (unchanged from Section 3):* **PR-per-agent /
     branch-per-agent merge automation** is the highest-leverage gap; isolation
     (worktree per session) exists, merge does not. And `oa-codex-control`'s
     durable queue is **off by default** capping concurrency at **1**
     (`OA_CODEX_QUEUE_ENABLED`, `OA_CODEX_QUEUE_MAX_CONCURRENCY`) — must be
     enabled + raised deliberately.
   - *Status:* **needs-infra** (small: SA grant + env wiring on the existing
     control daemon; PR-per-agent is the larger build).

### One-line bottom line

The Vertex *path itself works* (raw 200 OK; claude-code correctly dispatches to
`global`/`claude-haiku-4-5`). The only thing that stopped `claude -p` from
completing was an **expired ADC reauth (`invalid_rapt`)** — fixed by one
interactive `gcloud auth application-default login`, or designed away entirely by
running the fleet on a **GCE service-account metadata-ADC** identity. After that,
the gating concerns are **shared per-project quota** (haiku/sonnet OK for a few
agents; Opus quota = 0), **per-token cost**, and the pre-existing
**PR-per-agent** gap.

*No access tokens, OAuth tokens, service-account keys, or secret values appear in
this section — all credentials were redacted.*

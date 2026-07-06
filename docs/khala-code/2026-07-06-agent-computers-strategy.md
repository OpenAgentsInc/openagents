# Agent Computers — the cloud execution strategy for Khala Code

Date: 2026-07-06
Status: **owner decision.** This document is the authoritative strategy for
how Khala Code mobile turns execute in the cloud, what we call that thing,
how it bills, and what infrastructure it runs on. It supersedes the
"hosted Pylon pool" framing used in the #8473 lane and **rejects the exe.dev
substrate direction** proposed in
`docs/khala-code/2026-07-06-exe-dev-cloud-delegation-audit.md` (that doc
stays as a historical evaluation; its recommendation is not adopted).
The launch context is the mobile-only MVP pivot
(`docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md`,
epic #8467).

## 1. The concept, in one paragraph

When a Khala Code mobile user asks for work, that work runs on an
**agent computer**: an isolated Firecracker microVM on OpenAgents' own
Google Cloud infrastructure, booted for that user's work, with its own
kernel, filesystem, and network namespace, running the OpenAgents runtime
that consumes the user's turn, checks out their repo, runs the coding agent,
streams progress back into their thread, and writes results to their GitHub.
Agent computers are a **separately billable product primitive**: their
active compute time draws against the user's credit balance, on top of the
exact token usage the model consumes. One sentence for product copy (gate
through promises before publishing): *your work runs on an agent computer —
a clean, isolated machine we spin up for you and bill from your credits.*

The word "pylon" does not appear in this product concept. The Pylon runtime
program (`apps/pylon`) remains the internal software that executes turns —
an implementation detail inside the agent computer image — but the unit we
provision, monitor, meter, and bill is the **agent computer**. Retire
"hosted Pylon" from all planning and product language.

## 2. Why Firecracker on GCP — and why not exe.dev or shared VMs

### 2.1 We already own this path

This is the decisive fact the exe.dev evaluation underweighted: the
Firecracker path is **mostly built and already ours**.

- `apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.ts`
  is a typed, tested, flag-gated cloud coding-session surface **already in
  production code** — lane selection (`cloud-gcp`/`cloud-shc`), repo trust
  tiers, adapter choice, timeout ceilings, and a metering hook that emits
  `openagents.resource_usage_receipt.v1`. It fails closed today
  (`cloud_gce_provisioning_not_armed`) because the provisioner flag is off —
  the work is *arming and finishing it*, not building it.
- The private `cloud/` repo (`oa-node`, `oa-codex-control`) implements the
  actual **Firecracker microVM provisioner** (`cloud_vm_provisioner.v1`)
  with a receipt-bearing 8-state workroom lifecycle, placement API
  (`POST /v1/placement`), run-event streaming, and cleanup — designed for
  exactly this workload.
- `apps/pylon/src/cloud-control-client.ts` already speaks to that control
  plane (placement, events, cancel, GCE lease lifecycle events), tested
  against a fake control plane.
- #8473 (landed) built the runtime side: org-cloud executor mode consuming
  `khala_runtime_control_intent.v1` from Khala Sync, runtime events back
  into thread scopes, and exact per-turn usage receipts posted to
  `POST /api/khala/cloud/runtime-turn-usage`.

Choosing exe.dev would have meant onboarding a new vendor to *avoid*
finishing infrastructure we already built. That's backwards.

### 2.2 Isolation is the product requirement, not a nice-to-have

Agent computers execute **arbitrary user repo code** under an identity that
holds scoped GitHub credentials and charges real credits. That is the most
sensitive surface in the whole system. Firecracker gives every agent
computer its own kernel and device model (~125ms boot, the substrate AWS
Lambda/Fargate run on), so a malicious or compromised repo is contained to
its own microVM by a hardware-virtualization boundary — not a container
namespace, not a shared persistent VM another user's turn also runs in.
exe.dev's model (persistent VMs over a *pooled* resource plan, sparse
public security documentation) put the multi-tenant boundary exactly where
we can't afford softness. Third-party VM hosts also see the workspace and
runner environment by construction; on our own GCP project, the trust chain
is Google → us, with no additional party.

### 2.3 How it physically runs on GCP

Firecracker needs KVM, which GCE provides via **nested virtualization**
(the setup the upstream doc `projects/repos/firecracker/docs/dev-machine-setup.md`
walks through): host instances on N2/N1 (Haswell+, `--enable-nested-virtualization`),
`/dev/kvm` exposed in-guest. The shape:

- **Host**: a small number of GCE instances (start: one `n2-standard`-class
  host in the existing `openagentsgemini` project) running `oa-node` from
  the `cloud/` repo, each hosting many Firecracker microVMs.
- **Agent computer**: one Firecracker microVM per admitted run context —
  its own rootfs built from a pinned agent-computer image (OpenAgents
  runtime + coding agents + tooling), its own scratch disk, TAP-networked
  egress through the host with a restrictive policy.
- **Control plane**: `oa-codex-control` (private `cloud/` repo) owns
  placement, lifecycle (the 8-state workroom model), health, and reclaim;
  the public Worker talks to it through the already-built
  `cloud-coding-session-routes.ts` seam (`OA_CLOUD_CONTROL_URL` + token,
  `gceProvisioningArmed`).
- Nested-virt overhead is real but acceptable for coding-agent workloads
  (I/O + API-bound, not compute-bound). If/when density or performance
  demands it, the upgrade path is bare-metal (GCE metal instances or other
  providers) **without changing anything above the control plane** — that
  is the point of owning the seam.

### 2.4 What we take from the exe.dev evaluation anyway

The evaluation's *authority-model* conclusions were correct and carry over
verbatim: OpenAgents owns admission, exact accounting, credit charging,
sync projection, typed refusals, and the invariant that org-cloud execution
never routes through another user's machine. Its warm-pool insight also
carries over: **don't boot per message.** Agent computers are provisioned
per *work context* and reused across turns within it (§4), with a small
warm pool to hide boot latency later.

## 3. Billing: agent computers draw against credits

Agent computers are **separately billable**. A user's credit balance is
drawn by two meters:

1. **Model usage** (existing): exact token receipts per turn
   (`token_usage_events`, receipt-first, priced by
   `src/inference/pricing.ts`) — unchanged, landed, #8479 wires the charge.
2. **Agent computer time** (new rail): metered active compute drawn from
   the same credit balance, from the lifecycle receipts the provisioning
   path already emits (`openagents.resource_usage_receipt.v1` — the shape
   `makeLedgerCloudCodingMeteringHook` in `cloud-coding-session-routes.ts`
   was built to produce).

Rules, non-negotiable:

- **Receipt-first, exact-only.** Compute draw is computed server-side from
  provision/active/reclaim lifecycle receipts recorded by the control
  plane. Never from client-supplied values, never from wall-clock guesses
  in the app, never from third-party host metrics.
- **Same ledger, same boundary.** Draws hit the existing Pool B msat ledger
  (atomic, idempotent per receipt, `CHECK (balance_msat >= 0)`), respecting
  the RL-3 asset boundary. One balance; two clearly-labeled receipt kinds.
  The mobile balance/history UI (#8480, landed) and the Aiur credits
  console (#8500) itemize both.
- **Pre-flight + in-flight gating.** Admission (#8474) requires a positive
  balance before an agent computer is assigned or a turn dispatched;
  exhaustion mid-run finishes or stops per a documented policy (#8479
  decides and documents it) and emits a typed `insufficient_credit` event
  the app renders.
- **Pricing is an owner decision.** The rate (e.g. credits per active
  minute; whether idle-attached time bills at a lower rate or zero) is set
  by the owner before launch — file it as a NEEDS_OWNER item with a
  recommended default computed from actual GCE host cost + margin, do not
  invent it in code. The meter must be visible to the user *before* they
  dispatch (simple line in the composer/settings: what a turn costs).
- **Idle is bounded, not billed forever.** Reclaim policy (§4) exists
  precisely so a forgotten agent computer cannot silently drain a balance.
  Idle-reclaim timestamps come from the same lifecycle receipts.

Why bill compute separately at all: agent time and model tokens are
genuinely different costs (a long test suite burns compute with few
tokens; a chat-heavy turn is the reverse), separately metering them keeps
margins honest per the pricing engine's own cost model, and "your agent
computer" as a billable object is *legible to users* in a way "org-cloud
executor overhead amortized into token margin" never will be.

## 4. Lifecycle and isolation posture (the #8476 content, decided)

- **Unit**: one agent computer per admitted **work context** — MVP: one
  per user per active thread-with-repo-binding (the `repoBinding` contract
  from #8472). Turns within the same thread reuse the same agent computer
  (warm workspace, incremental work); a different repo/thread gets a
  different one.
- **States**: the `cloud/` repo's 8-state workroom lifecycle is the model —
  requested → provisioning → ready → active(turn) → idle → reclaiming →
  reclaimed, with a quarantine/failed branch. Every transition emits a
  receipt (billing + Aiur ops both read these).
- **Reclaim**: idle timeout (MVP default: reclaim after a bounded idle
  window, e.g. 15–30 min, owner-tunable), hard turn-timeout ceiling (the
  existing 14400s ceiling in the session routes), and explicit reclaim on
  sign-out/thread-delete. Reclaimed = microVM destroyed, scratch disk
  wiped. Nothing persists between provisions except what went back to
  GitHub or into Khala Sync.
- **Credentials inside the microVM**: short-lived, repo-scoped tokens via
  the SCM broker seam only (#8475); no raw user OAuth tokens, no org
  provider master keys, no wallet material on the agent computer, ever.
  The workspace credential scanner runs before closeout/writeback.
- **Network**: egress-restricted by host policy to what coding work needs;
  the agent computer serves no inbound traffic (progress flows out through
  the runtime's authenticated connection to OpenAgents).
- **Blast radius statement (honest)**: a fully compromised agent computer
  exposes that user's checked-out repo, that turn's scoped token, and its
  own runtime credential — and nothing of any other user's. That sentence
  is the isolation contract #8476 enforces and tests.

## 5. End-to-end flow (what MVP ships)

1. Mobile user (GitHub-signed-in, #8468–#8470) binds a repo to a thread
   (#8471/#8472) and dispatches a turn — the app pushes
   `chat.appendMessage` + `runtime.startTurn` into Khala Sync, exactly as
   today. **The mobile wire contract does not change.**
2. The Worker's admission gate (#8474) checks: valid mobile session,
   positive credit balance, per-user concurrency/rate, org capacity.
   Refusals are typed (`insufficient_credit`, `rate_limited`,
   `org_capacity_unavailable`) and rendered honestly in the app.
3. Admission ensures an agent computer for the thread's work context —
   reuse if one is ready/idle; otherwise placement via the armed
   `cloud-coding-session-routes.ts` → `oa-codex-control` → Firecracker
   microVM on our GCE host (provision receipt → billing meter starts per
   §3).
4. The runtime inside the agent computer consumes the intent (the #8473
   executor, unchanged), materializes the repo checkout (scoped credential
   via #8475), runs the turn (Gemini default; per-user model config from
   #8484), and streams `runtime_event`/`runtime_turn` entities into the
   thread scope — the mobile app renders them live, push notifies on
   completion (#8485/#8486).
5. Results write back as a branch/PR under the user's GitHub authorization
   (#8477); the link lands in the thread and the push notification.
6. Exact token receipts post to `/api/khala/cloud/runtime-turn-usage`
   (landed); lifecycle receipts record compute time; #8479 charges both
   against the balance; #8480's UI and Aiur (#8500/#8501) show the
   itemized draw.
7. Idle reclaim per §4. Aiur's ops view (#8501) shows the agent-computer
   fleet: states, owners, active turns, receipts, health.

## 6. What this changes in the issue tracker

- **NEW #8503 (AC-1)** — arm and finish the Firecracker agent-computer
  provisioning path (the issue filed alongside this doc): agent-computer
  image with the runtime baked in, GCE nested-virt host bring-up via
  `cloud/`'s `oa-node`, arming `CLOUD_CODING_SESSIONS_ENABLED` +
  `gceProvisioningArmed` + `OA_CLOUD_CONTROL_URL` against the real control
  plane, work-context↔agent-computer assignment, lifecycle receipts
  flowing, and the first real mobile turn executed inside a microVM.
  Public-repo seam work now lives in
  `apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.ts`
  and `apps/pylon/deploy/agent-computer/`: the Worker projects Agent Computer
  work-context/lifecycle/resource receipt refs from `cloud.gce.*` events, and
  the public GCE host script enforces nested virtualization and `/dev/kvm`
  verification. The issue remains open until the owner-gated live host/image
  receipts and first real mobile-dispatched microVM turn are recorded.
- **#8474** (admission) — unchanged in substance; capacity now means
  agent-computer capacity from the control-plane ledger. exe.dev pool
  framing dropped.
- **#8475** (repo checkout) — unchanged in substance; the SCM-broker
  credential is delivered into the agent computer. exe.dev GitHub
  Integration option dropped — our broker is the only path.
- **#8476** (isolation posture) — rescoped to §4 of this doc: document and
  enforce the Firecracker per-work-context posture; the exe.dev
  persistent-VM trust model is out.
- **#8477** (writeback) — unchanged; exe.dev `--act-as-user` notes moot.
- **#8479** (metering) — expanded: charges **both** meters (tokens +
  agent-computer time), owns the mid-run exhaustion policy, and surfaces
  the pre-dispatch cost line. The compute rate itself is NEEDS_OWNER.
- **exe.dev audit doc** — banner added: evaluation retained, recommendation
  superseded by this strategy.
- Launch audit §12 — S1 lane redirected to this doc's plan.

## 7. Later (explicitly not MVP)

Warm-pool pre-provisioning for instant first turns; per-user persistent
volumes ("your agent computer remembers your build cache") as a paid tier;
bare-metal hosts for density; multiple sizes/classes of agent computer at
different rates; regional placement; the SHC lane as a second placement
target (the routes already model it); agent computers as a directly
rentable primitive beyond Khala Code turns. None of this blocks launch;
all of it extends the same seams.

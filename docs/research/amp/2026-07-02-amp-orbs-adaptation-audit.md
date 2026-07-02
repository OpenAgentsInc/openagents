# Amp Orbs → OpenAgents Cloud Adaptation Audit (2026-07-02)

Companion to `2026-07-02-amp-orbs-summary.md`. This doc maps what Amp shipped
with orbs onto our existing Cloud/sandbox stack and states what we should
adapt, what we already have, and how it connects to Khala Code.

Context docs this builds on:

- `docs/sandboxes/2026-06-25-openagents-open-source-sandbox-platform-audit.md`
  (tiered isolation, `openagents.sandbox.v1` proposal, snapshot epic)
- `docs/autopilot-coder/2026-06-13-cloud-remote-execution-commercial-plan.md`
  (cost-plus-10% pricing, BYOK vs credits, GCE-first)
- `docs/autopilot-coder/2026-06-14-cloud-desktop-mobile-coding-sessions-full-flow-audit.md`
  (Pylon `openagents-cloud` provider → `/v1/placement`, lane-transparent events)
- `docs/autopilot-coder/2026-06-19-cloud-coding-session-surface-inert-scaffold.md`
  (`POST /v1/cloud-coding-sessions`, placement policy, metering seam)
- `docs/khala/2026-06-25-pylon-linked-coding-capacity-routing-spec.md`
  (Khala as coding orchestrator over the user's own capacity)

## 1. Where we already match orbs

Amp's substrate story is largely what the private `cloud/` repo already runs:

| Orb feature | OpenAgents equivalent | Status |
| --- | --- | --- |
| Per-thread ephemeral machine | Per-run ephemeral GCE VM via `oa-codex-control`; per-session workroom via `oa-node`/`oa-workroomd` | Live |
| Agent runs unsupervised in-box | `codex exec` / `opencode run` inside workroom lifecycle | Live |
| Minute billing | `resource_usage_receipt.v1` (VM-seconds, cost-plus-10%) | Seam live, pricing not public |
| Secrets injection | Workroom `secret_policy` + secret-free metadata endpoint | Live |
| Remote preview/terminal | Managed preview ingress (hashed tokens), capability gateways | Partially live; public preview proxy + PTY are open items |
| Quotas | `openagents.compute_quota_routing.v1` (4/2 per owner, 20/10 per org, 8h TTL) | Live |

We are not behind on isolation — we're ahead (Firecracker provisioner exists,
trust-tier placement policy, content-addressed closeout, quarantine, receipts).
What orbs have that we don't is **product shape**: one command, one fresh
machine per thread, pause-free-resume economics, and a repo-owned setup
contract.

## 2. What to adapt (ranked)

### A. Repo lifecycle hooks: `.agents/setup` / `.agents/resume`

The single highest-leverage adaptation. Today our workroom bring-up is
runtime-owned; the repo has no standard way to declare "here is how to make
this checkout agent-ready." Adopt the same two-file contract (we can honor the
exact `.agents/setup` / `.agents/resume` paths — it's becoming a de-facto
convention alongside `AGENTS.md`, which we already use):

- `oa-workroomd` runs `.agents/setup` after clone, before the first session
  turn; failures surface as a typed workroom event, not a silent broken box.
- `.agents/resume` runs on workroom resume with a bounded blocking budget
  (Amp uses 10s; adopt the same ceiling as a profile knob).
- Enforce under the existing sandbox profile (`timeout_ms`, `network_policy`,
  `secret_policy` apply to the hook too — a setup script is attacker-adjacent
  input on public-tier repos, so trust-tier placement gates what it may do).

### B. Post-setup snapshots with TTL

Orbs snapshot after setup and reuse for 24h. Our sandbox platform audit
already scopes this (Firecracker + R2 diff snapshots, Epic C) as a real build.
Adaptation: make the snapshot key `(repo, ref of .agents/setup + lockfiles,
image version)` with a 24h TTL, so warm starts skip dependency install. This
is the difference between "VM in ~2 min" and "agent working in seconds," and
it directly reduces billed minutes — which matters more for us than for Amp
because our locked pricing is cost-plus-10%, so faster setup is customer-visible
price, not just margin.

### C. Pause/resume economics

"A paused orb costs nothing" is a pricing promise backed by suspend + resume
hooks. GCE suspend (or Firecracker snapshot-to-R2 once that lane is default)
plus `.agents/resume` gives us the same: idle workrooms stop metering in
`resource_usage_receipt.v1` instead of hitting the 30m idle kill. Archived
thread → paused workroom → resume-on-message is a much better fit for
Khala Code's thread model than kill-and-recreate.

### D. Fixed shape, minute billing, one price

Orbs are 16 vCPU / 32 GB at $1.66/hr, no configurator. Adopt the same
simplification for the first sellable sandbox SKU (`cloud.sandbox_compute_service.v1`
is red partly because pricing is unresolved): one shape, one posted price,
billed by the minute in credits or sats (Spark primary; BTC discount per the
agent-cloud revshare doc). Resource customization later, exactly as Amp
sequenced it.

### E. Agent-ergonomics conventions (repo-side, cheap, do now)

These cost nothing on the backend and pay off in every lane including local:

- **Port registry**: dev servers write to a `.amp/dev-ports.json`-style file
  (ours: `.openagents/dev-ports.json`) instead of hardcoding ports; Khala Code
  and the QA harness read it.
- **Dev-only auth endpoints** in the openagents.com Worker dev mode
  (`/__dev/log-me-in`, `/__dev/preflight`) so cloud QA/coding agents stop
  burning turns on WorkOS/OpenAuth flows. Must be compiled out or flag-gated
  off in production builds.
- **Unified log inbox**: a single `.openagents/in/` directory workrooms tail,
  with browser console forwarded and tagged — pairs directly with the
  autonomous-QA runner design (2026-06-24 doc).
- **Setup-time agent guidance**: `.agents/setup` writes environment-specific
  notes the runtime injects into the session context (we already layer
  AGENTS.md; this adds the "you are in a workroom, here's what's true here"
  layer).

### F. `sync` back to local

`amp sync <thread-id>` mirrors orb changes to the local checkout. Our
equivalent seam is content-addressed artifact closeout + git writeback (the
known make-or-break gap in the 06-14 infra-mismatch audit). Adaptation:
`pylon sync <sessionId>` pulling the workroom diff via the existing artifact
adapter — worth building *after* git writeback is confirmed green, since a
branch push + local fetch covers 90% of the need.

### What NOT to adapt

- **Postgres/Redis baked into the base image**: Amp tuned the image for their
  own monorepo. Our base image should stay thin; `.agents/setup` + snapshots
  make per-repo services fast without bloating every workroom.
- **GitHub-only auth posture**: orbs ship an authenticated `gh`; our
  trust-tier placement means public-tier workrooms must NOT get ambient repo
  credentials. Keep credentials behind the capability gateways.
- **Subscription-account seats**: nothing in orbs changes our non-waivable
  rule — API-key inference only in cloud lanes.

## 3. Connection to Khala Code

Khala Code (browser at `apps/openagents.com`, desktop at
`clients/khala-code-desktop`) is the surface where orb-shaped UX lands:

1. **Thread → workroom binding.** Orbs' core UX is "new thread = fresh
   machine." Khala Code threads get an optional cloud lane: a thread bound to
   a workroom via the existing flag-gated `POST /v1/cloud-coding-sessions`
   Worker surface, whose `CloudCodingRuntimeAdapter` stub is exactly where the
   `cloud/` `POST /v1/placement` + GCE/workroom lease plugs in. Archive thread
   → pause workroom (C above); reopen → `.agents/resume`.
2. **Lane-transparent events.** Pylon's `openagents-cloud` provider already
   maps `codex_workroom_event.v1` → `SessionEvent`, and the Khala capacity
   routing spec puts execution on resumable-SSE Durable Streams. Khala Code
   renders one stream regardless of lane; the orb-style "remote with local
   feel" (file diffs, filesystem browse, terminal) rides the managed preview
   ingress + a PTY gateway (the one genuinely new gateway to build).
3. **Routing policy.** Per the 2026-06-25 capacity spec, Khala orchestrates
   coding onto the caller's own linked Pylon capacity (single-user, firm
   invariant). The workroom lane slots in as the *elastic* option in the same
   typed router: own Pylon when available, metered workroom when not — same
   thread UX, different receipt. Own-capacity stays free-of-compute-charge;
   workroom minutes settle through the existing credit ledger seam
   (`cloud-metering.ts`).
4. **`khala -x` parity.** Amp's `amp -ox "prompt"` maps to a Khala CLI /
   Khala Code command palette action: "run this thread in the cloud" — one
   command from prompt to unsupervised workroom execution, results back on
   the same Durable Stream.

## 4. Sequencing (proposed, not promised)

1. Lifecycle hooks in `oa-workroomd` (A) + repo-side conventions (E) — small,
   unblocks everything else, improves even the local lane.
2. Confirm git writeback green end-to-end (pre-existing RED), then
   thread↔workroom binding behind `CLOUD_CODING_SESSIONS_ENABLED` (§3.1).
3. Snapshots with 24h TTL (B) — the latency/price lever.
4. Pause/resume metering (C) + single-SKU pricing (D) — flips
   `cloud.sandbox_compute_service.v1` toward green with a posted price.
5. PTY gateway + `pylon sync` (F) — the "local feel" polish.

All of this stays inside decided owner policy: GCE-first/SHC-second,
cost-plus-10%, trust-tier placement before any VM, no subscription resale,
clean-room (no Daytona — and no Amp code either; these are public docs only).

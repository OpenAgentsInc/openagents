# OpenAgents Cloud Remote Execution — Commercial Plan

Date: 2026-06-13
Status: planning document. Changes no runtime invariant and makes no public
claim by itself. Any invariant amendment or public-copy change called for here
is tracked as its own issue with the tests the relevant invariant ledger
requires.

## Implementation Status (updated 2026-06-14)

**All of Cloud remote-exec C-0 through C-15 are filed as openagents issues
#4886–#4901 and are CLOSED as COMPLETED.** C-0/C-1/C-2/C-3/C-4/C-8 landed in
`openagents` (`apps/pylon` + docs); C-5/C-6/C-7 landed as contracts in the
private `cloud` repo; the remaining rungs (C-9, C-10, C-11, C-12, C-13, C-14,
C-15) are likewise closed-as-completed across `openagents`, `openagents.com`,
and `cloud`. The foundation wave was delivered via a 9-worker Pylon
multi-session fanout (7 Codex + 2 Claude accounts) across both repos in one
run; later rungs followed.

Decisions are locked (see "Decisions" below): cost-plus 10%, Model-2 ToS
confirmed, full-VM isolation now / microVM next.

Related runtime work shipped alongside: quota-aware account routing (#4884) and
**instant run-level/ambient account failover** — a session whose primary account
is quota-blocked is replaced instantly by another available account within the
same run (no second pass).

> **Note (2026-06-14) — closed ≠ end-to-end demonstrable.**
> Closed-as-completed does **NOT** mean the desktop→cloud→phone loop is
> demonstrable end-to-end; the desktop, mobile, and cloud audits show it is
> not. The integration/repair work is tracked by epic
> [openagents#4996](https://github.com/OpenAgentsInc/openagents/issues/4996)
> and the audit
> `docs/autopilot-coder/2026-06-14-cloud-desktop-mobile-coding-sessions-full-flow-audit.md`.

## Goal

Let **any Pylon user** deploy coding/agent workloads onto **OpenAgents-operated
Google Cloud infrastructure** and pay OpenAgents for that service. Dogfood it
ourselves first (single tenant, our own GCP project), then turn it into a
commercial multi-tenant service.

Two ways a customer brings model access:

1. **BYO-API-key (BYOK).** The customer supplies their own OpenAI/Anthropic (or
   other) API key. OpenAgents bills only for **compute** (the VM/runtime), never
   for model tokens.
2. **OpenAgents Inference Credits.** The customer prepays OpenAgents credits;
   OpenAgents routes inference through **its own commercial API accounts** and
   charges inference cost + a service fee, plus compute. This is a standard
   metered-inference gateway (the OpenRouter model).

## Resale Policy Clarification (canonical — read first)

This is the load-bearing distinction and prior wording across the repo has
sometimes blurred it. Authoritative as of 2026-06-13:

- **Forbidden, non-waivable: reselling/renting/proxying/pooling *subscription*
  accounts.** Consumer subscription logins (ChatGPT Plus/Max, Claude Max OAuth
  sessions) must never be resold, rented, proxied, brokered, or pooled across
  customers. A subscription connection is **never** resale authorization. Our
  pooled subscription accounts (the local Codex/Claude OAuth homes) are an
  **internal dogfooding tool only** and never a customer-facing capacity source.
- **Allowed, normal business: reselling *API inference*.** Selling inference
  bought on OpenAgents' *own* commercial API accounts — "buy credits, we route
  via our API keys for a fee" — is a standard product (OpenRouter and many
  others do exactly this). It is **not** the thing the no-resale rule forbids.
- **Governing invariant:** `apps/openagents.com/INVARIANTS.md` → *Provider
  Capacity Marketplace Gate*. It already encodes this: subscription connection
  is not resale authorization, and *"base inference resale remains blocked
  unless a future policy explicitly authorizes it with tests."* That clause is
  the **authorization path** for Model 2, not a permanent ban. Shipping Model 2
  means writing that authorizing policy plus the enumerated refs/tests
  (metering receipt, pricing policy, ToS boundary, dispatch, assignment receipt,
  settlement receipt), not bypassing the gate.

Surfaces whose "no-resale" language should be reconciled to say *subscription*
explicitly (tracked as issue C-0 below; do not silently rewrite invariant
ledgers or public copy without the required tests/gates):

- `apps/openagents.com/INVARIANTS.md` Provider Capacity Marketplace Gate
- `apps/pylon/README.md` and `apps/pylon/docs/codex-bridge.md` ("no-resale law")
- `docs/autopilot-coder/2026-06-11-provider-peer-tos-compliance-review.md`
- the live public product-promise copy / promise registry

## Product Shape: Client, Control Plane, Settlement

Map onto the crabbox control-plane/data-plane split (see
`docs/autopilot-coder/2026-06-13-crabbox-pylon-audit.md`):

- **Pylon (open source, `openagents/apps/pylon`) = the client.** Gains an
  execution-provider abstraction with an `openagents-cloud` backend. Users keep
  the Pylon they already run and gain a "deploy to OpenAgents Cloud" target.
- **`cloud/` (private, Rust) = the control plane / coordinator.** This is the
  commercial service. Its scope doc already commits to "managed cloud nodes,
  capacity-pool placement, capability brokers, internal accounting hooks,
  workroom orchestration," with `oa-node` (managed daemon), `oa-workroomd`
  (per-session sidecar), and an `openagents-cloud-contract` crate. Remote
  GCE-backed sessions are a new capacity class under this fleet.
- **`cloud/` = private billing logic.** Verified 2026-06-13, `cloud/` already
  has the private-billing primitives: `oa-node settlement` modes (`no-wallet`,
  `internal-accounting`) and the `openagents.resource_usage_receipt.v1` contract
  (run_ref, node_ref, micro-USD amounts, provider token usage, nullable costs),
  deliberately kept separate from public contributor wallet UX. Compute
  metering, pricing/markup, capacity cost, and Model-2 inference cost accounting
  live here.
- **`openagents.com` (public monorepo Worker/D1) = customer-facing money.**
  Already owns credits (`0018_billing_out_of_credits`), commerce catalog,
  reward/referral/payout ledgers, the payment→payout bridge, and the live
  `treasury_transactions` / `nexus_treasury_payout_authority` tables. Customer
  payment acceptance, credit purchase/balance, and invoices belong here, on the
  existing L402/Lightning + card-on-file rails.
- **`autopilot-omega` / `openagents.com` = tenant-facing product/billing UX**
  and the public capacity-marketplace projection authority.
- **Autopilot Desktop (`apps/autopilot-desktop`, Bun/Electrobun + Foldkit) =
  the operator console** for deploying and supervising sessions — local, remote
  (bridge), and OpenAgents Cloud — in one window. It consumes the cloud
  coordinator client and the shared `packages/autopilot-control-protocol`, and is the
  primary GUI for BYO-key vs credits sessions and quota/failover state. See
  `2026-06-13-autopilot-desktop-app-audit.md`.
- **`alpha/` = private strategy/roadmap** for the business framing.

**Do not route new work to the `treasury/` repo.** Verified 2026-06-13 it is a
dormant stub (`planned_no_dispatch`, no invoices/payouts/custody today); it only
becomes relevant at a future Treasury v0.3 LDK-custody cutover extracted from
Nexus. The live settlement surfaces today are `cloud/` (private metering /
`resource_usage_receipt.v1`) + the public-monorepo ledgers/credits + the
Nexus/MDK bridge.

Routing rule: **all private billing logic → `cloud/`; everything else →
the public monorepo (`openagents`/`openagents.com`).** The multi-tenant control
plane, credential brokering, fleet policy, compute metering, and pricing/markup
live in `cloud/`; the open-source client boundary, customer payment acceptance,
credit ledger, and invoice UX live in the public tree. This supersedes the
`CLAUDE.md` line that routed settlement to the `treasury/` repo.

## Credential Handling

`cloud/INVARIANTS.md` already dictates the safe design and we follow it exactly:

- Workrooms consume capabilities **through brokers/local gateways, not raw
  provider secrets on disk**; secret access produces **redacted evidence**
  auditable without leaking material; `danger_full_access` only as an
  **externally isolated VM/container profile** with **session-scoped provider
  auth, no broad host/cloud credentials, and cleanup receipts**.

Applied to the two models:

- **Model 1 (BYO key):** the customer's key travels **point-to-point from the
  Pylon client to the customer's own isolated session VM** (encrypted in
  transit, injected as env at exec, wiped on release). The control plane brokers
  *compute* and **never custodies the customer's model key** — only refs/metering
  cross the control plane. This keeps us out of the "honeypot of everyone's
  provider keys" liability.
- **Model 2 (OpenAgents credits):** OpenAgents' **own** API keys live in the
  control-plane secret broker, **never on the workload host**. Inference flows
  through an OpenAgents-operated gateway (egress-restricted); the session VM
  talks to the gateway, not to the provider directly, so the key is never
  exposed to the (customer-controlled) workload. Per-request metering feeds the
  credit ledger.
- **Subscription accounts:** never enter either customer path. Internal
  dogfooding only.

## What We Sell and How We Bill

- **Pricing principle (owner decision 2026-06-13): cost-plus 10%.** We charge a
  **10% fee over what we pay** — for Model-2 inference this is 10% over our API
  cost whether routed through a gateway (e.g. OpenRouter) or direct to the model
  provider. Compute is billed cost-plus on the same principle; the exact compute
  markup is finalized from Phase-0/microVM benchmarks.
- **Compute** (both models): VM-seconds × class (vCPU/RAM/GPU), egress, storage,
  billed cost-plus over GCP cost. Metering shapes ported from crabbox
  (`worker/src/usage.ts`, active-lease caps, reserved-USD caps).
- **Inference + service fee** (Model 2 only): per-request provider/gateway cost
  **+ 10%**, drawn down from prepaid OpenAgents credits.
- **Settlement** is split: private metering/usage receipts in `cloud/`
  (`resource_usage_receipt.v1`, `oa-node settlement internal-accounting`) →
  customer-facing credit/invoice ledgers in the public monorepo
  (`openagents.com` D1), with the invariant-required refs: metering receipt,
  pricing policy, ToS boundary, dispatch, assignment receipt, settlement
  receipt. No work in the dormant `treasury/` repo.

## Isolation and Abuse

Arbitrary customer workloads on our GCP project is a real abuse surface (mining,
attacks, illegal content, key-exfiltration attempts in Model 2). Required before
external launch: per-tenant ephemeral VM isolation (v1), egress policy, resource
+ spend caps, acceptable-use policy, per-tenant quotas, audit, and a kill
switch. Density/security upgrades later: Firecracker/gVisor microVMs, and
confidential compute (the seeded `firecracker` and `sek8s`/TDX references exist
for exactly this).

## Phased Plan

- **Phase 0 — Dogfood, single-tenant (now).** One GCE VM in our own project.
  Pylon session with composer **local**, `verify`/build **remote** (no model
  creds on the box). Prove sync → run → evidence → release. Either via a static-
  SSH Pylon prototype or by shelling out to `crabbox run --provider gcp` (direct
  GCE via ADC, no coordinator) to observe the full lifecycle. Repo:
  `openagents/apps/pylon`.
- **Phase 1 — Managed control plane, internal multi-user.** Coordinator in
  `cloud/` holds our GCP creds, provisions ephemeral per-session VMs via
  `oa-workroomd`, tracks lease/expiry/cleanup/usage. Pylon targets
  `provider: openagents-cloud`. Whole-session-remote works **without Vertex**:
  Claude via OAuth-token env (internal/dogfood) or BYO/our API key, Codex via
  brokered `auth.json`/API key. Metering on; billed internally. Repos: `cloud/`,
  `openagents/apps/pylon`.
- **Phase 2 — Commercial multi-tenant.** Tenant identity (WorkOS), per-tenant
  isolation + spend caps, acceptable-use/egress/abuse controls, private metering
  in `cloud/` → customer credit/invoice ledger in the public monorepo, billing
  UX. Launch **Model 1 (BYOK)** first. Land **Model 2 (credits)** by writing the
  authorizing policy + tests required by the Provider Capacity Marketplace Gate.
  Repos: `cloud/`, `openagents.com` (D1 ledger/credits),
  `autopilot-omega`/`openagents.com` (UX).
- **Phase 3 — Density and differentiation.** microVM/confidential isolation,
  warm pools, GPU classes, crabbox-style "pond" grouped multi-agent
  environments.

## What to Pull from Crabbox

Reference architecture and (optionally) the Phase 0–1 dogfood engine; the
commercial control plane is `cloud/`-native (Rust). Port **contracts and
control-flow**, not Go:

- SSH target + readiness model (`internal/cli/ssh.go`).
- Dirty-checkout sync: git-manifest, excludes, fingerprint-skip, **mass-deletion
  / huge-transfer guardrails** (`internal/cli/repo.go`).
- Lease records + idempotent release + cleanup receipts.
- Run recorder + bounded event retention (`run_recorder.go`,
  `run_output_events.go`).
- Provider-spec boundary (`provider_backend.go`: `Acquire/Resolve/Touch/Run/
  Release` + feature flags).
- Usage/spend-cap shapes (`worker/src/usage.ts`); brokered artifact upload
  grants (`worker/src/artifacts.ts`).
- **Compute-quota routing** ≈ the account-quota ledger we already shipped
  (#4884): active-session caps, TTL/idle, owner attribution, refs-only
  "couldn't acquire compute" receipt.

## Parallelizable Issue Breakdown (for local fanout)

Designed so the independent rows can run concurrently through the
multi-session/account-pool runner; blocked rows are sequenced. Final repo split
per the routing rule (most land in `cloud/`; the client boundary in
`openagents`).

| ID | Title | Repo | Parallel? |
| --- | --- | --- | --- |
| C-0 | Resale-policy wording reconciliation: scope "no-resale" to *subscription* across INVARIANTS/promise/pylon docs, with copy-gate + promise tests | openagents (+ openagents.com) | ✅ done |
| C-1 | `PylonExecutionProviderSpec` + Effect/typed services: local-process, static-SSH, openagents-cloud backends | openagents/apps/pylon | ✅ done |
| C-2 | Remote workspace sync design: git-manifest, `.pylonignore`, fingerprint-skip, remote git seeding, mass-deletion/large-sync guardrails | openagents/apps/pylon | ✅ done |
| C-3 | SSH target + readiness model (TCP/auth/ready distinctions, fallback ports, proxy) | openagents/apps/pylon | ✅ done |
| C-4 | Durable session/run records: sessionRef, ordered events, retained log refs, artifact refs, cleanup receipts (local backend; coordinator iface) | openagents/apps/pylon (+ cloud iface) | ✅ done |
| C-5 | `cloud/` GCE capacity class: provision ephemeral VM, SSH metadata, firewall, labels, lease lifecycle, cleanup (ADC, our project) | cloud | ✅ done |
| C-6 | Credential broker — Model 1: point-to-point BYO key to isolated VM, env-inject at exec, wipe on release, refs-only evidence | cloud | ✅ done |
| C-7 | Compute metering + quota routing: VM-seconds, active-session/TTL/idle caps, refs-only "no compute" receipt | cloud | ✅ done |
| C-8 | Required-artifact gates for `dev-proof-run.ts` / control sessions | openagents/apps/pylon | ✅ done |
| C-9 | Phase-0 static-SSH remote-verify prototype (composer local, verify remote) end-to-end | openagents/apps/pylon | ✅ done |
| C-10 | Inference gateway — Model 2: OpenAgents API keys in control-plane broker, egress-locked gateway, per-request metering → credit ledger | cloud | ✅ done |
| C-11 | Provider Capacity Marketplace Gate authorization for base-inference resale: policy + metering/pricing/ToS/settlement refs + tests | openagents.com | ✅ done |
| C-12 | Tenant identity + per-tenant spend caps + acceptable-use/egress/abuse controls + kill switch | cloud (+ openagents.com) | ✅ done |
| C-13 | Settlement: private metering/usage receipts in cloud → public-monorepo credit/invoice ledger (compute markup; credits + fee). No treasury repo. | cloud + openagents.com | ✅ done |
| C-14 | `openagents-cloud` provider backend in Pylon client wired to the `cloud/` coordinator | openagents/apps/pylon (+ cloud) | ✅ done |
| C-15 | Compute-cost + isolation benchmarks: full-VM baseline cost/latency, then microVM (Firecracker/gVisor) comparison to set compute markup and the microVM cutover | cloud | ✅ done |

> All C-0 … C-15 rungs above map to openagents issues **#4886–#4901**, all
> closed as completed (status reconciled 2026-06-14). See the "Note" under
> Implementation Status: closed does not mean the desktop→cloud→phone loop is
> demonstrable end-to-end — integration/repair is tracked by epic
> openagents#4996.

## Decisions (resolved 2026-06-13 by owner)

1. **Pricing/markup — RESOLVED: cost-plus 10%.** 10% fee over what we pay for
   API usage (gateway like OpenRouter or direct model usage); compute billed
   cost-plus on the same principle, exact compute markup pending Phase-0/microVM
   benchmarks (C-15).
2. **Provider ToS for Model 2 — RESOLVED: confirmed, proceed.** OpenAgents acting
   as a metered-inference reseller on its own commercial API accounts is
   approved. (Subscription resale remains the separate, non-waivable
   prohibition.)
3. **Public-copy reconciliation — RESOLVED: implementer's discretion.** Handle
   via C-0/C-11; still execute promise-registry/copy-gate edits as tested
   changes, not ad-hoc.
4. **Isolation tier — RESOLVED: full ephemeral VM now; microVM in the near
   future.** Launch on full per-session VMs; move to microVM (Firecracker/gVisor)
   once benchmarks justify it. Benchmark work tracked as C-15.

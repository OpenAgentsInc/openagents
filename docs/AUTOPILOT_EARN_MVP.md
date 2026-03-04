# Autopilot Earn MVP (Beacon Launch Spec)

Date: 2026-03-04  
Status: Draft MVP spec

## 1) Product Intent

This launch is a market signal, not a feature dump.

Core message to users and the ecosystem:

> Paid NIP-90 jobs are live right now, and providers are earning real sats.

The MVP is optimized for three moments only:

1. "I can sell my compute."
2. "There are real paid jobs."
3. "I just earned sats."

If those three moments are not obvious in the first session, MVP failed.

## 2) MVP Outcome

A fresh desktop user can complete this loop in minutes:

1. Open app.
2. Press `Go Online`.
3. Receive and execute at least one paid NIP-90 job.
4. See wallet-confirmed sats increase.
5. Withdraw by paying a Lightning invoice.

No synthetic payout-only states count as success.

## 3) UX Spec: Mission Control First

### 3.1 First screen (offline)

```text
Autopilot - Mission Control

Status: OFFLINE

[ GO ONLINE ]

Network Stats
Providers Online: <n>
Jobs Completed: <n>
Sats Paid: <btc>

Your Earnings
Today: 0 sats
Total: 0 sats

Recent Jobs
(empty)

Wallet: Connected
```

### 3.2 After pressing `GO ONLINE`

```text
Status: ONLINE
Listening for jobs...

[ GO OFFLINE ]

Your Earnings
Today: 0 sats
Total: 0 sats

Recent Jobs
Waiting for first job...
```

### 3.3 During a job

```text
Job #8123
Type: AI Inference
Reward: 50 sats
Status: Running...
```

### 3.4 Completion + payout moment

```text
Job #8123
Type: AI Inference
Reward: 50 sats
Status: Completed

Your Earnings
Today: 50 sats
Total: 50 sats
```

### 3.5 UI rules

- No onboarding maze on first run.
- No settings-first interaction.
- One primary action button (`Go Online`/`Go Offline`).
- Advanced panes may remain available via command palette, but not required for first earnings loop.

## 4) Scope Alignment To Current Repo

This spec aligns to current MVP authority docs:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PANES.md`

Ownership constraints:

- Product workflow/UI behavior lives in `apps/autopilot-desktop`.
- `crates/wgpui` remains product-agnostic UI infrastructure.
- Wallet primitives remain in `crates/spark`.

Existing pane/state surfaces already cover most required fields:

- provider mode: `ProviderRuntimeState`
- earnings: `EarningsScoreboardState`
- request intake: `JobInboxState`
- in-flight lifecycle: `ActiveJobState`
- receipts: `JobHistoryState`
- wallet authority: `SparkPaneState`

## 5) Minimal System Architecture

### 5.1 Desktop provider app (Rust)

Responsibilities:

- connect to relays,
- ingest NIP-90 requests,
- accept and execute supported jobs locally,
- publish result/feedback events,
- settle payment to wallet,
- render deterministic job/payment state.

### 5.2 Seed-demand buyer agent (server)

Purpose: bootstrap demand so first-run users reliably get a paid job.

Loop:

```text
while true:
  publish_paid_nip90_job()
  pay_provider_invoice()
  sleep(3s)
```

### 5.3 Lightning wallet lane

MVP settlement model:

- provider returns invoice,
- buyer pays invoice,
- provider UI only marks payout success once wallet receive evidence exists.

## 6) Protocol Flow (NIP-90 Canonical)

Use the in-repo NIP-90 model (`5000-5999` requests, `6000-6999` results, `7000` feedback).

Do not introduce custom `90/91/92/93` kinds for MVP.

1. Buyer publishes job request (`kind: 5000-5999`, example `5050` or `5930`).
2. Provider sees request and claims/starts work.
3. Provider emits feedback (`kind: 7000`) with `status=processing` (optional but recommended).
4. Provider publishes result (`kind: request_kind + 1000`) with result payload.
5. Buyer pays invoice.
6. Provider records success only when wallet-confirmed payment pointer is present.

Minimum tag expectations:

- request: `i`, `param`, `bid`, optional `relays`
- result: `e` (request id), `p` (customer), `amount` (+ `bolt11`), `status=success`
- feedback: `status` (`processing`/`payment-required`/`success`/`error`)

## 7) Launch Job Types (Controlled By Buyer)

| Job Type | Kind | Local task | Reward |
| --- | --- | --- | --- |
| Hash computation | `5930` | `sha256(random_string)` | 10 sats |
| Tiny inference | `5050` | short prompt inference | 50 sats |
| JSON transform | `5930` | deterministic transform/normalize | 20 sats |
| 5s benchmark | `5930` | fixed compute loop for 5s | 20 sats |

Start with deterministic jobs first (hash/json/benchmark), then add inference.

## 8) Seed Demand + Stress Strategy

Suggested test pool: `200,000 sats`.

Dispatch cadence: every `3 seconds` while pool remains.

Track:

- provider join rate,
- time-to-first-job,
- completion latency,
- payout success rate,
- fail/error distribution.

## 9) Metrics And Public Signal

Primary in-app and public stats:

- providers online,
- jobs posted,
- jobs completed,
- median completion time,
- sats paid,
- failed jobs.

Public endpoint/page target: `openagents.com/stats`.

Add top-line multiplayer signal in app header:

```text
Global Network Earnings Today: <btc>
```

## 10) 24-Hour MVP Cut (Build Order)

1. Ship one-screen Mission Control UI with `Go Online` as the dominant action.
2. Wire status and counters from existing provider/wallet/job state.
3. Ensure job lifecycle visible in one list: waiting -> running -> completed -> paid.
4. Wire seed-demand buyer to continuously publish paid jobs.
5. Enforce wallet-confirmed payout gate in displayed earnings.
6. Add simple public stats aggregation endpoint.

## 11) Out Of Scope (Hard No For MVP)

- complex dashboards,
- distributed scheduler/orchestration,
- reputation economy,
- escrow/dispute systems,
- plugin marketplace UX,
- broad multi-model infra controls.

## 12) Acceptance Criteria

MVP is ready when all are true:

1. User can go from app launch to `Go Online` in one click.
2. First paid job appears quickly (target: < 60s with seed demand active).
3. Job completes and earnings UI increments.
4. Increment is backed by real wallet receive evidence.
5. User can withdraw by paying an external invoice.
6. Stats page reflects live economic activity.

## 13) Narrative Check (Launch Message)

The launch story is:

> Paid NIP-90 jobs are happening now. Providers are earning sats now.

Not:

> Here is a complex provider dashboard.

Everything in this MVP must reinforce that difference.

## 14) Relationship To Existing Earnings Docs

This spec is the first-run provider-beacon cut.

- It complements (does not replace) `docs/MVP.md`.
- It is narrower than autonomous goal/swap operations documented in:
  - `docs/AUTOPILOT_EARNINGS_AUTOMATION.md`
  - `docs/AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`
  - `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`

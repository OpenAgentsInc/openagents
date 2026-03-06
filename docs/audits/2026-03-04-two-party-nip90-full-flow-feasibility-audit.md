# Two-Party NIP-90 Full-Flow Feasibility Audit

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/kernel/`. File paths, issue states, and implementation-status claims here may be superseded by later commits.


Date: 2026-03-04  
Author: Codex  
Scope: `apps/autopilot-desktop`, `crates/nostr/core`, and `/Users/christopherdavid/code/nips/90.md`

## Objective

Assess whether we can run a real two-party test where:

1. Two participants each have distinct Nostr identities and Spark wallets.
2. Each participant can create NIP-90 jobs for the other participant.
3. Jobs are ideally consumable only by the intended other participant.
4. Claim -> execute -> settlement -> paid is end-to-end.

## Executive Verdict

Short answer:

- NIP-90 itself supports "targeting" providers, but not hard exclusivity.
- Current desktop implementation does not yet support full two-sided NIP-90 request publishing or open-network paid settlement completion.
- A true full flow between two desktop participants is not yet possible without additional implementation.

## What NIP-90 Allows (Spec Readout)

From `/Users/christopherdavid/code/nips/90.md`:

- Request `p` tags represent preferred providers, but other providers may still process the job.
  - See: lines 69-70.
- Encrypted request params are supported using `p` and `encrypted` tags (NIP-04 shared secret).
  - See: lines 71-98.
- Results/feedback can include `amount` and optional `bolt11`; `payment-required` is the explicit status for payment gating.
  - See: lines 115-123, 172-193.

Implication:

- "Consumable only by the other person" is not guaranteed by `p` tags alone.
- Practically, encrypted params plus targeted `p` and private/shared relay strategy can make jobs effectively provider-specific.

## Current Repo State

## 1) Identity and wallet separation can be done, but defaults are shared

- Identity path can be overridden per process via `OPENAGENTS_IDENTITY_MNEMONIC_PATH`.
  - `crates/nostr/core/src/identity.rs:8,35-47`
- Spark signer derives from that mnemonic path and stores wallet state under the identity directory.
  - `apps/autopilot-desktop/src/spark_wallet.rs:407-445`
- App settings path defaults to `$HOME/.openagents/autopilot-settings-v1.conf`, so two local instances will collide unless you isolate env/home.
  - `apps/autopilot-desktop/src/app_state.rs:2306-2312`

Conclusion:

- Two identities/wallets are feasible by running two processes with different `OPENAGENTS_IDENTITY_MNEMONIC_PATH` (and ideally different `HOME`).

## 2) Provider ingest is live for request kinds, but not provider-target filtered

- Ingress subscription currently filters only by job request kinds (`5000-5999`) with no pubkey/provider filter.
  - `apps/autopilot-desktop/src/provider_nip90_lane.rs:724-729`
- Request parsing captures tags (including service providers in the underlying model), but desktop mapping does not enforce provider targeting.
  - Parse model supports service providers: `crates/nostr/core/src/nip90/model.rs:337,432,487`
  - Desktop mapping to inbox: `apps/autopilot-desktop/src/provider_nip90_lane.rs:924-991`
  - Service providers are only surfaced in shape text: `apps/autopilot-desktop/src/provider_nip90_lane.rs:1010-1028`

Conclusion:

- Today, targeted `p` requests are not restricted to "only intended provider" in desktop ingest logic.

## 3) Buyer-side request creation is not yet relay-published NIP-90

- `NetworkRequestsPaneAction::SubmitRequest` queues AC intent + local request submission state; it does not publish a NIP-90 request event to relays.
  - `apps/autopilot-desktop/src/input/actions.rs:3590-3715`
- Optional local injection path writes directly into inbox for simulation/dev.
  - `apps/autopilot-desktop/src/input/actions.rs:3670-3692`
- Starter demand path similarly injects local inbox requests instead of publishing to relay.
  - `apps/autopilot-desktop/src/input/actions.rs:3801-3888`

Conclusion:

- "Each side creating real network NIP-90 jobs for the other side" is not yet implemented in desktop.

## 4) Provider-side result/feedback publish exists, but payment handshake is incomplete

- Active-job flow publishes NIP-90 result and feedback events through provider relay lane.
  - `apps/autopilot-desktop/src/input/reducers/jobs.rs:299-382`
- Result/feedback include `amount` but currently pass `None` bolt11 invoice.
  - `apps/autopilot-desktop/src/input/reducers/jobs.rs:324,367`
- Publish success/degraded handling is wired by accepted relay count.
  - `apps/autopilot-desktop/src/provider_nip90_lane.rs:474-521`

Conclusion:

- Provider can publish completion signals, but not a complete invoice-based payment-required loop yet.

## 5) Open-network paid transition is blocked by missing runtime payment pointer wiring

- Active job `Delivered -> Paid` is gated on authoritative `payment_id`.
  - `apps/autopilot-desktop/src/app_state.rs:2507-2547`
- Non-test runtime assignment to `active_job.payment_id` is not present (only test assignments in `app_state.rs`).
  - `apps/autopilot-desktop/src/app_state.rs:4898,4988` (test-only)
- Starter demand has separate wallet-confirmed payout path by matching recent receive payments.
  - `apps/autopilot-desktop/src/input/actions.rs:3938-4051`

Conclusion:

- Open-network claim -> delivered -> paid is not fully closed in current runtime wiring.

## Feasibility Matrix

1. Two identities and two Spark wallets: `Partially yes now`
- Achievable with env/path isolation per process.

2. Each side publishes real NIP-90 job requests to relays from desktop: `No`
- Desktop buyer flow does not publish NIP-90 requests yet.

3. Jobs consumable only by intended other participant: `No (strict), partial (practical)`
- Strict no per NIP-90 semantics (`p` is preference).
- Practical yes only after implementing encrypted request flow + targeted filtering/policy.

4. Full claim -> execute -> paid loop for open-network jobs: `No`
- Paid transition requires payment pointer not currently wired from open-network settlement.

## What We Still Need

## P0 (required for your requested test)

1. Implement buyer-side NIP-90 request publish from desktop.
- Build `JobRequest` events with `i/param/bid/relays/p` and send via relay worker.

2. Add provider-target enforcement policy in ingress.
- If request has `p` providers and local provider pubkey is not listed, ignore by default.

3. Add encrypted request support.
- When `encrypted` tag present, decrypt params/content for targeted provider key.

4. Add open-network payment settlement wiring.
- Resolve wallet-confirmed payment pointer for active jobs and set `active_job.payment_id`.
- Auto-advance or enable authoritative `Delivered -> Paid`.

## P1 (needed for robust two-party UX)

1. Buyer-side subscription/projection for result and feedback events.
- Correlate by request id and display per-request lifecycle.

2. Payment-required + invoice handling.
- Include bolt11 in `amount` when requesting payment and monitor pay/zap outcomes.

3. Two-instance local test profile support.
- Document and/or add explicit profile root overrides to avoid shared `$HOME/.openagents` collisions.

## Recommended Next Test Strategy

Until P0 is implemented:

1. Use two isolated runtime profiles (distinct mnemonic paths, ideally distinct `HOME`) to validate identity/wallet separation.
2. Validate live relay ingress and provider result/feedback publishing between identities.
3. Treat payment closure as unsupported for open-network path (only starter path is wallet-confirmed today).

After P0:

1. Run true A->B and B->A request publication with targeted provider tags.
2. Validate settlement writes authoritative payment pointers and transitions jobs to `Paid`.
3. Add an automated integration harness that spins two identities, publishes targeted jobs, and asserts wallet-authoritative paid completion on both sides.


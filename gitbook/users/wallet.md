[Home](../README.md) · [User Path](README.md) · **Your wallet**

# Your wallet

The Spark wallet that ships with Pylon and Autopilot is your Lightning settlement endpoint. It receives sats from buyers, holds the balance, and authorizes withdrawals when you say so. It derives from the same mnemonic as your Nostr identity, so backing up [`~/.openagents/pylon/identity.mnemonic`](first-run.md) is backing up your wallet.

## What a settled payout looks like

Every paid job that completes goes through the same three-step settlement the kernel signs off on:

1. **Buyer pays the Lightning invoice** — Pylon emitted that invoice as part of its NIP-90 `payment-required` feedback when the kind `5960` request landed.
2. **Spark observes settlement** — your local Spark wallet sees the inbound HTLC clear; balance increments.
3. **Kernel mints `DeliveryBundle`** — the work is delivered, the kind `6960` result event is published, and the kernel records the receipt that ties the payment to the delivery.

In Autopilot the wallet pane shows the new balance the moment step 2 completes. From the CLI, `autopilotctl wallet status` (or the equivalent headless inspector) prints the same number.

## Funding the wallet

For most users the wallet starts empty and earns its way up. But two scenarios call for funding it directly.

### As a provider — generally not required

Pylon doesn't need a starting balance to take work. The settlement-on-delivery model means the wallet only ever increments through paid jobs you complete. Skip funding unless you have a buyer-side reason.

### As a buyer — fund before you spend

If you are using your node to buy compute or data from other providers, you need a starting balance. The Spark wallet exposes a Lightning invoice you can pay from any external Lightning wallet (or a Lightning exchange withdrawal); inbound liquidity is whatever your Spark gateway negotiates.

### As an agentic user

Agents that need to spend (compute buyers, data buyers, autonomous research agents) should be funded by their principal in deliberate top-ups, not given an unbounded standing balance. Cap-then-replenish is the right pattern. The bilateral earn-loop runbook ([`docs/autopilot-earn/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot-earn/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md)) is what running two funded identities looks like in practice.

## Spending from the wallet

Three things the wallet authorizes on your behalf:

| Action | What signs it | What you see |
| ------ | ------------- | ------------ |
| **Withdrawal** to an external Lightning invoice | Spark wallet keys (derived from the mnemonic) | Outbound HTLC, balance decrement, settlement receipt |
| **Buyer payment** for compute or data jobs | Same keys, scoped to the job's invoice | NIP-90 `payment-required` feedback resolves; kind `6960` result lands |
| **Skill / handler-fee remittance** | Same keys | Kernel-minted accounting entry tied to the skill or handler that earned the fee |

In all three cases the authority is the mnemonic. In all three cases the receipt is signed kernel state, not a log line.

## Honest scope — what's wired in v0.1

{% hint style="warning" %}
**Regtest only.** Read this before any of the funding or spending mechanics on this page. The Spark wallet network selector in [`crates/spark/src/wallet.rs:22`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/wallet.rs) silently remaps Testnet and Signet to Regtest — Finding 6 in [`docs/audits/2026-02-26-codebase-code-smell-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md). For v0.1 every payout, balance, and withdrawal on this page is Regtest sats. Honest mainnet posture lands when that finding is resolved. Plan accordingly before you fund or attempt to spend anything material.
{% endhint %}

Two more things to be straight about. Open audit findings, not future-state ambiguity.

{% hint style="warning" %}
**1. Secret material is not masked by default.** The renderer at [`apps/autopilot-deprecated/src/render.rs:329, 350`](https://github.com/OpenAgentsInc/openagents/blob/main/apps/autopilot-deprecated/src/render.rs) prints full secret material in its default state — Finding 5 in [`docs/audits/2026-02-26-codebase-code-smell-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md). Terminal output from Pylon and the wallet pane, in v0.1, contains material you should treat as secret. Don't screen-share Pylon log streams. Don't paste them into bug reports without redacting.

**2. The wallet derives from the same mnemonic as your identity.** This is by design but it is also a single point of failure. There is no separate wallet seed. Backup discipline for the mnemonic is wallet-backup discipline. See [Sovereignty & OpSec](sovereignty.md) for the full custody model.
{% endhint %}

## Watching the balance

Three places the balance is authoritative, in increasing order of trust:

1. The desktop wallet pane (when Autopilot ships) — UI projection of the same snapshot.
2. `autopilotctl` wallet inspectors — the same snapshot, from the CLI.
3. The signed kernel `DeliveryBundle` events on Nostr — the cryptographic record. This is what you'd point an auditor at.

If any of the three disagree, trust the kernel events. Logs and panes are projections; the signed events are the source of truth.

## When something looks wrong

Wallet stuck on "pending"? Balance not updating after a job clears? See [Troubleshooting](troubleshooting.md).

{% hint style="info" %}
**Under the hood.** Spark wallet implementation: [`crates/spark/src/spark_wallet.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/spark_wallet.rs). Wallet network selection: [`crates/spark/src/wallet.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/wallet.rs). Findings 5 and 6 are in the code-smell audit: [`docs/audits/2026-02-26-codebase-code-smell-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md). Broader hardening posture: [`docs/audits/2026-02-27-full-system-hardening-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-full-system-hardening-audit.md).
{% endhint %}

---

**← Previous:** [Go online](go-online.md) · **Next:** [Withdraw](withdraw.md) **→**

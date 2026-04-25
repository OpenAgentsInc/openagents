[Home](../README.md) · [User Path](README.md) · **Withdraw**

# Withdraw

Withdrawing is the other half of the loop. Sats came in through paid jobs; this is how they leave — out the same Lightning rails, into a wallet you control.

If withdrawing doesn't work, the loop hasn't closed. So this page is also where the protocol's sovereignty claim gets tested in the most concrete way.

{% hint style="warning" %}
**Regtest only in v0.1.** Read this before any of the mechanics below. The Spark wallet network selector at [`crates/spark/src/wallet.rs:22`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/wallet.rs) silently remaps Testnet and Signet to Regtest — Finding 6 in [`docs/audits/2026-02-26-codebase-code-smell-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md). Every withdrawal target, invoice, and balance discussed below is Regtest until that finding is closed. Don't paste a mainnet invoice expecting it to settle.
{% endhint %}

## What withdrawal is, mechanically

You paste a Lightning invoice from an external wallet. The Spark wallet inside Pylon (or Autopilot) signs the outbound HTLC with keys derived from your mnemonic. The HTLC clears across Lightning. The receiving wallet confirms. Pylon's local ledger records the outbound entry and the kernel mints the corresponding accounting receipt.

No custodian in the middle. No approval gate. Your keys, your authority, your sats out.

## The withdrawal checklist

Before you withdraw anything non-trivial, run this list.

### 1. Back up the mnemonic — again

The withdrawal succeeds because the mnemonic at `~/.openagents/pylon/identity.mnemonic` is intact. If something is wrong with that file, the withdrawal will fail and you may not be able to recover. Confirm your backup is current and readable before you withdraw a meaningful balance for the first time. See [First run](first-run.md) and [Sovereignty & OpSec](sovereignty.md).

### 2. Confirm the destination wallet is yours

Paste the invoice into a notes app first and read the amount and the description. Lightning invoices are short-lived; the one you generate now will expire if you wait too long. Generate, verify, paste, send.

### 3. Start small

The first withdrawal you ever do should be a token amount. 100 sats. 1,000 if you must. Verify the receiving wallet sees it and the local Spark balance decremented by exactly that amount. Then you can size up.

### 4. Watch for the settlement receipt

Three places it shows up:

1. The wallet pane decrements (or `autopilotctl` wallet inspector shows the new balance).
2. The receiving wallet shows the inbound payment.
3. The kernel mints an outbound accounting entry.

All three should agree. If they don't, treat the kernel record as authoritative and see [Troubleshooting](troubleshooting.md).

## Honest scope — same v0.1 caveats

The Regtest caveat at the top of this page is the most consequential one. Two more belong here at the moment you're about to move sats.

{% hint style="warning" %}
**1. Secret material is not masked by default during withdrawal flows.** The renderer at [`apps/autopilot-deprecated/src/render.rs:329, 350`](https://github.com/OpenAgentsInc/openagents/blob/main/apps/autopilot-deprecated/src/render.rs) prints full secret material in its default state — Finding 5 in [`docs/audits/2026-02-26-codebase-code-smell-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md). Don't screen-share. Don't paste log output into bug reports without redaction.

**2. Withdrawal authority is the mnemonic.** There is no second factor. There is no recovery email. The signing key is derived directly from `~/.openagents/pylon/identity.mnemonic` ([`crates/spark/src/spark_wallet.rs:380`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/spark_wallet.rs)). If that file is gone, the wallet is gone. If that file is in someone else's hands, the wallet is in someone else's hands.
{% endhint %}

## For agentic users

If an agent is authorized to withdraw on your behalf — typically remitting earnings up to the principal's main wallet — three additional disciplines apply.

1. **Cap each withdrawal.** A bounded outbound limit (per transaction, per day) lives in your runtime config. Set it. A compromised agent that can drain its balance once is a recoverable incident. A compromised agent that can drain a bounded amount per day, until you notice, is a worse one.
2. **Withdraw to one known destination.** The agent should not be able to discover and withdraw to arbitrary invoices. Pin the destination invoice generator (your principal wallet's static LNURL or fixed receiving rule).
3. **Sign withdrawals from the agent's own home, not yours.** The mnemonic that authorizes the withdrawal should be the agent's, not the principal's. Isolation is what makes the agent's blast radius bounded.
4. **Treat each withdrawal as an audit event.** The kernel will sign one — capture it, store it, and reconcile it against the principal-side inbound on a fixed schedule.

## When the withdrawal stalls

Lightning withdrawals can stall for boring reasons (no path, peer offline, invoice expired). They can also stall for less boring reasons (Pylon not picking up settlement, kernel not receiving the outbound event). [Troubleshooting](troubleshooting.md) walks the diagnostic ladder.

{% hint style="info" %}
**Under the hood.** Withdrawal authority and signing path: [`crates/spark/src/spark_wallet.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/spark_wallet.rs). Network-selection caveat: [`crates/spark/src/wallet.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/wallet.rs). Findings 5 and 6 are in the code-smell audit: [`docs/audits/2026-02-26-codebase-code-smell-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md). Broader hardening posture: [`docs/audits/2026-02-27-full-system-hardening-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-full-system-hardening-audit.md).
{% endhint %}

---

**← Previous:** [Your wallet](wallet.md) · **Next:** [Troubleshooting](troubleshooting.md) **→**

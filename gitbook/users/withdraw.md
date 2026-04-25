[Home](../README.md) · [User Path](README.md) · **Withdraw your sats**

# Withdraw your sats

{% hint style="warning" %}
This page is a stub. Full user-facing guidance lands ahead of Bitcoin Vegas 2026. Follow [@OpenAgentsInc on YouTube](https://www.youtube.com/@OpenAgentsInc) or [openagents.substack.com](https://openagents.substack.com) for live updates.
{% endhint %}

For the engineering story behind this step, see the [investor pathway](../investors/README.md). The [2026-04-23 earning proof](../investors/09-proof-receipts.md) walks through the end-to-end flow that this page will translate into plain-English desktop instructions.

{% hint style="info" %}
**Pre-emptive guidance for when this stub is filled in (withdrawal UX honesty):**

- Do **not** describe a "masked-by-default" secret-key UX during withdrawal flows. Today's renderer at `crates/nostr/.../render.rs:329, 350` prints full secret material by default (audit Finding 5 in [`docs/audits/2026-02-27-full-system-hardening-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-full-system-hardening-audit.md)).
- Do **not** claim Testnet or Signet withdrawal targets are supported — those network flags are silently remapped to Regtest at [`crates/spark/src/wallet.rs:22`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/wallet.rs) (audit Finding 6). Withdrawal instructions for v0.1 should be Regtest-only or marked clearly as forward-looking.
- Spark withdrawal authority comes from the mnemonic at `~/.openagents/pylon/identity.mnemonic` (see [`crates/spark/src/spark_wallet.rs:380`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/spark_wallet.rs)). Backup-before-withdraw guidance must point users at that file, not at a separate wallet seed.
{% endhint %}

---

**← Previous:** [Watch your wallet balance grow](wallet.md) · **Next:** [Troubleshooting](troubleshooting.md) **→**

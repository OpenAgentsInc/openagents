[Home](../README.md) · [User Path](README.md) · **Watch your wallet balance grow**

# Watch your wallet balance grow

{% hint style="warning" %}
This page is a stub. Full user-facing guidance lands ahead of Bitcoin Vegas 2026. Follow [@OpenAgentsInc on YouTube](https://www.youtube.com/@OpenAgentsInc) or [openagents.substack.com](https://openagents.substack.com) for live updates.
{% endhint %}

For the engineering story behind this step, see the [investor pathway](../investors/README.md). The [2026-04-23 earning proof](../investors/09-proof-receipts.md) walks through the end-to-end flow that this page will translate into plain-English desktop instructions.

{% hint style="info" %}
**Pre-emptive guidance for when this stub is filled in (wallet UX honesty):**

- Do **not** describe a "masked-by-default" secret-key UX. Audit Finding 5 in [`docs/audits/2026-02-27-full-system-hardening-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-full-system-hardening-audit.md) shows that today the renderer at `crates/nostr/.../render.rs:329, 350` prints full secret material by default. Until that lands as a hardened default, the page must either describe what users actually see, or be gated behind the fix.
- Do **not** claim Testnet or Signet support for the Spark wallet. Audit Finding 6 documents that those network flags are silently remapped to Regtest in [`crates/spark/src/wallet.rs:22`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/wallet.rs). Either say "Regtest only for v0.1" or wait for honest network selection to ship.
- The Spark wallet derives from the same mnemonic at `~/.openagents/pylon/identity.mnemonic` (see [`crates/spark/src/spark_wallet.rs:380`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/spark_wallet.rs)). Reuse that path — do not introduce a separate wallet seed.
{% endhint %}

---

**← Previous:** [Go online](go-online.md) · **Next:** [Withdraw your sats](withdraw.md) **→**

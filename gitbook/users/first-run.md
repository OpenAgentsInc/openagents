[Home](../README.md) · [User Path](README.md) · **First-run setup**

# First-run setup

{% hint style="warning" %}
This page is a stub. Full user-facing guidance lands ahead of Bitcoin Vegas 2026. Follow [@OpenAgentsInc on YouTube](https://www.youtube.com/@OpenAgentsInc) or [openagents.substack.com](https://openagents.substack.com) for live updates.
{% endhint %}

For the engineering story behind this step, see the [investor pathway](../investors/README.md). The [2026-04-23 earning proof](../investors/09-proof-receipts.md) walks through the end-to-end flow that this page will translate into plain-English desktop instructions.

{% hint style="info" %}
**Pre-emptive guidance for when this stub is filled in:**

- The canonical user identity authority is the mnemonic at `~/.openagents/pylon/identity.mnemonic` (see [`crates/nostr/core/src/identity.rs:44-46`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/core/src/identity.rs)). All first-run instructions must anchor to that path.
- Do **not** reference `~/.openagents/nostr/identity.json` as the user-facing identity location — that path appears as drift in `SettingsDocumentV1`'s default at `crates/autopilot-desktop/.../app_state.rs:1372` and is not the authoritative identity store. Mixing the two will confuse users and is flagged in [`docs/audits/2026-02-28-full-codebase-architecture-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-28-full-codebase-architecture-audit.md).
- The Spark wallet (covered on the next page) derives from this same mnemonic — first-run UX should make it clear that one mnemonic seeds both Nostr identity and Lightning wallet.
{% endhint %}

---

**← Previous:** [Download](download.md) · **Next:** [Go online](go-online.md) **→**

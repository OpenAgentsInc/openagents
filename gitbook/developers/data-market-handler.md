[Home](../README.md) · [Developer Path](README.md) · **Data Market handler**

# Build a Data Market handler

{% hint style="warning" %}
This page is a stub. The full handler guide is coming. In the meantime, the spec in [`packages/data-market-mvp/README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/packages/data-market-mvp/README.md) is the source of truth.
{% endhint %}

## The three NIP-90 kinds

| Kind   | Role    | What it carries                                |
| ------ | ------- | ---------------------------------------------- |
| `5960` | Request | buyer asks for a machine service               |
| `6960` | Result  | provider delivers the result (and the receipt) |
| `31990`| Handler | provider's public capability advertisement     |

{% hint style="info" %}
**Honest scope (when this stub is filled in):**

- Kind `31990` as published today is **NIP-89-shaped** but not yet fully NIP-89-conformant. Tracking issue and field-by-field gap is captured in [`docs/audits/2026-02-27-nostr-full-vision-nip-gap-analysis.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-nostr-full-vision-nip-gap-analysis.md).
- The same gap analysis flags six **Tier-A canonical NIPs** still pending in [`crates/nostr/core`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/nostr/core): NIP-42 (auth), NIP-65 (relay lists), NIP-17 (DMs), NIP-57 (zaps), NIP-47 (NWC), NIP-98 (HTTP auth). Any handler-author guidance on the page below should not assume these are wired yet.
- Authoritative spec for the local handler runtime today remains [`packages/data-market-mvp/README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/packages/data-market-mvp/README.md) plus [`crates/data-market/`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/data-market).
{% endhint %}

## Live relay set

- `wss://relay.damus.io`
- `wss://relay.primal.net`

## What to read next

- [Investor Chapter 6 — Data Market MVP](../investors/06-data-market-mvp.md) has the full architectural walkthrough.
- The [Compute Market glossary](../shared/glossary/compute-market.md) explains how kind `5960` becomes a buyer seat for paid compute on the same open protocol.

---

**← Previous:** [Quickstart](quickstart.md) · **Next:** [Economy Kernel integration](kernel-integration.md) **→**

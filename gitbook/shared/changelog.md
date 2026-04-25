[Home](../README.md) · **Changelog**

# Changelog

A running record of what's shipped on-chain, on-relay, and on-package. Everything here is verifiable against the [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) repo or [npm](https://www.npmjs.com/package/@openagentsinc/pylon).

**You will learn:**
- What actually shipped, when, and where to verify it
- Which releases are production-grade vs. preview
- How to follow along as new builds drop

---

## 2026

### April 23 — Pylon v0.1.13
- **Package:** [`@openagentsinc/pylon@0.1.13`](https://www.npmjs.com/package/@openagentsinc/pylon)
- **Commit:** [`8590d04a`](https://github.com/OpenAgentsInc/openagents/commit/8590d04a)
- **What shipped:**
  - Data-market handler reference (NIP-90 kinds 5960 / 6960 / 31990)
  - 25-sat default job price, 6,400-sat daily cap
  - Relay wiring for `wss://relay.damus.io` and `wss://relay.primal.net`
  - First verifiable earn proof: `0 → 25 sats`, payout id `019db8a2-98d2-7890-95e4-6a1d78709a3c`
- **Docs:** [Pylon Provider](../investors/05-pylon-provider.md), [Data Market MVP](../investors/06-data-market-mvp.md)

### April (rolling) — Contributor cadence
- 256 contributors tracked across the Pylon + Kernel work streams
- ~10-minute mean review cadence on active PRs

---

## What counts as "shipped"

We only list a change here once it is one of:

1. **Published to npm** under `@openagentsinc/*` (e.g. Pylon releases)
2. **Merged to `main`** on [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) with a git tag
3. **Observed on a live relay** (Damus, Primal) with a verifiable event id

Anything else — Spacetime experiments, internal branches, draft ADRs — lives in the repo, not here.

---

## Subscribe

- **GitHub releases:** [OpenAgentsInc/openagents/releases](https://github.com/OpenAgentsInc/openagents/releases)
- **npm feed:** [@openagentsinc/pylon](https://www.npmjs.com/package/@openagentsinc/pylon?activeTab=versions)
- **X / Nostr:** follow [@OpenAgentsInc](https://x.com/OpenAgentsInc)

---

**← Back:** [Home](../README.md) · **Next:** [Glossary](glossary/README.md) **→**

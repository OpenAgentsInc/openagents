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
  - Data-market handler relay wiring to `wss://relay.damus.io` and `wss://relay.primal.net` (Data Market only — the desktop `Relay Connections` / `Sync Health` panes are still on seeded local state for v0.1, see [Ch. 3](../investors/03-autopilot-wedge.md#the-pane--command-surface))
  - First verifiable earn proof on the headless `pylon` lane: `0 → 25 sats`, payout id `019db8a2-98d2-7890-95e4-6a1d78709a3c`
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

### Hardening-gate status (for honesty)

A separate — and more demanding — bar is the repo's strict production hardening lane, [`scripts/lint/strict-production-hardening-check.sh`](https://github.com/OpenAgentsInc/openagents/blob/main/scripts/lint/strict-production-hardening-check.sh). That gate's current state per the [2026-02-28 architecture audit](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-28-full-codebase-architecture-audit.md):

- **`pylon`** — passes the strict lane
- **`autopilot-desktop`** — in remediation (e.g. `eprintln!` violations across `input.rs`, `codex_lane.rs`, `app_state.rs`; `clippy-warning-budget-check` compile failures)

We intentionally *do not* require strict-hardening pass for an entry here — otherwise `autopilot-desktop` shipping moments would be invisible — but we list the gate so readers can tell "on npm" from "on npm *and* through the strict production lane."

---

## Subscribe

- **GitHub releases:** [OpenAgentsInc/openagents/releases](https://github.com/OpenAgentsInc/openagents/releases)
- **npm feed:** [@openagentsinc/pylon](https://www.npmjs.com/package/@openagentsinc/pylon?activeTab=versions)
- **X / Nostr:** follow [@OpenAgentsInc](https://x.com/OpenAgentsInc)

---

**← Back:** [Home](../README.md) · **Next:** [Glossary](glossary/README.md) **→**

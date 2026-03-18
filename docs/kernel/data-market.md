# Data Market

> Canonical market-status doc: [markets/data-market.md](./markets/data-market.md)

## Purpose

The Data Market prices access to useful context under explicit permissions.

It exists because machine work depends on more than raw model capability. It also depends on access to datasets, artifacts, stored conversations, local project context, and private knowledge that improve outcomes.

## Core objects

- `DataAsset`
- `AccessGrant`
- `PermissionPolicy`
- `DeliveryBundle`
- `RevocationReceipt`

## Authority flows

- register asset
- offer grant
- purchase access
- issue delivery bundle
- revoke access

## Settlement model

Access settles against explicit permissions, bounded terms, and receipted delivery.

The kernel should be able to answer:

- what was purchased
- under which permissions
- for how long
- with what delivery evidence
- under what revocation or refund conditions

## Current implementation status

- `implemented`: starter authority and authenticated read-model flows in `openagents-kernel-core` and `apps/nexus-control` for asset registration, access grants, grant acceptance, delivery bundles, and revocation receipts
- `implemented`: dedicated `Data Seller`, `Data Market`, and `Data Buyer` panes in the desktop app
- `implemented`: `autopilotctl data-market ...` and `autopilot_headless_data_market` for shell-first and no-window operation
- `implemented`: repo-owned `autopilot-data-seller`, `autopilot-data-market-control`, and `autopilot-data-seller-cli` skills for conversational and CLI-first agent flows
- `implemented`: targeted NIP-90 data-vending transport with request kind `5960`, result kind `6960`, and NIP-89 handler/capability kind `31990`
- `local prototype`: richer provenance modeling, pricing, and private-data packaging still live mostly in docs and desktop-local concepts
- `planned`: broader discovery, payout, provider economics, and richer buyer/product UX

## How it is used now

Desktop users can currently:

- author and publish assets/grants through the dedicated `Data Seller` pane
- inspect canonical market state through the read-only `Data Market` pane
- publish a narrow targeted request through `Data Buyer`
- receive seller-side targeted requests, publish `payment-required` when needed, issue deliveries, and revoke access

Operators and shell-first users can currently:

- drive the same app-owned state machine through `autopilotctl data-market ...`
- run the no-window runtime via `autopilot_headless_data_market`
- package local files or directories deterministically through `scripts/autopilot/data_market_package.py`
- verify the full local and public-relay flows through:
  - `scripts/autopilot/headless-data-market-e2e.sh`
  - `scripts/autopilot/headless-data-market-public-e2e.sh`
  - `scripts/autopilot/verify-data-market-cli-headless.sh`

Agents can currently:

- use `skills/autopilot-data-seller/` for the conversational seller pane
- use `skills/autopilot-data-market-control/` for the typed tool contract
- use `skills/autopilot-data-seller-cli/` for shell-first packaging and publication

Current detailed runbook and status docs:

- [../headless-data-market.md](../headless-data-market.md)
- [markets/data-market.md](./markets/data-market.md)

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
- `local prototype`: richer provenance modeling, pricing, and private-data packaging still live mostly in docs and desktop-local concepts
- `planned`: broader discovery, payout, provider economics, and product-facing UX

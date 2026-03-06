# Data Market

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

- `implemented`: none
- `local prototype`: provenance and data-source references exist in kernel docs and local modeling, but not as a standalone market
- `planned`: asset registry, grants, purchases, delivery receipts, revocation, and provider payout

# Omni Accepted Outcome Economics v1

Date: 2026-06-05

Status: implemented for issue #214.

## Purpose

Accepted outcome economics records the internal/operator-safe economics of a
fulfilled workroom without creating payment settlement, payout eligibility, or
revenue-share claims.

The model supports Sites and non-Sites workrooms. It is intentionally separate
from billing credits, Site commerce, referral rewards, MDK/LDK settlement, and
Pylon payout ledgers.

## D1 Record

`omni_accepted_outcome_economics` records:

- `id` and unique `idempotency_key`;
- required `workroom_id`;
- optional accepted outcome contract id;
- work kind;
- funding mode;
- buyer price asset;
- buyer price cents;
- credits charged;
- sats charged;
- runner cost cents;
- provider cost cents;
- retry cost cents;
- review minutes;
- review cost cents;
- artifact cost cents;
- derived total cost cents;
- accepted value cents;
- derived gross margin cents;
- public caveat ref;
- optional internal caveat ref;
- explicit `no_settlement_implication`;
- bounded metadata;
- lifecycle timestamps.

## Funding Modes

Supported modes:

- `free_beta`
- `credit_funded`
- `sats_funded`
- `internal_only`

Rules:

- `free_beta` cannot record buyer charges.
- `credit_funded` requires credits and forbids sats charges.
- `sats_funded` requires sats and forbids credits charges.
- `internal_only` cannot record buyer charges.

## Math

All monetary and usage values are integers.

`total_cost_cents` is derived from:

- runner cost;
- provider cost;
- retry cost;
- review cost;
- artifact cost.

`gross_margin_cents` is derived from accepted value minus total cost. It can be
negative.

## Projection Split

Public projection exposes only:

- workroom id;
- work kind;
- funding mode;
- public caveat ref;
- no-settlement implication flag.

Operator projection includes the full economics record.

## Guardrails

`recordOmniAcceptedOutcomeEconomics`:

- records idempotently by `idempotency_key`;
- requires an existing active workroom;
- requires economics `workKind` to match the workroom;
- validates optional accepted outcome contract refs;
- rejects negative or non-integer values;
- validates funding mode constraints;
- rejects raw provider, run-log, email, payment, settlement, payout, wallet,
  token, invoice, preimage, customer-private, and secret-like material in refs
  or metadata;
- always records `noSettlementImplication: true`.

## Boundaries

This slice does not:

- charge credits;
- collect sats;
- send invoices;
- mark a payment settled;
- create payout eligibility;
- publish revenue shares;
- connect to MDK/LDK settlement.

It creates the internal economics substrate that future billing, Site commerce,
referral, Pylon, and MDK/LDK policy layers can consume only through
receipt-backed write paths.

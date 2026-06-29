# Omni Public Proof Bundle v1

Date: 2026-06-05

Status: implemented for issue #216.

## Purpose

Public proof bundles export a redacted proof package from a private workroom.
They are designed for public/customer proof pages and agent APIs without
exposing private workroom mechanics.

This version explicitly does not create settlement claims, payout eligibility,
or payment-settled assertions.

## D1 Record

`omni_public_proof_bundles` records:

- `id` and unique `idempotency_key`;
- required `workroom_id`;
- work kind;
- status;
- legal-sensitive flag;
- source refs;
- artifact refs;
- receipt refs;
- review state ref;
- acceptance state ref;
- economics caveat ref;
- optional legal caveat ref;
- privacy caveat ref;
- public receipt ref;
- explicit `no_settlement_implication`;
- bounded metadata;
- lifecycle timestamps.

## Projection

`publicOmniProofBundleProjection` exposes the proof bundle record after all refs
have passed public-safe validation.

`operatorOmniProofBundleProjection` includes bounded metadata for operator
debugging.

## Guardrails

`createOmniPublicProofBundle`:

- records idempotently by `idempotency_key`;
- requires an existing active workroom;
- requires proof bundle `workKind` to match the workroom;
- requires legal-sensitive bundles to include a legal caveat ref;
- rejects raw provider payloads, provider-account material, raw run logs, raw
  emails, customer-private emails, payment/wallet material, settlement refs,
  payout refs, token-like refs, invoices, preimages, and secret-like material;
- always records `noSettlementImplication: true`.

## Boundaries

This slice does not:

- publish a route;
- make a private workroom public;
- claim settlement;
- claim payout eligibility;
- charge credits or sats;
- expose private logs.

It creates the durable public-safe proof package that projection and public
claim layers can consume.

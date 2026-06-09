# Accepted Outcome Contract v1

Date: 2026-06-05

Status: implemented for issue #209.

## Purpose

Accepted outcome contracts define what a work item is supposed to produce
before the workroom lifecycle, evidence bundle, acceptance lifecycle, Mission
Briefing, and economics layers expand around it.

The contract is intentionally not a payment, payout, settlement, or public
claim. It records expected artifacts, review policy, acceptance state, proof
policy, free/paid posture, and closeout requirements using public-safe refs.

## D1 Record

`omni_accepted_outcome_contracts` records:

- `id` and unique `idempotency_key`;
- `work_kind`;
- `subject_ref`;
- optional `customer_ref`;
- typed `expected_artifacts_json`;
- `review_policy`;
- `acceptance_state`;
- `proof_policy`;
- `economic_state`;
- typed `closeout_requirements_json`;
- `legal_sensitive`;
- `public_receipt_ref`;
- bounded `metadata_json`;
- lifecycle timestamps.

Supported work kinds:

- `site`
- `coding`
- `adjustment`
- `existing_project_import`
- `business`
- `legal_sensitive`

## Contract Fields

Expected artifact entries include:

- artifact kind;
- required flag;
- public-safe flag;
- source ref.

Closeout requirement entries include:

- requirement kind;
- required flag;
- source ref.

Review policies are:

- `operator_review`
- `customer_review`
- `dual_review`
- `owner_review`
- `no_review`

Acceptance states are:

- `draft`
- `pending_review`
- `provisionally_accepted`
- `accepted`
- `rejected`
- `revision_requested`
- `reopened`
- `unavailable`

Proof policies are:

- `private_receipt`
- `customer_safe_summary`
- `public_safe_proof`
- `legal_sensitive_private`

Economic states are:

- `free_beta`
- `paid_required`
- `credits_required`
- `sats_required`
- `internal_only`

## Service Contract

`createOmniAcceptedOutcomeContract`:

- records idempotently by `idempotency_key`;
- requires at least one expected artifact;
- requires at least one closeout requirement;
- validates all refs as public-safe refs;
- rejects raw provider, run-log, email, payment, wallet, token, invoice,
  preimage, customer-private, and secret-like material;
- requires legal-sensitive work to use `legal_sensitive_private` proof policy;
- rejects `public_safe_proof` contracts that require private artifacts.

`publicOmniAcceptedOutcomeContractProjection` exposes:

- work kind;
- subject ref;
- review policy;
- acceptance state;
- proof policy;
- economic state;
- legal-sensitive flag;
- expected artifact count;
- closeout requirement count;
- public-safe expected artifact refs only;
- public receipt ref.

It excludes metadata, customer refs, private expected artifact refs, and any
payment or settlement material.

## Boundaries

This slice does not:

- create workrooms;
- accept or reject work;
- request revisions;
- generate Mission Briefings;
- record economics;
- send emails;
- create payment evidence;
- grant payout eligibility;
- dispatch or settle funds.

It creates the accepted-outcome contract layer that later Omni workroom issues
can reference.

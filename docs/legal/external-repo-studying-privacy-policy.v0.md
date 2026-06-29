# External-Repo Studying Pilot — Customer Data Privacy Policy

Policy ref: `policy.external_repo_study_privacy.v0`
Version: `v0`
Effective date: 2026-06-20
Status: **published_inert** — this policy text is published, but the
external-repo-studying pilot itself remains gated and inert by default. No
customer repository is ingested, stored, fetched, unpacked, or studied unless a
separately flag-armed, owner-signed pilot run is authorized per
`proof.claim_upgrade_receipts.v1`.

> This is the customer-facing privacy policy / data-processing agreement (DPA)
> that the privacy-review and self-serve upload preflights reference. It is the
> canonical published target for a study's `dataProcessingAgreementRef` /
> `retentionPolicyRef`. Machine-readable mirror + content digest:
> `packages/probe/packages/runtime/src/benchmark/external-repo-studying-privacy-policy-registry.ts`.

This policy governs how OpenAgents handles a customer's external (non-OpenAgents)
repository when that customer participates in the gated external-repo-studying
pilot (`autopilot.external_repo_studying_pilot.v1`). It does not describe a
self-serve, generally-available ingestion product; the pilot is admission-gated.

## 1. Scope

This policy applies to:

- external (non-OpenAgents) repositories a customer authorizes for study; and
- the refs, digests, counts, and attestations exchanged at the pilot boundary.

This policy does **not** apply to public OpenAgents benchmark material, which is
governed by the StudyBench private-split boundary
(`docs/research/machine-studying/openagents-studybench/private-boundary.md`).

The OpenAgents repository (`OpenAgentsInc/openagents`) is never a valid customer
study target under this policy.

## 2. Boundary: what crosses, and what never does

The pilot control surface operates on **references and counts only**
(`sourceBoundary = "customer_refs_withheld"`). The following NEVER cross into any
public projection, log, marketplace listing, or shared record:

- raw repository file content or archive bytes;
- repository file paths beyond the external repo slug;
- secrets, credentials, tokens, or wallet/payment material;
- uploader/contributor PII values; or
- reviewer notes.

The following references MAY be recorded, because they disclose no content:

- a stable customer ref and uploader/contributor ref;
- a manifest digest (`sha256:…`) and declared byte size / file count;
- a clean secret/malware scan attestation ref;
- a data-processing-agreement ref, retention-policy ref, and
  customer-authorization ref; and
- a derived `privacyReviewRef` (only once a review would clear).

## 3. Lawful basis and authorization

OpenAgents processes a customer's external repository only with the customer's
recorded authorization (`customerAuthorizationRef`) under a signed
data-processing agreement (`dataProcessingAgreementRef`). A study with no
recorded authorization or no DPA does not clear privacy review and is blocked at
preflight.

## 4. Declared PII categories (closed set)

A pilot study may declare only the following PII categories. Any declared
category outside this closed set blocks the privacy review:

- `none` — the study declares it touches no customer PII;
- `contributor_handle`;
- `commit_author_email`; and
- `code_comment_text`.

This closed set is enforced in code by
`PrivacyReviewAllowedPiiCategory` in
`packages/probe/packages/runtime/src/benchmark/external-repo-studying-privacy-review.ts`.

## 5. Retention

Study artifacts are retained for the declared retention window only, which must
not exceed **365 days** (`PRIVACY_REVIEW_MAX_RETENTION_DAYS`). A declared window
that is non-positive, non-integer, or above the cap blocks the privacy review.
A real armed pilot run additionally enforces retention against the actual DPA and
durable-storage controls; this published cap is the conservative outer bound.

## 6. Data subject rights

Customers may request access to, correction of, or deletion of artifacts derived
from their authorized study, and may withdraw authorization. Withdrawal halts any
further study and triggers deletion of derived artifacts within the declared
retention window or sooner.

## 7. No widened claims

Publication of this policy does not, by itself, ingest any repository or grant any
customer, marketplace, payout, or settlement claim. The pilot remains inert and
gated: `intakeAdmitted`, `ingested`, `reviewCleared`, `effectsApplied`,
`customerPublicClaimAllowed`, `marketplacePackageAllowed`, and `payoutEligible`
are all false until a separately flag-armed, owner-signed run is authorized per
`proof.claim_upgrade_receipts.v1`.

## 8. What this policy does NOT yet establish

This document publishes the customer-facing privacy terms. It does not by itself
clear the `external_repo_studying_privacy_policy_missing` blocker, which still
requires: a human/legal review against a real customer study; durable,
access-controlled storage that enforces the declared retention window; and an
owner-signed armed clearance with a dereferenceable closeout receipt.

## 9. Versioning

This is version `v0`. Future versions get a new `policy.external_repo_study_privacy.vN`
ref and a new entry in the published policy registry. A study's references are
checked against the registry's published versions; an unknown or stale policy ref
does not satisfy the published-policy check.

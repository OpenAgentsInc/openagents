# autopilot.external_repo_studying_pilot.v1 — vertex-fleet note

State: **yellow** (unchanged — no green flip in this change).

## What this change advances

Blocker advanced: **`blocker.product_promises.external_repo_studying_self_serve_upload_missing`**
(partially — see "What remains").

It builds the smallest genuine missing piece of the self-serve upload control
surface: a **refs-only, inert-by-construction upload intake preflight** that
decides whether a customer/contributor's self-serve upload of an external
(non-OpenAgents) repo *would* be admitted for ingestion into the pilot — without
ever uploading, storing, fetching, unpacking, or studying a single byte.

## What was built

- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-self-serve-upload.ts`
  - `buildOpenAgentsExternalRepoStudySelfServeUploadPreflight(...)`
  - Schema `openagents.external_repo_study_self_serve_upload_preflight.v0`,
    decoded through `validateProbeBenchmarkPublicProjection` + a deterministic
    `preflightHash`.
  - Evaluates an upload **request expressed as refs/digests/counts only**
    (manifest digest, clean-scan attestation ref, privacy-review ref, declared
    byte size, file count, uploader-terms acceptance). No raw file content,
    archive bytes, repository tree, file paths, or uploader PII cross the
    boundary (`sourceBoundary = "customer_refs_withheld"`).
  - Preflight gates: declared size within cap, file count within cap, clean-scan
    attestation present, privacy-review present, uploader terms accepted.
  - **Inert by construction**: `intakeAdmitted`, `ingested`, and `effectsApplied`
    are ALWAYS false; `customerPublicClaimAllowed` / `marketplacePackageAllowed`
    / `payoutEligible` are ALWAYS false. The flag-gated seam
    (`EXTERNAL_REPO_STUDY_SELF_SERVE_UPLOAD_ENABLED`, default OFF) computes
    `wouldIngestWhenArmed` (armed + owner sign-off + preflight passed) but never
    ingests a real upload.
  - Refuses `OpenAgentsInc/openagents` as an upload target.
- `packages/probe/packages/runtime/tests/external-repo-studying-self-serve-upload.test.ts`
  — 8 passing tests (inert default-OFF, armed-ready/blocked, each blocker path,
  OpenAgents-repo rejection, no-leak serialization check).
- Exported from `packages/probe/packages/runtime/src/index.ts`.

## What genuinely remains (blocker NOT dropped)

This preflight is the *decision/control* layer only. The self-serve upload
blocker stays listed because it still requires, all owner/product-gated and out
of scope here:

- real durable, access-controlled upload storage + signed-URL intake;
- real malware/secret-scan execution (this module only checks an attestation
  *ref* exists, it does not run the scan);
- a real customer-data privacy review backing the `privacyReviewRef`
  (overlaps with `external_repo_studying_privacy_policy_missing`, also unaddressed
  here);
- an ARMED ingestion against a real customer upload with a dereferenceable
  closeout receipt and owner sign-off per `proof.claim_upgrade_receipts.v1`.

The remaining blockers (`privacy_policy_missing`, `marketplace_metering_missing`,
`pricing_package_policy_missing`, `payout_settlement_gates_missing`) are
untouched. No promise state changed; any future green flip remains receipt-first
and owner-signed.

---

## Update (2026-06-20): privacy-review preflight

Blocker advanced: **`blocker.product_promises.external_repo_studying_privacy_policy_missing`**
(partially — see "What remains" below; blocker NOT dropped).

This adds the smallest genuine missing piece of the privacy-policy control
surface: a **refs-only, inert-by-construction customer-data privacy-review
preflight** that decides whether a customer-data privacy review for an external
(non-OpenAgents) repo study *would* clear — and, only when it would, derives the
deterministic `privacyReviewRef` that the sibling self-serve upload preflight
already consumes as a presence check. It performs no real legal/privacy review
and processes no customer data.

### What was built

- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-privacy-review.ts`
  - `buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight(...)`
  - Schema `openagents.external_repo_study_privacy_review_preflight.v0`,
    decoded through `validateProbeBenchmarkPublicProjection` + a deterministic
    `preflightHash`.
  - Evaluates a review **request expressed as refs/counts/enums only**
    (DPA ref, retention policy ref, customer-authorization ref, retention window
    in days, and a closed set of declared PII categories). No raw customer data,
    PII values, repository content, or reviewer notes cross the boundary
    (`sourceBoundary = "customer_refs_withheld"`).
  - Preflight gates: DPA present, retention policy present, customer
    authorization present, retention window within cap
    (`PRIVACY_REVIEW_MAX_RETENTION_DAYS = 365`), declared PII categories within
    the allowed closed set.
  - Derives `privacyReviewRef` ONLY when the review would clear; a blocked review
    derives `null`, so it cannot satisfy the upload preflight's privacy-review
    presence check.
  - **Inert by construction**: `reviewCleared` and `effectsApplied` are ALWAYS
    false; `customerPublicClaimAllowed` / `marketplacePackageAllowed` /
    `payoutEligible` are ALWAYS false. The flag-gated seam
    (`EXTERNAL_REPO_STUDY_PRIVACY_REVIEW_ENABLED`, default OFF) computes
    `wouldClearWhenArmed` (armed + reviewer sign-off + preflight passed) but never
    clears a real review or authorizes ingestion.
  - Refuses `OpenAgentsInc/openagents` as a review target.
- `packages/probe/packages/runtime/tests/external-repo-studying-privacy-review.test.ts`
  — 7 passing tests (inert default-OFF, armed-ready/blocked, missing-DPA/auth,
  retention/PII out-of-policy, OpenAgents-repo rejection, no-leak serialization).
- Exported from `packages/probe/packages/runtime/src/index.ts`.

### What genuinely remains (blocker NOT dropped)

This preflight is the *decision/control* layer only. The privacy-policy blocker
stays listed because it still requires, all owner/product-gated and out of scope
here:

- a real, human/legal customer-data privacy review (not just a ref presence
  check) backing an ARMED clearance;
- a published customer-facing privacy policy / DPA for external-repo studying;
- real durable, access-controlled storage and retention enforcement that honours
  the declared retention window;
- an armed clearance against a real customer study with a dereferenceable
  closeout receipt and owner sign-off per `proof.claim_upgrade_receipts.v1`.

The remaining blockers (`self_serve_upload_missing`, `marketplace_metering_missing`,
`pricing_package_policy_missing`, `payout_settlement_gates_missing`) are
untouched. No promise state changed; any future green flip remains receipt-first
and owner-signed.

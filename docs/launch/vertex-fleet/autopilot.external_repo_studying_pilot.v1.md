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

---

## Update (2026-06-20): upload ↔ privacy-review binding

Blocker advanced: **`blocker.product_promises.external_repo_studying_self_serve_upload_missing`**
(partially — see "What remains"; blocker NOT dropped).

This closes the seam between the two existing sibling preflights. Before this
change the self-serve upload preflight's privacy-review gate was only a STRING
PRESENCE check (`privacyReviewPresent = (privacyReviewRef ?? "").trim().length > 0`):
any non-empty string satisfied it, including a forged or stale ref. The privacy
review preflight already derives a real `privacyReviewRef` ONLY when a review
would clear, but nothing forced the upload to consume THAT ref.

### What was built

- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-upload-privacy-binding.ts`
  - `buildOpenAgentsExternalRepoStudyUploadPrivacyBinding(...)`
  - Schema `openagents.external_repo_study_upload_privacy_binding.v0`, decoded
    through `validateProbeBenchmarkPublicProjection` + a deterministic
    `bindingHash`.
  - Takes a privacy-review preflight verdict and an upload request **with its
    `privacyReviewRef` omitted from the input type** (`Omit<…, "privacyReviewRef">`),
    so a caller cannot inject their own ref. The binding derives the ref FROM a
    review that (a) covers the SAME `customerRef` + `repo` and (b) is in
    `review_ready_held` (non-null derived ref), then builds the upload preflight
    from it. A blocked or mismatched review binds NO ref, so the upload blocks on
    `…self_serve_upload.privacy_review_missing` instead of trusting a string.
  - **Inert by construction**: `intakeAdmitted`, `ingested`, `effectsApplied`
    are ALWAYS false (asserted on both the binding and the nested upload
    preflight); `customerPublicClaimAllowed` / `marketplacePackageAllowed` /
    `payoutEligible` are ALWAYS false. `sourceBoundary = "customer_refs_withheld"`.
    Refuses `OpenAgentsInc/openagents` as a target.
- `packages/probe/packages/runtime/tests/external-repo-studying-upload-privacy-binding.test.ts`
  — 7 passing tests (bound-held inert, blocked-review→unbound + upload blocked,
  customer-ref mismatch, repo mismatch, armed-still-inert, OpenAgents rejection,
  no-leak serialization).
- Exported from `packages/probe/packages/runtime/src/index.ts`.

### What genuinely remains (blocker NOT dropped)

The binding is still the *decision/control* layer only. The self-serve upload
blocker stays listed because it still requires, all owner/product-gated and out
of scope here: real durable, access-controlled upload storage + signed-URL
intake; real malware/secret-scan execution; a real customer-data privacy review
backing an ARMED clearance; and an ARMED ingestion against a real customer upload
with a dereferenceable closeout receipt and owner sign-off per
`proof.claim_upgrade_receipts.v1`. No promise state changed; any future green
flip remains receipt-first and owner-signed.

---

## Update (2026-06-20): published privacy policy + registry

Blocker advanced: **`blocker.product_promises.external_repo_studying_privacy_policy_missing`**
(partially — see "What remains"; blocker NOT dropped).

Until now the privacy-review and self-serve upload preflights checked only that a
study's `dataProcessingAgreementRef` / `retentionPolicyRef` were *present* (any
non-empty string passed) — and there was no published, canonical privacy policy
for a customer or reviewer to actually read. This change publishes that policy
and makes it a verifiable, content-hashed, canonical reference.

### What was built

- `docs/legal/external-repo-studying-privacy-policy.v0.md`
  — the customer-facing privacy policy / DPA for the external-repo-studying
  pilot: scope, the refs-only boundary (`customer_refs_withheld`), lawful basis +
  customer authorization, the closed PII-category set, the 365-day retention cap,
  data-subject rights, no-widened-claims, and an explicit "what this does NOT yet
  establish" section. Status `published_inert` — the pilot stays gated.
- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-privacy-policy-registry.ts`
  - `buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry(...)`
  - Schema `openagents.external_repo_study_privacy_policy_registry.v0`, decoded
    through `validateProbeBenchmarkPublicProjection` + a deterministic
    `registryHash`; each published version carries a `termsDigest` =
    `sha256(stableJson(terms))`.
  - The published caps **mirror what the privacy-review preflight enforces** —
    it imports `PRIVACY_REVIEW_MAX_RETENTION_DAYS` and
    `PrivacyReviewAllowedPiiCategory`, so the published policy cannot silently
    drift from the code (a test asserts they match).
  - `isPublishedExternalRepoStudyPrivacyPolicyRef(registry, ref)` closes the
    forgeable-string seam: a policy ref must match a KNOWN published version, not
    just be non-empty.
  - **Inert by construction**: `effectsApplied` / `customerPublicClaimAllowed` /
    `marketplacePackageAllowed` / `payoutEligible` are ALWAYS false. Publishing
    policy text grants no clearance and ingests no repository.
- `packages/probe/packages/runtime/tests/external-repo-studying-privacy-policy-registry.test.ts`
  — 5 passing tests (canonical/inert publish, caps-mirror-code, known-vs-forged
  ref check, deterministic stable hash, no-leak serialization).
- Exported from `packages/probe/packages/runtime/src/index.ts`.

### What genuinely remains (blocker NOT dropped)

Publishing the policy text and its canonical reference does not by itself clear
`privacy_policy_missing`. Still required, all owner/legal-gated and out of scope
here: legal/owner ratification of the policy text; a real human/legal review
against a real customer study (not a ref/digest check); durable,
access-controlled storage that enforces the declared retention window; and an
owner-signed armed clearance with a dereferenceable closeout receipt per
`proof.claim_upgrade_receipts.v1`. The remaining blockers
(`self_serve_upload_missing`, `marketplace_metering_missing`,
`pricing_package_policy_missing`, `payout_settlement_gates_missing`) are
untouched. No promise state changed; any future green flip remains receipt-first
and owner-signed.

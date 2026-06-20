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

## Update (2026-06-20): policy registry ↔ on-disk document digest binding

Blocker advanced: **`blocker.product_promises.external_repo_studying_privacy_policy_missing`**
(partially — see "What remains"; blocker NOT dropped).

The published policy registry already pinned the *structured* terms with a
`termsDigest`, but the actual human-readable legal text a customer reads
(`docs/legal/external-repo-studying-privacy-policy.v0.md`) was bound to the
registry only by a `documentPath` string. Nothing detected drift: the legal text
could be edited (e.g. weakening the retention cap, the PII set, or the
no-widened-claims clause) without changing the registry, or the registry could
point at a stale/forged document. This change closes that document-drift seam.

### What was built

- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-privacy-policy-registry.ts`
  - Adds `documentDigest` (`sha256:…`) to each published policy version, pinning
    the EXACT canonical document content, plus the exported constant
    `EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_DIGEST`.
  - `externalRepoStudyPrivacyPolicyDocumentDigest(text)` — deterministic content
    digest helper, and the registry validator now requires `documentDigest` to be
    a sha256 ref.
  - `isMatchingPublishedExternalRepoStudyPrivacyPolicyDocument(registry, ref, text)`
    closes the document-drift seam exactly as `isPublished…Ref` closed the
    forgeable-ref seam: a study/reviewer can verify the policy text actually
    served matches the pinned content, not just that a path string exists. Tampered
    text or an unknown/empty ref does not verify.
  - **Inert by construction** is preserved: `effectsApplied` /
    `customerPublicClaimAllowed` / `marketplacePackageAllowed` / `payoutEligible`
    remain ALWAYS false; this only pins/verifies published text.
- `packages/probe/packages/runtime/tests/external-repo-studying-privacy-policy-registry.test.ts`
  — 2 new tests (now 7 total): one reads the on-disk legal document, recomputes
  the digest, and asserts it equals the pinned constant (a CI guard against
  doc↔registry drift); one verifies exact text passes while tampered text and
  unknown/empty refs fail.

### What genuinely remains (blocker NOT dropped)

Pinning the document content does not by itself clear `privacy_policy_missing`.
Still required, all owner/legal-gated and out of scope here: legal/owner
ratification of the policy text; a real human/legal review against a real
customer study (not a ref/digest check); durable, access-controlled storage that
enforces the declared retention window; and an owner-signed armed clearance with
a dereferenceable closeout receipt per `proof.claim_upgrade_receipts.v1`. The
remaining blockers (`self_serve_upload_missing`, `marketplace_metering_missing`,
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

---

## Update (2026-06-20): review ↔ published-policy binding

Blocker advanced: **`blocker.product_promises.external_repo_studying_privacy_policy_missing`**
(partially — see "What remains"; blocker NOT dropped).

The published policy registry (previous update) introduced
`isPublishedExternalRepoStudyPrivacyPolicyRef` to close the forgeable-string
seam — but nothing yet forced the privacy-review preflight to actually consume a
KNOWN published policy. The review preflight's DPA / retention gates were still
plain string-presence checks: any non-empty `dataProcessingAgreementRef` /
`retentionPolicyRef` passed, even pointing at no published policy. This change
closes that seam, exactly as the upload↔privacy binding did for the
`privacyReviewRef`.

### What was built

- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-review-policy-binding.ts`
  - `buildOpenAgentsExternalRepoStudyReviewPolicyBinding(...)`
  - Schema `openagents.external_repo_study_review_policy_binding.v0`, decoded
    through `validateProbeBenchmarkPublicProjection` + a deterministic
    `bindingHash`.
  - Takes the published policy registry, a `policyRef`, and a privacy-review
    request **with `dataProcessingAgreementRef` + `retentionPolicyRef` omitted
    from the input type** (`Omit<…>`), so a caller cannot inject their own refs.
    The binding derives both refs FROM a policy ref that matches a KNOWN
    published version (via `isPublishedExternalRepoStudyPrivacyPolicyRef`), then
    builds the review preflight from them. An unknown / forged / empty policy ref
    binds NO refs, so the review blocks on `data_processing_agreement_missing` +
    `retention_policy_missing` instead of trusting arbitrary strings. The matched
    version's `termsDigest` is recorded as evidence.
  - **Inert by construction**: `reviewCleared` / `effectsApplied` are ALWAYS
    false (asserted on both the binding and the nested review preflight);
    `customerPublicClaimAllowed` / `marketplacePackageAllowed` / `payoutEligible`
    are ALWAYS false. `sourceBoundary = "customer_refs_withheld"`. Refuses
    `OpenAgentsInc/openagents` as a target.
- `packages/probe/packages/runtime/tests/external-repo-studying-review-policy-binding.test.ts`
  — 7 passing tests (bound-held inert, forged-ref→unbound + review blocked,
  empty-ref unbound, retention-out-of-policy still blocked, armed-still-inert,
  OpenAgents rejection, no-leak serialization).
- Exported from `packages/probe/packages/runtime/src/index.ts`.

### What genuinely remains (blocker NOT dropped)

The binding is still the *decision/control* layer only. The privacy-policy
blocker stays listed because it still requires, all owner/legal-gated and out of
scope here: legal/owner ratification of the policy text; a real human/legal
review against a real customer study (not a ref/digest check); durable,
access-controlled storage that enforces the declared retention window; and an
owner-signed armed clearance with a dereferenceable closeout receipt per
`proof.claim_upgrade_receipts.v1`. The remaining blockers
(`self_serve_upload_missing`, `marketplace_metering_missing`,
`pricing_package_policy_missing`, `payout_settlement_gates_missing`) are
untouched. No promise state changed; any future green flip remains receipt-first
and owner-signed.

---

## Update (2026-06-20): scan-attestation ↔ upload binding

Blocker advanced: **`blocker.product_promises.external_repo_studying_self_serve_upload_missing`**
(partially — see "What remains"; blocker NOT dropped).

This closes the LAST forgeable-string seam in the self-serve upload preflight —
the malware/secret clean-scan gate — and is the exact next step the previous
"clean-scan attestation registry" update named. The registry already exposed
`isCleanScanAttestationRef(...)` to verify a ref matches a KNOWN, CLEAN
attestation for an exact `(customerRef, repo, uploadManifestDigest)`, but nothing
yet forced the upload to consume only such a ref: the upload preflight's
`scanAttestationPresent` was still a plain string-presence check. This change
binds them, mirroring the existing upload↔privacy binding.

### What was built

- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-scan-upload-binding.ts`
  - `buildOpenAgentsExternalRepoStudyScanUploadBinding(...)`
  - Schema `openagents.external_repo_study_scan_upload_binding.v0`, decoded
    through `validateProbeBenchmarkPublicProjection` + a deterministic
    `bindingHash`.
  - Takes the scan-attestation registry, a candidate `scanAttestationCandidateRef`,
    and an upload request **with its `scanAttestationRef` omitted from the input
    type** (`Omit<…, "scanAttestationRef">`), so a caller cannot inject their own
    ref. The binding derives the upload's `scanAttestationRef` ONLY from a
    candidate that `isCleanScanAttestationRef` verifies as a registry-known CLEAN
    attestation covering the SAME customer + repo + upload manifest digest, then
    builds the upload preflight from it. An unknown / stale / non-clean /
    different-manifest ref binds NO ref, so the upload blocks on
    `…self_serve_upload.clean_scan_attestation_missing` instead of trusting a
    string.
  - **Inert by construction**: `intakeAdmitted`, `ingested`, `effectsApplied`
    are ALWAYS false (asserted on both the binding and the nested upload
    preflight); `customerPublicClaimAllowed` / `marketplacePackageAllowed` /
    `payoutEligible` are ALWAYS false. `sourceBoundary = "customer_refs_withheld"`.
    Refuses `OpenAgentsInc/openagents` as a target.
- `packages/probe/packages/runtime/tests/external-repo-studying-scan-upload-binding.test.ts`
  — 7 passing tests (bound-verified inert, forged-ref→unbound + upload blocked,
  different-manifest unbound, findings-verdict never binds, armed-still-inert,
  OpenAgents rejection, no-leak serialization).
- Exported from `packages/probe/packages/runtime/src/index.ts`.

### What genuinely remains (blocker NOT dropped)

With this, BOTH refs-only gates of the upload preflight (privacy review and clean
scan) are now backed by verified, content-bound references rather than arbitrary
strings. The binding is still the *decision/control* layer only. The self-serve
upload blocker stays listed because it still requires, all owner/product-gated
and out of scope here: real malware/secret-scan EXECUTION (the registry only
mirrors a verdict); real durable, access-controlled upload storage + signed-URL
intake; a real customer-data privacy review backing an ARMED clearance; and an
ARMED ingestion against a real customer upload with a dereferenceable closeout
receipt and owner sign-off per `proof.claim_upgrade_receipts.v1`. The remaining
blockers (`privacy_policy_missing`, `marketplace_metering_missing`,
`pricing_package_policy_missing`, `payout_settlement_gates_missing`) are
untouched. No promise state changed; any future green flip remains receipt-first
and owner-signed.

---

## Update (2026-06-20): clean-scan attestation registry

Blocker advanced: **`blocker.product_promises.external_repo_studying_self_serve_upload_missing`**
(partially — see "What remains"; blocker NOT dropped).

The upload↔privacy binding (earlier update) closed the forgeable-string seam for
the upload preflight's `privacyReviewRef`. The upload preflight's OTHER refs-only
gate — the malware/secret clean-scan attestation — was still a plain
string-presence check: any non-empty `scanAttestationRef` satisfied it, including
a forged or stale string pointing at no real scan, or a clean scan of a
*different* upload manifest. This change introduces the verification primitive
that closes that seam, mirroring the privacy-policy registry.

### What was built

- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-scan-attestation-registry.ts`
  - `buildOpenAgentsExternalRepoStudyScanAttestationRegistry(...)`
  - Schema `openagents.external_repo_study_scan_attestation_registry.v0`, decoded
    through `validateProbeBenchmarkPublicProjection` + a deterministic
    `registryHash`. Records issued scan verdicts as refs/digests/enums/counts
    only; each attestation is bound to a SPECIFIC `(customerRef, repo,
    uploadManifestDigest)` and pinned with a deterministic `attestationDigest`
    (`externalRepoStudyScanAttestationDigest`), from which `attestationRef` is
    derived — so a recorded verdict cannot drift from the manifest it covered.
  - `isCleanScanAttestationRef(registry, ref, { customerRef, repo, uploadManifestDigest })`
    closes the forgeable-string seam exactly as `isPublished…PolicyRef` did: an
    upload's scan ref must match a KNOWN, CLEAN attestation for THAT exact
    manifest. A non-clean (`findings`) verdict, an unknown/empty ref, or any
    customer/repo/manifest mismatch returns false.
  - Validator enforces verdict↔findings-count consistency (clean ⇒ 0 findings,
    findings ⇒ ≥1), unique refs, sha256 digests, and digest/ref recomputation.
  - **Inert by construction**: `effectsApplied` / `customerPublicClaimAllowed` /
    `marketplacePackageAllowed` / `payoutEligible` are ALWAYS false. Recording a
    verdict runs NO scan, reads no repo bytes, and grants no ingestion.
    `sourceBoundary = "customer_refs_withheld"`. Refuses `OpenAgentsInc/openagents`.
- `packages/probe/packages/runtime/tests/external-repo-studying-scan-attestation-registry.test.ts`
  — 7 passing tests (inert clean record, empty registry verifies nothing,
  known-clean-ref-only matching with forged/mismatch rejection, findings never
  verifies clean, verdict/count inconsistency rejected, OpenAgents-repo rejection,
  no-leak serialization).
- Exported from `packages/probe/packages/runtime/src/index.ts`.

### What genuinely remains (blocker NOT dropped)

This registry is the *verification primitive* only. The next step (a future run,
mirroring how the policy registry was later consumed by the review↔policy
binding) is a scan↔upload binding that DERIVES the upload preflight's
`scanAttestationRef` from a registry-known clean attestation, so the upload's
clean-scan gate can no longer accept a caller-supplied string. The self-serve
upload blocker also still requires real malware/secret-scan EXECUTION (this
registry only mirrors a verdict), real durable, access-controlled upload storage +
signed-URL intake, and an ARMED ingestion against a real customer upload with a
dereferenceable closeout receipt and owner sign-off per
`proof.claim_upgrade_receipts.v1`. The remaining blockers
(`privacy_policy_missing`, `marketplace_metering_missing`,
`pricing_package_policy_missing`, `payout_settlement_gates_missing`) are
untouched. No promise state changed; any future green flip remains receipt-first
and owner-signed.

---

## Update (2026-06-20): combined upload-intake binding (both gates at once)

Blocker advanced: **`blocker.product_promises.external_repo_studying_self_serve_upload_missing`**
(partially — see "What remains"; blocker NOT dropped).

The two earlier bindings each closed only ONE of the upload preflight's two
refs-only gates: `upload<->privacy` derives the `privacyReviewRef` but still lets
a caller pass an arbitrary `scanAttestationRef`, and `scan<->upload` derives the
`scanAttestationRef` but still lets a caller pass an arbitrary `privacyReviewRef`.
So a single caller composing one binding by hand could STILL forge whichever ref
that binding left open. Nothing yet derived BOTH refs in one place. This change
adds that composition so the upload has NO forgeable ref input at all.

### What was built

- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-upload-intake-binding.ts`
  - `buildOpenAgentsExternalRepoStudyUploadIntakeBinding(...)`
  - Schema `openagents.external_repo_study_upload_intake_binding.v0`, decoded
    through `validateProbeBenchmarkPublicProjection` + a deterministic
    `bindingHash`.
  - Takes a privacy-review preflight, a scan-attestation registry + candidate
    ref, and an upload request **with BOTH `privacyReviewRef` and
    `scanAttestationRef` omitted from the input type**
    (`Omit<…, "privacyReviewRef" | "scanAttestationRef">`). It derives the
    privacy ref from a cleared, customer/repo-matched review AND the scan ref from
    a registry-verified clean attestation for the same customer + repo + manifest,
    then builds the upload preflight from both. `bound_held` requires BOTH gates
    genuinely backed; each gate independently controls its own ref, so a failure
    in one gate blocks only that gate's upload check (the validator asserts an
    unbacked gate derives no ref and never leaves the upload's gate satisfied).
  - **Inert by construction**: `intakeAdmitted`, `ingested`, `effectsApplied`
    are ALWAYS false (asserted on both the binding and the nested upload
    preflight); `customerPublicClaimAllowed` / `marketplacePackageAllowed` /
    `payoutEligible` are ALWAYS false. `sourceBoundary = "customer_refs_withheld"`.
    Refuses `OpenAgentsInc/openagents` as a target.
- `packages/probe/packages/runtime/tests/external-repo-studying-upload-intake-binding.test.ts`
  — 8 passing tests (both-gates bound + inert, blocked-review→privacy unbound,
  forged-scan→scan unbound, customer mismatch, different-manifest scan unbound,
  armed-still-inert, OpenAgents rejection, no-leak serialization).
- Exported from `packages/probe/packages/runtime/src/index.ts`.

### What genuinely remains (blocker NOT dropped)

With this, BOTH refs-only gates of the upload preflight are now derived in a
single binding from verified, content-bound sources — no forgeable ref input
remains on the self-serve upload path. The binding is still the *decision/control*
layer only. The self-serve upload blocker stays listed because it still requires,
all owner/product-gated and out of scope here: real malware/secret-scan EXECUTION
(the registry only mirrors a verdict); a real customer-data privacy review backing
an ARMED clearance; real durable, access-controlled upload storage + signed-URL
intake; and an ARMED ingestion against a real customer upload with a
dereferenceable closeout receipt and owner sign-off per
`proof.claim_upgrade_receipts.v1`. The remaining blockers
(`privacy_policy_missing`, `marketplace_metering_missing`,
`pricing_package_policy_missing`, `payout_settlement_gates_missing`) are
untouched. No promise state changed; any future green flip remains receipt-first
and owner-signed.

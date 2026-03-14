# Train Security Posture Reference

> Status: canonical `PSI-284` / `#3589` reference record, updated 2026-03-14
> after landing the train security posture layer in
> `crates/psionic/psionic-train/src/security_posture.rs`.

This document records the first explicit train-security contract for rollout
submission inside Psionic.

## Canonical Runner

Run the security-posture harness from the repo root:

```bash
scripts/release/check-psionic-train-security-posture.sh
```

## What Landed

`psionic-train` now owns a train-security controller that connects environment
verification, artifact trust roots, untrusted-worker admission, poisoning
controls, and validator-facing receipts.

The new typed surfaces include:

- `EnvironmentVerificationPolicy`
- `ArtifactTrustRoot`
- `SignedArtifactAttestation`
- `UntrustedWorkerAdmissionPolicy`
- `RolloutPoisoningControls`
- `TrainSecurityPolicy`
- `TrainSecurityController`
- `TrainSecurityReceipt`

## What The Contract Makes Explicit

The security posture now makes these train-specific hardening seams
machine-legible:

- expected environment package identity and digest
- required artifact-verification and package-policy references
- trusted signer roots for rollout artifacts
- minimum signature counts
- untrusted-worker rate limits and burst controls
- required execution-proof posture for untrusted workers
- duplicate-artifact rejection
- duplicate-response-signature quarantine
- validator-policy binding for security receipts

## Pass Criteria

The security posture is green only if all of the following remain true:

- a correctly signed and verified untrusted submission can be admitted
- unsigned or bursty untrusted submissions are rejected
- duplicate response signatures are quarantined instead of blending into the
  accepted stream
- security decisions are surfaced through typed reason codes and receipts

## Current Limits

This issue does not claim that train security is complete. It does not yet
implement:

- network-level attestation or hardware-rooted identity for rollout workers
- canonical authority-side trust-root distribution
- full challenge-response adjudication inside the controller itself
- host binary attestation for environment entrypoints

What it does do is give Psionic one Rust-owned security surface for
environment-package verification, artifact signing, untrusted-worker admission,
and poisoning controls.

# Network Execution Identity Reference

> Status: canonical `PSI-289` / `#3594` reference record, updated 2026-03-14
> after landing runtime manifests and session-claims bundles in
> `psionic-runtime` and `psionic-net`.

This document records the first Psionic-owned contract for policy-meaningful
networked execution identity.

## Canonical Runner

Run the identity harness from the repo root:

```bash
scripts/release/check-psionic-network-execution-identity.sh
```

## What Landed

Two previously planned Psionic-wide contracts now exist:

- `RuntimeManifest` in `crates/psionic/psionic-runtime/src/proof.rs`
- `SessionClaimsBundle` in `crates/psionic/psionic-net/src/lib.rs`

Together they make the network lane say, in typed form:

- which runtime backend and toolchain were actually bound to the lane
- which environment and artifact refs were identity-relevant
- which static config changed execution identity
- which mutable variables were observed but intentionally excluded from the
  identity digest
- which peer key and transport path the claims were bound to
- whether the lane is claiming `none`, `best_effort`, or proof-bearing posture

## Runtime Manifest Contract

`RuntimeManifest` now owns the digest split that the architecture doc had only
described abstractly.

Identity-relevant inputs:

- runtime backend and toolchain
- selected device facts
- validation reference
- environment refs
- artifact bindings
- static config bindings
- claims profile identifier

Mutable-but-non-identity inputs:

- runtime variables such as env vars or live knobs that operators still need to
  see but that must not silently collapse into the same execution identity

The manifest now carries both:

- `identity_digest`
- `manifest_digest`

Changing only mutable variables changes `manifest_digest` while leaving
`identity_digest` stable.

## Session Claims Contract

`SessionClaimsBundle` is now carried inside the authenticated wire envelope in
`psionic-net`, which means the existing transport signature covers the claims
payload directly.

The bundle now binds:

- peer node id
- peer auth public key
- selected transport path kind
- relay id and session tag when the path is relay-bound
- runtime manifest
- proof posture

The current proof posture values are:

- `none`
- `best_effort`
- `proof_bearing`

The current policy posture values are:

- `none`
- `best_effort`
- `required`

## Enforcement And Operator Surfaces

The current enforcement point is `authenticate_incoming_envelope` in
`psionic-net`.

What it does now:

- accepts missing claims when posture is `none`
- accepts missing claims but marks posture unavailable when policy is
  `best_effort`
- rejects missing claims with `ClusterJoinRefusalReason::SessionClaimsMissing`
  when policy is `required`
- rejects detached claims with
  `ClusterJoinRefusalReason::SessionClaimsDetached`
- rejects invalid claims with
  `ClusterJoinRefusalReason::SessionClaimsInvalid`

Operator-facing visibility now exists through `PeerSnapshot.session_claims`,
which surfaces:

- policy posture
- verification disposition
- runtime-manifest identity digest
- runtime-manifest digest
- claims profile id
- proof posture
- degraded detail when verification is unavailable, invalid, or detached

## Current Limits

This issue does not yet implement:

- a separately signed inner claims object distinct from the authenticated wire
  envelope
- external manifest registries or policy-authority evaluation
- kernel or Nexus settlement policy over claims digests
- non-cluster service productization for sandbox or validator session identity

What it does do is close the Psionic-side gap: networked execution now has a
typed manifest-plus-claims contract, explicit degraded modes, real required-path
rejection, and operator-visible posture without reconstructing identity from raw
logs.

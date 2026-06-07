# Pylon v0.2 Artanis bootstrap evidence

Date: 2026-06-07

## Purpose

Record the current Artanis/Pylon launch-bootstrap evidence for the Pylon v0.2
release path.

Artanis is not the Pylon wallet and is not a public provider node. For this
release, Artanis is the training-program overseer path that should turn source
policy into bounded SHC Codex workroom artifacts for the Pylon launch,
continual-learning loop, signature mining, work-order drafts, and proof bundle.

## Source contract

The Artanis bootstrap contract currently lives in the private sibling `cloud`
repo, not in public Pylon:

- Cloud commit: `65972fe286ebe25866f49569901b36925fc0e7dc`
- Contract doc:
  `cloud/docs/contracts/openagents.artanis_bootstrap_assignment.v1.md`
- Bootstrap doc:
  `cloud/docs/bootstrap/CND-055-artanis-pylon-bootstrap.md`
- Fixture:
  `cloud/fixtures/artanis_bootstrap_assignment_v1/pylon-launch-bootstrap.json`

The fixture targets the SHC control lane at `oa-shc-katy-01`, uses
`provider_account_ref` plus `auth_grant_ref`, and requires
`wallet_authority=false`.

Required artifacts:

- `result.md`
- `artanis-source-map.json`
- `pylon-launch-plan.json`
- `continual-learning-plan.json`
- `signature-mining-plan.json`
- `work-order-drafts.json`
- `artifact-manifest.json`
- `proof-bundle.json`

## Verification run

Commands run from the sibling `cloud` repo:

```bash
cargo test -p openagents-cloud-contract artanis_bootstrap_assignment_fixture_parses_and_validates
cargo test -p oa-codex-control artanis_bootstrap
```

Result:

- `openagents-cloud-contract`: 1 Artanis fixture test passed.
- `oa-codex-control`: 2 Artanis bootstrap tests passed.

The control tests prove the bootstrap fixture renders a bounded Codex request
and that a fake `oa-workroomd` can complete the Artanis path, persist
`artanis-bootstrap-assignment.json`, emit the Artanis context events, and
capture the expected fake launch artifacts.

## Live release proof

The local contract and fake-workroomd evidence above was not sufficient by
itself. The live account-backed SHC run has now also completed through:

```text
POST /v1/artanis/bootstrap/start
```

Current accepted proof:

- report:
  `docs/reports/nexus/2026-06-07-pylon-v02-live-artanis-shc-bootstrap-proof.md`
- SHC run id: `artanis.bootstrap.pylon-launch.20260607141825`
- Omega external run id:
  `shc-codex:oa-shc-katy-01:artanis.bootstrap.pylon-launch.20260607141825`
- wallet authority: `false`
- Omega status: `completed`
- required artifacts captured:
  - `result.md`;
  - `artanis-source-map.json`;
  - `pylon-launch-plan.json`;
  - `continual-learning-plan.json`;
  - `signature-mining-plan.json`;
  - `work-order-drafts.json`;
  - `artifact-manifest.json`;
  - `proof-bundle.json`.

The live Artanis run is separate from the MDK/Omega real-bitcoin payment proof;
both are now recorded as release-candidate evidence before the Pylon v0.2
GitHub release is created.

For this MDK-default release, the required payment proof no longer depends on
the old GCP-hosted native Nexus public edge or native-LDK continuity state. The
current accepted payment proof is the Omega Cloudflare MDK sidecar smoke
recorded on 2026-06-07: Worker to Cloudflare Container sidecar, a
100-bitcoin-sat checkout, local MDK wallet payment, merchant status
`PAYMENT_RECEIVED`, and a payer wallet balance delta. A newer equivalent
public-safe proof can replace that evidence.

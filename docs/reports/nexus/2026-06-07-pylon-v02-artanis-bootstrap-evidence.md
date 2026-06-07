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

## Remaining release gap

This is not enough to release Pylon v0.2.

Before creating `pylon-v0.2.0`, OpenAgents still needs one live account-backed
SHC run through:

```text
POST /v1/artanis/bootstrap/start
```

That run must:

- use a fresh approved ChatGPT/Codex provider-account grant;
- keep `wallet_authority=false`;
- run on the no-wallet SHC workroom boundary;
- capture every required artifact listed above;
- retain a public-safe proof receipt that names the Cloud commit, OpenAgents
  release-candidate commit, SHC run id, artifact refs, and closeout status.

The live Artanis run is separate from the Nexus real-bitcoin accepted-work
payout proof. Both are required before the Pylon v0.2 GitHub release should be
created.

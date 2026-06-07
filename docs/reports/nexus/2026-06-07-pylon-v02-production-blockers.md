# Pylon v0.2 Release Gate Status

Date: 2026-06-07

## Current decision

Pylon v0.2 is an MDK-default release. It should not be blocked on the old
GCP-hosted native Nexus public edge, native-LDK continuity state, or local
`gcloud` deploy credentials unless a task explicitly changes that historical
native Nexus lane.

The old blocker chain was:

- public `nexus.openagents.com` returned Cloudflare `530` / `1033`;
- native Nexus treasury status could not be read;
- `#4548` could not verify whether a stale native-LDK
  `continuity_alert:confirmations_stalled` had cleared;
- `#4504` therefore kept the Pylon release tracker open.

That blocker chain is no longer the correct default gate for Pylon v0.2. The
current release path uses:

- Pylon's local MoneyDevKit `agent-wallet` wrapper as the default wallet
  runtime;
- Omega on Cloudflare as the product/payment control plane;
- a Cloudflare Container MDK sidecar for the Node/native MDK checkout route;
- local proof runtime evidence for accepted-work behavior;
- a live account-backed Artanis SHC bootstrap run with `wallet_authority=false`;
- post-release Pylon install and paid-work smoke evidence.

Native LDK remains useful for lower-level regression and hardening, but it is
not the default user wallet runtime and it is not the reason to delay the
MDK-default release.

## Evidence already green

Pylon source evidence:

- Pylon wraps MDK under a Pylon-scoped `HOME`.
- Pylon sets a stable Pylon-scoped `MDK_WALLET_PORT`, avoiding the MDK
  default `localhost:3456` daemon collision across multiple Pylon homes.
- Pylon wallet commands return structured JSON through the Pylon CLI for
  status, balance, history, BOLT11 invoice, BOLT12 offer, and lifecycle
  control.
- The two-home MDK wrapper smoke passed.
- The local `cs336-a1-hosted-starter` proof lane completed with simulated
  accepted-work payout behavior.

Omega Cloudflare MDK evidence:

- Omega deployed a live Cloudflare Container sidecar for `@moneydevkit/core`.
- The Worker route at `https://openagents.com/api/mdk` reaches that sidecar
  through the `MDK_SIDECAR` binding.
- A corrected MDK app binding for `https://openagents.com` was provisioned.
- A signed MDK core ping through production returned HTTP `200`.
- A live production `SAT` checkout for `100` bitcoin sats was created.
- A local funded MDK agent wallet paid the checkout.
- The merchant checkout status reached `PAYMENT_RECEIVED`.
- The payer wallet balance decreased by `100` bitcoin sats.

Cloud/Artanis contract evidence:

- The sibling `cloud` Artanis bootstrap contract and fake-workroomd tests
  passed against Cloud commit `65972fe286ebe25866f49569901b36925fc0e7dc`.
- The remaining Artanis gap is one live account-backed SHC run through
  `POST /v1/artanis/bootstrap/start`.

## Remaining release gates

Pylon v0.2 should not be released until these are true:

- focused Pylon repo checks from `docs/pylon/PYLON_VERIFICATION_MATRIX.md`
  pass from current `main`;
- one live account-backed Artanis SHC bootstrap run completes with
  `wallet_authority=false`;
- the live Artanis run captures:
  - `result.md`;
  - `artanis-source-map.json`;
  - `pylon-launch-plan.json`;
  - `continual-learning-plan.json`;
  - `signature-mining-plan.json`;
  - `work-order-drafts.json`;
  - `artifact-manifest.json`;
  - `proof-bundle.json`;
- a public-safe Artanis/Pylon receipt is committed under
  `docs/reports/nexus/`;
- the public GitHub `pylon-v0.2.0` release assets and npm bootstrap package are
  published from pushed `main`;
- a fresh Pylon home proves the public install/user path;
- a post-release Artanis/Pylon paid-work proof uses the MDK/Omega proof path or
  records a precise remaining settlement-bridge blocker.

## Issue disposition

`#4548` should be closed as superseded for the MDK-default release. The source
fix for native-LDK tracked payout reconciliation remains useful, but the old
native Nexus production deploy/refresh is no longer a Pylon v0.2 release gate.

`#4504` should be closed as complete/superseded after this documentation update
is pushed. Its Spark removal and MDK-wrapper cleanup scope has been completed;
remaining v0.2 release work is tracked by `#4551` and by this release-gate
report.

`#4551` remains the active release issue for the live Artanis bootstrap.

## What still belongs to old native Nexus

If a future task explicitly asks for native Nexus or native LDK regression:

- use `docs/deploy/NEXUS_GCP_RUNBOOK.md`;
- use `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md`;
- verify `https://nexus.openagents.com` health if that old public edge is in
  scope;
- run native-LDK treasury refresh and payout proof as required by that issue.

Those checks are scoped to native Nexus. They are not the normal MDK-default
Pylon v0.2 release gate.

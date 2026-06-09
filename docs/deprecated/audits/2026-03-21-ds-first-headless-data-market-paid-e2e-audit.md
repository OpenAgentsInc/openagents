# DS-First Headless Data Market Paid E2E Audit

Date: 2026-03-21

## Scope

Audit the DS-first headless Data Market launch path after the NIP-DS
publication work and the CLI/headless verification updates.

This pass focused on:

- no-window seller and buyer runtimes
- DS listing and DS offer publication
- DS-backed buyer discovery
- DS-DVM request, payment, delivery, and consume
- the portable repo-owned verification bundle
- the current state of the public-relay strict harness

## Bottom line

The DS-first headless local launch path is ready.

The repo now has:

- a passing no-window smoke publish flow
- a passing portable zero-price local verifier bundle
- a passing paid local DS-first end-to-end run with real Spark payment,
  delivery, and consume

The public-relay harness is updated to the DS-first shape, but live public
relay health was not deterministic in this audit session. The harness is still
useful as an operator probe, but it is not a portable launch gate.

## Fresh passing proofs

### Portable local verification bundle

Command:

- `scripts/autopilot/verify-data-market-cli-headless.sh`

What passed:

- smoke publish flow
- zero-price local DS-first E2E
- seller lifecycle regression:
  `cargo test -p autopilot-desktop data_seller_full_lifecycle_progresses_from_grant_to_revocation -- --nocapture`
- Nexus authority lifecycle regression:
  `cargo test -p nexus-control data_market_flow_receipts_asset_grant_delivery_and_revocation -- --nocapture`

Local zero-price summary artifact:

- `target/headless-data-market-e2e/summary.json`

### Fresh paid local DS-first run

Command:

- `OPENAGENTS_HEADLESS_DATA_MARKET_E2E_RUN_DIR=target/headless-data-market-e2e-ds-first-paid-final2 OPENAGENTS_HEADLESS_DATA_MARKET_BUYER_PREFUND_SATS=20 OPENAGENTS_HEADLESS_DATA_MARKET_PREFUND_PAYER_IDENTITY_PATH=target/headless-data-market-e2e-ds-first-paid-final/buyer-home/.openagents/pylon/identity.mnemonic OPENAGENTS_HEADLESS_DATA_MARKET_PREFUND_PAYER_STORAGE_DIR=target/headless-data-market-e2e-ds-first-paid-final/buyer-home/.openagents/pylon/spark/mainnet scripts/autopilot/headless-data-market-e2e.sh`

What passed:

- seller published DS listing `30404`
- seller published DS offer `30406`
- buyer refreshed the DS-backed selection and derived a DS-DVM draft
- buyer published DS-DVM request kind `5960`
- seller matched the request back to the DS listing and DS offer coordinates
- seller issued `payment-required`
- buyer settled the invoice through Spark
- seller observed the payment and issued a delivery bundle
- seller published DS-DVM result kind `6960`
- buyer observed the result live from the relay
- buyer consumed the local delivery bundle
- consumed payload matched the source dataset byte for byte

Paid local summary artifact:

- `target/headless-data-market-e2e-ds-first-paid-final2/summary.json`

## What changed

### Headless verification is now DS-first

The local smoke and E2E scripts now verify:

- authority read-back contains DS listing and DS offer publication refs
- buyer discovery is anchored on DS-selected listing and offer coordinates
- seller request matching records the DS listing and DS offer coordinates
- local relay logs show `30404`, `30406`, `5960`, and `6960`
- buyer consume still verifies the delivered payload against the source bundle

### Paid local automation is stable enough to use

The E2E harness now:

- retries idempotent `autopilotctl` actions when the desktop drops or times out
  a control response
- recovers `buyer-publish-request` from buyer state read-back when the action
  response is lost
- prefunds the buyer wallet in chunks and waits on an effective spendable floor
  instead of assuming Spark reports a monotonic balance increase after every
  receive

### Headless verification can disable Codex safely

`autopilot_headless_data_market` now honors `OPENAGENTS_DISABLE_CODEX=true`.

The repo-owned smoke and E2E harnesses set that env var because they do not use
the conversational `seller-prompt` path. The normal operator runtime still
keeps Codex available by default, so the documented `seller-prompt` flow
remains intact.

## Current public-relay status

The public-relay harness was updated to the DS-first shape, but the live relay
session was not portable in this audit window.

Observed failures:

- `wss://relay.damus.io` intermittently returned `503 Service Unavailable`
  during DS publish
- Damus plus Primal sessions could also terminate the buyer runtime before
  request publication, producing `Desktop dropped the control action response`
  from `autopilotctl`

Relevant run directories from this audit session:

- `target/headless-data-market-public-e2e-ds-first-final/`
- `target/headless-data-market-public-e2e-ds-first-damus-only/`

Conclusion:

- public-relay DS-first automation is still worth keeping in-repo
- it should remain operator-invoked and relay-health-dependent
- it should not be the portable launch gate

## Launch truth after this audit

The launch-safe DS-first claims are:

- sellers can package and publish DS-backed listings and offers locally
- buyers can discover those listings through the app-owned DS selection state
- the DS-DVM request, payment, delivery, and consume flow works locally end to
  end
- the repo-owned portable verifier proves the local seller and buyer loop

The non-portable claim is:

- strict public-relay automation depends on external relay health and is not a
  deterministic CI-style gate right now

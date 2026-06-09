# Psionic Intro Data Market Sale Audit

Date: 2026-03-18

## Scope

This audit covers one concrete `v0.2` Data Market MVP sale setup:

- create a small sample dataset with introductory Psionic material
- package it with the current seller CLI flow
- publish the asset and starter grant
- verify that a buyer runtime can see the listing
- verify that a buyer can publish a targeted request
- verify that the seller can quote the real `5 sat` payment step

This audit does not claim a full paid delivery completion. The current repo
already documents that the full headless paid buyer path is not yet the
demonstrated happy path.

## Why I Used A Local Authority Run

The already-running desktop app on this machine exposed a normal
`desktop-control.json` manifest and the Data Market tool surfaces were live,
but its broader authority-backed surfaces were reporting:

- `kernel authority unavailable: hosted control endpoint is not configured`

That meant the safe reproducible publish path for this task was not the live
desktop session. I used the documented local MVP path instead:

1. local `nexus-control`
2. local headless seller runtime
3. local headless buyer runtime
4. local relay for the NIP-90 request and feedback loop

That path is exactly the current repo-supported headless Data Market publish
shape described in `docs/headless-data-market.md`.

## Dataset Created

I created a small tracked fixture dataset at:

- `fixtures/data-market/psionic-intro-v0.2/`

It contains:

- `README.md`
- `01-what-is-psionic.md`
- `02-system-architecture.md`
- `03-inference-training-and-backends.md`
- `04-openagents-boundary.md`
- `provenance.json`

The content is a derived introductory bundle built from a review of the current
Psionic repo at `/home/christopherdavid/code/psionic`, especially:

- `README.md`
- `docs/PSI_FOR_HUMANS.md`
- `docs/ARCHITECTURE.md`
- `docs/TRAIN_SYSTEM.md`
- `docs/INFERENCE_ENGINE.md`
- `docs/BACKENDS.md`

This was intentionally a curated intro bundle rather than a raw mirror of the
entire Psionic tree. That keeps the sample dataset small, saleable, and easy to
hand to an agent.

## Packaging Result

I packaged the dataset with the first-party seller CLI wrapper:

```bash
skills/autopilot-data-seller-cli/scripts/package_data_asset.sh \
  --source fixtures/data-market/psionic-intro-v0.2 \
  --output-dir target/data-market-runs/psionic-intro-v0.2-2026-03-18/package \
  --title "Psionic Intro Dataset" \
  --description "A small introductory Psionic documentation bundle derived from the current psionic repo for the OpenAgents data market v0.2 MVP." \
  --asset-kind documentation_bundle \
  --default-policy licensed_bundle \
  --grant-policy-template licensed_bundle \
  --price-sats 5 \
  --grant-price-sats 5 \
  --grant-expires-hours 168 \
  --grant-warranty-window-hours 72
```

Packaging outputs:

- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/package/listing-template.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/package/grant-template.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/package/packaging-manifest.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/package/packaging-summary.json`

Important packaging facts:

- package label: `psionic-intro-dataset`
- asset kind: `documentation_bundle`
- content digest:
  `sha256:c329f43edff702617e9c249ffc859559c590752c7fb746ac8efefd38f11323fc`
- file count: `6`
- total bytes: `7859`
- listing price hint: `5 sats`
- grant price hint: `5 sats`
- policy template: `licensed_bundle`
- starter grant consumer: `null` / open offer

The open-offer grant shape matters. It lets a buyer publish a targeted access
request without me hard-binding the grant to one buyer `npub`.

## Publish Run

I published through an isolated local run directory:

- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/headless-local/`

That run created:

- a local relay
- a local `nexus-control`
- a seller identity and buyer identity
- a seller desktop-control manifest
- a buyer desktop-control manifest

The important generated control manifests are:

- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/headless-local/seller-desktop-control.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/headless-local/buyer-desktop-control.json`

The important publish artifacts are:

- `publish-asset.json`
- `publish-grant.json`
- `buyer-refresh.json`
- `buyer-request.json`
- `request-payment.json`
- `seller-status-after-request.json`
- `buyer-status-after-invoice.json`
- `summary.json`

## What Happened

### 1. Asset published

The asset publish succeeded and read back from kernel authority.

Published asset id:

`data_asset.npub1gqmvgj8ult39dnwufhx37z3tv7mrjmjz7g3g9tf9mhu0cdru9zfqhwlj8m.documentation_bundle.Psionic_Intro_Dataset.sha256_c329f43edff702617e9c249ffc859559c590752c7fb746ac8efefd38f11323fc`

Asset publish receipt id:

`receipt.kernel.data.asset.register:sha256:9d39bfba008b28dd24af63a005436f321076d88981ac9728ca74e2fec880350e`

The asset read-back confirms:

- title: `Psionic Intro Dataset`
- asset kind: `documentation_bundle`
- price hint: `5 sats`
- default policy: `licensed_bundle`
- provenance ref:
  `oa://local-packages/psionic-intro-dataset/c329f43edff702617e9c249ffc859559c590752c7fb746ac8efefd38f11323fc`

### 2. Grant published

The grant publish also succeeded and read back from kernel authority.

Published grant id:

`access_grant.npub1gqmvgj8ult39dnwufhx37z3tv7mrjmjz7g3g9tf9mhu0cdru9zfqhwlj8m.data_asset.npub1gqmvgj8ult39dnwufhx37z3tv7mrjmjz7g3g9tf9mhu0cdru9zfqhwlj8m.documentation_bundle.Psionic_Intro_Dataset.sha256_c329f43edff702617e9c249ffc859559c590752c7fb746ac8efefd38f11323fc.licensed_bundle.open_offer`

Grant publish receipt id:

`receipt.kernel.data.grant.offer:sha256:0c8df4f324d36e5d27ec17da1186b8f68817c8e0ed5d08147dd9a298b85b23c5`

Important read-back facts:

- `consumer_id` is `null`
- grant status is `offered`
- offer price is `5 sats`
- policy id is `licensed_bundle`
- expiry window is `168` hours
- warranty window is `72` hours

### 3. Buyer saw the listing

The buyer runtime refreshed market state and selected the asset successfully.

Buyer-side read-back confirmed:

- local buyer id:
  `npub1njhqm2vhqg4006zz75g9c7yn5n8xf3sqeqtq2gh75ep6v7qg00gq9ztvmd`
- selected asset id matched the published asset
- selected offer grant matched the published grant
- selected offer grant price was `5 sats`
- derived request draft used:
  - `delivery_mode = delivery_bundle_ref`
  - `preview_posture = metadata_only`
  - `bid_sats = 5`
  - `timeout_seconds = 120`

### 4. Buyer published a targeted request

The buyer published a targeted Data Market request for the asset.

Published request id:

`6fa22569be7281d1ec09c7a9f17ed31ad3266ffe251ddd8be2b8718c303cde2c`

Seller read-back after ingest showed:

- the request matched the exact asset
- the request matched the open-offer grant
- `required_price_sats = 5`
- `evaluation_disposition = ready_for_payment_quote`

That is the expected MVP posture for a paid sale before invoice publication.

### 5. Seller issued the payment-required quote

The seller then issued the payment-required step for the request.

Seller-side state after the quote:

- payment state: `awaiting_payment`
- feedback event id:
  `c63584e8810c5fc9cacfa92a1665fc7480b0701f4be30476f4a7c0dc574bfed8`
- settlement payment hash:
  `fbbf883c73f60351f4a62a1e28c8d25a2a42d1ee81d2820849f754db966f9ee8`
- pending invoice amount: `5000 msats`

Buyer-side read-back confirmed the corresponding NIP-90 feedback:

- request status: `payment-required`
- last feedback status: `payment-required`
- last feedback amount: `5000 msats`
- last feedback invoice was present in the buyer observation

That means the run proved the first paid half of the loop:

- listing exists
- grant exists
- buyer can see it
- buyer can request it
- seller can issue the real `5 sat` invoice step

## What This Did Not Prove

This run did not finish:

- invoice payment
- payment settlement observation
- post-payment delivery issuance
- buyer consume-delivery

That is intentional in the audit language. The repo already records that the
full headless paid buyer flow is not yet the demonstrated happy path. I did not
want this audit to overstate what the current MVP has actually verified.

## How A Buyer Can Buy This Dataset

Assume the buyer is using an agent and you are pointing that agent at the repo
skills and the current local runtime surfaces.

### Minimum things to hand the buyer agent

Give the agent:

- `skills/autopilot-data-market-control/SKILL.md`
- a buyer manifest path
- the asset id

In this run, the buyer manifest was:

- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/headless-local/buyer-desktop-control.json`

In a real user run, the buyer can use either:

- the normal desktop-control manifest from a running app session
- or a dedicated headless buyer manifest from `autopilot_headless_data_market`

### Practical buyer-agent flow

The current MVP-safe buyer flow is:

1. Refresh the market snapshot.
2. Select the asset id.
3. Publish a targeted request for that asset.
4. Wait for a `payment-required` feedback event.
5. Read the returned `bolt11` invoice from buyer status.
6. Pay the invoice.
7. Wait for the seller to move into post-payment delivery.
8. Once delivery exists, consume it with `autopilotctl data-market consume-delivery`.

The concrete buyer commands from this run shape are:

```bash
target/debug/autopilotctl --manifest <buyer-manifest> --json data-market buyer-refresh

target/debug/autopilotctl --manifest <buyer-manifest> --json data-market buyer-publish-request \
  --asset-id <asset-id> \
  --refresh-market

target/debug/autopilotctl --manifest <buyer-manifest> provider online --wait --timeout-ms 120000

target/debug/autopilotctl --manifest <buyer-manifest> --json data-market buyer-status
```

The buyer should inspect `buyer-status` for:

- `latest_request.status`
- `latest_request.last_feedback_status`
- `latest_request.provider_observations[*].last_feedback_bolt11`

In this run, the buyer-side status correctly showed:

- `status = payment-required`
- `last_feedback_status = payment-required`
- a real Lightning invoice under `last_feedback_bolt11`

### What to point a seller-side agent at

If the seller wants to reproduce or extend this sale, point the seller agent
at:

- `skills/autopilot-data-seller-cli/SKILL.md`
- `skills/autopilot-data-market-control/SKILL.md`
- the source dataset directory:
  `fixtures/data-market/psionic-intro-v0.2`
- the generated packaging directory:
  `target/data-market-runs/psionic-intro-v0.2-2026-03-18/package`

That gives the agent the package templates, the manifest digest, and the typed
publish discipline the current MVP expects.

## Exact Local Artifacts Worth Keeping

If someone wants to inspect this run later, the most useful files are:

- `fixtures/data-market/psionic-intro-v0.2/`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/package/packaging-summary.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/package/listing-template.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/package/grant-template.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/headless-local/publish-asset.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/headless-local/publish-grant.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/headless-local/buyer-request.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/headless-local/request-payment.json`
- `target/data-market-runs/psionic-intro-v0.2-2026-03-18/headless-local/summary.json`

## Bottom Line

The repo now contains a tracked sample dataset for Psionic onboarding, and that
dataset was successfully listed for sale at `5 sats` through the current Data
Market MVP flow.

What is proven by this audit:

- deterministic packaging works
- asset publish works
- grant publish works
- buyer market refresh works
- buyer request publication works
- seller payment quoting works
- buyer sees the `payment-required` invoice step

What remains outside the proven claim:

- paid settlement completion
- delivery issuance after payment
- buyer-side consume of the paid bundle

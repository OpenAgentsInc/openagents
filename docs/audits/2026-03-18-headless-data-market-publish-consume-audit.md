# Headless Data Market Publish/Consume Audit

Date: 2026-03-18

## Scope

Audit the real headless Data Market MVP flow in `openagents` after wiring:

- seller-side asset/grant publication through `autopilotctl`
- buyer-side targeted request publication
- seller-side delivery issuance
- buyer-side result observation
- buyer-side local payload consume

This audit is based on a successful local run of:

- `scripts/autopilot/headless-data-market-e2e.sh`
- `scripts/autopilot/verify-data-market-cli-headless.sh`

## Bottom line

The headless Data Market MVP now works end to end for a local targeted-access
flow.

A seller can:

- package a local dataset
- publish a `DataAsset`
- publish a targeted `AccessGrant`
- go online for relay intake without a compute-ready runtime
- receive a buyer request
- issue a `DeliveryBundle`
- publish the matching NIP-90 result

A buyer can:

- refresh the visible market snapshot
- publish a targeted NIP-90 request
- go online in a relay-only posture for result tracking
- observe the seller result
- resolve the matching `DeliveryBundle`
- materialize the delivered local payload through `autopilotctl data-market consume-delivery`

The verified consume path copies the delivered payload and manifests into a
buyer-owned output directory and matches the consumed files byte-for-byte
against the original dataset.

## Verified runtime topology

The passing local harness uses:

- one local relay started by `autopilot-headless-compute relay`
- one local `nexus-control`
- one seller desktop session and bearer token
- one buyer desktop session and bearer token
- one seller Nostr identity
- one buyer Nostr identity
- two `autopilot_headless_data_market` runtimes

Authority truth comes from `nexus-control`.
Relay truth for targeted request/result traffic comes from the local relay.
The CLI drives both runtimes through their desktop-control manifests.

## Published event kinds and relays

The successful audited run published the Data Market transport events to the
local relay:

- relay: `ws://127.0.0.1:45773`
- request kind: `5960`
- result kind: `6960`
- handler/capability kind: `31990`

More concretely:

- the buyer published a targeted Data Market request as NIP-90 kind `5960`
  onto `ws://127.0.0.1:45773`
- the seller published the matching NIP-90 result as kind `6960` onto
  `ws://127.0.0.1:45773`
- both seller and buyer published NIP-89 handler/capability advertisements as
  kind `31990`, also accepted on `ws://127.0.0.1:45773`

The relay-local evidence from the verified run shows:

- the request capture recorded `request_kind: 5960` and
  `source_relay_url: ws://127.0.0.1:45773`
- the seller publish log recorded the result event with `accepted_relays=1`
- the buyer result snapshot recorded
  `last_result_relay_urls: [ws://127.0.0.1:45773]`

The runtime also maintained the managed NIP-28 chat lane against
`wss://relay.damus.io`, but that relay was not part of the audited Data Market
request/result publish path. For this audit, the market flow itself was proven
on the local relay only.

## Verified flow

The working E2E path is:

1. Package a local dataset with `scripts/autopilot/data_market_package.py`.
2. Draft, preview, and publish a `DataAsset` through seller `autopilotctl`.
3. Draft, preview, and publish a targeted zero-price `AccessGrant`.
4. Bring the seller online for request intake.
5. Refresh the buyer market view and publish a targeted request for the asset.
6. Bring the buyer online in relay-only mode for result tracking.
7. Wait for seller intake to mark the request `ready_for_delivery`.
8. Stage a local delivery bundle rooted in `file://.../payload`.
9. Prepare and issue the `DeliveryBundle`.
10. Wait for the buyer to observe the NIP-90 result event.
11. Run `autopilotctl data-market consume-delivery`.
12. Verify the consumed dataset matches the original source files.

The current verified harness output includes:

- authoritative `asset_id`
- authoritative `grant_id`
- authoritative `delivery_bundle_id`
- buyer `request_id`
- seller `result_event_id`
- buyer-side consumed payload directory
- copied manifest paths

## What had to be fixed

The flow did not work cleanly before this pass. The main gaps were:

### 1. Seller grant matching treated buyer identity too narrowly

The seller originally compared the grant `consumer_id` only against the raw
request event pubkey. In practice:

- the grant was targeted to buyer `npub`
- the request reached seller evaluation with buyer pubkey in raw hex

Fix:

- seller intake now extracts `buyer_id` / `consumer_id` from the request JSON
  payload
- seller grant evaluation now treats that payload identity as the canonical
  economic consumer identity

### 2. Buyer result matching treated seller identity too narrowly

The buyer originally ignored valid result events when:

- the request targeted seller `npub`
- the result event arrived from seller raw hex pubkey

Fix:

- buyer provider-key normalization now treats `npub` and raw hex as equivalent
  for targeted request/result matching

### 3. Data Market online posture was still compute-gated

The headless MVP originally inherited compute-runtime blockers from Mission
Control, which made both seller intake and buyer result tracking fail if:

- GPT-OSS was not present
- Apple FM was unavailable

Fix:

- seller-side online posture already bypassed compute registration when a data
  vending profile exists
- buyer-side online posture now also bypasses compute preflight when there is
  an active data-market request that needs relay result tracking
- stale compute-runtime error state is cleared for this relay-only posture so
  `provider online` does not fail after queuing the correct transition

### 4. Zero-budget targeted requests were incorrectly rejected

The current Data Market MVP uses zero-price grants in the verified local loop.
The generic request queue previously rejected `budget_sats == 0`.

Fix:

- `openagents.data_market.access_request.v1` now allows zero-budget submission

### 5. Buyer local consume needed a real materialization path

The repo had delivery objects and read models but not a buyer-side command that
turned a delivery into actual files.

Fix:

- `autopilotctl data-market consume-delivery` now resolves a matching delivery
  and copies local payload/manifests into an operator-selected output directory

## Current CLI/headless truth

What works now:

- `autopilot_headless_data_market`
- seller `autopilotctl data-market draft-asset`
- seller `autopilotctl data-market preview-asset`
- seller `autopilotctl data-market publish-asset --confirm`
- seller `autopilotctl data-market draft-grant`
- seller `autopilotctl data-market preview-grant`
- seller `autopilotctl data-market publish-grant --confirm`
- buyer `autopilotctl data-market buyer-refresh`
- buyer `autopilotctl data-market buyer-publish-request`
- seller `autopilotctl data-market prepare-delivery`
- seller `autopilotctl data-market issue-delivery`
- buyer `autopilotctl data-market consume-delivery`
- full shell-first verification through the repo scripts

What the CLI is really doing:

- driving the desktop-owned seller/buyer logic through desktop-control
- not creating a second market implementation
- using kernel objects as canonical economic truth
- using local relay events for the targeted NIP-90 request/result transport

## Important current limitations

The current headless MVP is real, but still narrow.

### Delivery transport is local-first

`consume-delivery` currently supports:

- local `file://...`
- plain local filesystem paths

It is not yet:

- a general remote blob retrieval client
- an encrypted pointer retrieval client
- a resumable remote bundle transport

### Buyer UX is still narrow

The buyer flow is currently:

- targeted request only
- asset selected from a visible snapshot
- no broad discovery/catalog UX
- no generalized buyer transaction workspace

### Payment is not yet the verified happy path

The passing E2E loop uses a zero-price grant and therefore verifies:

- request
- delivery
- result
- consume

It does not yet prove a real paid path with:

- `payment-required`
- invoice settlement
- post-payment delivery

The seller lifecycle tests still cover the authority state machine, but the
full headless buyer payment loop is not yet the demonstrated end-to-end happy
path.

### Headless is still a desktop host

This remains:

- a no-window desktop runtime
- driven by desktop-control manifests

It is not yet:

- a minimal standalone daemon with a separate contract

That is still the correct MVP choice because it keeps one control surface and
one truth path.

## Files that now matter most

- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/data_buyer_control.rs`
- `apps/autopilot-desktop/src/data_seller_control.rs`
- `apps/autopilot-desktop/src/nip90_compute_semantics.rs`
- `apps/autopilot-desktop/src/state/operations.rs`
- `scripts/autopilot/headless-data-market-e2e.sh`
- `scripts/autopilot/verify-data-market-cli-headless.sh`
- `docs/headless-data-market.md`

## Recommended next work

1. Verify a paid headless path end to end with real `payment-required`,
   settlement observation, and post-payment delivery.
2. Add a remote delivery transport path so `consume-delivery` is not limited to
   local files.
3. Add first-class CLI commands for buyer result/status inspection instead of
   using the broader status snapshot as the waiting surface.
4. Add richer audit artifacts around request/result identity normalization so
   operator logs make `npub`/hex equivalence explicit.

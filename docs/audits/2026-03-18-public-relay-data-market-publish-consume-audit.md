# Public Relay Data Market Publish/Consume Audit

Date: 2026-03-18

## Scope

Audit the real public-relay Data Market MVP flow in `openagents` after fixing
seller-side runtime reconciliation for targeted Data Market requests.

This audit is based on a successful strict run of:

- `OPENAGENTS_HEADLESS_DATA_MARKET_REQUIRE_LIVE_INGEST=true scripts/autopilot/headless-data-market-public-e2e.sh`

## Bottom line

The Data Market MVP now has a fully live public-relay headless demo.

In the verified strict run:

- the buyer published the targeted Data Market request to real relays
- the seller ingested that request live through the runtime with no manual
  import
- the seller published the matching delivery result back to real relays
- the buyer observed the result live from those relays
- the buyer consumed the delivered dataset locally and the files matched byte
  for byte

The previous seller-side relay-import fallback is no longer part of the
verified public path. It remains in the repo only as an operator recovery tool.

## Verified relay set

The verified strict run used:

- `wss://relay.damus.io`
- `wss://relay.primal.net`

## Published event kinds

The verified strict run published:

- request kind `5960`
- result kind `6960`
- handler/capability kind `31990`

More concretely:

- the buyer published the targeted Data Market request as kind `5960`
- the seller published the matching delivery result as kind `6960`
- seller and buyer each published NIP-89 handler/capability events as kind
  `31990`

## Verified strict run facts

From the verified strict run:

- `request_id`: `38a334fe1f15e49ef4c64f857186bbfd4078b59f2e51409f3c59721f2815edeb`
- `result_event_id`: `ae75aa2c9d04681c1c9851229f9fe24a9093afa6358c1196750d54a545a78cb9`
- `request_kind`: `5960`
- `result_kind`: `6960`
- `seller_request_ingest_mode`: `live_relay`
- `buyer_result_ingest_mode`: `live_relay`
- `request_source_relay_url`: `wss://relay.primal.net`
- `result_relay_urls`:
  - `wss://relay.damus.io`
  - `wss://relay.primal.net`

From the relay publish logs:

- buyer request publish: `accepted_relays=2`
- seller result publish: `accepted_relays=2`
- seller capability publish: `accepted_relays=2`
- buyer capability publish: `accepted_relays=2`

So the request and result were both accepted on both configured public relays
in the verified strict run.

## What the working public flow looks like

1. Start two no-window runtimes plus local `nexus-control`.
2. Package a dummy dataset into deterministic listing/grant templates.
3. Publish a `DataAsset`.
4. Publish a targeted `AccessGrant`.
5. Bring the seller online for relay intake and result publication.
6. Publish the buyer request as NIP-90 kind `5960` to Damus and Primal.
7. Bring the buyer online in relay-only posture for result tracking.
8. Let the seller runtime reconcile live targeted request intake from the
   configured relays.
9. Stage and issue the `DeliveryBundle`.
10. Publish the seller result as NIP-90 kind `6960` to Damus and Primal.
11. Let the buyer observe the result live from the relays.
12. Consume the delivered dataset locally.
13. Verify the consumed files match the original packaged payload.

## What changed to make this work

The runtime fix was not a protocol rewrite. It was a relay-ingest fix.

The important change was tightening the provider lane reconciliation path so
the seller no longer depends on a manual import step when public relays miss a
long-lived multi-filter subscription update. The lane now:

- refreshes ingress subscriptions more aggressively
- performs direct relay reconciliation queries on a shorter cadence
- queries one filter at a time during reconciliation instead of batching them
  into one ambiguous multi-filter catchup subscription

That was enough to turn the public flow from hybrid into fully live in the
verified strict run.

## Current architectural truth

Economic truth still comes from kernel authority:

- `DataAsset`
- `AccessGrant`
- `DeliveryBundle`

Relay truth is the transport/evidence surface for:

- buyer request publication
- seller result publication
- handler/capability advertisement

The public demo is therefore not “the relays are the market.”
It is:

- kernel authority for canonical market objects
- public relays for the targeted NIP-90 request/result transport

## Remaining limitations

### The recovery commands still matter operationally

The strict verified run did not need manual recovery, but the recovery commands
should remain available because public-relay behavior can still drift:

- `autopilotctl data-market seller-import-request`
- `autopilotctl data-market buyer-import-response`

They are now operator escape hatches, not part of the verified happy path.

### `issue-delivery` still returns before result publication fully settles

The immediate seller action payload may still show delivery publication as
`publishing_result` before the final result event id is reflected everywhere.
The stronger truth is the later publish log plus buyer result observation.

## Judgment

This is now a real public-relay, fully live headless demo.

The precise current truth is:

- request publication to Damus/Primal works
- seller live request intake from those relays works
- result publication to Damus/Primal works
- buyer live result observation works
- buyer consume verification works

That is strong enough to claim an end-to-end public-relay Data Market MVP
demo, with the usual MVP caveat that operator recovery paths are still worth
keeping.

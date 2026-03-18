# Public Relay Data Market Publish/Consume Audit

Date: 2026-03-18

## Scope

Audit the real public-relay Data Market MVP flow in `openagents` after adding:

- real-relay headless verification against Damus and Primal
- seller-side relay import recovery for request intake
- explicit recording of request/result kinds and ingest mode in the run summary

This audit is based on a successful run of:

- `scripts/autopilot/headless-data-market-public-e2e.sh`

## Bottom line

The Data Market MVP now has a real public-relay demo.

In the verified run:

- the buyer published a targeted Data Market request to real relays
- the seller published the matching delivery result back to real relays
- the buyer observed the result live from those relays
- the buyer consumed the delivered dataset locally and the files matched byte
  for byte

The remaining public-relay gap is narrower than before:

- seller-side live request intake from public relays is still inconsistent
- the repo now compensates for that explicitly and truthfully by fetching the
  published request event back from the configured relays by event id and
  importing it into the seller lane after a short live wait

That means the public demo is real, but currently hybrid:

- publish path: real public relays
- seller request recovery: relay fetch + import when live intake misses
- result publication: real public relays
- buyer result observation: live public relays in the verified run

## Verified relay set

The verified public run used:

- `wss://relay.damus.io`
- `wss://relay.primal.net`

## Published event kinds

The verified public run published:

- request kind `5960`
- result kind `6960`
- handler/capability kind `31990`

More concretely:

- the buyer published the targeted Data Market request as kind `5960`
- the seller published the matching delivery result as kind `6960`
- seller and buyer each published NIP-89 handler/capability events as kind
  `31990`

## Verified public run facts

From the verified run summary:

- `request_id`: `a5708aa5f299f1fd9171ff4f32f763958d476bf7a57afdea28bf1e0fa74a1173`
- `result_event_id`: `c278be4ba5d964f6a6f1898f1979632d50e3feb970ae0aa8237d4fa8bf055ab1`
- `request_kind`: `5960`
- `result_kind`: `6960`
- `seller_request_ingest_mode`: `relay_import`
- `buyer_result_ingest_mode`: `live_relay`
- `request_source_relay_url`: `wss://relay.damus.io`

From the relay publish logs:

- buyer request publish: `accepted_relays=2`
- seller result publish: `accepted_relays=2`
- seller capability publish: `accepted_relays=2`
- buyer capability publish: `accepted_relays=2`

So the request and result were both accepted on both configured public relays
in the verified run.

## What the working public flow looks like

1. Start two no-window runtimes plus local `nexus-control`.
2. Package a dummy dataset into deterministic listing/grant templates.
3. Publish a `DataAsset`.
4. Publish a targeted `AccessGrant`.
5. Bring the seller online for relay intake and result publication.
6. Publish the buyer request as NIP-90 kind `5960` to Damus and Primal.
7. Bring the buyer online in relay-only posture for result tracking.
8. Wait briefly for live seller intake.
9. If seller live intake misses, fetch the published request event from the
   relays by event id and import it into the seller lane.
10. Stage and issue the `DeliveryBundle`.
11. Publish the seller result as NIP-90 kind `6960` to Damus and Primal.
12. Let the buyer observe the result live from the relays.
13. Consume the delivered dataset locally.
14. Verify the consumed files match the original packaged payload.

## The new recovery surface

The important new CLI recovery command is:

```bash
autopilotctl --manifest <seller-manifest> --json data-market seller-import-request \
  --event-id <request-event-id> \
  --relay-url wss://relay.damus.io \
  --relay-url wss://relay.primal.net
```

What it does:

- fetches the published request event from the configured relays by event id
- decodes the Nostr event into the normal Data Market request shape
- applies it to the same seller lane state machine the live relay worker uses

The verified run used that recovery path automatically after the short live
wait expired.

There is also a symmetric buyer-side recovery command:

```bash
autopilotctl --manifest <buyer-manifest> --json data-market buyer-import-response \
  --event-id <result-or-feedback-event-id> \
  --relay-url wss://relay.damus.io \
  --relay-url wss://relay.primal.net
```

That buyer-side recovery path was not needed in the verified public run because
the buyer observed the result live.

## Current architectural truth

Economic truth still comes from kernel authority:

- `DataAsset`
- `AccessGrant`
- `DeliveryBundle`

Relay truth is only the transport/evidence surface for:

- buyer request publication
- seller result publication
- handler/capability advertisement

The public demo is therefore not “the relays are the market.”
It is:

- kernel authority for canonical market objects
- public relays for the targeted NIP-90 request/result transport

## Remaining limitations

### Seller live public-relay intake is not yet fixed

The seller did not ingest the public request live in the verified run.
That is why `seller-import-request` exists and why the public harness now uses
it automatically after a short wait.

### Buyer result relay URLs are not yet surfaced cleanly in the summary

The buyer did observe the result live, but `last_result_relay_urls` is still
not a reliable operator-facing summary field for this path.
The stronger truth today is in the publish logs and buyer response logs.

### `issue-delivery` returns before result publish confirmation settles

The immediate seller action payload still shows delivery publication as
`publishing_result` and may not yet contain the final result event id.
The actual result event id is confirmed shortly afterward by the publish logs
and buyer response observation.

## Judgment

This is now a real public-relay demo, not a local-only simulation.

The precise current truth is:

- request publication to Damus/Primal works
- result publication to Damus/Primal works
- buyer live result observation works
- seller live request intake still needs explicit recovery

That is good enough for a truthful MVP demo, but not yet good enough to claim
fully live end-to-end public-relay seller intake without caveat.

# Headless Data Market

This document describes the current no-window Data Market control path.

## Purpose

Run the existing desktop-owned Data Seller control plane without opening the UI
window.

This path intentionally does **not** create a second seller implementation.
It reuses:

- the normal desktop startup path
- the normal desktop-control runtime
- the same typed `Data Market` desktop-control actions that `autopilotctl`
  already drives

The buyer-side consume step now also has a semantic CLI path:

- `autopilotctl data-market consume-delivery ...`

For the current targeted NIP-90 flow, seller and buyer online posture is now
split cleanly:

- the seller goes online for request intake and result publication
- the buyer goes online in a relay-only posture for result tracking
- neither path requires a compute-ready GPT-OSS or Apple FM runtime for the
  Data Market headless MVP

## Current MVP shape

The current MVP-safe headless path is:

- start `autopilot-headless-data-market`
- let it write the normal desktop-control manifest
- target it with `autopilotctl data-market ...`
- optionally package and publish through the repo-owned
  `skills/autopilot-data-seller-cli/` skill wrappers

Because it is the same desktop-control host, confirmation and read-back
discipline remain unchanged:

- asset publish still requires preview + explicit confirm
- grant publish still requires preview + explicit confirm
- delivery and revocation still route through the same seller logic
- kernel read-back remains the canonical authority confirmation surface

## Publish prerequisite

Preview-only seller work can run without a kernel authority endpoint, but real
asset or grant publish needs a hosted control session:

- `OA_CONTROL_BASE_URL`
- `OA_CONTROL_BEARER_TOKEN`

For local work, the simplest pattern is:

1. start `nexus-control`
2. mint a desktop session at `POST /api/session/desktop`
3. export the returned access token as `OA_CONTROL_BEARER_TOKEN`
4. start `autopilot_headless_data_market`

The no-window smoke harness now bootstraps a temporary local `nexus-control`
and session token automatically so publish is mechanically verified rather than
only previewed.

## Start the runtime

```bash
cargo run -p autopilot-desktop --bin autopilot_headless_data_market -- \
  --manifest-path /tmp/openagents-data-market-desktop-control.json
```

The runtime starts the Autopilot app with its window hidden and writes the
standard desktop-control manifest at the usual manifest location.

In another shell, target it with `autopilotctl`:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest /tmp/openagents-data-market-desktop-control.json \
  --json data-market seller-status
```

If `--manifest-path` is omitted, the runtime uses the default desktop-control
manifest location under `~/.openagents/logs/autopilot/desktop-control.json`.
Using an explicit manifest path is safer when a normal desktop session might
already be running on the same machine.

## Typical flow

1. Package local material:

```bash
scripts/autopilot/data_market_package.py \
  --source ./my-data \
  --output-dir ./tmp/package \
  --title "My Data Bundle" \
  --default-policy targeted_request \
  --grant-policy-template targeted_request \
  --consumer-id npub1buyerexample \
  --price-sats 250
```

2. Draft, preview, and publish the asset:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- --json data-market draft-asset \
  --file ./tmp/package/listing-template.json

cargo run -p autopilot-desktop --bin autopilotctl -- --json data-market preview-asset

cargo run -p autopilot-desktop --bin autopilotctl -- --json data-market publish-asset \
  --confirm
```

3. Draft, preview, and publish the grant:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- --json data-market draft-grant \
  --file ./tmp/package/grant-template.json

cargo run -p autopilot-desktop --bin autopilotctl -- --json data-market preview-grant

cargo run -p autopilot-desktop --bin autopilotctl -- --json data-market publish-grant \
  --confirm
```

4. Continue with payment, delivery, and revocation using the same
   `autopilotctl data-market ...` command tree.

## Smoke harness

The repo now includes a no-window smoke harness:

```bash
scripts/autopilot/headless-data-market-smoke.sh
```

That harness verifies:

- the hidden runtime starts
- `autopilotctl` can reach it through the standard manifest
- deterministic packaging output can be drafted into seller state
- asset preview and publish work without opening the UI window
- grant preview and publish work without opening the UI window

## Real local E2E harness

The repo now also includes a full two-party local harness:

```bash
scripts/autopilot/headless-data-market-e2e.sh
```

That harness does a real local loop:

- starts a local relay
- starts a local `nexus-control`
- creates isolated seller and buyer homes + identities
- launches two no-window Data Market runtimes
- packages a dummy dataset
- publishes an asset and a zero-price targeted grant
- publishes a buyer-side targeted request
- brings the buyer online in relay-only mode for response tracking
- waits for seller intake
- issues a real `DeliveryBundle`
- waits for the buyer-side NIP-90 result
- consumes the delivered local payload into a buyer output directory
- verifies the consumed files match the original dummy dataset

The verified runtime path also now handles the two identity-normalization edges
that mattered in practice:

- seller-side grant evaluation accepts the buyer `buyer_id` from the targeted
  request payload even when the raw Nostr event pubkey arrives in hex
- buyer-side result tracking accepts seller results when the request targeted
  the seller `npub` but the result event is authored by the seller hex pubkey

## Public relay E2E harness

The repo now also includes a real public-relay harness:

```bash
scripts/autopilot/headless-data-market-public-e2e.sh
```

By default it targets:

- `wss://relay.damus.io`
- `wss://relay.primal.net`

The current verified public-relay truth is:

- the buyer publishes the targeted Data Market request as NIP-90 kind `5960`
  to the configured public relays
- the seller publishes the delivery result as NIP-90 kind `6960` back to the
  same configured public relays
- seller and buyer NIP-89 handler/capability events remain kind `31990`
- buyer-side result intake worked live in the verified public run
- seller-side live public-relay request intake is still inconsistent today

Because seller-side live intake is still inconsistent on public relays, the
public harness now falls back automatically after a short wait:

- it waits `OPENAGENTS_HEADLESS_DATA_MARKET_LIVE_INGEST_WAIT_SECONDS`
  seconds for live seller intake
- if live intake does not happen, it fetches the request event back from the
  configured relays by event id
- it imports that event into the seller lane through the normal desktop-owned
  state machine

The current operator commands for that fallback are:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest /tmp/seller-desktop-control.json \
  --json data-market seller-import-request \
  --event-id <request-event-id> \
  --relay-url wss://relay.damus.io \
  --relay-url wss://relay.primal.net
```

and, if buyer-side public relay intake ever needs the same recovery:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest /tmp/buyer-desktop-control.json \
  --json data-market buyer-import-response \
  --event-id <result-or-feedback-event-id> \
  --relay-url wss://relay.damus.io \
  --relay-url wss://relay.primal.net
```

The verified public run summary now records:

- configured relay URLs
- request kind
- result kind
- seller request ingest mode (`live_relay` vs `relay_import`)
- buyer result ingest mode (`live_relay` vs `relay_import`)
- request and result event ids
- final consumed payload path

## Full verification bundle

For a local end-to-end verification pass, run:

```bash
scripts/autopilot/verify-data-market-cli-headless.sh
```

That verification bundle currently proves:

- the headless CLI path can package, preview, publish, request, deliver, and
  consume a local dummy dataset end to end
- the seller state machine has a mechanical payment -> delivery -> revocation
  lifecycle test
- the Nexus authority slice still proves the canonical asset -> grant ->
  delivery -> revocation receipt flow

## Consume-delivery command

For local or operator-controlled flows where the seller publishes a local
`file://` delivery reference, the buyer can materialize the delivered payload
through `autopilotctl`:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest /tmp/buyer-desktop-control.json \
  --json data-market consume-delivery \
  --request-id <request-id> \
  --grant-id <grant-id> \
  --output-dir ./tmp/consumed-dataset \
  --refresh-market \
  --overwrite
```

Current behavior:

- resolves the matching `DeliveryBundle` from the app-owned Data Market state
- supports local `file://` and plain local-path `delivery_ref` values
- copies the payload into `output-dir/payload/`
- copies any local manifest refs into `output-dir/manifests/`
- writes `output-dir/consumed-delivery.json`

Current limitation:

- this is a local/headless materialization path, not yet a general remote blob
  transport or encrypted retrieval client

## Truth boundary notes

- Packaging outputs are draft inputs, not published market truth.
- `autopilotctl` drives the app-owned seller path; it does not create a second
  authority surface.
- Publish and revoke remain confirm-gated.
- Kernel read-back remains the canonical confirmation surface after mutation.

## Boundary note

This is currently a **no-window desktop host**, not yet a fully displayless
minimal seller daemon.

That is deliberate for the MVP because it keeps the headless path on the same
seller logic and desktop-control contract while avoiding a second unofficial
control plane.

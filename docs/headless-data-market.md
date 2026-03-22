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

For the current DS-first market shape, seller and buyer posture is split
cleanly:

- the seller publishes DS listings and DS offers first
- the buyer discovers those DS objects through the relay-backed catalog
- the seller goes online for DS-DVM request intake and result publication
- the buyer goes online in a relay-only posture for DS-DVM result tracking
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
- seller, buyer, and market snapshots are the confirmation surface after each
  mutation

## Publish prerequisite

The current DS-first headless publication path is relay-only.

That means the normal local prerequisites are now:

- a Nostr identity for the runtime
- one or more configured relays
- a Spark wallet when you need priced verification or payment settlement

You do not need `OA_CONTROL_BASE_URL`, `OA_CONTROL_BEARER_TOKEN`, or a local
`nexus-control` process to publish the seller asset/grant pair or to complete
the local relay-only buyer flow.

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

You can also send a plain-language seller instruction into the dedicated
`Data Seller` lane without opening the window:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest /tmp/openagents-data-market-desktop-control.json \
  --json data-market seller-prompt \
  "In the Data Seller pane, turn ./my-data into a saleable listing for 250 sats targeted to npub1buyerexample, then preview before publish."
```

That path still uses the same seller thread, skills, and typed Data Market
tools as the visible `Data Seller` pane. It is intended for automation,
testing, and terminal-driven audits of the documented conversational seller
flow.

Leave Codex enabled if you plan to use `seller-prompt`. The repo-owned smoke
and E2E harnesses disable Codex only because they exercise the typed DS-first
CLI flow directly instead of this conversational seller lane.

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

If the asset is a bundle of Codex conversations, use the dedicated redaction
packager instead:

```bash
skills/autopilot-data-seller-cli/scripts/package_codex_conversations.sh \
  --limit 5 \
  --output-dir ./tmp/codex-package \
  --title "Redacted Codex conversation bundle" \
  --price-sats 500
```

That wrapper reads rollout JSONL from `~/.codex/sessions`, exports a redacted
conversation bundle into `redacted-codex-conversations/`, and still emits the
normal `listing-template.json` / `grant-template.json` artifacts used by the
rest of the seller flow.

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
- buyer refresh can immediately discover the published DS listing and DS offer
  through the local relay catalog

## Real local E2E harness

The repo now also includes a full two-party local harness:

```bash
scripts/autopilot/headless-data-market-e2e.sh
```

That harness does a real local loop:

- starts a local relay
- creates isolated seller and buyer homes + identities
- launches two no-window Data Market runtimes
- packages a dummy dataset
- publishes a DS listing-backed asset and a DS offer-backed targeted grant
- refreshes the buyer-side relay catalog against the published DS objects
- publishes a buyer-side targeted DS-DVM request that references those DS
  coordinates
- brings the buyer online in relay-only mode for response tracking
- waits for seller intake
- issues a real `DeliveryBundle`
- waits for the buyer-side DS-DVM result
- consumes the delivered local payload into a buyer output directory
- verifies the consumed files match the original dummy dataset
- asserts buyer-side DS listing and DS offer selection against the published DS
  coordinates
- asserts seller-side request matching against the same DS coordinates
- asserts local relay publication of `30404`, `30406`, `5960`, and `6960`

For priced local runs, the harness also supports an explicit prefund payer:

```bash
OPENAGENTS_HEADLESS_DATA_MARKET_BUYER_PREFUND_SATS=20 \
OPENAGENTS_HEADLESS_DATA_MARKET_PREFUND_PAYER_IDENTITY_PATH=/path/to/payer/identity.mnemonic \
OPENAGENTS_HEADLESS_DATA_MARKET_PREFUND_PAYER_STORAGE_DIR=/path/to/payer/spark/mainnet \
scripts/autopilot/headless-data-market-e2e.sh
```

If you already have a funded isolated Spark wallet from a prior run, point the
prefund payer env vars at that wallet and its identity rather than assuming the
default `~/.openagents/pylon` wallet has enough spendable sats.

The repo-owned harnesses now launch the no-window runtime with
`OPENAGENTS_DISABLE_CODEX=true` because those verification paths do not use the
conversational seller lane. Normal operator runs keep Codex enabled by default,
which preserves `autopilotctl data-market seller-prompt ...`.

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

The public harness is a live operator probe, not a portable CI gate.

The current DS-first public harness is designed to prove:

- the seller publishes DS listings (`30404`) and DS offers (`30406`) to the
  configured public relays before any buyer request is sent
- the buyer discovers those DS objects through the relay-backed catalog
- the buyer publishes the targeted DS-DVM request as NIP-90 kind `5960` to the
  configured public relays
- the seller publishes the DS-DVM delivery result as NIP-90 kind `6960` back
  to the same configured public relays
- seller and buyer NIP-89 handler/capability events remain kind `31990`
- seller-side request intake worked live in the verified strict run
- buyer-side result intake also worked live in the verified strict run
- the seller observed the request from `wss://relay.primal.net` in the
  verified strict run
- the buyer tracked the result on both configured relays in the verified
  strict run

Relay health is external, so current operator truth is:

- keep the defaults for manual Damus + Primal probing when they are healthy
- override `OPENAGENTS_HEADLESS_DATA_MARKET_RELAY_URLS` when a specific public
  relay is degraded
- treat the local verifier bundle as the portable launch gate
- use the fresh relay-only DS-first audit for current status:
  `docs/audits/2026-03-22-relay-only-headless-data-market-paid-e2e-audit.md`

The strict public verification command is:

```bash
OPENAGENTS_HEADLESS_DATA_MARKET_REQUIRE_LIVE_INGEST=true \
  scripts/autopilot/headless-data-market-public-e2e.sh
```

The manual recovery commands still exist as operator escape hatches if a relay
regression appears again:

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest /tmp/seller-desktop-control.json \
  --json data-market seller-import-request \
  --event-id <request-event-id> \
  --relay-url wss://relay.damus.io \
  --relay-url wss://relay.primal.net

cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest /tmp/buyer-desktop-control.json \
  --json data-market buyer-import-response \
  --event-id <result-or-feedback-event-id> \
  --relay-url wss://relay.damus.io \
  --relay-url wss://relay.primal.net
```

When the public harness succeeds, its summary records:

- DS listing and DS offer coordinates
- configured relay URLs
- request kind
- result kind
- seller request ingest mode (`live_relay` vs `relay_import`)
- buyer result ingest mode (`live_relay` vs `relay_import`)
- seller request source relay URL
- buyer result relay URLs
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
- the portable local E2E gate runs at `OPENAGENTS_HEADLESS_DATA_MARKET_PRICE_SATS=0`,
  so it does not require a funded payer wallet on the machine running the
  verifier
- the relay-only local verifier no longer depends on `nexus-control` or
  `OA_CONTROL_*`
- the buyer-side consume path now resolves delivery details from the DS relay
  result/access-contract state when no legacy kernel `DeliveryBundle` row is
  present

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

- resolves the matching delivery from relay-native DS result/access-contract
  state, falling back to legacy local `DeliveryBundle` rows when present
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
  publication surface.
- Publish and revoke remain confirm-gated.
- DS listings, DS offers, DS access contracts, and DS-DVM results on the relay
  are the current market truth for headless publish and buyer fulfillment.

## Boundary note

This is currently a **no-window desktop host**, not yet a fully displayless
minimal seller daemon.

That is deliberate for the MVP because it keeps the headless path on the same
seller logic and desktop-control contract while avoiding a second unofficial
control plane.

# 2026-03-18 Data Market End-to-End Architecture Audit

> Historical note: this is a point-in-time audit from 2026-03-18. Current
> product and architecture authority still lives in `README.md`, `docs/MVP.md`,
> `docs/OWNERSHIP.md`, `docs/kernel/`, and the current desktop/kernel code.

## Scope

This audit answers one question:

> after reading the data-market docs and code in `openagents` plus the related
> material in `~/code/alpha`, how does the current OpenAgents Data Market
> actually work end to end?

I read the current data-market-specific docs and implementation in:

- `README.md`
- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PANES.md`
- `docs/kernel/README.md`
- `docs/kernel/data-market.md`
- `docs/kernel/markets/data-market.md`
- `docs/headless-data-market.md`
- `docs/plans/data-market-mvp-plan.md`
- `docs/plans/data-market-mvp-implementation-spec.md`
- `proto/openagents/data/v1/data.proto`
- `crates/openagents-kernel-core/src/data.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `crates/nostr/core/src/nip90/data_vending.rs`
- `apps/nexus-control/src/kernel.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/data_market_control.rs`
- `apps/autopilot-desktop/src/data_buyer_control.rs`
- `apps/autopilot-desktop/src/data_seller_control.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/bin/autopilot_headless_data_market.rs`
- `apps/autopilot-desktop/src/panes/data_market.rs`
- `apps/autopilot-desktop/src/panes/data_seller.rs`
- `apps/autopilot-desktop/src/panes/data_buyer.rs`
- `apps/autopilot-desktop/src/skill_autoload.rs`
- `scripts/autopilot/data_market_package.py`
- `scripts/autopilot/package_codex_conversations.py`
- `scripts/autopilot/headless-data-market-e2e.sh`
- `scripts/autopilot/headless-data-market-public-e2e.sh`
- `scripts/autopilot/verify-data-market-cli-headless.sh`

I also read the related material in `~/code/alpha`:

- `ROADMAP.md`
- `markets/data-market-mvp-plan.md`
- `spec/system-spec.md`
- `autopilot/autopilot-fbp-remote-worker-speculation.md`
- `seed/20260314-1218-park-monologue.md`

Important scope conclusion:

- `alpha` contains design intent, launch framing, and some speculative adjacent
  prose
- the actual runtime and authority implementation lives in `openagents`
- I did not find standalone executable data-market code in `alpha`

## Executive Verdict

The current Data Market is real, but narrow on purpose.

It is not a broad searchable marketplace. It is a seller-first,
permissioned-context vending system built from:

- kernel authority objects for `DataAsset`, `AccessGrant`, `DeliveryBundle`,
  and `RevocationReceipt`
- a checked-in `openagents.data.v1` proto package
- a desktop-owned seller and buyer state machine that is reused by the visible
  UI, the CLI, the headless runtime, and the in-app Codex lane
- a local OpenAgents NIP-90 data-vending profile for targeted request,
  feedback, and result traffic

`alpha` and `openagents` are aligned on the core thesis:

- data sells permissioned access to useful context
- NIP-90 is the request/result transport
- kernel objects are the economic truth
- targeted requests come before broad discovery
- `DeliveryBundle` is the authoritative delivery record

The main implementation fact that has overtaken older planning text is this:

- the checked-in data proto package now exists in
  `proto/openagents/data/v1/data.proto`
- the authority routes and combined snapshot route also exist now

So the current system is beyond plan-only. It is a real MVP lane with product
surfaces, a kernel authority slice, a live relay path, packaging helpers, and
verification scripts.

## The Product Thesis

Across `docs/MVP.md`, `docs/kernel/data-market.md`,
`docs/kernel/markets/data-market.md`, and `~/code/alpha/markets/data-market-mvp-plan.md`,
the same product idea repeats:

- the Data Market prices access to useful context
- the thing being sold is not raw compute time and not labor completion
- the sale must be explicit, permissioned, and receipted
- MVP should avoid pretending there is already a broad public catalog with rich
  discovery

That thesis shows up in the retained product surface:

- `Data Seller` pane: the main authoring and fulfillment surface
- `Data Market` pane: a read-only authority and lifecycle surface
- `Data Buyer` pane: a narrow targeted-request surface
- `autopilotctl data-market ...`: shell-first control over the same logic
- `autopilot_headless_data_market`: a no-window host for the same logic

This means the product is currently seller-led and operator-legible. It is not
yet buyer-marketplace-led.

## Alpha Versus OpenAgents

`alpha` is useful here because it explains why the system looks the way it
does.

### What `alpha` contributes

- launch timing and prioritization in `ROADMAP.md`
- the MVP thesis in `markets/data-market-mvp-plan.md`
- the canonical object vocabulary in `spec/system-spec.md`
- some adjacent future thinking about data procurement in
  `autopilot/autopilot-fbp-remote-worker-speculation.md`

### What `openagents` contributes

- all actual code
- all actual routes
- all actual product surfaces
- the concrete NIP-90 profile implementation
- the current packaging and headless scripts
- the current tests and verification harnesses

### The practical alignment

The implementation in `openagents` is very close to the narrow MVP proposed in
`alpha`:

- targeted request flow instead of broad public discovery
- kernel truth objects instead of opaque prompt state
- NIP-90 transport instead of inventing a second transport ideology
- `DeliveryBundle` as the authoritative delivery object
- explicit revoke and expire controls

### The important delta

Some older planning text in `alpha` talks about the lack of a checked-in
dedicated data proto package. That is stale relative to the current repo. The
package now exists and is already wired into the starter authority slice.

## The Core Data Model

The canonical domain model is implemented in:

- `proto/openagents/data/v1/data.proto`
- `crates/openagents-kernel-core/src/data.rs`

The important objects are:

### `PermissionPolicy`

This is the contract that constrains use of the sold context. It carries:

- allowed scopes
- allowed tool tags
- allowed origins
- export permission
- derived-output permission
- retention window
- max bundle size
- metadata

In practice, this is the policy boundary between "buyer can see this" and
"buyer can do whatever they want with it."

### `DataAsset`

This is the seller's listed thing.

Important fields include:

- `asset_id`
- `provider_id`
- `asset_kind`
- `title`
- `description`
- `content_digest`
- `provenance_ref`
- `default_policy`
- `price_hint`
- `metadata_json`
- `status`

The asset is the visible inventory unit.

### `AccessGrant`

This is the actual access offer or contract.

Important fields include:

- `grant_id`
- `asset_id`
- `provider_id`
- `consumer_id`
- `permission_policy`
- `offer_price`
- `warranty_window_ms`
- `expires_at_ms`
- `accepted_at_ms`
- `metadata_json`
- `status`

The key design choice here is that publication of the asset and publication of
the grant are separate economic actions.

### `DeliveryBundle`

This is the authoritative record that delivery happened.

Important fields include:

- `delivery_bundle_id`
- `asset_id`
- `grant_id`
- `provider_id`
- `consumer_id`
- `delivery_ref`
- `delivery_digest`
- `bundle_size_bytes`
- `manifest_refs`
- `expires_at_ms`
- `metadata_json`
- `status`

The NIP-90 result can point at a delivery, but the bundle is the authority
object that says what was delivered and to whom.

### `RevocationReceipt`

This is the explicit terminal correction control.

Important fields include:

- `revocation_id`
- `asset_id`
- `grant_id`
- `provider_id`
- `consumer_id`
- `reason_code`
- `refund_amount`
- `revoked_delivery_bundle_ids`
- `replacement_delivery_bundle_id`
- `metadata_json`
- `status`

This is what makes revoke and expire truthfully visible rather than being
quiet state changes.

### Current statuses

The status enums in the current code are:

- `DataAssetStatus`: `Active`, `Disabled`, `Retired`
- `AccessGrantStatus`: `Offered`, `Accepted`, `Delivered`, `Revoked`,
  `Refunded`, `Expired`
- `DeliveryBundleStatus`: `Issued`, `Accessed`, `Revoked`, `Expired`
- `RevocationStatus`: `Revoked`, `Refunded`

### Snapshot model

The read model also defines:

- `DataMarketSummary`
- `DataMarketSnapshot`

The summary counts the total and active objects, plus offered, accepted,
delivered, and terminal grant counts. The snapshot is what the desktop refresh
uses so it does not have to stitch four lists together manually.

## The Three Real Planes

The current system only makes sense if you separate three planes.

### 1. Authority plane

This is the economic truth plane.

It lives in:

- `crates/openagents-kernel-core/src/data.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/kernel.rs`
- `apps/nexus-control/src/lib.rs`

It owns:

- object validation and normalization
- authenticated mutation routes
- authenticated read routes
- receipt emission
- status transitions
- combined market snapshot

### 2. Relay plane

This is the targeted request/feedback/result transport.

It lives in:

- `crates/nostr/core/src/nip90/data_vending.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/data_buyer_control.rs`
- `apps/autopilot-desktop/src/data_seller_control.rs`

It owns:

- NIP-90 request publication
- NIP-90 `payment-required` feedback
- NIP-90 result publication
- handler advertisement
- relay ingestion and matching

### 3. App-owned control plane

This is the operational state machine that makes the feature usable.

It lives in:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/bin/autopilot_headless_data_market.rs`

It owns:

- seller draft state
- buyer request drafting
- preview and confirm gating
- seller request intake and evaluation
- payment and delivery orchestration
- revoke and expire orchestration
- UI rendering
- CLI and headless parity

This split is important because the current Data Market is not "the relay" and
not "the UI." It is the combination of kernel truth plus relay transport plus
desktop-owned orchestration.

## Authority Plane Details

The authority client in `crates/openagents-kernel-core/src/authority.rs`
exposes the full starter surface:

- list and get assets
- list and get grants
- list and get deliveries
- list and get revocations
- get the combined snapshot
- register an asset
- create a grant
- accept a grant
- issue a delivery bundle
- revoke a grant

The live authenticated HTTP routes are:

- `POST /v1/kernel/data/assets`
- `POST /v1/kernel/data/grants`
- `POST /v1/kernel/data/grants/{grant_id}/accept`
- `POST /v1/kernel/data/grants/{grant_id}/deliveries`
- `POST /v1/kernel/data/grants/{grant_id}/revoke`
- `GET /v1/kernel/data/assets`
- `GET /v1/kernel/data/assets/{asset_id}`
- `GET /v1/kernel/data/grants`
- `GET /v1/kernel/data/grants/{grant_id}`
- `GET /v1/kernel/data/deliveries`
- `GET /v1/kernel/data/deliveries/{delivery_bundle_id}`
- `GET /v1/kernel/data/revocations`
- `GET /v1/kernel/data/revocations/{revocation_id}`
- `GET /v1/kernel/data/snapshot`

The important implementation behavior in `apps/nexus-control/src/kernel.rs` is:

- asset registration normalizes IDs, fills provider identity, applies default
  policy normalization, stores the asset, and emits a receipt
- grant creation requires the referenced asset, keeps provider consistency,
  fills policy from the asset when needed, requires scopes, stores the grant,
  and emits a receipt
- grant acceptance records the economic consumer identity and settlement price,
  moves the grant to `Accepted`, and emits a receipt
- delivery issuance fills the delivery from the accepted grant, stores the
  bundle, moves the grant to `Delivered`, and emits a receipt
- revocation loads related deliveries, marks deliveries and grant terminal,
  stores the revocation, and emits a receipt

The receipt families are explicit:

- `kernel.data.asset.register`
- `kernel.data.grant.offer`
- `kernel.data.grant.accept`
- `kernel.data.delivery.issue`
- `kernel.data.revocation.record`

The HTTP layer in `apps/nexus-control/src/lib.rs` also emits observability
events like:

- `kernel.data.asset.registered`
- `kernel.data.grant.offered`
- `kernel.data.grant.accepted`
- `kernel.data.delivery.issued`
- `kernel.data.revocation.recorded`

So the kernel slice is not just a bag of structs. It is already a real
mutation and read authority.

## Relay and NIP-90 Transport

The data-request wire layer is implemented in
`crates/nostr/core/src/nip90/data_vending.rs`.

The OpenAgents profile identifier is:

- `openagents.data-vending.v1`

The important transport constants are:

- request kind: `5960`
- result kind: `6960`
- handler advertisement kind: `31990`

The request/result schema carries specific OpenAgents parameters and tags,
including:

- `oa_profile`
- `oa_asset_ref`
- `oa_scope`
- `oa_delivery_mode`
- `oa_preview_posture`
- `oa_grant_id`
- `oa_delivery_bundle_id`
- `oa_delivery_ref`
- `oa_reason_code`

The transport shapes are:

- `DataVendingRequest`
- `DataVendingResult`
- `DataVendingFeedback`

The delivery modes currently modeled are:

- `inline_preview`
- `encrypted_pointer`
- `delivery_bundle_ref`

The preview posture values are:

- `none`
- `metadata_only`
- `inline_preview`

### What the relay lane actually does

`apps/autopilot-desktop/src/provider_nip90_lane.rs` is the live runtime bridge
between the market logic and relays.

Important current behavior:

- when a seller profile exists, the provider lane exposes the data-vending
  request kind as supported handler capability
- it publishes NIP-89 metadata including the current data-vending profile
- it accepts request ingress if compute is ready or if a data-vending profile
  is present, which is what allows relay-only seller operation
- for targeted-only data vending it subscribes to `#p`-targeted requests and
  also keeps a kind-only fallback filter because some relays do not reliably
  fan out custom DVM kinds from pure `#p` subscriptions
- it tracks buyer request IDs and subscribes to feedback/result events via
  `#e`
- it parses valid data-vending requests into the desktop inbox capability
  `openagents.data.access`

### Validation at ingress

The current ingress validation requires, among other things:

- an asset reference
- scopes
- a targeted provider identity
- encrypted payload when the request says it is encrypted

If the request has no bid or a zero bid, it can still remain valid for the
current Data Market flow. That matters because the current verified headless
E2E loop uses zero-price grants and zero-price requests.

## The App-Owned Seller State Machine

The core seller implementation lives in `apps/autopilot-desktop/src/app_state.rs`
and `apps/autopilot-desktop/src/data_seller_control.rs`.

### Seller draft model

The seller draft is already a real publication contract, not vague chat state.

It includes:

- asset identity and title
- description
- content digest
- provenance reference
- default policy
- grant policy template
- target consumer
- grant expiry and warranty window
- price hint
- delivery modes
- visibility posture
- sensitivity posture
- metadata and grant metadata
- exact preview payloads

The seller flow computes readiness blockers and produces exact preview payloads
for:

- `RegisterDataAssetRequest`
- `CreateAccessGrantRequest`

That is an important design choice: preview is not a vague summary. It is the
exact authority payload that will be sent on publish.

### Policy templates

The seller lane currently includes reusable policy templates such as:

- `targeted_request`
- `evaluation_window`
- `licensed_bundle`

These templates shape the current MVP posture for allowed scopes, export,
derived outputs, retention, and bundle size.

### Seller runtime states

The seller lane tracks:

- request evaluation disposition
- payment state
- delivery state
- revocation state
- incoming requests
- staged delivery drafts

The current evaluation logic checks things like:

- whether there is a published asset
- whether the asset matches the request
- whether there is a matching published grant
- whether the requester's identity matches the targeted consumer
- whether requested scopes fit the grant policy
- whether the delivery mode is supported
- whether the bid is below the required offer

Representative dispositions include:

- `no_published_asset`
- `grant_required`
- `scope_mismatch`
- `unsupported_delivery_mode`
- `bid_below_offer`
- `ready_for_payment_quote`
- `ready_for_delivery`

### Seller publication path

The seller publication path in `data_seller_control.rs` works like this:

1. draft the asset
2. generate the exact asset preview payload
3. require explicit confirmation
4. publish the asset through kernel authority
5. read the canonical asset back
6. reflect it into seller state and the read-only market pane
7. derive and sync the current NIP-90 data-vending profile
8. draft the grant
9. generate the exact grant preview payload
10. require explicit confirmation
11. publish the grant through kernel authority
12. read the canonical grant back
13. reflect it into seller state and the market pane

The repo is very deliberate about this part: asset publication and grant
publication are distinct steps because they are distinct truths.

### Seller payment path

The current seller code does support paid requests.

The logic can:

- request a Lightning invoice quote for a matched request
- create a Spark invoice
- publish NIP-90 `payment-required` feedback
- track a request through `invoice_requested`, `publishing_feedback`,
  `awaiting_payment`, and `paid`
- detect settled wallet receives and advance the seller state

This is implemented, but the current verified shell E2E harness still uses
zero-price grants. So paid-path logic exists in code and tests, but the
standard headless happy-path proof is still centered on the zero-price loop.

### Seller delivery path

The current issue-delivery path does real authority work:

- load the authoritative grant
- accept the grant first if it is still `Offered`
- issue the `DeliveryBundle`
- read the bundle and grant back from authority
- update the market pane snapshot
- publish the matching NIP-90 result event

The seller result is not treated as final truth by itself. The result is tied
to the authoritative `DeliveryBundle`.

### Seller revocation path

The same seller flow can also:

- revoke a grant explicitly
- expire a grant when the timing window permits it
- record the `RevocationReceipt`
- read back the updated grant, delivery, and revocation objects
- reflect the terminal state into the read-only market pane

This is one of the stronger parts of the MVP because it makes termination a
first-class visible event.

## The Buyer State Machine

The buyer implementation lives in:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/data_buyer_control.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`

The current buyer flow is intentionally narrow.

### What the buyer sees

The buyer derives its request draft from the current `DataMarketSnapshot`.

It prefers:

- a visible active asset
- a matching active targeted grant
- a local buyer identity that matches the grant consumer when present

The buyer pane shows:

- selected asset
- pricing and policy posture
- packaging and bundle hints when present
- targeted offer context
- revocation state if relevant

### Buyer request publication

The buyer request builder publishes a targeted NIP-90 request containing:

- request type `openagents.data_market.access_request.v1`
- `asset_id`
- target provider pubkey
- `grant_id`
- permission scopes
- delivery mode
- preview posture
- `bid_sats`
- timeout
- `buyer_id`
- `targeting_posture = targeted_only`

The actual NIP-90 request is then signed and published as kind `5960`.

### Buyer result tracking

The buyer can run online in a relay-only posture to watch for feedback and
result events tied to the request.

Current behavior includes:

- tracking by request event reference
- provider identity normalization across `npub` and raw hex forms
- import-recovery commands if a relay path needs manual help

### Buyer consume path

The buyer-side local materialization path lives in `autopilotctl`.

`autopilotctl data-market consume-delivery`:

- resolves the matching `DeliveryBundle`
- resolves the local `delivery_ref`
- copies the delivered file or directory into an output directory
- copies any locally resolvable manifest refs into that output directory
- writes `consumed-delivery.json` as a local consume summary

Current important limitation:

- `consume-delivery` only supports local `file://` refs and plain local paths
- it is not yet a remote blob retrieval client

So the current buyer-side consume path is real, but still local-first.

## Packaging and Redaction

The packaging helper is an important part of how the current system stays
truthful.

### `scripts/autopilot/data_market_package.py`

This script:

- packages selected local files or directories
- rejects symlinks
- computes per-file digests
- computes a canonical package digest from a sorted manifest
- emits stable draft artifacts

The generated outputs include:

- `listing-template.json`
- `grant-template.json`
- `packaging-manifest.json`
- `packaging-summary.json`

Those map directly into the seller flow:

- listing template into `draft-asset`
- grant template into `draft-grant`
- packaging summary into operator or agent review

### `scripts/autopilot/package_codex_conversations.py`

This script is a specialized packaging layer for Codex rollout sessions.

It:

- reads sessions from `~/.codex/sessions` or explicit input paths
- redacts emails, URLs, home paths, temporary paths, keys, invoices, and
  other secret-like material
- exports the redacted conversation bundle
- then runs the normal packaging helper to turn it into Data Market draft
  artifacts

This is why the current Data Market docs and UI mention package posture and
redacted Codex-export markers. That is already wired into the seller asset
metadata story.

## Why UI, CLI, Headless, and Skills Stay in Sync

One of the cleaner implementation choices in the current repo is that the Data
Market is not reimplemented four different ways.

### Visible desktop panes

The panes have distinct jobs:

- `Data Market`: read-only kernel-backed snapshot plus recent lifecycle entries
- `Data Seller`: conversational authoring and fulfillment surface
- `Data Buyer`: narrow targeted-request surface

The pane code in `apps/autopilot-desktop/src/panes/` confirms that split.

### Desktop control contract

`apps/autopilot-desktop/src/desktop_control.rs` exposes a typed action set for:

- seller status
- buyer status
- buyer refresh
- draft, preview, and publish asset
- draft, preview, and publish grant
- request payment
- prepare delivery
- issue delivery
- revoke grant
- buyer publish request
- seller import request
- buyer import response
- resolve delivery
- snapshot

This is the real reuse point.

### CLI reuse

`apps/autopilot-desktop/src/bin/autopilotctl.rs` maps the
`autopilotctl data-market ...` command tree onto those desktop-control actions.

So the CLI is not a separate market implementation. It is a shell wrapper over
the same app-owned state machine.

### Headless reuse

`apps/autopilot-desktop/src/bin/autopilot_headless_data_market.rs` simply boots
the normal desktop host in a no-window posture and writes the desktop-control
manifest.

Again, no second implementation.

### Codex and skill reuse

The in-app seller lane also reuses the same state machine through typed dynamic
tools in:

- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`

The important tool family is:

- `openagents.data_market.seller_status`
- `openagents.data_market.draft_asset`
- `openagents.data_market.preview_asset`
- `openagents.data_market.publish_asset`
- `openagents.data_market.draft_grant`
- `openagents.data_market.preview_grant`
- `openagents.data_market.publish_grant`
- `openagents.data_market.request_payment`
- `openagents.data_market.prepare_delivery`
- `openagents.data_market.issue_delivery`
- `openagents.data_market.revoke_grant`
- `openagents.data_market.snapshot`

`apps/autopilot-desktop/src/skill_autoload.rs` then auto-provisions the
managed first-party seller skills so the dedicated seller lane uses the same
typed control path instead of generic pane poking.

## End-to-End Flow

The full current MVP loop is:

1. Package local material into a deterministic listing and grant draft.
2. Draft and preview a `DataAsset`.
3. Confirm and publish the asset through kernel authority.
4. Draft and preview an `AccessGrant`.
5. Confirm and publish the grant through kernel authority.
6. Sync the seller's OpenAgents NIP-90 data-vending profile to the provider
   relay lane.
7. Bring the seller online for targeted request intake.
8. Refresh the buyer market snapshot and select an active asset or default
   offer.
9. Publish a targeted NIP-90 buyer request.
10. Let the seller ingest and evaluate the request.
11. If priced, create and publish `payment-required`; if zero-price, go
    straight to delivery readiness.
12. Prepare a delivery draft that points at the local file or directory to
    deliver.
13. Accept the grant if needed and issue the authoritative `DeliveryBundle`.
14. Publish the linked NIP-90 result event.
15. Let the buyer observe the result and resolve the matching delivery.
16. Materialize the local payload with `consume-delivery`.
17. Later, revoke or expire the access if needed, producing a visible
    `RevocationReceipt`.

That is the current Data Market.

It is not "chat about selling data." It is a narrow but real publish, request,
deliver, and revoke loop with authority objects and relay transport.

## Validation and Test Story

The current repo has a real verification story for this slice.

The most important scripts and tests are:

- `scripts/autopilot/headless-data-market-e2e.sh`
- `scripts/autopilot/headless-data-market-public-e2e.sh`
- `scripts/autopilot/verify-data-market-cli-headless.sh`
- `cargo test -p autopilot-desktop data_seller_full_lifecycle_progresses_from_grant_to_revocation`
- `cargo test -p nexus-control data_market_flow_receipts_asset_grant_delivery_and_revocation`

The verified public-relay posture documented in the repo is:

- buyer request kind `5960`
- seller result kind `6960`
- handler kind `31990`
- successful operation against `wss://relay.damus.io` and
  `wss://relay.primal.net`

That matters because it means this MVP is not only a local mock. The relay path
has been exercised against real public infrastructure.

## Current Limits

The current system is honest about its limits.

### What is implemented now

- seller-first publication and fulfillment
- narrow buyer request publication
- authority-backed assets, grants, deliveries, and revocations
- real CLI and headless control
- relay transport for targeted request and result traffic
- deterministic packaging and local consume

### What is still intentionally narrow or missing

- no broad public data-market search or rich catalog browsing
- no mature buyer procurement workspace beyond the current narrow pane
- no general remote blob retrieval client in `consume-delivery`
- no rich provider-economics or payout product surface specific to data access
- no claim that the market is already a generalized multi-seller discovery
  system

### The most important practical limitation

The current system is best understood as:

- permissioned data vending over NIP-90
- with kernel authority as the economic ledger
- plus a desktop-owned seller and buyer state machine

It is not yet a full public marketplace product.

## Bottom Line

After reading both repos, the cleanest summary is:

`alpha` explains the thesis, but `openagents` is where the Data Market really
exists.

The current implementation works as a narrow MVP because it keeps four things
coherent:

- kernel objects define the authoritative economic state
- NIP-90 carries targeted access demand and fulfillment signaling
- the desktop app owns the operational workflow and exposes it consistently to
  UI, CLI, headless, and Codex tools
- deterministic packaging keeps the sold thing truthful instead of vague

So the Data Market today is a real seller-led, targeted, permissioned context
vending lane. It is not the final marketplace vision from the broader OpenAgents
story, but it is already beyond spec-only and already coherent enough to run
publish, request, deliver, consume, and revoke loops against live relays.

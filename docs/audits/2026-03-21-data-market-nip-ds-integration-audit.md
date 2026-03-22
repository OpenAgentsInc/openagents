# 2026-03-21 Data Market NIP-DS Integration Audit

> Historical note: this is a point-in-time audit from 2026-03-21. Current
> product and implementation authority still lives in `docs/MVP.md`,
> `docs/OWNERSHIP.md`, `docs/v02.md`, `docs/kernel/`, and the current
> desktop/kernel code.

## Scope

This audit answers one concrete question:

> the current OpenAgents Data Market works, but it is centered on a local
> NIP-90 data-vending profile. What would it take to integrate the new
> `NIP-DS: Datasets` thoroughly, without breaking the real seller/buyer flow we
> already have?

I read the current product and implementation surface in:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/v02.md`
- `docs/headless-data-market.md`
- `docs/PANES.md`
- `docs/kernel/README.md`
- `docs/kernel/economy-kernel.md`
- `docs/kernel/markets/data-market.md`
- `docs/PROTOCOL_SURFACE.md`
- `proto/openagents/data/v1/data.proto`
- `crates/openagents-kernel-core/src/data.rs`
- `crates/openagents-kernel-core/src/data_contracts.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/kernel.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/data_market_control.rs`
- `apps/autopilot-desktop/src/data_buyer_control.rs`
- `apps/autopilot-desktop/src/data_seller_control.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/nip28_chat_lane.rs`
- `apps/autopilot-desktop/src/app_state/chat_projection.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/bin/autopilot_headless_data_market.rs`
- `apps/autopilot-desktop/src/panes/data_market.rs`
- `scripts/autopilot/data_market_package.py`
- `scripts/autopilot/headless-data-market-smoke.sh`
- `scripts/autopilot/headless-data-market-e2e.sh`
- `scripts/autopilot/headless-data-market-public-e2e.sh`
- `crates/nostr/core/src/lib.rs`
- `crates/nostr/core/src/nip01.rs`
- `crates/nostr/core/src/nip28.rs`
- `crates/nostr/core/src/nip90/data_vending.rs`
- `crates/nostr/core/src/nip99.rs`
- `crates/nostr/core/src/nip_skl/manifest.rs`
- `crates/nostr/nips/DS.md`

I also reread the related local design context in `~/code/alpha`, mainly:

- `~/code/alpha/markets/data-market-mvp-plan.md`
- `~/code/alpha/spec/world-computer-object-model.md`
- `~/code/alpha/spec/system-spec.md`

## Executive Verdict

The current OpenAgents Data Market is already a real product slice, but it is
not yet a DS market.

What exists today is:

- kernel-authoritative `DataAsset`, `AccessGrant`, `DeliveryBundle`, and
  `RevocationReceipt` state
- a seller-first desktop/UI/CLI/headless flow that publishes those kernel
  objects correctly
- a working targeted access loop carried over a local OpenAgents NIP-90 profile
  on `5960` / `6960`, advertised with `31990`

That means the current system is best described as:

- authority-first
- seller-first
- targeted-request-first
- NIP-90-fulfilled

It is not yet:

- a DS-native public dataset catalog
- a DS-native offer/discovery surface
- a DS-native negotiation/chat surface
- a buyer flow driven by DS listing and offer coordinates

The right migration is not to rip out the current kernel model.

The right migration is:

1. keep kernel authority as the economic and operational truth
2. add DS as the canonical Nostr listing/offer identity layer
3. keep NIP-90 only as the optional DS-DVM fulfillment path
4. add NIP-99 wrappers and NIP-28 dataset discussion channels on top
5. leave NIP-15 for later, because the repo already has NIP-99 code but does
   not yet have NIP-15 implementation support

If done this way, the existing seller publish, payment, delivery, and
revocation work does not get thrown away. It gets re-framed correctly:

- `DataAsset` becomes the authority-side dataset object
- `AccessGrant` becomes the authority-side offer/contract object
- DS `30404` / `30405` / `30406` become the public Nostr-facing identity and
  discovery layer for those objects
- NIP-90 becomes one fulfillment profile, not the whole market

## What Exists Today

### Kernel truth is already strong

The current data-market kernel slice is coherent and useful:

- `proto/openagents/data/v1/data.proto`
- `crates/openagents-kernel-core/src/data.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/kernel.rs`
- `apps/nexus-control/src/lib.rs`

The core object model is already close to what DS needs operationally:

- `DataAsset`
- `AccessGrant`
- `PermissionPolicy`
- `DeliveryBundle`
- `RevocationReceipt`

This is the best part of the current implementation. It means OpenAgents
already has an authority-backed dataset commerce model instead of needing to
invent one from scratch.

### Seller, buyer, CLI, and headless are already one flow

The desktop app, `autopilotctl`, and the no-window host all share the same
state machine and control surface:

- `apps/autopilot-desktop/src/data_seller_control.rs`
- `apps/autopilot-desktop/src/data_buyer_control.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/bin/autopilot_headless_data_market.rs`

That is good news for DS integration. We only need to change one real flow.

### The public network layer is still NIP-90-first

The live access path is built around:

- `crates/nostr/core/src/nip90/data_vending.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/data_buyer_control.rs`

This code is not "wrong". It is just narrower than DS:

- buyer publishes a targeted request
- seller quotes `payment-required`
- buyer pays
- seller issues `DeliveryBundle`
- seller publishes result

That is a valid DS-DVM slice, but it is not DS discovery or DS identity.

## Where NIP-90 Is Hard-Wired Today

The current code does not merely "also support NIP-90". It is architected
around it.

### 1. The buyer request model is NIP-90-native, not DS-native

`apps/autopilot-desktop/src/data_buyer_control.rs` builds a
`DataVendingRequest` directly and publishes a signed `kind 5960` event.

Important properties of the current buyer request:

- it references `draft.asset_id` as `oa_asset_ref`
- it derives from the kernel `DataAsset` / `AccessGrant` snapshot
- it targets the seller via `service_providers`
- it does not reference a DS dataset listing `a` coordinate
- it does not reference a DS offer `a` coordinate

So today the buyer is buying by internal authority identity, not by public DS
identity.

### 2. The DS-DVM request/result helper is still pre-DS

`crates/nostr/core/src/nip90/data_vending.rs` defines:

- `DataVendingRequest`
- `DataVendingFeedback`
- `DataVendingResult`
- `OPENAGENTS_DATA_VENDING_PROFILE`

The current shape is centered on:

- `oa_asset_ref`
- `oa_scope`
- `oa_delivery_mode`
- `oa_preview_posture`

That is enough for the current MVP, but it is not enough for full DS
integration because DS wants canonical dataset and offer references via
addressable event coordinates, not just an opaque asset-ref string param.

### 3. Seller intake and validation are NIP-90-specific

`apps/autopilot-desktop/src/provider_nip90_lane.rs` does all of the following:

- parses incoming requests as `JobRequest`
- tries to decode `DataVendingRequest`
- validates `oa_asset_ref`, `oa_scope`, delivery mode, targeted provider, and
  bid
- emits a `capability=openagents.data.access` request into the seller inbox
- publishes `31990` handler metadata containing the data-vending profile

That means the live seller-side network posture is "I am a NIP-90 data-vending
provider", not "I publish DS listings and optional DS-DVM fulfillment".

### 4. Seller fulfillment publishes NIP-90 feedback and result events

`apps/autopilot-desktop/src/data_seller_control.rs`:

- builds `payment-required` feedback as NIP-90
- builds delivery results as NIP-90 `DataVendingResult`
- queues those publishes through `ProviderNip90LaneCommand::PublishEvent`

Kernel asset and grant publication are local authority mutations. Nostr only
sees the NIP-90 request/feedback/result loop.

### 5. Seller request evaluation is singleton inventory, not market inventory

The seller-side evaluation logic in `apps/autopilot-desktop/src/app_state.rs`
does not evaluate against a real inventory index.

It evaluates against:

- `last_published_asset`
- `last_published_grant`

That is enough for the current audited MVP, but it is fundamentally too small
for DS:

- one seller should be able to publish multiple dataset listings
- one listing should be able to expose multiple offers
- incoming access requests should match by DS listing and offer coordinates
- the seller should not have to rely on "whatever I published last"

### 6. Scripts and docs assume `5960 / 6960` as the market surface

The current packaging and E2E harnesses revolve around:

- `scripts/autopilot/data_market_package.py`
- `scripts/autopilot/headless-data-market-smoke.sh`
- `scripts/autopilot/headless-data-market-e2e.sh`
- `scripts/autopilot/headless-data-market-public-e2e.sh`
- `docs/v02.md`
- `docs/headless-data-market.md`

They all treat:

- kernel asset/grant publication as the listing phase
- NIP-90 targeted request/result as the live market phase

Again, that is workable, but it is not yet DS.

## What Already Maps Cleanly To DS

The current implementation has more DS alignment than it first appears.

### `DataAsset` already looks like the authority-side half of a DS listing

`DataAsset` already carries:

- human title and description
- dataset or bundle kind
- digest
- provenance reference
- default policy
- price hint
- metadata

That is almost exactly the information needed to build a DS `30404` dataset
listing or `30405` draft listing.

### `AccessGrant` already looks like the authority-side half of a DS offer

`AccessGrant` already carries:

- asset linkage
- provider id
- optional consumer target
- permission policy
- offer price
- expiry window
- metadata

That maps naturally onto DS `30406` dataset offers.

### Packaging is already digest-first

`scripts/autopilot/data_market_package.py` already gives the seller:

- deterministic manifest
- digest
- provenance ref
- listing and grant templates

That is exactly the right basis for DS identity. The packaging layer already
knows enough to derive a stable DS `d` tag and dataset scope id.

### Visibility and sensitivity intent already exist

`apps/autopilot-desktop/src/app_state.rs` already models:

- `targeted_only`
- `operator_only`
- `public_catalog`
- `private`
- `restricted`
- `public`

This is important because DS needs more than a file hash. It needs a stated
access posture. The repo already has the beginnings of that vocabulary.

### NIP-99 support already exists in `nostr` core

`crates/nostr/core/src/nip99.rs` is already implemented and exported.

That means DS public listing wrappers via NIP-99 are much closer than they
look.

### NIP-28 support already exists, but not in dataset form

The repo already has:

- `crates/nostr/core/src/nip28.rs`
- `apps/autopilot-desktop/src/nip28_chat_lane.rs`
- `apps/autopilot-desktop/src/app_state/chat_projection.rs`

So the repo can already parse, project, and publish NIP-28 channels and
messages. The missing piece is dataset-aware channel creation and discovery.

## Structural Gaps Versus Full DS Integration

### 1. There is no runtime DS module yet

The draft spec exists in `crates/nostr/nips/DS.md`, but there is no
implementation module in `crates/nostr/core/src/`.

There is no:

- `DatasetListing`
- `DraftDatasetListing`
- `DatasetOffer`
- DS helper for deriving coordinates
- DS parser/serializer tests

This is the first missing layer.

### 2. Kernel objects do not store public Nostr publication identity

`proto/openagents/data/v1/data.proto` and
`crates/openagents-kernel-core/src/data.rs` do not currently expose first-class
fields for:

- DS listing coordinate
- DS offer coordinate
- DS listing event id
- DS offer event id
- NIP-99 wrapper event id
- NIP-28 discussion channel id

Right now the only place this could live is free-form `metadata_json`.

That is not enough for thorough DS integration. Once DS becomes the public
identity layer, the authority needs to track the publication refs explicitly.

### 3. `public_catalog` is intent-only right now

The seller draft has `public_catalog`, but current publication does not do any
of the following:

- publish `30404`
- publish `30406`
- publish `30402`
- create a dataset discussion channel

So `public_catalog` currently changes policy metadata but does not actually
produce a public catalog presence.

### 4. The seller lane is too stateful and too singular

`DataSellerPaneState` tracks:

- `last_published_asset`
- `last_published_grant`
- `last_published_delivery`
- `last_published_revocation`

That is fine for an MVP walkthrough, but it is not a market inventory model.

For DS, the seller side needs at least:

- an index of active assets for the local provider
- an index of active offers for those assets
- matching by DS coordinate, kernel id, and digest

### 5. The buyer sees only the authority snapshot

`DataMarketPane` and `DataBuyerPane` are wired to:

- `get_data_market_snapshot`
- local selection over active kernel assets and grants

There is no relay catalog lane for:

- `30404`
- `30405`
- `30406`
- `30402`

So the buyer has no DS discovery flow at all yet.

### 6. The current DS-DVM payload shape needs a DS upgrade

The current request/result helpers do not use DS-native references.

For full DS integration, DS-DVM should carry at least:

- the dataset listing coordinate
- the dataset offer coordinate when applicable
- the kernel ids only as bridge or settlement hints

Today the live path still treats a kernel asset id string as the primary public
reference.

### 7. NIP-28 dataset negotiation cannot work as-is

Three concrete issues block DS chat integration today:

1. `apps/autopilot-desktop/src/input/actions.rs` creates a standard NIP-28
   kind-40 channel with only name/about metadata. It cannot attach DS `a` tags.
2. `apps/autopilot-desktop/src/app_state/chat_projection.rs` projects channel
   metadata but does not preserve dataset/offer reference tags as first-class
   channel properties.
3. `apps/autopilot-desktop/src/nip28_chat_lane.rs` subscribes only to the
   configured main channel and optional team channel. It is not a dataset
   discussion discovery lane.

So the repo has NIP-28 plumbing, but not NIP-DS chat integration.

### 8. NIP-99 is available, NIP-15 is not

For DS-Market integration, the current repo is asymmetric:

- NIP-99 support exists in `crates/nostr/core/src/nip99.rs`
- there is no equivalent NIP-15 stall/product runtime module yet

That strongly suggests the right rollout order:

- DS core first
- NIP-99 wrapper second
- NIP-28 discussion third
- NIP-15 only after that

## Recommended Target Architecture

The correct end state is:

- kernel objects remain the authority-side truth
- DS becomes the canonical public dataset identity and offer layer
- DS-DVM remains the targeted access/fulfillment profile when a request/result
  loop is needed
- NIP-99 becomes the first public wrapper surface for broad catalog exposure
- NIP-28 becomes the public discussion and negotiation surface tied to DS
  coordinates

The key architectural rule should be:

> public buyers identify a thing by DS coordinate, not by internal kernel id.

That implies this split:

- kernel `asset_id` and `grant_id`: internal authority and settlement handles
- DS `30404` / `30406` coordinates: public discovery and linking handles

The current repo has those layers reversed.

## Recommended Data Mapping

### Kernel object to DS object mapping

Recommended public mapping:

- `DataAsset` -> one DS dataset listing head
- `AccessGrant` -> one DS dataset offer head
- `DeliveryBundle` -> optional DS-DVM result linkage and private delivery refs
- `RevocationReceipt` -> authority-side terminal control, optionally linked from
  private negotiation history or future labels

### Recommended publication tracking model

Do not hide DS identity inside ad hoc metadata strings forever.

For thorough integration, add an explicit publication-ref structure to the data
proto and Rust model. A good shape would be a repeated publication record on
`DataAsset`, `AccessGrant`, and possibly `DeliveryBundle`, carrying:

- protocol
- role
- kind
- coordinate
- event_id
- relay_urls
- published_at_ms
- status

This is better than one-off fields because it covers:

- DS listing
- DS draft listing
- DS offer
- NIP-99 wrapper
- NIP-28 discussion channel
- DS-DVM request/result linkage

## Phased Integration Plan

### Phase 1: Add DS runtime primitives to `crates/nostr/core`

Files:

- `crates/nostr/core/src/lib.rs`
- new `crates/nostr/core/src/nip_ds/`
- `docs/PROTOCOL_SURFACE.md`

Implement:

- `DatasetListing` for `30404`
- `DraftDatasetListing` for `30405`
- `DatasetOffer` for `30406`
- canonical coordinate helpers
- digest and `d`-tag validation helpers
- conversion to and from `EventTemplate` / `Event`
- DS-specific tests

Recommended module shape, following existing patterns like `nip_skl`:

- `crates/nostr/core/src/nip_ds/mod.rs`
- `crates/nostr/core/src/nip_ds/listing.rs`
- `crates/nostr/core/src/nip_ds/offer.rs`
- `crates/nostr/core/src/nip_ds/discovery.rs`
- optional `crates/nostr/core/src/nip_ds/dvm.rs`

Important rule:

- do not bury DS inside `nip90/data_vending.rs`
- DS is now its own top-level protocol surface

### Phase 2: Promote Nostr publication identity into the kernel model

Files:

- `proto/openagents/data/v1/data.proto`
- `crates/openagents-kernel-core/src/data.rs`
- `crates/openagents-kernel-core/src/data_contracts.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/kernel.rs`
- `apps/nexus-control/src/lib.rs`

Add explicit publication refs for:

- DS listing publication
- DS offer publication
- NIP-99 wrapper publication
- NIP-28 discussion channel

Why this phase matters:

- the seller flow needs durable read-back after relay publication
- the buyer flow needs stable mapping from DS coordinate -> kernel object
- revocation and delivery history need to be linked back to the same public
  listing/offer identity

### Phase 3: Convert seller publication from kernel-only to kernel-plus-DS

Files:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/data_seller_control.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `scripts/autopilot/data_market_package.py`

Recommended behavior:

1. seller drafts kernel asset/grant as today
2. publish kernel `DataAsset`
3. derive and publish DS listing
4. persist DS publication refs back into authority state
5. publish kernel `AccessGrant`
6. derive and publish DS offer
7. persist DS offer refs back into authority state

Visibility posture should control publication:

- `operator_only`: authority only, no public DS publish
- `targeted_only`: publish DS listing and offer without public market wrappers
- `public_catalog`: publish DS listing and offer, then also publish NIP-99
  wrapper and optionally dataset discussion channel

This is also where the packaging tool should begin emitting DS-oriented helper
fields:

- stable DS `d` tag
- dataset scope id
- optional DS listing template and DS offer template

### Phase 4: Stop matching incoming requests against only the "last published" item

Files:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/data_market_control.rs`
- possibly a new seller inventory helper module

Required changes:

- replace singleton seller inventory assumptions with indexed active inventory
- match incoming requests by DS listing coordinate first
- match by DS offer coordinate second
- use kernel `asset_id` / `grant_id` only as bridged settlement identifiers
- keep asset-kind fallback only as a transitional compatibility path

Without this phase, DS can be published but seller fulfillment will still feel
like a one-item demo lane.

### Phase 5: Add a real DS relay catalog for the buyer and market panes

Files:

- `apps/autopilot-desktop/src/data_market_control.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/panes/data_market.rs`
- `apps/autopilot-desktop/src/data_buyer_control.rs`
- likely a new relay catalog lane module

Add a relay-backed catalog projection for:

- `30404`
- `30405`
- `30406`
- `30402`

Recommended UI/control split:

- `Data Market` pane becomes a combined authority plus relay view
- `Data Buyer` pane selects DS listings and offers, not just kernel assets
- authority snapshot remains visible because it is still the settlement truth

This phase is what turns the current seller-first tool into an actual market.

### Phase 6: Upgrade the DS-DVM fulfillment path

Files:

- `crates/nostr/core/src/nip90/data_vending.rs`
- or new DS DVM helper files under `crates/nostr/core/src/nip_ds/`
- `apps/autopilot-desktop/src/data_buyer_control.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/data_seller_control.rs`
- `apps/autopilot-desktop/src/input/actions.rs`

Recommended change:

- keep `5960 / 6960` as the optional fulfillment kinds
- stop treating raw `oa_asset_ref` as the canonical public reference
- require DS listing `a` references
- optionally include DS offer `a` references
- keep kernel ids only as bridge metadata during migration

Also recommended:

- introduce a new DS-aligned profile id instead of letting
  `openagents.data-vending.v1` become a permanent pre-DS compatibility fossil
- keep the legacy profile readable during migration

### Phase 7: Add DS-Market wrappers, starting with NIP-99

Files:

- `crates/nostr/core/src/nip99.rs`
- new desktop publication helpers
- seller control and buyer discovery code

Recommended wrapper order:

1. NIP-99 first
2. NIP-15 later

Reason:

- NIP-99 runtime support is already present
- NIP-15 runtime support is not yet present
- NIP-99 is enough to give DS listings public classified-ad visibility quickly

### Phase 8: Add DS-linked NIP-28 negotiation channels

Files:

- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/nip28_chat_lane.rs`
- `apps/autopilot-desktop/src/app_state/chat_projection.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`

Required changes:

- allow channel creation with extra DS `a` tags
- preserve channel reference tags in the chat projection
- add discovery for dataset-tagged channels
- subscribe to DS discussion channels, not only the configured main channel
- expose dataset-channel creation, listing, join, and send actions in
  `autopilotctl` and desktop control

Recommended DS chat semantics:

- public NIP-28 channel for listing-level discussion and negotiation
- NIP-17 / NIP-44 / NIP-59 for private negotiation and delivery-sensitive
  details

## Concrete File-Level Worklist

If this were turned into implementation issues, I would break it down roughly
like this:

1. `crates/nostr/core`: add `nip_ds`
2. `docs/PROTOCOL_SURFACE.md`: include DS kinds and DS-DVM posture
3. `proto/openagents/data/v1/data.proto`: add explicit publication refs
4. `crates/openagents-kernel-core/src/data.rs`: add publication-ref model
5. `crates/openagents-kernel-core/src/data_contracts.rs`: wire proto roundtrip
6. `apps/nexus-control/src/kernel.rs`: persist DS/Nostr publication refs
7. `apps/nexus-control/src/lib.rs`: expose new proto fields cleanly
8. `apps/autopilot-desktop/src/app_state.rs`: replace singleton seller
   inventory assumptions
9. `apps/autopilot-desktop/src/data_seller_control.rs`: dual-publish kernel and
   DS
10. `apps/autopilot-desktop/src/data_buyer_control.rs`: buyer discovery and
    request from DS coordinates
11. `apps/autopilot-desktop/src/provider_nip90_lane.rs`: DS-aware fulfillment
    matching
12. `apps/autopilot-desktop/src/data_market_control.rs`: authority plus relay
    catalog merge
13. `apps/autopilot-desktop/src/panes/data_market.rs`: show DS listing/offer
    state, not only kernel snapshot rows
14. `apps/autopilot-desktop/src/input/tool_bridge.rs`: add DS discovery,
    publication, and discussion-channel tools
15. `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`: expose the same
    new typed tools to the in-app agent lane
16. `apps/autopilot-desktop/src/input/actions.rs`: allow DS-tagged NIP-28
    channel creation
17. `apps/autopilot-desktop/src/nip28_chat_lane.rs`: dataset channel discovery
    and subscriptions
18. `apps/autopilot-desktop/src/app_state/chat_projection.rs`: preserve DS
    reference tags on channels
19. `scripts/autopilot/data_market_package.py`: add DS helper outputs
20. `scripts/autopilot/headless-data-market-*.sh`: assert DS publishes and DS
    references, not only NIP-90 request/result

## Recommended Rollout Order

This is the order I would actually implement and merge:

1. DS runtime module and tests in `crates/nostr/core`
2. kernel publication-ref model and proto changes
3. seller dual-publish of DS listing and offer
4. buyer DS discovery and request generation
5. seller-side DS-aware request matching
6. NIP-99 public catalog wrapper
7. NIP-28 dataset discussion channels
8. optional NIP-15 storefront support

That order matters because it keeps the system working at every step:

- after step 3, sellers can publish DS-backed listings
- after step 4, buyers can discover and target them
- after step 5, the live access loop is DS-aware end to end
- steps 6 through 8 improve discoverability and negotiation, not basic sale
  correctness

## Recommendation

OpenAgents should not describe the current implementation as "already DS". It
is not.

But the current implementation is a strong base for DS because:

- the kernel model is already good
- the seller flow is already proven
- the fulfillment path is already proven
- NIP-99 and NIP-28 building blocks already exist

The main conceptual change is this:

> stop treating NIP-90 as the market itself, and start treating it as one
> fulfillment profile under a DS-first market model.

That is the path from the current audited MVP to a real NIP-DS-integrated Data
Market.

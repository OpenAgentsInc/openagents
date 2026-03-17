# Data Market MVP Plan

Status: proposed  
Date: 2026-03-17

## Purpose

Define the narrowest honest MVP for the OpenAgents Data Market, grounded in the current kernel docs and in the reality that the existing compute-market wedge already runs through NIP-90.

This plan treats the Data Market MVP as:

* a **permissioned context vending** lane
* transported over **NIP-90**
* settled with the same **Lightning-first** pattern as the compute MVP
* mirrored into **kernel authority objects** so economic truth does not live only in relay events

## Documents this plan is based on

This draft is grounded in:

* `~/code/openagents/docs/kernel/README.md`
* `~/code/openagents/docs/kernel/economy-kernel.md`
* `~/code/openagents/docs/kernel/economy-kernel-proto.md`
* `~/code/openagents/docs/kernel/data-market.md`
* `~/code/openagents/docs/kernel/markets/data-market.md`
* `~/code/openagents/docs/kernel/markets/compute-market.md`
* `~/code/openagents/docs/v01.md`
* `~/code/openagents/docs/autopilot-earn/AUTOPILOT_EARN_MVP.md`
* `~/code/nips/90.md`

## Current repo truth

The kernel docs currently establish three important facts.

### 1. Compute already has a real product wedge

The compute market is already productized enough to matter:

* the desktop earn loop is live
* the transport is NIP-90
* settlement is Lightning-first
* provider/buyer flow is already app-owned and real
* kernel authority objects for compute are already deeper than the other markets

The live compute wedge is effectively:

* buyer publishes NIP-90 request
* provider processes
* provider emits `payment-required` and/or result
* buyer pays
* success is only counted once payment truth exists

### 2. Data already has a starter kernel authority slice

The data market is not greenfield.
The kernel docs say the repo already has starter authority flows for:

* `DataAsset`
* `AccessGrant`
* `PermissionPolicy`
* `DeliveryBundle`
* `RevocationReceipt`

That means the missing piece is not "invent a data market ontology."
The missing piece is productization, transport, and a truthful MVP loop.

### 3. NIP-90 is already the right family for this

NIP-90 is explicitly the **Data Vending Machine** spec.
Its core framing is already "money in, data out," not only "remote inference."

That matters because it means the data market does not need a second transport ideology.
The compute MVP is not an exception to the protocol. It is the first concrete profile of it.

## Core thesis

The Data Market MVP should use **NIP-90 as its primary request/response transport**, with the kernel data authority slice as the canonical economic truth.

More concretely:

* **NIP-90** should be the buyer/seller transport and payment signaling surface
* **Kernel/Nexus** should hold the canonical authority objects for asset, grant, delivery, and revocation
* **Nostr relay events** should not be the only record of who bought what
* **DeliveryBundle** should usually be the authoritative delivery object, even when the relay result contains a small inline preview or pointer

The MVP should therefore look like a **data vending machine** much more than a generic asset listing site.

## Why this is the right MVP

This is the right launch shape for five reasons.

### 1. It reuses the live OpenAgents wedge

Compute already proved:

* NIP-90 transport works
* Lightning settlement works
* app-owned orchestration works
* wallet-authoritative closure works

The data market should copy that wedge before it invents a new one.

### 2. It matches the kernel docs

The kernel docs already define the Data Market as permissioned access to context under explicit permissions.
That maps naturally to:

* request
* quote or grant
* payment-required
* delivery
* receipt
* revocation

### 3. It avoids blocking on missing proto work

The docs are explicit that there is not yet a dedicated checked-in `openagents.data.v1` package.

For a near-term MVP, the system should **not** wait for:

* dedicated data proto packages
* broad public data-market read APIs
* full listing/discovery UX

The MVP can launch against:

* NIP-90 transport
* existing kernel data authority routes
* thin app-owned request/delivery logic

### 4. It keeps truth planes clean

Transport truth and economic truth should stay separate:

* relay events show the request/response/payment choreography
* kernel objects show what counted economically

That preserves the same execution/economic split the larger spec set is built around.

### 5. It is a better fit for private data than an open race market

Unlike public compute, many data transactions are sensitive.
That means the Data Market MVP should default to:

* targeted requests
* encrypted parameters where needed
* off-relay payload delivery for the real bundle
* explicit permission and revocation state

Open broadcast data vending can exist later for public datasets.
It should not be the default MVP posture.

## Product definition

For MVP, the Data Market sells **permissioned access to useful context**.

The first honest product families should be narrow and local:

* stored conversation bundles
* local project-context bundles
* document or research bundles
* small structured artifact bundles needed by a downstream agent or workflow

The MVP should not pretend it already supports a broad commodity market for arbitrary datasets.

## MVP scope

The MVP should support the following end-to-end loop:

1. A seller registers or exposes a `DataAsset`.
2. A buyer sends a targeted NIP-90 request for access.
3. The seller evaluates the request against local policy and asset posture.
4. The seller creates or offers an `AccessGrant` with bounded permissions.
5. The seller emits `payment-required` if payment is required before delivery.
6. The buyer pays.
7. The seller publishes a NIP-90 result containing either:
   * a small inline result, or
   * an encrypted pointer/manifest for the real bundle.
8. The kernel records the `DeliveryBundle`.
9. The system can later revoke or expire access and emit a `RevocationReceipt`.

## MVP non-goals

This MVP should explicitly not try to do all of the following:

* a broad public data marketplace with rich search and browsing
* generalized data-provider economics across many asset classes
* large-scale dataset hosting or replication
* final `openagents.data.v1` wire completeness
* full underwriting or dispute depth for data delivery failures
* a generic "all context everywhere" sync product

## Recommended architectural shape

### 1. NIP-90 as transport

Use NIP-90 request/result/feedback as the transport profile.

The baseline should remain the canonical NIP-90 pattern already used by compute:

* request kinds in `5000-5999`
* result kinds in `6000-6999`
* feedback in `7000`
* `payment-required` for gated delivery

### 2. Targeted by default

For the Data Market MVP, requests should usually be **targeted** using NIP-90 `p` tags rather than broadcast to all reachable providers.

Reason:

* private assets should not be advertised through an open race unless intentionally public
* even the request parameters may be sensitive
* unlike compute, duplicate work by multiple providers is often not desirable here

### 3. Encrypted params where needed

If the asset identifier, permissions, or request details are sensitive, use the NIP-90 encrypted-params pattern.

That means:

* keep cleartext relay exposure minimal
* avoid leaking private asset names or buyer intent into public relay history

### 4. Kernel as canonical authority

Every successful MVP flow should map into the kernel starter object model:

* `DataAsset`
* `AccessGrant`
* `PermissionPolicy`
* `DeliveryBundle`
* `RevocationReceipt`

The NIP-90 flow is the vending lane.
The kernel is the authority lane.

## Proposed NIP-90 Data Market profile

Because the docs do not yet define a dedicated checked-in OpenAgents data proto package or a canonical public data-market request kind, the MVP should define a narrow **OpenAgents data-vending profile** on top of NIP-90.

### Request kind posture

The MVP should stay inside the NIP-90 DVM range.

The local NIP docs make two constraints explicit:

* `5000-5999`, `6000-6999`, and `7000` are reserved for NIP-90 DVM use
* concrete job request kinds are defined separately from NIP-90 itself

Recommendation:

* if there is an acceptable existing DVM job kind for the exact launch behavior, reuse it
* if not, do **not** treat this plan as authority to mint a permanent numeric kind on its own
* before launch, check the current DVM job-kind registry and either:
  * upstream a new kind, or
  * choose an explicitly temporary OpenAgents-local kind only after confirming it does not collide with the current registry and documenting the migration path
* the corresponding result kind should remain `request_kind + 1000`

This plan deliberately does not assign a specific numeric request kind.
The point is to keep the MVP on the NIP-90 rail without creating avoidable kind conflicts.

### In-repo helper profile

The in-repo helper surface should stay explicit about what is local policy versus public protocol.

For the MVP, the Nostr crate can expose a narrow OpenAgents helper profile that:

* requires the caller to choose the request kind rather than hard-coding a permanent public number
* keeps the transport on generic NIP-90 request/result/feedback events
* carries the OpenAgents-specific request fields through namespaced profile params or tags such as:
  * `oa_profile`
  * `oa_asset_ref`
  * `oa_scope`
  * `oa_delivery_mode`
  * `oa_preview_posture`
  * `oa_grant_id`
  * `oa_delivery_bundle_id`
* makes result and feedback linkage to kernel objects parseable without inventing a second transport

That gives the codebase a truthful in-repo profile now while leaving the public kind decision open.

### Request shape

The request should carry enough information to ask for data access without requiring a new transport:

* asset reference or asset selector
* requested delivery mode
* permission or usage scope
* desired duration or TTL
* pricing ceiling / bid
* optional provenance or freshness requirements
* optional buyer relay hints
* target provider `p` tag by default

Useful request parameters include:

* `asset_id`
* `asset_family`
* `delivery_mode`
* `max_bytes`
* `ttl`
* `usage_scope`
* `freshness_hint`
* `preview_only`

### Provider discoverability

Providers should advertise their supported data-vending profile through the same Nostr discovery posture already implied by NIP-90 and NIP-89.

For MVP this can stay narrow:

* advertise support for the checked data-vending request kind through NIP-89 `k` tags
* advertise the OpenAgents data-vending profile version in handler metadata or custom tags
* advertise coarse asset families
* avoid publishing sensitive asset details in public announcements

### Feedback shape

The seller should use `kind:7000` feedback for:

* `processing`
* `payment-required`
* `error`
* optional `partial` preview

For data vending, `partial` can mean:

* preview metadata
* a sample row or snippet
* bundle manifest summary

### Result shape

The result should generally not dump the whole private dataset onto relays.

The result event should usually carry:

* result linkage to the request
* `amount` if not already settled
* a small inline preview when safe
* or an encrypted delivery pointer / manifest
* hashes or identifiers that can be tied to the `DeliveryBundle`

## Delivery model

The MVP should support two delivery modes.

### 1. Inline small delivery

Use this only for tiny, low-sensitivity payloads.

Examples:

* small JSON context pack
* tiny metadata manifest
* short summarized context result

### 2. Bundle-pointer delivery

This should be the default.

The NIP-90 result carries:

* encrypted pointer
* bundle digest
* bundle metadata
* optional expiry

The authoritative delivery object is the kernel `DeliveryBundle`.

## Kernel object mapping

The MVP should make the mapping explicit.

### `DataAsset`

Registered seller-side asset or context bundle family.

For MVP it should minimally include:

* stable asset id
* owner
* asset family
* size or shape hints
* pricing posture
* sensitivity posture
* delivery modes allowed

### `PermissionPolicy`

Policy envelope for what the buyer is allowed to do.

For MVP it should at least express:

* who can access
* duration
* revocation conditions
* redistribution prohibition or allowance
* allowed downstream usage class

### `AccessGrant`

Offer or accepted grant for one buyer / one asset / one policy scope.

This is the economic object that should bridge:

* request intent
* seller approval
* buyer payment
* permissioned delivery

### `DeliveryBundle`

Canonical record that the promised data bundle or access package was delivered.

For MVP it should include:

* bundle id
* associated grant id
* bundle digest
* delivery pointer or delivery metadata
* delivery time
* visibility posture

### `RevocationReceipt`

Canonical record that access was revoked or expired.

This matters because data delivery is often not a one-shot completion fact in the same way compute is.

## Recommended MVP UX

### Seller side

Keep seller UX narrow.

The seller should be able to:

* register a small number of assets
* choose price and permission defaults
* see incoming targeted access requests
* approve or reject
* require payment
* deliver the bundle
* revoke later if policy allows

### Buyer side

Keep buyer UX targeted and intentional.

The buyer should be able to:

* choose a provider or asset
* request access
* inspect price and permission terms
* pay
* receive the delivered bundle or bundle pointer
* see grant duration and revocation posture

### Activity / operator truth

As with the compute MVP, the UI should show:

* request published
* grant offered / accepted
* payment-required seen
* invoice paid
* delivery recorded
* revocation / expiry state

## Launch sequencing

### Phase 0: freeze the MVP profile

Define:

* initial asset families
* targeted-request default
* delivery modes
* minimum request/result/feedback fields
* kernel object mapping

### Phase 1: seller inventory and buyer request path

Implement:

* asset registration UX
* targeted NIP-90 request path
* basic permission and price review

### Phase 2: grant, payment, and delivery

Implement:

* `AccessGrant` lifecycle
* `payment-required` flow
* delivery-bundle issuance
* buyer receipt of delivered pointer or inline bundle

### Phase 3: revocation and observability

Implement:

* revocation / expiry path
* operator views
* durable history of who bought what and under which permissions

## Acceptance criteria

The Data Market MVP should only be considered real when all of the following are true:

* a seller can register at least one honest data asset family
* a buyer can publish a targeted NIP-90 access request
* a seller can respond with bounded permission and price terms
* the flow can require Lightning payment before delivery
* the delivered result can be tied to a kernel `DeliveryBundle`
* the kernel can answer what was purchased, under which permissions, for how long, and with what delivery evidence
* the UI does not mark success until payment and delivery truth both exist
* access can expire or be revoked with a visible `RevocationReceipt`

## Major risks and how to handle them

### 1. Leaking private data onto relays

Mitigation:

* targeted requests by default
* encrypted params
* off-relay bundle delivery by default
* only publish previews, pointers, and hashes on relays

### 2. Treating relay events as final authority

Mitigation:

* always mirror successful flows into kernel objects
* do not let "result seen on relay" equal "economic completion"

### 3. Waiting for perfect data-market infrastructure

Mitigation:

* launch against the existing authority slice
* defer dedicated `openagents.data.v1` until after the MVP loop exists

### 4. Copying the compute race model too literally

Mitigation:

* default to targeted vending
* do not assume public race semantics are good for private data

## Recommendation

The strongest near-term move is:

* **do not invent a second transport**
* **do not wait for a dedicated data proto package**
* **do not make the first data MVP an open public search marketplace**

Instead:

* reuse the compute MVP transport pattern
* use NIP-90 as the data-vending-machine layer
* target sellers explicitly
* keep payload delivery mostly off-relay
* record the economic truth in `DataAsset`, `AccessGrant`, `PermissionPolicy`, `DeliveryBundle`, and `RevocationReceipt`

That gives OpenAgents a truthful Data Market MVP this week:

**money in, permissioned context out, with kernel-backed receipts.**

## Immediate follow-on after MVP

After the MVP loop exists, the next steps should be:

* dedicated `openagents.data.v1` proto work
* richer data-asset read models and browsing surfaces
* better provenance and packaging semantics
* broader provider economics and payout visibility
* later public-dataset and multi-seller discovery modes

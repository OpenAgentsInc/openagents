# Data Market MVP Implementation Spec

Status: proposed  
Date: 2026-03-17

Companion docs:

- `docs/audits/2026-03-17-conversational-data-market-seller-pane-audit.md`
- `docs/plans/data-market-mvp-plan.md`
- `docs/kernel/data-market.md`
- `docs/kernel/markets/data-market.md`
- `docs/kernel/markets/compute-market.md`
- `docs/codex/CAD_CHAT_BUILD_IMPLEMENTATION.md`
- `docs/codex/CODEX_PANE_CAD_TOOLING.md`

## Intent

This document answers one concrete product-and-implementation question:

> if OpenAgents wants data to be listed for sale conversationally through a
> special Codex-backed pane, while still delivering a truthful Data Market MVP,
> what exactly should we build, where should it land, and in what order?

This spec does not replace `docs/plans/data-market-mvp-plan.md`.

That plan establishes the market thesis:

- data is a permissioned context-vending market,
- NIP-90 is the right transport family,
- kernel authority objects are the economic truth,
- and the launch shape should stay narrow and targeted.

This document is the implementation-oriented companion. It defines:

- the product surface we should ship first,
- the desktop and Codex architecture for it,
- the kernel and transport integration boundaries,
- and the issue-ready backlog for building it.

## Executive Thesis

The first real product wedge for the Data Market should be a dedicated
conversational seller surface:

- a `Data Seller` pane in `apps/autopilot-desktop`,
- backed by the existing Codex lane,
- using a seller-specific Codex skill/policy profile,
- with typed OpenAgents data-market tools,
- and a draft/preview/confirm publication discipline before any kernel write.

The rest of the MVP should be built around that wedge:

1. kernel data authority remains the economic truth plane,
2. the current `Data Market` pane remains the read-only control and observability
   surface,
3. seller publication becomes conversational and productized,
4. targeted NIP-90 data-vending is added on top rather than replacing the seller
   flow,
5. and buyer/payment/delivery flows are layered in after the seller path is
   truthful.

The crucial design choice is:

> the "special version of Codex" should not be a separate runtime or separate
> hidden service. It should be a constrained seller profile on the existing
> Codex lane, exposed through a dedicated pane and a typed data-market tool
> contract.

## Current Repo Truth

The implementation plan has to start from what the repo already has.

### 1. The authority objects already exist

The starter kernel slice is real today:

- `DataAsset`
- `AccessGrant`
- `PermissionPolicy`
- `DeliveryBundle`
- `RevocationReceipt`

The mutation calls already exist in `crates/openagents-kernel-core` and
`apps/nexus-control`:

- register asset
- create grant
- accept grant
- issue delivery
- revoke access

That means the problem is not ontology. The problem is productization.

### 2. The desktop currently has only a read-only data pane

The current `Data Market` pane can:

- refresh kernel-side assets
- refresh grants
- refresh deliveries
- refresh revocations

It cannot:

- draft a listing
- preview a listing
- publish a listing
- create a grant
- handle seller intake conversationally

This pane should remain the read-model and operator snapshot surface. It should
not be overloaded into the primary authoring surface for listings.

### 3. Codex is already deeply integrated

The repo already has:

- a Codex lane and thread/turn model,
- deterministic skill attachment per turn,
- OpenAgents `openagents.*` dynamic tool execution,
- pane-control and CAD-control precedent,
- remote and exec variants that prove the lane is reusable.

This is enough to build a special seller agent without inventing a second agent
stack.

### 4. The shipped CAD path is the precedent

The CAD path already proves OpenAgents can do all of the following:

- use conversation as the primary UX,
- attach special-purpose skills automatically,
- constrain Codex to a typed tool contract,
- surface deterministic progress and failure state,
- and keep mutation behavior inspectable.

The Data Seller path should copy that architectural pattern, not invent a
different one.

### 5. Compute already proved the market transport wedge

The compute market already established the MVP logic we want to reuse:

- NIP-90 transport
- Lightning-first payment posture
- app-owned orchestration
- kernel-linked truth

The data lane should look like "permissioned context vending" on that same
architecture, not like a separate marketplace ideology.

## Product Thesis

For the MVP, the Data Market should be understood through three distinct
surfaces.

### 1. `Data Seller`

This is the new pane we should build first.

Its job is:

- conversational listing intake,
- draft generation,
- preview and confirmation,
- publication of truthful kernel objects,
- and later seller-side request, grant, and delivery handling.

This is the first real productized wedge.

### 2. `Data Market`

This is the existing read-only pane.

Its job is:

- operator visibility,
- authority readback,
- audit and refresh,
- and market-state inspection.

It should remain the truth-inspection surface, not the primary seller UX.

### 3. later buyer / access-request surface

This can land as:

- a dedicated buyer pane,
- or a constrained buyer flow in an existing request surface,

but it should come after the seller path is stable and truthful.

## What We Should Build

## 1. Add a dedicated `Data Seller` pane

We should add a new pane to `apps/autopilot-desktop` with its own pane state,
renderer, actions, and command routing.

This pane should contain:

- a transcript area for the seller conversation,
- a seller-composer input,
- current draft status,
- readiness blockers,
- a payload preview card,
- explicit confirm/publish controls,
- and a compact published-history summary.

It should be separate from:

- generic `Autopilot Chat`
- the read-only `Data Market` pane

because seller authoring is a distinct product task with distinct truth and
safety requirements.

## 2. Implement a seller-specific Codex profile

The "special version of Codex" should be implemented as a constrained profile on
the existing Codex lane.

For MVP, that profile should mean:

- dedicated thread/session identity per seller pane
- fixed or default seller-safe personality and collaboration posture
- required seller skills auto-attached
- no dependency on generic manual skill selection
- bounded tool contract for data-market actions

Recommended initial defaults:

- personality: `Pragmatic`
- collaboration mode: `Default`
- approval policy: match the app's current lane default, but keep seller writes
  behind in-app confirmation instead of raw Codex approvals

The profile should remain configurable later, but MVP needs a stable default.

## 3. Add seller-specific first-party skills

Add first-party skills under `skills/`:

- `autopilot-data-seller`
- `autopilot-data-market-control`

`autopilot-data-seller` should teach Codex how to:

- gather missing listing facts,
- ask only necessary clarifying questions,
- normalize free-form seller intent into structured draft fields,
- avoid claiming a listing is live until preview and confirm have happened,
- and explain readiness blockers clearly.

`autopilot-data-market-control` should teach Codex how to:

- use only the typed OpenAgents data-market tools,
- move from draft to preview to publish in the correct order,
- read back published state,
- and avoid inventing economic state from prose alone.

For the core publish path, the seller flow should prefer typed data tools over
generic `openagents.pane.*` UI poking.

## 4. Add a typed data-market tool family

We should extend the OpenAgents dynamic tool surface with data-market-native
tools instead of trying to run the whole seller flow through generic pane input
setting.

Recommended MVP tool family:

- `openagents.data_market.seller_status`
  - return pane/draft/readiness/publication status
- `openagents.data_market.draft_asset`
  - update normalized draft fields from structured input
- `openagents.data_market.preview_asset`
  - produce the exact `RegisterDataAssetRequest` preview plus blockers
- `openagents.data_market.publish_asset`
  - submit the exact asset payload after explicit confirmation state exists
- `openagents.data_market.draft_grant`
  - create or update a structured draft for a default grant posture
- `openagents.data_market.preview_grant`
  - show the exact `CreateAccessGrantRequest` preview plus blockers
- `openagents.data_market.publish_grant`
  - create a concrete grant after explicit confirmation
- `openagents.data_market.snapshot`
  - return a compact summary of current draft + latest published ids

Later-scope tools:

- `openagents.data_market.prepare_delivery`
- `openagents.data_market.issue_delivery`
- `openagents.data_market.revoke_grant`
- `openagents.data_market.publish_dvm_offer`

These tools should use the same structured response-envelope pattern already
used by the existing `openagents.*` tools.

## 5. Add explicit seller draft state in desktop

The new pane needs a first-class draft model in app state.

Recommended `DataSellerDraft` contour:

- `asset_kind`
- `title`
- `description`
- `content_digest`
- `provenance_ref`
- `default_policy`
- `price_hint`
- `delivery_modes`
- `visibility_posture`
- `sensitivity_posture`
- `preview_posture`
- `metadata`
- `readiness_blockers`
- `last_previewed_asset_payload`
- `last_previewed_grant_payload`
- `last_published_asset_id`
- `last_published_grant_id`

Recommended `DataSellerPaneState` contour:

- pane load/state machine
- seller Codex thread id
- conversation transcript
- active draft
- publish/preview status
- last action
- last error
- latest kernel readback summary

This state should be inspectable by both the user and the tool responses.

## 6. Enforce draft -> preview -> confirm -> publish

This sequence should be a hard product rule.

The seller flow should not allow:

- free-form conversation to mutate kernel state directly
- implicit publication
- or "looks ready" prose to stand in for concrete payload inspection

The required path should be:

1. conversational intake
2. structured draft normalization
3. exact payload preview
4. explicit user confirmation
5. authority mutation
6. read-back confirmation

That keeps the seller path safe and truthful without relying on Codex approval
prompts as the economic confirmation mechanism.

## 7. Keep `DataAsset` and `AccessGrant` semantically separate

The implementation should not blur:

- publishing an asset identity and policy posture
- issuing a concrete access offer

For MVP, the recommended sequencing is:

1. publish `DataAsset`
2. optionally draft and publish a default `AccessGrant` posture
3. later connect that to targeted buyer requests and deliveries

This is cleaner than having "listing for sale" silently do several unrelated
economic mutations at once.

## 8. Keep NIP-90 as the transport layer above the seller wedge

The seller pane should be designed so it leads naturally into the broader Data
Market MVP already defined in `docs/plans/data-market-mvp-plan.md`.

That means:

- kernel objects remain the canonical economic truth,
- targeted NIP-90 data-vending is the transport layer for access requests and
  deliveries,
- relay events do not become final authority,
- and delivery bundles should remain the authoritative delivery objects.

The pane does not need to wait for full NIP-90 completion to ship, but it
should gather fields that later NIP-90 flows will need.

## Proposed MVP Flow

The recommended MVP should land in three product slices.

### Slice A: conversational listing

Seller flow:

1. Open `Data Seller`.
2. Describe what is being sold in natural language.
3. Codex asks bounded follow-up questions only when necessary.
4. Codex normalizes the draft.
5. The pane shows exact preview payload and readiness blockers.
6. The user confirms.
7. Desktop publishes `DataAsset`.
8. The read-only `Data Market` pane shows the resulting authority state.

This is the first must-have wedge.

### Slice B: default sale posture

Seller flow:

1. From the same pane, draft a default grant or offer posture.
2. Preview the exact `AccessGrant` payload.
3. Confirm and publish.
4. Read back the created grant from Nexus.

This makes the listing feel concretely "for sale" rather than merely "registered."

### Slice C: targeted vending and delivery

Market flow:

1. Buyer sends targeted NIP-90 access request.
2. Seller evaluates request against asset + grant posture.
3. Seller emits `payment-required` if needed.
4. Buyer pays.
5. Seller delivers inline preview or bundle pointer.
6. Desktop records `DeliveryBundle`.
7. Later revocation or expiry emits `RevocationReceipt`.

This is where the full Data Market MVP becomes real end to end.

## Recommended Landing Zones

### Desktop app

Primary landing zone:

- `apps/autopilot-desktop`

Expected files/modules:

- new seller pane under `src/panes/`
- new seller control module
- new pane state in `src/app_state.rs`
- new pane actions in `src/pane_system.rs`
- new input/action handlers
- tool-bridge additions for typed data-market tools
- optional seller-thread/session helpers alongside current Codex session logic

### Skills

- `skills/autopilot-data-seller/SKILL.md`
- `skills/autopilot-data-market-control/SKILL.md`

### Kernel authority client

- `crates/openagents-kernel-core/src/authority.rs`

This likely already has the needed methods, but may need minor additions for:

- richer preview validation support
- seller-ready response shapes
- or additional readbacks

### Data-market docs

- `docs/kernel/markets/data-market.md`
- `docs/plans/data-market-mvp-plan.md`
- this implementation spec

## Acceptance Criteria

The seller-pane slice should only count as real when all of the following are
true:

- a user can list a data asset for sale through a dedicated conversational pane
- the pane uses a seller-specific Codex profile on the existing lane
- the system maintains a structured draft, not just transcript prose
- publication is blocked until exact preview and explicit confirmation exist
- publication creates a real `DataAsset` in kernel authority
- published state can be read back immediately in the existing `Data Market`
  pane
- failure states are explicit and do not silently produce partial publication

The broader Data Market MVP should only count as real when all of the following
are also true:

- a seller can publish at least one truthful asset family
- a seller can create a bounded access offer or grant
- a buyer can issue a targeted NIP-90 request for access
- payment-required and Lightning payment can gate delivery
- delivery can be tied to a `DeliveryBundle`
- revocation or expiry can be represented through `RevocationReceipt`
- the UI does not declare success until payment and delivery truth both exist

## Non-Goals

The MVP should not try to do any of the following:

- turn the first release into a broad open data marketplace
- publish private asset metadata widely on relays
- rely on generic `Autopilot Chat` as the sole seller UX
- auto-publish listings directly from free-form transcript interpretation
- store large raw payloads inside kernel authority objects
- block the whole seller wedge on a complete `openagents.data.v1` package
- overfit the data lane to the public race semantics of compute

## Important Design Rules

1. Keep the current `Data Market` pane read-only and authoritative.
2. Make `Data Seller` the authoring surface.
3. Use typed data-market tools for the core mutation flow.
4. Keep draft state explicit and inspectable.
5. Separate asset publication from grant publication.
6. Keep kernel objects as economic truth.
7. Treat NIP-90 as transport, not sole authority.
8. Default to targeted and permissioned posture, not open broadcast.

## Proposed Phase Order

The correct order is:

1. add the seller pane shell, draft model, and tool contract
2. make asset publication truthful with preview/confirm/read-back
3. add default grant publication
4. bind the seller flow into targeted NIP-90 request/payment/delivery behavior
5. add revocation and richer market observability
6. only then widen into broader buyer discovery and richer provider economics

This order matters because a seller pane without real authority writes is a toy,
and NIP-90 transport without a truthful seller product surface creates protocol
motion without a real market.

## Proposed GitHub Issue Sequence

These titles are phrased so they can be opened directly as GitHub issues.

## Wave 0: Pane, profile, and tool contract

1. **[DATA P0] Add `Data Seller` pane shell and state model**
   - Register a new pane, pane state, renderer, and action routing in
     `apps/autopilot-desktop`.
   - The first version only needs transcript shell, draft card, status area,
     and explicit preview/publish controls.

2. **[DATA P0] Define `DataSellerDraft` model and readiness rules**
   - Add the structured draft model, readiness blockers, preview state, and
     explicit publish gating.
   - This is the core truth object that prevents free-form transcript drift.

3. **[DATA P0] Add seller-specific Codex pane session/profile wiring**
   - Give the new pane a dedicated Codex thread/session path on the existing
     lane with stable defaults for personality, collaboration, and session
     metadata.
   - This issue makes the seller pane a real specialized Codex surface rather
     than a generic chat clone.

4. **[DATA P0] Add `autopilot-data-seller` and `autopilot-data-market-control` skills**
   - Create the first-party seller skills and wire them into the new pane's
     turn assembly path.
   - The goal is deterministic seller behavior, not ad hoc prompting.

5. **[DATA P0] Define `openagents.data_market.*` tool contract and response envelopes**
   - Extend the dynamic-tool registry and desktop tool bridge with typed
     data-market tools for draft, preview, publish, and snapshot.
   - Keep the response schema consistent with the existing OpenAgents tool
     envelope contract.

## Wave 1: truthful asset publication

6. **[DATA P1] Implement conversational draft normalization for asset listing**
   - Teach the seller pane to move from conversation into structured draft
     fields with bounded clarification questions.
   - The output of conversation must be a draft object, not only transcript
     text.

7. **[DATA P1] Implement asset preview and explicit confirm/publish path**
   - Build the exact payload preview card, confirm action, and publish gate.
   - Publication must be impossible until preview has produced a valid request.

8. **[DATA P1] Wire `publish_asset` to kernel `register_data_asset` and immediate read-back**
   - Connect the publish path to the real Nexus authority route and then read
     the resulting asset back into pane state.
   - This is the first point where the seller flow becomes a real market action.

9. **[DATA P1] Reflect newly published assets in the read-only `Data Market` pane**
   - Ensure the operator pane and seller pane remain visibly consistent after
     publication.
   - This keeps the authoring surface and truth-inspection surface aligned.

## Wave 2: sale posture and offers

10. **[DATA P2] Add default permission-policy templates and grant draft flow**
   - Add reusable policy templates plus seller-side grant drafting.
   - The pane should make bounded sale posture legible, not force users to
     invent raw policy objects manually.

11. **[DATA P2] Implement grant preview and `create_access_grant` publication**
   - Extend the same preview/confirm discipline to grant creation.
   - Keep `DataAsset` and `AccessGrant` distinct in both UI and tool semantics.

12. **[DATA P2] Add seller-side published inventory summary and status card**
   - Show current published assets, default offers, last published ids, and
     readiness or policy warnings in the seller pane.
   - This is the first real seller inventory surface.

## Wave 3: targeted NIP-90 data vending

13. **[DATA P3] Define the initial OpenAgents NIP-90 data-vending profile in desktop/runtime**
   - Implement the narrow targeted profile described in
     `docs/plans/data-market-mvp-plan.md` without hard-coding an unverified
     permanent kind number.
   - Keep request kind choice explicit and registry-safe.

14. **[DATA P3] Add seller-side targeted request intake and evaluation flow**
   - Surface incoming targeted access requests to the seller and let the seller
     evaluate them against asset/grant posture.
   - This is the bridge from published inventory into actual market demand.

15. **[DATA P3] Add `payment-required` and buyer-payment linkage for data access**
   - Reuse the compute-market MVP payment posture for gated delivery.
   - A data sale should not count as complete until payment truth exists.

## Wave 4: delivery and post-sale control

16. **[DATA P4] Add delivery-bundle issuance and result linkage from seller flow**
   - Connect seller-side delivery publication to `DeliveryBundle` authority
     objects and NIP-90 result linkage.
   - The delivery object must remain the authoritative delivery truth.

17. **[DATA P4] Add revocation and expiry controls with `RevocationReceipt` read-back**
   - Let the seller revoke or expire access under explicit policy and read the
     resulting receipt back into both panes.
   - This matters because data access often remains live after initial delivery.

18. **[DATA P4] Add seller/buyer/operator observability for data-market lifecycle**
   - Make listing, grant, payment, delivery, and revocation state visible in
     durable UI summaries and activity views.
   - The system should answer what was sold, under which permissions, and with
     which receipts.

## Wave 5: later buyer and market widening

19. **[DATA P5] Add buyer-side targeted request surface for data access**
   - Add the narrowest honest buyer surface for selecting a seller/asset and
     issuing a targeted NIP-90 request.
   - Do not turn this into broad public discovery yet.

20. **[DATA P5] Add dedicated `openagents.data.v1` proto and richer read models**
   - After the MVP loop exists, codify the fuller wire package and richer
     market read models.
   - This should follow reality instead of blocking reality.

## Final Recommendation

If we want "listing for sale" to happen conversationally, the right thing to
ship is not a generic form and not a new secret agent service.

The right thing to ship is:

- a dedicated `Data Seller` pane,
- a constrained seller profile on the existing Codex lane,
- typed OpenAgents data-market tools,
- and a draft/preview/confirm discipline wired to real kernel authority
  mutations.

That is the shortest truthful path from the current starter data-market slice to
an actual productized Data Market MVP.

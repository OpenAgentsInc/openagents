# 2026-03-17 Conversational Data-Market Seller Pane Audit

> Historical note: this is a point-in-time audit from 2026-03-17. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, `docs/kernel/`, and the latest desktop/runtime code.

## Scope

This audit evaluates whether OpenAgents already has the right substrate to make
data listing and sale happen conversationally through a special Codex-backed
pane, and what is still missing.

Primary docs reviewed:

- `docs/PANES.md`
- `docs/codex/README.md`
- `docs/codex/CODEX_PANE_CAD_TOOLING.md`
- `docs/codex/CAD_CHAT_BUILD_IMPLEMENTATION.md`
- `docs/codex/EXEC.md`
- `docs/codex/REMOTE.md`
- `docs/kernel/data-market.md`
- `docs/kernel/markets/data-market.md`
- `docs/kernel/markets/compute-market.md`
- `docs/plans/data-market-mvp-plan.md`

Primary implementation surfaces reviewed:

- `apps/autopilot-desktop/src/codex_lane.rs`
- `apps/autopilot-desktop/src/codex_remote.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/panes/data_market.rs`
- `apps/autopilot-desktop/src/data_market_control.rs`
- `skills/autopilot-pane-control/SKILL.md`
- `skills/autopilot-cad-builder/SKILL.md`
- `crates/openagents-kernel-core/src/data.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`

## Executive Verdict

OpenAgents already has enough Codex infrastructure to build a conversational
data-listing flow without inventing a new agent runtime.

The repo already contains:

- a mature Codex lane and thread/turn model,
- deterministic per-turn skill attachment,
- an auto-executed `openagents.*` desktop tool bridge,
- shipped precedent for a special-purpose Codex workflow in the CAD path,
- and real kernel-side data-market authority objects and write routes.

What it does not contain yet is the actual seller surface:

- no dedicated seller pane,
- no data-market mutation tools in the Codex bridge,
- no seller-specific skill or policy profile,
- no draft/preview/confirm flow for publishing a listing,
- and no desktop path that turns a conversation into `DataAsset` and
  `AccessGrant` mutations.

So the right next move is not a new standalone agent stack. The right next move
is a new pane and a seller-specialized Codex policy surface built on the
existing chat, skills, and tool-call architecture.

## What Exists Today

### 1. The data-market authority slice is real

The current kernel data-market model is already concrete:

- `DataAsset`
- `AccessGrant`
- `PermissionPolicy`
- `DeliveryBundle`
- `RevocationReceipt`

Those objects live in `crates/openagents-kernel-core/src/data.rs`.

Mutation requests and client methods already exist in
`crates/openagents-kernel-core/src/authority.rs`:

- `register_data_asset`
- `create_access_grant`
- `accept_access_grant`
- `issue_delivery_bundle`
- `revoke_access_grant`

Nexus exposes the corresponding authenticated HTTP routes under
`/v1/kernel/data/*` in `apps/nexus-control/src/lib.rs`.

This matters because the seller-pane problem is not blocked on inventing the
economic object model. The authority objects already exist.

### 2. The current Data Market desktop pane is read-only

The current pane in `apps/autopilot-desktop/src/panes/data_market.rs` is a
read-only snapshot surface.

Its backing control path in `apps/autopilot-desktop/src/data_market_control.rs`
only loads:

- assets
- grants
- deliveries
- revocations

`DataMarketPaneAction` currently only supports `Refresh`, and the tool bridge
only exposes snapshot/refresh-style interaction for that pane. There is no
desktop-side action to register an asset, create a grant, or publish a listing.

So the repo has a control-plane view of the market, but not a seller workflow.

### 3. Codex integration is already strong and app-owned

`apps/autopilot-desktop/src/codex_lane.rs` shows a substantial Codex app-server
integration:

- thread lifecycle,
- turn lifecycle,
- approvals,
- tool calls,
- account/model/config flows,
- realtime sessions,
- skills listing and config,
- remote and exec adjacencies.

The lane already supports the session-level controls a dedicated seller flow
would want to preset:

- model
- service tier
- personality
- collaboration mode
- sandbox posture

This means a seller pane can be a specialized product surface on top of the
existing lane rather than a separate runtime.

### 4. The generic chat path already supports policy-attached skills

`run_chat_submit_action_with_trigger` in
`apps/autopilot-desktop/src/input/actions.rs` already assembles a turn from:

- prompt text,
- mentions/images,
- user-selected skills,
- goal-selected skills,
- policy-required skills.

`assemble_chat_turn_input` then turns those into Codex `UserInput` items,
including `UserInput::Skill`.

That is important because the repo already has the mechanism needed for a
special seller agent. A seller surface can attach required seller skills the
same way CAD turns attach `autopilot-cad-builder` and
`autopilot-pane-control`.

### 5. The repo already has a proven pattern for special-purpose Codex behavior

The CAD stack is the closest existing precedent:

- `docs/codex/CAD_CHAT_BUILD_IMPLEMENTATION.md`
- `docs/codex/CODEX_PANE_CAD_TOOLING.md`
- `skills/autopilot-cad-builder/SKILL.md`
- `skills/autopilot-pane-control/SKILL.md`

The CAD path proves that OpenAgents can already do all of the following:

- keep the user in a conversational surface,
- attach required skills deterministically,
- let Codex call `openagents.*` tools,
- route those tools into app-native typed actions,
- keep mutation results inspectable and auditable,
- and preserve a bounded operator rollback posture.

That is exactly the architectural shape the data-listing flow should reuse.

### 6. The Codex tool bridge is real, but data-market writes are not on it yet

`apps/autopilot-desktop/src/openagents_dynamic_tools.rs` and
`apps/autopilot-desktop/src/input/tool_bridge.rs` expose and auto-execute
allowlisted `openagents.*` tools.

Today that surface covers things like:

- pane list/open/focus/close/set_input/action,
- CAD intent/action,
- treasury and swap tools,
- provider control,
- labor-related helpers.

What is missing is a data-market-native tool family. There is no typed tool for:

- drafting a listing,
- previewing a listing,
- publishing a `DataAsset`,
- creating an `AccessGrant`,
- checking seller-ready status,
- or summarizing current listing posture.

This is the cleanest technical gap between "Codex can drive desktop" and
"Codex can list data for sale conversationally."

### 7. Compute already established the NIP-90 pattern

The current market docs and MVP planning make the transport direction clear:

- the compute wedge is already NIP-90-centered,
- the data MVP plan argues data should also use a NIP-90 data-vending profile,
- and the data market should default to targeted, permissioned access.

That means the seller conversation should not be designed as a fake web-store
listing flow. It should be designed as the front-end authoring surface for:

- kernel-side market objects, and
- later targeted NIP-90 data-vending behavior.

## What Does Not Exist Yet

The repo does not yet have any of the following:

### 1. No seller-specific conversational pane

There is no dedicated pane for:

- preparing a data listing conversationally,
- keeping seller context separate from general Autopilot chat,
- showing draft listing state,
- previewing the exact `DataAsset` or `AccessGrant` that will be published,
- or confirming publication.

### 2. No seller-specialized Codex skill/profile

There is no equivalent yet to `autopilot-cad-builder` for the data market.

The repo does not yet define a first-party skill such as:

- `autopilot-data-seller`
- `autopilot-data-market-control`

There is also no seller-specific policy that auto-attaches such a skill for
seller turns.

### 3. No draft/preview/confirm write discipline

Right now the data market has authority mutations, but no product UX enforcing:

- draft first,
- preview exact mutation payload,
- confirm explicitly,
- then publish.

That is a problem if listing is going to be conversational. Free-form chat
should not jump straight to authority mutation without a clearly inspectable
draft object.

### 4. No data-market mutation tools in the OpenAgents tool namespace

The current bridge can open the data-market pane and refresh it, but not create
or publish anything.

Using generic `openagents.pane.set_input` as the primary mutation surface for
seller flows would be the wrong abstraction. Data listing needs typed economic
tools, not only generic UI poking.

### 5. No seller-side productization of the real sale boundary

There is still an important semantic distinction in the current data model:

- `DataAsset` is the listed asset identity and posture,
- `AccessGrant` is the concrete bounded offer of access.

A seller pane will need to make that legible. Right now the repo does not yet
have product language or UI that teaches the user whether they are:

- publishing an asset,
- preparing a default sale posture,
- or issuing a specific grant.

### 6. No NIP-90 seller orchestration for the data lane

The plan document points toward targeted NIP-90 data vending, but the desktop
seller flow does not yet expose:

- publishable data-vending offers,
- targeted request handling,
- delivery-pointer authoring,
- or NIP-90-facing seller inventory posture.

## Architectural Conclusion

The correct design is:

- not generic `Autopilot Chat`,
- not the current read-only `Data Market` pane,
- and not a brand-new agent stack.

The correct design is a dedicated conversational seller pane that reuses:

- the existing Codex lane,
- the existing thread/turn/session model,
- the existing skill-attachment path,
- and the existing OpenAgents tool bridge,

while adding:

- seller-specific policy,
- seller-specific tools,
- seller-specific state,
- and explicit economic confirmation boundaries.

## Recommended Product Shape

### 1. Add a dedicated conversational pane

Create a new pane, separate from the current read-only `Data Market` pane.

Suggested role:

- `Data Seller`
- or `Data Listing Agent`

Its purpose should be:

- conversational intake,
- listing draft generation,
- seller-side preview,
- explicit confirm/publish,
- and later grant/delivery management.

The current `Data Market` pane should remain the read-model and operator
snapshot surface.

### 2. Build it on the existing Codex lane, not a separate runtime

The special seller agent should be a configured use of the current Codex
implementation, not a second agent framework.

That means the new pane should own or preset:

- model choice,
- service tier,
- personality,
- collaboration mode,
- and required seller skills.

The strongest current analogue is the CAD path: special-purpose behavior built
on the same Codex lane and chat/skill/tool plumbing.

### 3. Introduce seller-specific first-party skills

Add a first-party skill analogous to the CAD skills.

Suggested split:

- `autopilot-data-seller`
  - teaches the agent how to gather asset details, normalize them into draft
    objects, ask only necessary clarifying questions, and never publish without
    preview/confirmation.
- `autopilot-data-market-control`
  - teaches the agent the typed OpenAgents data-market tool contract.

This is the cleanest way to create a "special version of Codex" without
forking Codex itself.

### 4. Add typed data-market tools instead of relying on generic pane poking

The seller flow should get its own typed `openagents.*` tools.

Suggested minimum family:

- `openagents.data_market.seller_status`
- `openagents.data_market.draft_asset`
- `openagents.data_market.preview_asset`
- `openagents.data_market.publish_asset`
- `openagents.data_market.create_grant`
- `openagents.data_market.snapshot`

Optional later additions:

- `openagents.data_market.update_asset`
- `openagents.data_market.revoke_grant`
- `openagents.data_market.prepare_delivery`
- `openagents.data_market.publish_dvm_offer`

These tools should return structured response envelopes like the existing
OpenAgents tool contract, not ad hoc prose.

### 5. Make draft state explicit and inspectable

The conversational pane should maintain a seller draft object in pane state.

Minimum draft contour:

- asset kind
- title
- description
- content digest
- provenance ref
- default permission policy
- price hint
- visibility posture
- listing readiness blockers
- last previewed payload
- last published asset id

That state should be inspectable both by the human user and by the Codex tool
responses.

### 6. Keep publish behind explicit confirmation

Listing must not mean "agent mutates authority state as soon as it thinks it has
enough information."

The required sequence should be:

1. conversational intake,
2. draft normalization,
3. preview exact payload,
4. explicit user confirmation,
5. publish `DataAsset`,
6. optionally prepare default `AccessGrant` posture.

That keeps the system truthful and auditable.

### 7. Keep the market semantics explicit

The pane should not blur these two acts:

- publishing an asset identity to the market,
- offering or issuing a concrete access grant.

For MVP, the pane can publish `DataAsset` first and make grant creation a second
explicit step. That is closer to the current kernel truth and easier to reason
about than silently doing both in one opaque mutation.

### 8. Treat NIP-90 as the next transport layer, not a blocker

The seller pane can and should launch before full NIP-90 data-vending
orchestration is complete, as long as it writes truthful kernel objects.

But the design should preserve the intended path:

- publish asset and permission posture now,
- later bind those into targeted NIP-90 data-vending requests and deliveries,
- keep kernel objects as the economic truth plane.

That means the conversational pane should gather fields that will later be
needed for NIP-90 vending anyway, rather than inventing a UI model that has to
be thrown away.

## Recommended Implementation Order

### Phase 1: seller draft pane

Ship:

- new conversational seller pane,
- seller-specific chat state,
- seller skill(s),
- typed data-market draft and preview tools,
- explicit confirm gate,
- `register_data_asset` write path,
- read-back snapshot in the existing `Data Market` pane.

This is the narrowest honest first release.

### Phase 2: seller offer posture

Add:

- `create_access_grant` flow,
- default permission templates,
- expiry and pricing posture,
- richer readiness checks,
- grant preview and confirmation.

This is where "listed" becomes more concretely "for sale."

### Phase 3: data-vending transport

Add:

- targeted NIP-90 data-vending publication,
- request intake for purchase/access,
- delivery-pointer packaging,
- grant acceptance and delivery flow from the seller pane or an adjacent pane.

This is the transport/productization layer, not the first prerequisite.

## What Should Not Be Done

The seller flow should not:

- live only in the generic `Autopilot Chat` pane,
- depend on free-form prose without draft object normalization,
- mutate kernel authority state through hidden or implicit chat behavior,
- overload the current read-only `Data Market` pane with ad hoc form logic,
- or pretend that asset registration and grant issuance are the same act.

## Bottom Line

OpenAgents already has the core substrate to make data listing happen
conversationally through a special Codex-backed pane.

The repo does not need a new agent runtime for this. It needs:

- a dedicated seller pane,
- a seller-specific Codex skill/policy profile,
- typed data-market mutation tools,
- and a draft/preview/confirm discipline that turns conversation into truthful
  kernel objects.

That is the shortest path from the current read-only data-market slice to a
real conversational seller MVP.

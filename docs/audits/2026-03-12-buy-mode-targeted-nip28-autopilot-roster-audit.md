# Buy Mode Targeting Through NIP-28 Autopilot Roster Audit

Date: 2026-03-12

## Scope

This audit answers a specific product ask:

- Mission Control Buy Mode should stop broadcasting an open NIP-90 request to every reachable provider.
- Instead, Buy Mode should target specific pubkeys belonging to other Autopilot agents that participate in the single configured NIP-28 main channel.
- Each request should be scoped to one provider identity that has recently and explicitly indicated it is online for compute.
- The same truth must be visible in the UI and through the programmatic control plane (`autopilotctl` / desktop control / runtime logs).

This audit is limited to MVP-owned behavior inside `apps/autopilot-desktop` and intentionally excludes signing/notarization concerns.

## Product Constraint

`docs/MVP.md` still governs this work:

- the earn loop must stay truthful,
- wallet/payment state must remain explicit,
- `Go Online` must mean something real,
- the app must remain deterministic and replay-safe,
- the desktop control plane must stay synced with the UI rather than inventing a separate shadow runtime.

`docs/OWNERSHIP.md` also constrains the implementation:

- this feature belongs in `apps/autopilot-desktop`,
- not in shared crates,
- not in `wgpui`,
- and not as a new reusable protocol abstraction unless a narrower primitive is clearly warranted later.

## Executive Summary

The good news is that most of the low-level machinery already exists:

1. The app already knows how to publish targeted NIP-90 requests with explicit `p` tags.
2. Provider ingress already enforces target policy and ignores requests aimed at a different provider pubkey.
3. The headless buyer/provider flow already supports explicit target pubkeys.
4. The desktop control plane and `autopilotctl` already support going online and interacting with the NIP-28 main channel programmatically.

The missing product layer is the important part:

1. Mission Control Buy Mode still dispatches untargeted requests.
2. The NIP-28 main-channel implementation has no app-owned notion of an "Autopilot compute roster."
3. There is no pubkey-level online signal tied to NIP-28 chat activity.
4. The UI/control plane cannot currently explain which peer Buy Mode will target, why that peer is eligible, or why no peer is eligible.
5. There is no end-to-end verification that a provider can go online, announce itself through the main NIP-28 channel, and then receive a Buy Mode request targeted specifically to that pubkey.

So the problem is no longer "can the app target a provider?" The problem is "how does the app derive a trustworthy, app-owned target set from the NIP-28 channel and provider-online state?"

## Current System Status

### 1. Mission Control Buy Mode

Current Buy Mode is single-flight and open broadcast.

Relevant code:

- `apps/autopilot-desktop/src/input/actions.rs`
  - `run_mission_control_buy_mode_tick`
  - `submit_mission_control_buy_mode_request`
  - `build_mission_control_buy_mode_request_event`

What it does today:

- runs on a fixed cadence,
- refuses to dispatch if another Buy Mode request is still in flight,
- publishes a normal NIP-90 request,
- uses Mission Control's fixed budget and timeout,
- does not attach any target provider pubkeys.

The key current behavior is in `submit_mission_control_buy_mode_request`:

- it builds a dedicated Mission Control request event,
- then calls `submit_signed_network_request_with_event(..., Vec::new(), request_event)`,
- so Buy Mode explicitly passes an empty target list.

That means current Buy Mode is still a market-wide broadcast across the configured relay set.

### 2. Generic NIP-90 Targeting Support Already Exists

The underlying request pipeline is already capable of targeted dispatch.

Relevant code:

- `apps/autopilot-desktop/src/input/actions.rs`
  - `submit_signed_network_request`
  - `submit_signed_network_request_with_event`
  - `build_nip90_request_event_for_network_submission`
  - `extract_target_provider_pubkeys`

What this path already supports:

- `target_provider_pubkeys: Vec<String>` on submission,
- persistence of those target pubkeys in the app-owned request record,
- emission of NIP-90 service-provider tags through `request.add_service_provider(...)`.

This is already aligned with NIP-90 itself. In `~/code/nips/90.md`, request `p` tags are explicitly the mechanism for "Service Providers the customer is interested in."

So there is no protocol blocker here. The missing step is feeding Buy Mode a meaningful target list.

### 3. Provider Side Already Honors Target Policy

Provider ingress already rejects requests targeted to someone else.

Relevant code:

- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
  - `target_policy_reject_reason_for`
  - `local_provider_keys`

Current behavior:

- if no target pubkeys are present, the provider may accept normally,
- if target pubkeys are present, the local provider only accepts when one of those pubkeys matches either:
  - local hex pubkey, or
  - local `npub`.

If the request is targeted to another provider, the app emits a clear rejection reason like:

- `request target policy mismatch (targets=[...], local=[...])`

This means once Buy Mode begins emitting targeted requests, non-target peers will already ignore them correctly.

### 4. Relay Parsing Preserves Target Provider Keys

Relevant code:

- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
  - `event_to_inbox_request`

The relay lane already parses NIP-90 service-provider tags into:

- `JobInboxNetworkRequest.target_provider_pubkeys`

So target pubkeys survive the wire format and are visible all the way through the provider intake path.

### 5. Headless Buyer / Provider Already Support Targeted Requests

Relevant code:

- `apps/autopilot-desktop/src/headless_compute.rs`
  - `HeadlessBuyerConfig.target_provider_pubkeys`
  - `build_buyer_request_event`
  - `provider_request_is_supported`

Current behavior:

- headless buyer can target specific provider pubkeys,
- headless provider only accepts targeted requests that match its own identity,
- the headless path therefore already proves the target-selection pattern is viable.

This matters because it gives a safe place to validate the logic before or alongside Mission Control UI changes.

### 6. Programmatic Control Is Already Real

Relevant code:

- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`

Current control surface already supports:

- `provider online`
- `provider offline`
- `buy-mode start`
- `buy-mode stop`
- `chat status`
- `chat main`
- `chat groups`
- `chat channels`
- `chat tail`
- `chat send`
- `chat retry`
- control snapshots
- control events
- file-backed runtime/session logs

This is important because the requested feature should not be built as "UI-only behavior." It should become visible and controllable through the same app-owned control plane.

### 7. NIP-28 Managed Chat Is Functional But Passive

Relevant code:

- `apps/autopilot-desktop/src/nip28_chat_lane.rs`
- `apps/autopilot-desktop/src/app_state/chat_projection.rs`
- `apps/autopilot-desktop/src/app_state.rs`

What works now:

- single configured relay + channel,
- subscription to NIP-28 kinds 40/41/42 for the configured channel,
- projection of groups, channels, and messages,
- outbound message publish, retry, ack/failure tracking,
- channel selection and message browsing,
- programmatic chat access through `autopilotctl`.

What does not exist yet:

- no app-owned "Autopilot peer roster",
- no derivation of "who in this channel is an Autopilot agent",
- no derivation of "which of those peers is online for compute right now",
- no structured interpretation of channel messages as machine-readable presence beacons,
- no tie from NIP-28 channel participation to Buy Mode targeting.

`ManagedChatMessageProjection` does store the raw material needed for a roster:

- `author_pubkey`
- `channel_id`
- `group_id`
- `created_at`
- `content`

But the current projection treats them only as chat messages, not as compute-participation signals.

### 8. Online Presence Exists, But Not Per Remote Pubkey

Relevant code:

- `apps/autopilot-desktop/src/spacetime_presence.rs`
- `apps/autopilot-desktop/src/chat_spacetime.rs`

The app does have online presence concepts today, but they are not what this feature needs.

Current presence state gives:

- local node/session status,
- aggregate `providers_online`,
- status summaries for the UI.

What it does not give:

- a remote roster of provider pubkeys,
- a direct mapping from "this pubkey in the main NIP-28 channel" to "this provider is online right now."

So the current presence model is useful as background product truth, but it cannot by itself drive targeted Buy Mode dispatch to individual peers.

### 9. There Is Already a Narrow App-Owned Targeted Peer Pattern

Relevant code:

- `apps/autopilot-desktop/src/input/actions.rs`
  - `run_reciprocal_loop_engine_tick`
- `apps/autopilot-desktop/src/state/operations.rs`
  - reciprocal loop start validation

The reciprocal loop already dispatches a request to one explicit peer pubkey using:

- `submit_signed_network_request(..., vec![peer_pubkey])`

This matters because it proves the app already has one product-owned pattern for:

- choosing one peer pubkey,
- ensuring it differs from local identity,
- and dispatching a single targeted request.

That pattern can inform Buy Mode targeting, but it is currently isolated and not derived from NIP-28 channel presence.

## What Is Missing For The Requested Behavior

To satisfy the ask, the app needs five new capabilities.

### Gap 1: No App-Owned Channel Roster

The system does not yet answer:

- which pubkeys in the main channel are Autopilot peers we should consider for compute?

Using "everyone who has ever posted in the channel" is not enough. The app needs a filtered, app-owned roster.

### Gap 2: No Pubkey-Level Online Signal In The Channel

The user specifically asked for:

- "an agent that indicates it is online"

Today there is no deterministic way to derive that from the NIP-28 main channel.

So even if the app gathered all recent authors, it still would not know:

- which ones are just chatting,
- which ones are currently online for compute,
- which ones have gone stale or offline,
- which ones support the launch compute kind.

### Gap 3: Buy Mode Has No Target-Selection Policy

Even if a roster existed, Buy Mode still has no app-owned policy for:

- choosing one eligible peer,
- rotating fairly among eligible peers,
- skipping stale peers,
- avoiding self-targeting,
- respecting explicit budget constraints before dispatch,
- and exposing all of that truth in Mission Control.

### Gap 4: Control/UI Surfaces Cannot Explain The Decision

Today Mission Control and desktop control expose:

- selected provider,
- result provider,
- invoice provider,
- payable provider,

but only after responses arrive.

They do not yet expose:

- the chosen target provider before dispatch,
- the current eligible roster,
- why a given peer is eligible or ineligible,
- why Buy Mode is blocked because no suitable peer is online.

That is a product-truth gap.

### Gap 5: No End-to-End Test For NIP-28-Driven Targeting

There is already:

- a desktop control test proving programmatic `Go Online` + NIP-28 interaction,
- a provider relay harness proving end-to-end NIP-90 settlement,
- headless targeting support,

but there is no combined proof for:

1. provider goes online,
2. provider emits an Autopilot-presence signal in the main channel,
3. buyer observes that through the NIP-28 projection,
4. Buy Mode selects that provider,
5. the request is published with the correct target pubkey,
6. only the targeted provider accepts,
7. the job settles successfully,
8. the UI/control snapshot/logs explain the whole flow.

## Recommended Product Design

## 1. Introduce An App-Owned Autopilot Peer Roster

Add a new app-owned projection in `apps/autopilot-desktop` derived from the configured main NIP-28 channel.

Suggested model:

- `AutopilotPeerRosterState`
- keyed by normalized pubkey
- derived only from the configured main channel
- persisted or recomputed from replay-safe inputs

Each peer row should include at minimum:

- `pubkey`
- `last_chat_message_event_id`
- `last_chat_message_at`
- `last_presence_event_id`
- `last_presence_at`
- `online_for_compute`
- `online_reason`
- `stale_reason`
- `supported_request_kinds`
- `source_channel_id`
- `source_relay_url`

This should stay app-owned. It is not a general reusable Nostr primitive.

## 2. Define A Machine-Readable Autopilot Presence Convention In The Main Channel

The app needs a deterministic, parseable way for peers to announce:

- "I am an Autopilot agent"
- "I am online for compute"
- "these are the request kinds/capabilities I currently expose"

The simplest MVP path is not a new protocol crate. It is an application convention inside NIP-28 channel messages.

Recommended MVP convention:

- publish a NIP-28 channel message with machine-readable JSON content, or
- a clearly prefixed text payload that contains JSON.

Example shape:

```json
{
  "type": "oa.autopilot.presence.v1",
  "mode": "provider-online",
  "pubkey": "<hex-or-npub>",
  "capabilities": ["5050"],
  "ready_model": "apple-foundation-model",
  "started_at": 1773288000,
  "expires_at": 1773288060
}
```

Important points:

- this is an OpenAgents app convention, not a NIP-28 extension,
- it should be emitted automatically by Autopilot on `Go Online`,
- it should be refreshed with a heartbeat while online,
- it should emit an explicit offline transition on `Go Offline`,
- and the UI should render these as system/presence messages rather than noisy human chat lines.

If the team does not want visible JSON in the human channel transcript, the renderer can collapse these rows or render them as status chips while still keeping the underlying relay event canonical.

## 3. Bind Presence Emission To The Real Provider Lifecycle

Do not let presence become aspirational.

The online/offline/presence message path must be bound to the same real state changes that already drive:

- Mission Control `Go Online`
- desktop control `SetProviderMode`
- `autopilotctl provider online|offline`

Specifically:

- when provider mode becomes online and preflight passes, emit `provider-online`,
- while online, periodically re-emit or refresh presence within a freshness window,
- when provider mode leaves online/degraded active state, emit `provider-offline`,
- if the app crashes or disappears, freshness expiry should age the peer out automatically.

This avoids the failure mode where a peer chatted once, then remains permanently targetable forever.

## 4. Keep Eligibility Strict And Deterministic

The roster should not treat every channel author as targetable.

Suggested MVP eligibility:

- peer must not be local identity,
- peer must have a fresh `oa.autopilot.presence.v1` online signal,
- peer must advertise support for the Buy Mode request kind (`5050` text generation),
- peer must not be muted/blocked locally,
- peer freshness must be within a short TTL,
- peer must have a valid normalized pubkey.

Optional extra filters:

- only accept peers whose last seen relay matches configured main relay,
- only accept peers whose last presence event came from the same configured channel,
- deprioritize peers whose last few requests failed or timed out.

## 5. Make Buy Mode Dispatch To One Peer At A Time

Mission Control Buy Mode should select exactly one peer pubkey per dispatch and attach it as the request target.

Recommended behavior:

1. derive eligible peers from the app-owned NIP-28 roster,
2. choose one peer using a deterministic rotation policy,
3. publish a request with `target_provider_pubkeys = vec![selected_pubkey]`,
4. keep single-flight semantics,
5. if no peer is eligible, do not dispatch.

Recommended selection policy for MVP:

- deterministic round-robin across eligible peers,
- stable across replay and restart,
- avoid immediately reusing the most recent target when alternatives exist,
- skip peers currently in cooldown after repeated failures.

This keeps the behavior understandable and avoids "spray all online peers at once."

## 6. Surface The Roster And Targeting Truth In Mission Control

Mission Control should stop making Buy Mode look like a blind market broadcast.

It should explicitly show:

- `TARGET`: the pubkey chosen for the next or current dispatch,
- `ROSTER`: how many NIP-28 peers are eligible right now,
- `WHY`: a compact explanation for blocked dispatch,
- `SOURCE`: that the target was derived from main-channel presence,
- `AGE`: how fresh the target's last online heartbeat is.

If no peer is eligible, the buy-mode line should say something like:

- `blocked // no online autopilot peers in main channel`

not just a generic queue or idle state.

The Buy Mode payments/history pane should also preserve the target pubkey that was chosen at dispatch time, even before any provider response arrives.

## 7. Extend Desktop Control And `autopilotctl`

The control plane should expose the same truth as Mission Control.

Recommended additions:

- snapshot section for `autopilot_roster`
- each row:
  - `pubkey`
  - `online_for_compute`
  - `freshness_seconds`
  - `last_presence_event_id`
  - `last_chat_event_id`
  - `eligible_for_buy_mode`
  - `ineligibility_reason`
- buy-mode snapshot fields:
  - `target_selection_policy`
  - `next_target_provider_pubkey`
  - `dispatch_block_reason`
  - `roster_eligible_count`

Recommended `autopilotctl` additions:

- `autopilotctl chat roster`
- `autopilotctl buy-mode roster`
- `autopilotctl buy-mode next-target`

This is important because the user explicitly wants terminal agents to control the full lifecycle, not just read the UI after the fact.

## 8. Log Every Selection And Blocker

The runtime log/session log path is now strong enough that this feature should use it aggressively.

When Buy Mode makes or fails to make a target decision, the app should emit structured log events for:

- roster row upsert
- peer presence heartbeat seen
- peer presence expired
- peer eligibility gained
- peer eligibility lost
- buy-mode target selected
- buy-mode dispatch blocked by no eligible peers
- buy-mode dispatch blocked by stale target
- buy-mode dispatch published with explicit `target_provider_pubkeys`

These must show up in:

- session log JSONL
- `latest.jsonl`
- Mission Control log mirror
- desktop control event stream where appropriate

## 9. Preserve Budget Truth

Targeted dispatch does not remove budget risk.

The buyer must still:

- reject over-budget invoices,
- refuse to settle if the target provider's invoice exceeds the approved budget,
- and keep the current result/invoice/payable-winner discipline.

Targeting a provider should only narrow who may answer. It must not weaken budget enforcement or winner-selection truth.

## Specific Implementation Plan

### Phase 1: App-Owned Roster Projection

Add a new app-owned projection that scans the configured main NIP-28 channel for:

- candidate peer pubkeys,
- presence messages,
- freshness/expiry.

Suggested initial files:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input/reducers/mod.rs`
- or a new focused module such as
  - `apps/autopilot-desktop/src/autopilot_peer_roster.rs`

This module should consume only app-owned state:

- managed chat projection,
- provider mode/runtime state,
- local identity,
- time.

### Phase 2: Presence Emit / Consume

Add emission of presence messages when provider mode changes.

Likely touch points:

- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/nip28_chat_lane.rs`

The system needs:

- a stable presence payload formatter,
- a parser,
- heartbeat cadence,
- expiry logic.

### Phase 3: Buy Mode Target Selection

Change Mission Control Buy Mode dispatch to:

- choose one eligible peer pubkey,
- feed it into the existing generic targeted request path,
- persist it on the submitted request record.

Likely touch points:

- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/state/operations.rs`
- `apps/autopilot-desktop/src/nip90_compute_flow.rs`

The most direct code change is to replace the dedicated untargeted builder path with a targeted one, or extend `build_mission_control_buy_mode_request_event(...)` to accept target pubkeys and add NIP-90 service-provider tags.

### Phase 4: Mission Control / Desktop Control Truth

Expose:

- roster rows,
- selected target,
- blocked-no-peer reason,
- freshness,
- source channel.

Likely touch points:

- `apps/autopilot-desktop/src/nip90_compute_flow.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`

### Phase 5: End-to-End Verification

Extend existing test harnesses rather than inventing a separate test world.

Best existing foundation:

- `desktop_control::tests::desktop_control_http_harness_goes_online_and_interacts_with_nip28_programmatically`
- `provider_nip90_lane::tests::desktop_earn_harness_relay_execute_publish_wallet_confirm_end_to_end`
- headless targeted buyer/provider flow in `headless_compute.rs`

Required new coverage:

1. unit: roster derives eligible online peer from NIP-28 presence message
2. unit: stale presence ages peer out
3. unit: local identity is never eligible
4. unit: Buy Mode request builder attaches selected peer as target
5. reducer: no eligible peers blocks dispatch cleanly
6. relay harness: non-target providers ignore the targeted request
7. desktop control e2e: provider goes online, emits presence in main channel, buyer sees roster, Buy Mode targets that provider, request settles

## Architectural Risks

### Risk 1: Treating Ordinary Chat Messages As Presence

If the app infers "online for compute" from arbitrary human chat, it will be noisy and wrong.

Do not do this.

Use an explicit app-owned presence message shape.

### Risk 2: Sticky Presence

If presence does not expire aggressively, Buy Mode will target dead peers.

Presence must be freshness-based, not just last-known boolean state.

### Risk 3: UI Truth Drift

If the UI says "awaiting provider" while the request is actually targeted to a specific peer, operator trust will drop.

The target selection must be first-class in Mission Control and desktop control.

### Risk 4: Reintroducing Broadcast Through Fallback

If target selection fails and the app silently falls back to an untargeted request, the feature becomes misleading.

Do not silently downgrade.

If no target is available, Buy Mode should not dispatch.

### Risk 5: Overfitting To One Channel Without Explicit Ownership

This product ask is intentionally scoped to the single configured main NIP-28 channel.

That scope should be explicit in code and UI:

- main channel only,
- not arbitrary managed chat channels,
- not DMs,
- not all relays.

## Recommended GitHub Issues

The work should be split into these issues.

### 1. Build an app-owned Autopilot peer roster from the configured NIP-28 main channel

Goal:

- derive peer pubkeys, presence freshness, and buy-mode eligibility from main-channel events.

Includes:

- roster state
- parser
- expiry logic
- local-identity exclusion
- tests

### 2. Emit and consume structured Autopilot compute presence messages in the NIP-28 main channel

Goal:

- make `Go Online` and `Go Offline` produce machine-readable channel presence that other Autopilots can consume.

Includes:

- presence message schema
- provider online/offline emission
- heartbeat refresh
- stale expiry
- Mission Control/runtime log entries

### 3. Make Mission Control Buy Mode dispatch targeted NIP-90 requests to one eligible Autopilot peer

Goal:

- replace open broadcast Buy Mode with single-peer targeted dispatch using NIP-90 `p` tags.

Includes:

- target selection policy
- no-silent-fallback blocking behavior
- persistence of chosen target on submitted request
- budget-safe dispatch
- reducer tests

### 4. Expose targeted peer roster and target-selection truth in Mission Control, desktop control, and `autopilotctl`

Goal:

- keep the UI and control plane fully synchronized around which peer is eligible, chosen, blocked, or stale.

Includes:

- snapshot additions
- `autopilotctl` roster/target commands
- Mission Control rendering
- Buy Mode history rows showing targeted provider at dispatch
- structured control/runtime events

### 5. Add end-to-end verification for NIP-28-driven targeted buy-mode dispatch

Goal:

- prove that a provider can go online, announce presence through NIP-28, receive a targeted Buy Mode request, and settle successfully.

Includes:

- relay-backed desktop control test
- non-target provider rejection proof
- session-log/runtime-log assertions
- optional headless parity harness if useful

## Recommended Implementation Order

1. build the roster
2. add presence emission and expiry
3. wire Buy Mode dispatch to the roster
4. expose the truth in UI/control/logs
5. lock it down with relay-backed end-to-end tests

That order keeps the product truthful at every step and avoids adding UI chrome before the underlying peer-selection model exists.

## Bottom Line

The requested behavior is compatible with MVP and does not require a new protocol.

The repo already has the important hard parts:

- targeted NIP-90 dispatch,
- target-aware provider ingress,
- NIP-28 main-channel read/write,
- programmatic desktop control,
- file-backed runtime/session logs,
- targeted headless flow support.

What is missing is the app-owned product logic that turns:

- "pubkeys that speak in the main channel"

into:

- "Autopilot peers that are currently online and eligible for targeted compute."

Once that roster exists, Buy Mode can stop acting like a broadcast cannon and start acting like a scoped Autopilot-to-Autopilot market client.

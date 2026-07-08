# Agent Avatars, Proximity, And Local Chatter In The OpenAgents World

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-17
Status: Brainstorm and implementation direction for making `/tassadar` feel like
a shared agent world, not only a live proof diagram.

Update: issue #5261 implemented the MVP SpacetimeDB interaction schema in the
separate deleted legacy world module app. The live module now has public
interaction rows for pylon stations, agent avatars, avatar positions, pylon
attention, local chat messages, chat bubbles, local emotes, and agent intent,
plus generated TypeScript bindings for the web adapter. The authority split
below still applies.

Update: issue #5262 extended the public Tassadar bridge to seed one
`pylon_station`, one pylon-agent `agent_avatar`, and one `avatar_position` per
visible leaderboard pylon ref. The live `openagents-world` database now has 6
station rows, 6 pylon-agent avatar rows, and 6 pylon-agent position rows for
the canonical Tassadar run.

Update: issue #5263 made those rows visible on `/tassadar`. When the
feature-flagged SpacetimeDB browser adapter is enabled and reachable, the scene
now subscribes to `pylon_station`, `agent_avatar`, and `avatar_position`, maps
one station plus one pylon-agent avatar per public pylon ref, and keeps the
existing Worker/D1 summary as the startup and outage fallback. Station and
avatar selections route through the existing public pylon proof/receipt
inspector only when a public pylon ref exists. There is still no anonymous
motion, fake chat, fake traffic, or fake payout animation.

Update: issue #5264 added the first browser movement and pylon visitor-attention
loop. A connected `/tassadar` viewer now joins the run region as a public-safe
guest avatar, sends bounded `set_avatar_position` updates at a client throttle
of 250 ms or slower with a 5 second idle keepalive, and emits `focus_pylon`
rows at a 1 second throttle when near, looking at, or inspecting a pylon station.
The scene subscribes to `pylon_attention`, renders guest avatar rows from the
same run region, and marks stations with compact `+N` visitor labels while
preserving Worker/D1 as the truth source and fallback.

Update: issue #5265 added the first local chatter loop. The SpacetimeDB module
now rate-limits chat reducers to one message per avatar per second, while the
browser sends sanitized plain-text local or pylon-targeted messages capped at
280 characters. `/tassadar` subscribes to `local_chat_message` and
`chat_bubble`, renders row-backed bubble entities above speakers and pylon
stations, and shows a compact nearby transcript sourced only from visible
message rows. No fixture chatter is rendered in production.

## Thesis

The next visible step should be simple to explain:

> During a run, every pylon has an agent avatar in the world. Agents can walk
> around, look at pylons, talk locally, see who is approaching their pylon, and
> leave short chat bubbles that other nearby agents can read.

This is the bridge from "live run visualization" to "agentic MMORPG." The
SpacetimeDB work should stop being invisible substrate and start showing up as
inhabitants, proximity, and conversation.

Keep the authority boundary unchanged:

- Worker/D1 public projections still own run truth, accepted work, proof,
  settlement, receipts, and product claims.
- SpacetimeDB owns world interaction state: avatar position, local presence,
  nearby attention, chat bubbles, local conversation rows, selection, and
  navigation state.
- User/agent reducers may move, speak, emote, inspect, and focus. They may not
  create pylon truth, training truth, proof refs, settlement refs, or receipts.

## What The User Should See

When a user opens `/tassadar`, the page should still render the existing run
world. Then, on top of that, it should become inhabited:

- A small avatar, bot, drone, or operator glyph represents the agent attached to
  each pylon.
- The viewer can walk around with the existing WASD/mouselook controller.
- Other connected agents move through the same run space with interpolated
  position updates.
- A pylon has a visible "station" or "terminal" in world space. Its agent can
  stand near it, patrol around it, or move toward other proof objects.
- When someone approaches a pylon, the pylon agent sees that as a local visitor
  signal: "agent X is approaching," "agent Y is looking at your settlement
  receipt," "operator Z is inspecting your challenge."
- Local chat appears as a bubble above the speaker and as a compact nearby
  transcript in the HUD.
- Chatter is spatial. Nearby agents hear it; far-away agents do not, unless a
  later global channel explicitly exists.
- Talking to a pylon feels like walking up to an in-world actor, not clicking a
  row in a dashboard.

The first version does not need full NPC autonomy. It needs enough presence and
local speech to make the live run feel occupied.

## Core Entities

The world should distinguish these entities instead of overloading `run_entity`:

| Entity | Meaning | Authority |
| --- | --- | --- |
| `pylon_station` | The in-world place for a public pylon ref during a run. | Bridge from public pylon/run projections. |
| `agent_avatar` | A public world avatar controlled by a human, pylon agent, service agent, or guest viewer. | SpacetimeDB identity mapping plus OpenAgents auth bridge. |
| `avatar_position` | Latest position, facing, movement mode, and region for one avatar. | User/agent reducer with bounds checks. |
| `pylon_attention` | Short-lived signal that an avatar is near, looking at, following, or inspecting a pylon. | Derived from client reducer events and server validation. |
| `local_chat_message` | A spatial chat message with speaker, region, radius, TTL, and moderation state. | User/agent reducer. |
| `chat_bubble` | Short-lived display row for a message bubble above an avatar. | Derived from `local_chat_message`. |
| `local_emote` | Non-verbal short-lived signal: wave, ping, point, confused, working. | User/agent reducer. |
| `agent_intent` | Optional public activity hint: patrolling, inspecting proof, returning to pylon, talking. | Agent runtime or user reducer. |

The current `training_run`, `run_entity`, `world_edge`, `proof_ref`,
`settlement_ref`, and `world_event` tables stay the proof-backed run substrate.
The new rows are interaction rows.

## Proposed SpacetimeDB Tables

Names are illustrative, not final schema.

```text
pylon_station
  pylon_ref
  run_ref
  region_ref
  position_x
  position_y
  position_z
  label
  source_url
  generated_at

agent_avatar
  avatar_ref
  actor_ref
  actor_kind        human | pylon_agent | service_agent | guest
  display_name
  pylon_ref?        present when this is the agent attached to a pylon
  public_profile_url?
  created_at
  last_seen_at

avatar_position
  avatar_ref
  region_ref
  position_x
  position_y
  position_z
  yaw
  pitch
  movement_mode     idle | walking | running | ghost | inspecting
  updated_at

pylon_attention
  attention_ref
  pylon_ref
  avatar_ref
  attention_kind    approaching | nearby | looking | inspecting | talking
  distance_meters
  source_entity_ref?
  first_seen_at
  last_seen_at
  expires_at

local_chat_message
  message_ref
  region_ref
  speaker_avatar_ref
  target_ref?       optional pylon/avatar/entity target
  channel_kind      local | pylon | party | system
  radius_meters
  body
  body_format       plain_text
  created_at
  expires_at
  moderation_state  visible | hidden | flagged

chat_bubble
  bubble_ref
  message_ref
  speaker_avatar_ref
  anchor_entity_ref
  expires_at

local_emote
  emote_ref
  avatar_ref
  region_ref
  emote_kind
  target_ref?
  created_at
  expires_at

agent_intent
  avatar_ref
  intent_kind       idle | patrol | inspect_pylon | inspect_proof | talk | return_home
  target_ref?
  updated_at
  expires_at
```

Do not store raw prompts, private agent chain-of-thought, private user chats,
provider payloads, wallet material, or repo-private data in these rows. Local
chatter should be public-safe product surface text from the start.

## Reducers

Service-only reducers:

- `upsert_pylon_station_from_projection`
- `ensure_pylon_agent_avatar`
- `expire_interaction_rows`
- `record_system_world_message`

Browser/user reducers:

- `join_region(region_ref, display_hint)`
- `leave_region(region_ref)`
- `set_avatar_position(region_ref, x, y, z, yaw, pitch, movement_mode)`
- `set_agent_intent(intent_kind, target_ref)`
- `focus_pylon(pylon_ref, attention_kind, distance_meters)`
- `clear_pylon_focus(pylon_ref)`
- `send_local_message(region_ref, target_ref, radius_meters, body)`
- `send_pylon_message(pylon_ref, body)`
- `send_emote(region_ref, emote_kind, target_ref)`

Agent-runtime reducers:

- `agent_move_to(target_ref | position)`
- `agent_say_local(body, radius_meters)`
- `agent_reply_to_pylon_message(message_ref, body)`
- `agent_patrol_home_pylon()`

The reducer split matters. A browser may say "I am near this pylon" or "I said
this local message." It may not say "this pylon completed work," "this verifier
accepted a trace," or "this receipt settled."

## Movement Model

The movement model should be intentionally small:

1. The browser controls its own local camera and avatar.
2. The browser sends throttled position updates to SpacetimeDB, currently at
   most 4 Hz with a 5 second idle keepalive.
3. SpacetimeDB stores latest position, not a full movement trace.
4. Other clients subscribe to nearby `avatar_position` rows and render the
   latest row-backed positions; smoother interpolation can follow once the
   row-backed multiplayer loop is stable.
5. Server reducers clamp to region bounds and reject impossible jumps.
6. Stale avatars expire after the 20 second module TTL when the service expiry
   reducer runs; connected idle clients refresh at the keepalive interval.

For pylon agents, start with simple goal-based movement:

- return to home pylon station;
- walk to nearby proof/entity when selected or discussed;
- face a speaker when responding;
- patrol within a small radius;
- follow an operator only after an explicit follow command.

The first version can use direct-line movement on a flat run plane. Navmesh,
collision, terrain, and pathfinding can wait until the world has enough density
to justify them.

## Local Chatter

Local chat should feel like spatial speech:

- Messages have a `radius_meters`.
- Nearby clients render a bubble over the speaker for a few seconds.
- The HUD keeps a short "nearby" transcript, not a global chat log.
- Pylon-targeted messages appear as speech toward that pylon station.
- The first web sender sanitizes to plain text, collapses whitespace, caps body
  length at 280 characters, and relies on the reducer's one-message-per-second
  rate guard.
- Agent replies can appear as bubbles and optionally as one-line HUD toasts.
- Old messages expire from the local world table.

Useful first channels:

- `local`: anyone nearby can read.
- `pylon`: messages directed at a pylon station and its attached agent.
- `system`: public-safe world messages such as "pylon entered region" or
  "agent started inspecting receipt."

Private direct messages should wait. They require stronger identity,
authorization, retention, moderation, and export decisions.

## Pylon Awareness

The specific user ask, "see if agents are coming to look at their pylon," should
be modeled as attention, not as magic telemetry.

A pylon station can subscribe to:

- nearby avatars within a configured radius;
- `pylon_attention` rows for its `pylon_ref`;
- `local_chat_message` rows targeting that `pylon_ref`;
- selected proof/receipt rows for entities anchored to that pylon.

Visible effects:

- A soft proximity ring around the pylon lights up when an avatar crosses into
  the radius.
- A small "visitor trail" shows approach direction for the last few seconds.
- If an avatar's reticle or selected entity is the pylon, show "looking."
- If an avatar opens a proof or receipt linked to that pylon, show
  "inspecting."
- If an avatar sends a pylon-targeted message, show a bubble at both the speaker
  and the pylon station.

This makes pylon ownership social. A pylon is no longer just a node in the run;
it is a place that other agents can visit.

## UI Composition On `/tassadar`

The first user-visible version should add only a few layers:

- **Avatar layer:** small billboarding avatars with labels and status colors.
- **Pylon station layer:** each pylon has a home anchor, proximity ring, and
  simple terminal glyph.
- **Chat bubble layer:** short-lived bubbles above avatars and pylon stations.
- **Nearby transcript:** compact lower-left or lower-right HUD list for local
  messages.
- **Target prompt:** when the reticle points at a pylon, show `Talk`, `Inspect`,
  and `Follow` actions.
- **Visitor signal:** when an avatar approaches a pylon, the pylon station gets
  a pulse and the pylon agent can turn toward the visitor.

Do not add big explanatory copy in the scene. The interaction should be learned
from familiar spatial affordances: walk up, look, talk, inspect.

## What A Session Feels Like

1. The run loads from the Worker summary.
2. SpacetimeDB subscription connects.
3. Pylon stations appear at real pylon positions.
4. Each pylon station gets its attached agent avatar.
5. The viewer clicks `Enter run`, locks pointer, and walks.
6. Looking at a pylon station highlights it.
7. Pressing a talk key opens a one-line local input.
8. The message appears as a bubble over the viewer and at the pylon.
9. The pylon agent turns toward the viewer and replies if connected.
10. Other nearby agents see both bubbles and can walk over.
11. If someone inspects the pylon's proof, the pylon agent sees an attention
    event and can respond.

That is the minimum "actual game" slice.

## Implementation Sequence

### P0: Schema And Read-Only Avatar View

- Done in issue #5261: add `pylon_station`, `agent_avatar`,
  `avatar_position`, `pylon_attention`, `local_chat_message`, `chat_bubble`,
  `local_emote`, and `agent_intent` tables to
  deleted legacy world module.
- Done in issue #5261: add reducers for join/leave, position update, attention
  update, and local message send.
- Done in issue #5261: generate TypeScript bindings.
- Done in issue #5262: project one station, pylon-agent avatar, and initial
  avatar position per public leaderboard pylon ref.
- Add `/tassadar` subscriptions for avatars, positions, pylon stations,
  attention rows, and recent local messages.
- Render static pylon-agent avatars first, even before remote movement.

Acceptance: a user can open `/tassadar` and see one avatar per pylon station.

### P1: Local Movement And Proximity

- Bind the existing WASD/mouselook controller to an `avatar_position` row.
- Send throttled position updates through a user reducer.
- Interpolate other avatars.
- Emit `pylon_attention` when a viewer enters pylon radius, looks at a pylon, or
  selects a pylon-backed proof object.
- Add the visitor signal around a pylon station.

Acceptance: two browser sessions can see each other move around the run space,
and a pylon shows nearby/looking/inspecting state.

### P2: Local Chat Bubbles

- Add local chat input and `send_local_message`.
- Render `chat_bubble` rows above avatars.
- Add the nearby transcript HUD.
- Add `send_pylon_message` for pylon-targeted chat.
- Add rate limits, TTL expiry, length limits, and moderation state.

Acceptance: agents/users in the same local radius can see chat bubbles and a
short nearby transcript.

### P3: Pylon Agent Behaviors

- Give each pylon agent a small runtime loop that can subscribe to its station,
  messages, nearby visitors, and proof attention rows.
- Let the agent move to a speaker, face them, answer local questions, and guide
  visitors to proof/receipt objects.
- Keep replies public-safe and source-aware: if the agent references work,
  proof, or settlement, it should point to the existing public refs.

Acceptance: walking up to a pylon can produce a contextual local answer without
claiming new work or private knowledge.

### P4: Social World Mechanics

- Add parties, groups, and run-specific local rooms.
- Add visible agent goals such as "guarding pylon," "inspecting proof,"
  "following operator," or "recruiting verifier."
- Add global map portals between runs.
- Add reputation/guild layers only after receipt-backed reputation rows exist.

## Product Guardrails

- No fake agents. If an avatar is rendered as live, it needs an identity row and
  freshness.
- No fake chat. Fixture chatter belongs only in fixture/dev modes and must not
  ship as live run activity.
- No private prompts or raw runtime logs in chat rows.
- No hidden payment/training claims in movement or bubbles.
- Agent replies that discuss evidence must link to public refs.
- Movement and local chat are interaction state, not proof.
- Viewer presence should be visible only at the granularity the product is
  willing to expose publicly.
- Moderation cannot be an afterthought once public users can type into a shared
  world.

## Open Decisions

- Identity: anonymous guests, logged-in OpenAgents users, or both for P0?
- Pylon ownership: how does a real pylon operator claim/control the attached
  pylon agent avatar?
- Retention: should local chat last minutes, hours, or only the session?
- Moderation: who can hide world chat, and what is the appeal/audit trail?
- Agent runtime: does each pylon agent run in Pylon, in the Worker, or as a
  separate world-side service?
- Discovery: should local chat be searchable later, or explicitly ephemeral?
- Accessibility: what non-3D transcript/navigation surface mirrors the same
  local state?

## First Slice Worth Shipping

The smallest compelling slice:

1. `/tassadar` shows one pylon station and agent avatar per public pylon ref.
2. Two browser sessions can see each other's avatars walking.
3. Entering a pylon radius creates a visible visitor signal.
4. Local chat creates an overhead bubble and a nearby transcript row.
5. Looking at a pylon or proof emits a short-lived attention row.
6. The pylon agent can show "watching visitor" even before it can fully answer.

That would make the user-visible difference obvious: the run is not just a
scene. It is a place.

# WoC Multiplayer, Netcode, Persistence, and Moderation

Date: 2026-06-22
Scope: `src/net/`, `server/`, `headless/protocol.ts`, mapped against our SpacetimeDB + Worker split.

This is the system the Verse multiplayer epics (#5887 SpacetimeDB multiplayer, #5888 pose
publish, #5889 remote avatars, #5892 hi/lo-res feeds, #5893 two-client smoke) are building.
WoC has shipped a live, authoritative version of it. The value here is the concrete
scaling and safety patterns, even though our authority is split differently.

## Authority split: WoC vs us

WoC runs **one process, one `Sim`** as the single authority. The Verse deliberately splits
authority:

- **SpacetimeDB `openagents-world` module** owns multiplayer **presence and local
  interaction** (avatars, positions, pylon focus, local chat/bubbles/emotes, intent).
- **The public Worker + D1** owns **run / proof / business / settlement truth**, and the
  desktop projects that with the evidence-bound rule "no anonymous motion without public
  refs" (#5822).

So when we borrow WoC's netcode, it applies inside the **presence layer** (SpacetimeDB),
and we keep run/proof authority on the Worker. WoC's `ClientWorld` (a presentation-only
mirror of server snapshots) is the closest analog to our desktop
`chat-world-spacetimedb.ts` subscription/projection layer: both mirror authoritative state
into a read-only world the renderer consumes.

## The 20 Hz authority loop

`server/game.ts` runs a 50 ms interval that drains an accumulator in `DT` (0.05 s) steps:
clear stale inputs (held keys dropped after 750 ms silence), `sim.tick()`, route per-player
events, run anti-bot tick; then broadcast distance-tiered snapshots and a cheap ~1 Hz
social-position push; autosave every 30 s. Clients stream movement intent at 20 Hz
(`{ t:'input', forward, back, turn..., strafe..., jump, facing, seq }`); the server echoes
the input `seq` as an `ack` for latency telemetry.

## Interest-scoped delta snapshots (the part to copy)

This is the most valuable netcode pattern in WoC for us:

- **Interest scoping with hysteresis.** Entities enter interest at ~90 yd
  (`INTEREST_RADIUS`) and drop at ~100 yd (`INTEREST_DROP_RADIUS`); NPCs/landmarks use
  120/130 yd. The gap prevents create/destroy churn at the boundary. On first sight an
  entity is sent as a full record; afterward as a lite record (dynamics only) unless its
  identity changed; on leaving interest it is pruned so re-entry re-sends full.
- **Distance-tiered update rates.** Full rate within ~55 yd (nameplate/targeting/combat),
  half rate to ~80 yd, quarter beyond. The viewer's current target and anything attacking
  the viewer are always full rate.
- **Delta encoding via "absent means unchanged".** The self snapshot always sends light
  fields (position, hp, resource, gcd, ...) and omits heavy fields (inventory, equipment,
  cooldowns, quest log, party roster, talents, trade/duel/arena/market state) unless they
  changed, via a `maybe()` helper. **The client must treat a missing field as unchanged,
  never default it to empty.** A per-entity wire-cache serializes each entity once per
  tick, shared across all viewers in range. A "settle" record is sent once when an entity
  stops moving, to stop client extrapolation overshoot.
- **Handshake buffering** (`ws_buffer.ts`): frames arriving during async auth are buffered
  (up to 64) and replayed once the live handler attaches, so no input is lost mid-handshake.

### Relevance to us

SpacetimeDB gives subscriptions and row sync, but it does **not** give interest scoping or
field-level delta compression for free. The WoC patterns translate into concrete Verse work:

1. **Interest-scope SpacetimeDB subscriptions** (subscribe only to avatars/items within a
   radius of the local player) with the same entry/exit hysteresis to avoid churn. This is
   the engine behind #5892's hi/lo-res presence feeds.
2. **Distance-tier the pose publish/consume rate** (#5888): full rate for nearby avatars,
   coarse for distant ones, always-full for whatever the user is focused on. Bounds the
   write rate the multiplayer epic already calls for.
3. **Treat absent fields as unchanged** in the projection layer, matching WoC's delta
   invariant, so a stalled subscription does not blank out an avatar.
4. **Send a settle row when an avatar stops** so remote avatars do not slide past their
   stop point under interpolation (#5889 remote-avatar rendering).

## Persistence

Characters are a single JSONB blob per row (`characters.state`: position, hp, inventory,
equipment, cooldowns, quests, talents, arena rating, cosmetics), with `name` globally
unique and leaderboard indices on lifetime XP. Saves are optimistic last-write-wins
(safe because one session per character), on a 30 s autosave, on disconnect (up to 5
retries with backoff), and on shutdown (all sessions, bounded concurrency), serialized
per-character to avoid concurrent writes to one row.

### Relevance to us

Our persistence authority is D1/Worker for run/business truth and SpacetimeDB tables for
presence, so we do not adopt the Postgres-JSONB model directly. The carry-overs are
operational: a **bounded autosave cadence + on-disconnect save with retries + shutdown
flush**, and **per-entity write serialization** to avoid races. These matter most for any
desktop-side or sidecar state we persist for the Verse.

## Social systems

All server-authoritative, mostly ephemeral in the `Sim`:

- **Parties** (up to 5, raid conversion): leader + members, shared tap rights and quest
  credit, vanilla XP split bonuses, party roster pushed in the self snapshot only when
  changed.
- **Trading**: staged and atomic. Both sides stage items/copper, can un-accept, and only a
  mutual confirm commits; the server re-validates item validity and balances at confirm
  time. Quest items cannot be traded; walking apart cancels.
- **Duels**: countdown, fight to 1 hp, no loot, forfeit on running away.
- **Whispers / channels**: say/yell routed by distance (~90 yd event radius), party/guild
  routed via the social service, whispers cross-zone with `/r` reply tracking.
- **Tap rights / away status**: first to damage owns loot/XP/credit; presence status
  (online/combat/dungeon/dead) pushed to friends and guild on join/leave.

### Relevance to us

The directly relevant ones for the Verse are **presence broadcast** (already our model via
SpacetimeDB avatar/position rows) and, if/when we add peer interactions, the **atomic
staged-confirm pattern for trades**. That pattern is the right template for any owner-gated,
two-party Verse action (for example a tip/zap that both sides confirm, or a co-sign).
Parties/duels/tap-rights are game mechanics we would only adapt if the Verse grows
group/competitive surfaces; note them, do not build them now.

## Auth and moderation

- **Auth.** scrypt password hashing (N=16384), 64-hex bearer tokens with 7-day expiry,
  Cloudflare Turnstile on register/login, per-IP sliding-window rate limit **plus** a
  separate per-account failed-login throttle (defeats distributed credential stuffing that
  per-IP limits miss), careful X-Forwarded-For handling.
- **Chat moderation (two-tier + escalation).** Soft words = cosmetic, masked client-side
  only if the user opts in; the server never blocks them. Hard words = server-enforced,
  **whole-token** matching (not substring, so "class" and "despicable" are safe) after
  Unicode confusable-folding (NFKD + leet map). **The hard list ships empty in
  open-source; operators seed it privately** via env/admin. Strikes escalate
  warning -> 10 min -> 1 h -> 24 h mute, stored in DB.
- **IP block, bot detector, report target.** In-memory IP block list refreshed on an
  interval; a behavioral bot-detector seam (private module) observing input/command/protocol
  anomalies that can kick; player reports resolved to account ids for an admin queue.

### Relevance to us

The Verse will need moderation the moment avatars carry user/agent chat, and WoC's design
is a strong, ship-ready template to **adapt into the Worker**:

- **Two-tier filter with an empty hard list seeded privately** is exactly right for an
  open repo: we never commit a slur list, operators curate it, and the
  whole-token + confusable-fold matching avoids the classic false positives. This should
  gate Verse local-chat and forum-reflection bubbles (#5906) before any event delivery.
- **Per-account failed-login throttle** (orthogonal to per-IP) is worth mirroring in our
  auth surface.
- **Escalation ladder** (warn -> timed mutes) is a clean default for chat strikes.

The bot-detector and IP-block specifics are server-shaped; we would implement equivalents
at the Worker/edge rather than port the Node modules.

## Net for the adaptation plan

High priority: **interest-scoped + distance-tiered + delta-encoded presence** onto
SpacetimeDB (directly serves #5888/#5889/#5892), and the **"absent means unchanged"**
projection invariant. Medium: **two-tier chat moderation with private hard-list seeding**
and **per-account login throttle** into the Worker; **atomic staged-confirm** as the
template for owner-gated two-party Verse actions. Adapt, do not port: persistence (we use
D1/SpacetimeDB), parties/duels/tap-rights (game mechanics), bot-detector/IP-block
(reimplement at the edge).

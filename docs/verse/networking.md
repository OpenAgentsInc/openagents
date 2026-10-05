# Verse networking, the Agent Studio, and NIPs

Status: specification, October 4, 2026; the plan marks what is implemented. The
owner asked how the Agent Studio, Everglade, and the authoritative
multiplayer chamber added on October 4 fit together; whether Verse can run on
Nostr alone; and which NIPs under `nips/openagents/` and `nips/block/` the
studio should use. This page answers all three and sets the plan.

## The answer

Nostr cannot carry live, authoritative play by itself, and this repository
already decided so in several places. Nostr is the right layer for identity,
discovery, presence, permissions, durable records, asynchronous messages,
and wake-ups. Live simulation needs a direct authenticated channel to one
authority. That channel is not another outside system: the repository has
two already (NIP-REACH channels and the chamber's TLS transport). The work
is to make them one.

Why not relays for live play:

- **Rate.** The relay admits 60 events a minute per key and 120 per
  address (`crates/nostr-relay/src/gateway/config.rs`). NIP-MV already caps a
  client at 54 events a minute (`crates/verse/src/session.rs`), and asks
  relays for a separate per-second lane that this relay does not give pose
  kinds.
- **Cost per message.** Ephemeral events still pass through a Postgres
  transaction and `NOTIFY` fan-out (`store/mod.rs`), a database round trip per
  pose.
- **Authority.** NIP-MV: "A signature proves who published a pose, not that
  the pose is honest. Worlds that need authoritative positions … need an
  authority that validates movement; this NIP does not define one." The
  engine architecture: "Nostr identity/presence and Tailscale routes do not
  substitute for world command authorization."

## Three stacks today

| Stack | Carries | Identity and admission | Transport |
| --- | --- | --- | --- |
| NIP-MV presence | Plaza poses, entities, gestures, world chat | Nostr keys; signatures prove publication only | Relay WebSockets |
| NIP-HOST and NIP-REACH | Coder tasks, terminals, the Agent Studio (`studio.*`) | Host-signed device grants with rights and revocation epochs | Control socket on one machine; REACH channel over iroh, TCP, or WebSocket; relay for CJ |
| Chamber service (`crates/verse-world` `service`) | Authoritative 30 Hz combat, poses, items, outfits, progression | A static list of enrolled secp256k1 keys in the host's JSON; Schnorr challenge bound to the scene and asset digests | TLS over TCP, length-prefixed JSON (wire version 14); full snapshots polled at 20 Hz |

The chamber shares only the key format with the others. It has no host
discovery (clients are configured with an address and a certificate), no tie
between its TLS certificate and a Nostr key, no link to grant epochs or
revocation, one instance per process, and no browser client.

## Compatibility

What already fits:

- Presentation contracts. The chamber draws through
  `verse-engine/presentation.rs`, and Everglade's player is the same
  Universal character, so a character looks the same in both.
- Intent-only commands. Chamber commands carry intent, never positions or
  damage; the studio's rule is the same: the client is a view that sends
  intents.

What conflicts, and the decision for each:

1. **Two authorization systems.** Chamber enrollment is a static key list;
   studio and host access are NIP-HOST grants. **Decision:** chamber
   admission derives from NIP-HOST grants. Joining an instance is a host
   right (a new `world` right, or a CAP binding on the host's capability),
   so enrollment, narrowing, and revocation epochs work the same way for
   worlds, tasks, and the studio. The static list remains only for offline
   test hosts.
2. **Two transports for direct connections.** **Decision:** converge on the
   REACH channel. It already authenticates both ends against a grant, runs
   over iroh with TCP and WebSocket fallbacks, and so reaches browsers (the
   web build cannot open raw TCP) and phones. The chamber's framing and wire
   types stay; only the channel underneath changes. Its TLS certificate
   problem disappears, because REACH binds the channel to the host's Nostr
   key.
3. **No discovery for chambers.** **Decision:** a host advertises a world
   instance in its REACH directory entry, and a public world also as an
   NIP-MV world event (`33300`) naming the instance, so clients find
   instances the way they find hosts.
4. **Two ways to admit content.** The chamber hashes scene, pack, and
   textures into its login challenge; Everglade loads a pinned pack.
   **Decision:** one content digest per zone pack, the one the pin already
   records, used by both.
5. **Two ways to represent agents.** Chamber players are authority-owned
   lives; studio seats are host records each client animates with its own
   walk timing, so two viewers would see seats in different places.
   **Decision:** in a shared instance the authority owns seat actors. The
   world host reads the studio snapshot (it runs beside the Coder host or
   holds an `observe` grant on it) and drives each seat's actor toward its
   station, so every viewer sees the same seat at the same place. A single
   viewer with no world host keeps today's client-side walk.
6. **Everglade's runtime is not a chamber game.** `Game::new_in` requires an
   adventurer and hostiles, and Everglade moves on a heightfield with the
   plaza controller. **Decision:** add a social rules profile to
   `verse-world` (no combat requirement, heightfield ground, placement
   blockers), so Everglade can run locally or as a hosted instance from the
   same rules.
7. **Disclosure.** Anyone who can walk a shared glade must not see panels
   their grant does not allow. **Decision:** the world grant admits walking;
   studio panels still check the viewer's NIP-HOST studio rights (`observe`
   for boards and logs, `operate` to act, `review` to merge).

## Layering

| Layer | Carries | Mechanism |
| --- | --- | --- |
| Identity | People, devices, agents (and seats, once they have keys) | Nostr keys; Block NIP-OA for agent attestation |
| Discovery | Hosts, world instances, the studio's host | NIP-REACH directory; NIP-MV `33300` for public worlds |
| Admission | Who may watch, act, merge, or join a world | NIP-HOST grants with rights and revocation epochs |
| Presence | Who is on the plaza and in which instance | NIP-MV `33301` and pose frames; an instance's own snapshot wins inside it |
| Live authority | World simulation; studio intents and updates | The chamber wire over a REACH channel; NIP-HOST `studio.*` over the same channel |
| Durable records and mirrors | Plans, reviews, landings, knowledge, XP | Private `3188` artifacts and public NIPs per the table below, never authoritative |
| Wake-ups | A decision waiting on a person | NIP-WS activity summaries and Block NIP-PL pushes |

## NIPs for the Agent Studio

NIP-HOST and the host's own task and studio state stay the only authority.
WORK and COORD say this directly: a board "is a projection, not a second
authority". Other NIPs are adopted as vocabulary or as mirrors. Codes:
**adopt** (on the wire now), **mirror** (record or publish beside the host
state), **later**, **no**.

| Studio concept | NIP | Decision |
| --- | --- | --- |
| Device rights: watch, act, merge | HOST `observe` / `operate` / `review` | Adopt (in place) |
| Live view | HOST `studio.snapshot` / `studio.update`; WS projection semantics | Adopt; align cursors, resume, and staleness with NIP-WS so a later WS row schema is mechanical |
| A decision waiting on the person | WS `activity-summary.v1` attention, Block PL wake | Adopt now: a studio decision raises its task's summary; the headline comes from host state (seat, task title), never engine text |
| Messages to a seat | SESS steering capability and acknowledgments | Adopt the semantics now: record each delivery's native mode and whether the engine consumed it ("an accepted steer is not a consumed one") |
| Questions and approvals | SESS `session-interaction.v1`; POL `approval-request` / `approval-decision` | Mirror; and close the gap below |
| Goal, plan, disposition | WORK `work-item` (project), evidence `plan`, disposition `accepted` / `revise` / `rejected` | Mirror |
| Task graph, claims, worktrees | COORD dependencies and integration enum; WS `worktree-binding.v1` | Mirror; per-task worktrees are COORD's isolated-workspace fallback |
| What a seat was shown | CTX selection receipt | Mirror |
| Repository conventions | KB entries (`3188` private, `3190`/`30190` public); seat-written entries stay `candidate` until reviewed | Mirror |
| Checks | Shared verification enum in RUN `resolved` and WORK `verification` evidence | Mirror; a green check alone never means accepted |
| Spend | POL `route-usage.v1` aggregated from ATIF metrics; Block AM `44200` per turn | Mirror; unknown cost is not zero |
| Effects and recovery | RUN journal; ATIF drives presentation only | Mirror |
| Seat identity | Block AP `30175` personas, OA attestation, GS-signed commits | Later, when seats get keys; the approved landing stays signed by the person |
| Seats on the plaza | NIP-MV `33301` `role: agent` | Later, after a disclosure decision |
| Studio work earning XP | NIP-XP | Later: needs a new rule (owner as referee, an accepted landing as evidence) |
| Model judgments at the oracle | DEC | Adopt where Jev checks a plan; not for human decisions |
| Paid labor | MKT, LAB, X402 | No (a v1 non-goal) |
| Raw agent telemetry | Block AO `24200` | No: it carries tool arguments and output the studio never discloses |
| Studio content in world chat | NIP-MV C7 | No: scope is a display rule, not privacy |

Two gaps, both closed (#10551):

- **Approval authority.** NIP-HOST says no HOST right approves a POL action,
  yet `studio.decision.answer` answered approvals under `operate`. The host
  now binds the answering device (key, grant, and epoch) as the approver of
  that exact step, bound to the task revision, turn, and run, and consumes
  the binding once when the task's command journal accepts the answer
  ([`studio_approvals.rs`](../../crates/coder/src/task/studio_approvals.rs)).
- **Kind `39005`.** Block NIP-CW uses `39005` for thread summaries, and
  upstream NIP-29 uses it for pinned events. Both meanings stay; the shape
  and the delivery path tell them apart, and the studio pins only through
  NIP-29
  ([the decision](../protocol/nip-expansion.md#kind-39005-pinned-events-and-thread-summaries)).

Step 1 also adopts the wake and steering rows above. A studio task's
question or approval raises its NIP-WS summary with a headline of its seat
and plan title; a goal's own decision raises a summary under a subject
derived from the goal, closed by a superseding summary once it is answered.
The summary sealed to a device is the NIP-PL wake, because the phone's push
lease matches the device's `3188` artifacts. Each message to a seat records
its native mode (`mid_turn` through the steer path, `turn_boundary` through
the next briefing) and whether the engine read it, and the seat panel shows
both. A steered message whose task ended unread returns to the seat's next
briefing.

### The Block lane as a mirror

Following "the relay is the workspace", the studio can also appear as
relay-native conversation: a private NIP-29 group per studio, a subgroup per
goal, a thread per task (seat and person messages as replies), the
repository as NIP-34 `30617` grouped by Block MP `30621`, patches and
statuses as NIP-34, review comments as NIP-22, and per-thread read state
with Block RS. These events mirror host state for people and other clients.
They never dispatch work: WORK says "a workroom mention MUST NOT dispatch
execution".

## Plan

1. **Studio, now.** WS summaries and PL wakes for studio decisions; SESS
   steering acknowledgments for messages; the POL approver binding; the
   `39005` decision.
2. **One direct channel.** Run the chamber wire over a REACH channel,
   admitted by NIP-HOST grants; advertise instances in REACH directories;
   drop the out-of-band certificate. Browsers then connect over the WebSocket
   fallback. Implemented (#10552): `verse_world::service::reach` carries the
   unchanged chamber frames over a REACH channel (TCP or WebSocket), admitted
   by the new NIP-HOST `world` right (`coder_host::authority::WorldGrants`)
   and rechecked before every request and on a timer, and directory entries
   carry `worlds`. `openagents chamber host` serves it when its
   configuration says `"transport": {"type": "reach"}`: it opens the Coder
   host's access store and key the way `openagents host serve` does
   (`--state`, `--keys`, or `--keychain`), names the instance as the
   channel's generation, and, when its keys include the owner key, adds the
   instance to the host's existing directory entry. `openagents chamber
   --reach HOST` and the `verse_remote` example join as this device with the
   grant from the computers store. TLS stays the default, `verse_host`
   serves TLS only, and NIP-MV `33300` world events remain.
3. **Shared Everglade.** A social rules profile in `verse-world`; the shared
   content digest; authority-owned seat actors fed by the studio snapshot;
   panel access checked against studio rights.
4. **Mirrors.** WORK, COORD, CTX, KB, RUN, and POL records beside host state;
   the Block-lane group mirror.
5. **Later.** Seat keys (AP, OA, GS), seats on the plaza, studio XP.

The chamber's own roadmap items (subscribed deltas instead of polled full
snapshots, client prediction, instance management) remain in
[the engine roadmap](engine/roadmap.md) and are prerequisites for
step 3 at more than a handful of players.

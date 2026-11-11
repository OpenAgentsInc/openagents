# Realm networking

These Coax contracts operate on caller-owned facts. A realm keeps its world,
rules, storage and codec. The host supplies sockets, monotonic time and storage
effects. An interface does not create a second model world for the consumer.

## Delivered consumers

| Decision | Shared interface | Everglade | Orbit League v2 |
| --- | --- | --- | --- |
| Timing and trust | `profile.coax`, `realm.coax` | 30 Hz, one capsule step, authority | 60 Hz, two vehicle substeps, authority |
| Initialization and admission | `net_world_steps`, `net_session_admit` | Constructor completes before stepping or pin admission | Constructor completes before hello admission or vehicle stepping |
| Session lifetime | `net_session_current`, `net_session_holds`; existing `aethyr/session/lifecycle.coax` | Positive generation fence, bounded reconnect hold, young command resend | Positive connection generation; disconnect ends the seat; no durable resume |
| Commands and accepted results | `command.coax`; existing `life_room_takes` | Earned 30 Hz input credit, bounded catch-up and queue excess, ACK after stepping | Latest valid controls per beat; ACK after consuming controls; no extra steps from input volume |
| Replication and body lifetime | `replication.coax` | Per-recipient receipt ring, baselines, replacement/removal and complete-tick ACK | Complete absolute snapshots, complete-tick ACK and absence removal; no unused delta/removal queue |
| Recipient fields | `realm.coax`, `row.coax` | Owner/public body fields; distant party HP/MAX only; blocks deny fields | Public pose, paint and life; fuel only for the owner; no party fields |
| Persistence | `net_session_saved`; existing checkpoint flow | Schema 2 requires an accepted capture within the existing bound; old schemas keep their existing policy | Explicit `NET_DURABLE_NONE`; no implied checkpoint or resume promise |
| Relevance at scale | `grid.coax` (#13178) | The room's interest grid: uniform cells of half the relevance radius, queries by neighbourhood | — |
| Priority, budget and backpressure | `budget.coax` (#13178) | Per-seat link (15 KB/s of rows, halved and spaced while its socket queues or HEARD lags), priority by distance, view, combat, speed and age, starved rows first | — |
| Reliable spawn and despawn | `spawn.coax` (#13178) | Spawns and x's lead each frame (the reliable lane); on datagrams an unheard spawn's updates wait | — |
| Diagnostics | `diagnostics.coax` | Readiness, selected profile, world tick, live/held seats; unavailable counters are −1 | Readiness, selected profile, world tick, admitted humans and bots, consumed controls, refusals and snapshots |

`coax/web/room/room.coax` selects profiles and exports
`rm_net_observation(server, realm, room, metric)`. Realm 1 is Everglade;
realm 2 is authoritative Orbit. The room index belongs to the selected realm.
An unavailable room, metric or counter returns −1. Metrics 0–14 are readiness,
Hz, substeps, trust, collision, durability, fairness, tick, seats, accepted
controls, refusals, snapshot count, public mask, owner mask and party mask.
Everglade's seat count includes its reconnect holds. Orbit counts admitted
humans and the quick match's bot seats.
The native host reads this interface for its local per-room diagnostic log.
The published signed room-stats body keeps its existing format.

## The transport

`transport.coax` (#13176) is the one interface between a page and its room,
with two implementations: the WebSocket, and WebTransport (the room's
`coax/web/room/wt.c` and `room-wt.c`, the page host's `coax/web/room-wt.js`).
A realm sends each frame one of five ways:

| Channel | Delivery | Everglade's frames |
| --- | --- | --- |
| `NET_DATAGRAM` | Unreliable, unordered, at most 1,200 bytes, each one decodes alone | A seat's state each tick (`D`, part 0 only), inputs (`I`, `J1`), anchors (`g2`, `h2`, `m2`) |
| `NET_COMBAT` | Reliable, ordered | Casts and their outcomes (`C`, `F`, `E`, `a`, `d`, `g3`, `h3`, `r`), and out-of-band `D` frames (flag 4) |
| `NET_LOOT` | Reliable, ordered | Inventory and loot (`b`, `z`, `u1`) |
| `NET_CHAT` | Reliable, ordered | Chat |
| `NET_SESSION` | Reliable, ordered | Everything else: joins, seats, keyframes too large for a datagram, `nq` |

On a WebSocket every channel is the socket's one ordered stream, and every
byte is the same as before the interface. On WebTransport each reliable
channel is its own stream, so a resend in one never holds up another.

- **Open.** `net_conn_url` prefixes `wt+`. The host tries WebTransport for
  2.5 seconds, then opens the WebSocket; its opened event says `wt` when
  WebTransport won. `?transport=wt|ws` on the page picks one;
  `E2N_WT_DEFAULT` is the default.
- **Inputs.** `net_conn_input` sends each input in a datagram that repeats
  every input the room hasn't acknowledged, oldest first (32 kept, up to
  1,200 bytes). The room's `net_gate_fresh` lets each through once.
- **State.** The room writes one datagram of state per tick per seat, by
  priority, and leaves the rest for the next tick. What doesn't fit, and
  every cue and outcome, goes out of band on the combat stream. A tick's
  state replaces the last tick's if it still waits for the congestion window
  (dropped stale, at the sender).
- **The overlay.** `?net` shows `LINK WT LOSS 1.2%` (state ticks lost on the
  way) and `KEY 3 STALE 7 WAIT 0` (snapshots, and the room's word each
  second: datagrams dropped stale and waiting now).

## Adoption points

- `coax/web/room/room-everglade.coax` calls the shared initialized-step,
  admission, reconnect-hold, earned-credit and accepted-result decisions.
  `room-relay.coax` fences active and closing sockets by the shared generation
  decision. `room-checkpoint.coax` selects receipt freshness explicitly.
- `apps/openagents-web/everglade-wire.coax` converts its packed slot storage
  to `NetReplica` for receipt, removal, replacement and baseline decisions.
  It copies only occupied slots. Its former local copies of those decisions
  are removed. `net_row_delta` supplies the explicit field mask.
- `room-blocks.coax` passes selected fields through `wire_seat_frame_fields`.
- `room-everglade.coax` steps each seat's `NetLink` and passes its limit, the
  seat's camera and each slot's combat to `wire_seat_frame_look`; a packed page
  hears its link once a second (`k`) for the `?net` overlay's BUDGET and DROP.
  `coax/web/tool.sh coax/web/room/repl-bench.coax` measures the layer.
  Party health does not create a visible body in `everglade-net.coax`.
  Widening permissions invalidates a baseline and sends an absolute row.
  Narrowing permissions cannot expose a previously private field.
- `room-orbit-v2.coax` calls the same session, accepted-result, receipt and
  field contracts. It runs `orbitleague.coax`'s vehicle/ball physics with an
  explicit active-car mask. Unadmitted cars do not collide, take fuel pads
  or affect play. No second vehicle or collision kernel was introduced.
- `orbit-session.coax` defines the realm's bounded commands and admission;
  `orbit-snapshot.coax` defines its atomic complete snapshot. Both use
  `record.coax`, the bounded whole-number record codec. The page takes
  complete snapshots through `net_snapshot_visible` and never simulates its
  own online car as authority.

Existing product adoptions remain in place: compatibility pins, owner fencing,
travel cutover and receipts, party visibility, lifecycle and latency. Their
bounded model checks do not imply that every modeled workflow runs in a
product. In particular, this extraction does not add an item ledger to rooms
that have no item inventory.

## Profile identity and qualification

The published unversioned Orbit endpoint remains a reported-car/server-ball
hybrid with `NET_TRUST_PRESENCE` and `NET_COLLIDE_REPORTED`. It cannot qualify
as an authoritative realm. Presence and lockstep remain named trust profiles.
Orbit's additive authoritative endpoint and wire are specified in
[`orbit-session-v2.md`](../../docs/protocol/orbit-session-v2.md). Everglade's
existing versioned wire keeps its identity.

Run consumer checks on the changed room and page units. Mutation-check each
changed authority with zero survivors. Then build the actual native host,
worker and page modules against their unchanged byte budgets. Use
`coax/web/tool.sh everglade-drift` for Everglade and
`coax/web/tool.sh orbit-qualification URL OUT_DIR` for two real Orbit pages
against a local host. Browser checks are headless and muted. Keep red checks,
passing logs, artifact hashes and source revisions with the verification
record. Shared contract tests alone do not qualify a consumer.

The book remains paused under #12610. This inventory records delivered calls
without editing its chapters or promoting their teaching status.

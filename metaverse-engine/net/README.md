# Realm networking

These Coax contracts operate on caller-owned facts. A realm keeps its world,
rules, storage and codec. The host supplies sockets, monotonic time and storage
effects. An interface does not create a second model world for the consumer.

## Delivered consumers

| Decision | Shared interface | Everglade | Orbit League v2 |
| --- | --- | --- | --- |
| Timing and trust | `profile.coax`, `realm.coax` | 30 Hz, one capsule step, authority | 60 Hz, two vehicle substeps, authority |
| Initialization and admission | `net_world_steps`, `net_session_admit` | Constructor completes before stepping or pin admission | Constructor completes before hello admission or vehicle stepping |
| Session lifetime | `net_session_current`, `net_session_holds`; existing `metaverse-engine/session/lifecycle.coax` | Positive generation fence, bounded reconnect hold, young command resend | Positive connection generation; disconnect ends the seat; no durable resume |
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
| `NET_DATAGRAM` | Unreliable, unordered, at most 1,200 bytes, each one decodes alone | A seat's state each tick (`D`, part 0 only), inputs (`I`, `J1`), a familiar's anchor (`m2`) |
| `NET_COMBAT` | Reliable, ordered | Casts and their outcomes (`C`, `F`, `E`, `a`, `d`, `r`), and out-of-band `D` frames (flag 4) |
| `NET_LOOT` | Reliable, ordered | Inventory and loot (`b`, `z`, `u1`) |
| `NET_CHAT` | Reliable, ordered | Chat |
| `NET_SESSION` | Reliable, ordered | Everything else: joins, seats, keyframes too large for a datagram, `nq` |

On a WebSocket every channel is the socket's one ordered stream, and every
byte is the same as before the interface. On WebTransport each reliable
channel is its own stream, so a resend in one never holds up another.

- **Open.** `net_conn_url` prefixes `wt+`. The host tries WebTransport for
  2.5 seconds, then opens the WebSocket; its opened event says `wt` when
  WebTransport won. `?transport=wt|ws` on the page picks one; with
  neither, `E2N_WT_DEFAULT` (on) applies on a shard's own endpoint. The load
  balancer's path is TCP only, so it stays on the WebSocket.
- **Inputs.** `net_conn_input` sends each input in a datagram that repeats
  the inputs the room hasn't acknowledged, oldest first: the newest
  `NET_OWED` (6) at most, within 1,200 bytes. An owed input that is the next
  one's but for its number and its last word goes as one byte, `~`, made
  whole by `net_bundle_expand`. The room's `net_gate_fresh` lets each
  through once.
- **Casts and launches.** A `C` or `F` frame goes on the combat stream, and
  also rides the input datagrams under the next input's number
  (`net_conn_owe`) until the room acknowledges it. The room takes each once
  by its sequence number (`net_seen_fresh`, `room-wt.coax` `rm_wt_once`),
  whichever way comes first, so no event depends on which channel brought
  it.
- **Arrows loosed and ended.** The room sends `a` and `d` on the combat
  stream and as a datagram now and again next tick (`rm_wt_hasten`). The
  page takes each once by the arrow's id.
- **State.** The room writes one datagram of state per tick per seat, by
  priority, and leaves the rest for the next tick. What doesn't fit, and
  every cue and outcome, goes out of band on the combat stream. A tick's
  state replaces the last tick's if it still waits for the congestion window
  (dropped stale, at the sender).
- **Profiles.** [`aethyr-transport-v1.json`](../../proto/verse/aethyr-transport-v1.json)
  pins the mark byte, each channel's number and an input datagram with a
  repeat byte ([doc](../../docs/protocol/aethyr-transport-v1.md));
  [`everglade2-combat-v2.json`](../../proto/verse/everglade2-combat-v2.json)
  pins `F`, `a` and `d` ([doc](../../docs/protocol/everglade-projectiles-v1.md))
  beside what Everglade II's guard and monsters did, the `e guard` and
  `e mon` entries of `D`
  ([doc](../../docs/protocol/everglade-guard-monsters-v2.md)); where they
  stand is their rows' (session v6).
  [`everglade2-familiar-v2.json`](../../proto/verse/everglade2-familiar-v2.json)
  pins a familiar's requests and the Healer's Kit's as `E` kinds and the
  room's answers as the `e fam`, `e fam-cast` and `e kit` entries of `D`
  ([doc](../../docs/protocol/everglade-familiar-v2.md)); a familiar itself
  is a row (session v7). Their checks,
  `coax/web/room/transport-profile.coax` and `combat-profile.coax`, run the
  codecs. [`room-relay-v1.json`](../../proto/verse/room-relay-v1.json) pins
  the relay room's own frames (`y j l m` and the clock's ping and pong,
  [doc](../../docs/protocol/room-relay-v1.md)); `relay-wire.coax` is their
  one codec, for the room and for every page in a relay room, and
  `coax/web/room/relay-profile.coax` runs it.
- **The overlay.** `?net` shows `LINK WT LOSS 1.2%` (state ticks lost on the
  way) and `KEY 3 STALE 7 WAIT 0` (snapshots, and the room's word each
  second: datagrams dropped stale and waiting now).

## Prediction, reconciliation and interpolation

A page whose room is the authority keeps this bookkeeping in two engine
modules (#13277). The realm supplies its step and its row's fields.

| Module | What | Everglade |
| --- | --- | --- |
| `predict.coax` (`NetPredict`) | Your inputs by number, the newest one the room stepped, and whether a re-run is due. A realm packs an input into whole numbers (`net_predict_input`), takes the room's ACK (`net_predict_heard`) and steps the inputs still held (`net_predict_pending`, `net_predict_take`, `net_predict_done`). A command numbered with the inputs takes `net_predict_number` and is never re-run | Both pages: `everglade/client/net.coax` and v1's `apps/openagents-web/everglade-net.coax`. `net_move_words` packs an `EgMoveIn`; `NetFix` is the row's word on you |
| `predict.coax` (`NetLedger`) | Your actions that wait for the room: pressed, then confirmed or refused by the room's word, or timed out. A new socket times out what waited | Everglade II's casts and rests (`E2N_ACT_CAST`, `E2N_ACT_REST`, 5 s) |
| `interp.coax` | The interpolation buffer (`NetState`, `net_ring_push`, `net_ring_at`, `NetLead`), the jitter buffer and tick map (`net_jitter_heard`, `net_jitter_delay`, `net_map_follow`, `net_lag_step`), the ping pick, and the easing of your own corrections (`NetCorrect`) | `everglade/client/net-core.coax` keeps `NetEnt` (an entity as Everglade's rows describe it) and calls these by `net_push`, `net_state_at` and `net_ease` |

Everglade II's guard and monsters are rows (#13276): each is a `NetEnt` of
the page's one list, drawn from the engine's ring, with its state and its
act riding the ring so each shows at the moment the room names.

Everglade II's familiars are rows too (#13276, #13277): each is a `NetEnt`
of that list, drawn from the engine's ring, so a step is blended at its
tick and a recall (a new life of its row) is never drawn as a walk. Whose
senses its owner has rides the ring's flags.

Orbit League's v2 page is the second realm on both modules (#13277,
`apps/openagents-web/orbitleague.coax`, "ol-predict"):

| What | How Orbit League does it |
| --- | --- |
| Its step | The game's one step, `physics_step_masked`, for your car alone: the room's two substeps a control |
| An input's words | `THROTTLE STEER FLAGS`, the whole numbers the wire carries, read back by `orb_drive` on both sides |
| The room's word on you | The snapshot's row for your seat and its `ACK` (`word[YOU]`, `word_fuel`) |
| What a re-run restores that no snapshot carries | Your car's wheel and jump state, as your own steps left it after the acknowledged control (`pred_car`) |
| When a re-run runs | Only when the snapshot is more than 2 cm, 0.1 m/s or 1 fuel from where your steps had you after that control |
| The others | The ball and the other cars are `NetState` rings read with `net_ring_at`, the jitter buffer's delay behind; a body's turn rides the realm's three scalars and is blended with `net_ring_span` |

The room steps a seat's controls one a tick in the order sent
(`orbit/server/room.coax`, `ORB_QUEUE`), so the page and the room step the
same ones. Your car collides with the ball where it is drawn; car against
car is the room's alone, and shows as a correction.

Not proved from a headless run: how it feels. Qualify it with two real
pages (`coax/web/tool.sh orbit-qualification URL OUT_DIR`) and by hand.

### The drift check

`coax/web/tool.sh everglade2-drift --page ORIGIN --room PORT` plays the page
against a local room through a shaped network
(`coax/web/everglade2-drift.coax`). Three headless Chromes join one room. A
walks for ten seconds, turning, with two jumps. B watches A through the
shaped network. C watches A over a clean link, so its states are the room's
rows for A, tick by tick. For each cell (no shaping, then 80 and 250 ms
round trips with 1% and 5% of reads lost) the check pins:

- **Your own corrections.** The room's re-runs that moved A more than 5 cm:
  at most one a second, and none over 0.5 m.
- **The others as drawn.** How far B draws A from where the room's rows put
  A at the moment B draws: the 95th percentile and the most, against the
  cell's bounds (`dd_bound`).

Start the room and the page first with
`coax/web/tool.sh coax/web/room/local.coax --static`, which prints `PAGE=`
and `ROOM=`. Let it build the room: a remote room check leaves the host's
Linux binary in `coax/web/room/bin/`, so don't pass `--no-build` after one. The check needs a GPU, so run it on a Mac, not on a build host.
Its numbers on a loaded machine are not the page's: check `uptime` first.
`--report` prints the numbers and judges nothing, for setting the bounds
after a change that moves them on purpose.

Measured on 2026-11-12, three runs, the worst of each (mm):

| Round trip | Reads lost | Your corrections: over 5 cm, the most | B draws A off by: median | 95th percentile | the most | Bound: 95th, the most |
| --- | --- | --- | --- | --- | --- | --- |
| none | none | 0, 18 | 1 | 3 | 4 | 20, 60 |
| 80 ms | 1% | 2, 182 | 3 | 125 | 426 | 250, 900 |
| 80 ms | 5% | 1, 168 | 138 | 513 | 1,315 | 900, 2,400 |
| 250 ms | 1% | 0, 50 | 94 | 881 | 2,321 | 1,500, 3,800 |
| 250 ms | 5% | 0, 23 | 235 | 1,310 | 2,758 | 2,200, 4,500 |

The link is a WebSocket, so a lost read holds every read behind it for a
resend, and the others are drawn up to metres off for a moment. That is
what the page does today, pinned so it can't get worse unseen. The check
does not cover WebTransport, where a lost datagram holds nothing:
`coax/web/room/net-matrix.coax` times that path.

## Adoption points

- `coax/web/room/room-everglade.coax` calls the shared initialized-step,
  admission, reconnect-hold, earned-credit and accepted-result decisions.
  `room-relay.coax` fences active and closing sockets by the shared generation
  decision. `room-checkpoint.coax` selects receipt freshness explicitly.
- `everglade/shared/everglade-wire.coax` converts its packed slot storage
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
- Orbit League's room module (`orbit/server/room.coax`, behind the room's one
  game registration, `room-games.coax`) calls the same session, accepted-result,
  receipt and field contracts. It runs `orbitleague-game.coax`'s vehicle/ball physics with an
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

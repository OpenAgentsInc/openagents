# Appendix A. Everglade rooms against an MMO engine

This appendix compares Everglade's multiplayer implementation on main with
the capabilities in [docs/research/mmo.md](../../docs/research/mmo.md): its
section 17 capability table and development order, and the sections on
login, queues, transport, interest, replication, prediction, crowds,
partitioning, persistence, shared services, deploys, observability, testing
and security.

Our scale decides what matters. Everglade runs rooms of 16 seats, 16 rooms to
a process, on 6 shards that place 2 rooms each (192 players), and a world
event of up to 60 raiders in one square. A gap that matters only for
thousands of players in one fight, or for one world across many machines, is
left out below with the reason.

Evidence is from origin/main at `9d2e7598ef` (2026-11-04) unless a commit is
named. Paths are relative to the repository root.

## The capability table

| Capability (section 17) | Where we are | Matters at our scale? | Issue |
| --- | --- | --- | --- |
| Headless runtime | oa-room steps each room's Everglade world with nothing drawn (`coax/web/room/room-host.c` `everglade_tick`; `room-world.c`; `build.sh` links the page's units). | Yes: it is the authority. Done for rooms; offline play doesn't use it yet. | #12784 |
| Stable identity model | Player id in the signed ticket (`room-ticket.coax` `Ticket`); seat `SS`, townsperson `100000 + I`, made things `200000 + EID` with ids that never repeat in a room's life (`everglade5e-room-rules.coax` `next_id`). Seat slots are reused with no generation. | Somewhat: on one TCP socket a late record can't overtake its `x`, but a reused seat can draw the old holder's last pose (chapter 91). | #12783 (comment) |
| Explicit authority | The server owns HP, rolls, casts, deaths, the town and WANTED (`everglade-wire.coax` header). The page saves WANTED itself (`coax/web/api/player.c:364-394`); talk, doors, boats and boards are refused as `unbuilt` (`room-everglade.coax` `eg_heard`). | Yes: a page can clear its own WANTED level. | #12794, #12784 |
| Command pipeline | Numbered intents; old and repeated inputs dropped; a 16-input queue; steps paced by `EG_BANK` and `EG_TICK_STEPS`; refusals with reasons (`room-everglade.coax` `eg_heard_input`, `rm_eg_input`). A cast waits at most 0.5 s for the inputs sent before it, with no client clock (`room-casts.coax`, 67b59c14e2). | Yes. Done for casts; interactions are still refused as `unbuilt`. | #12790 (closed), #12784 |
| Scheduling and dependencies | One thread a process; the tick's time is measured by part (`room-host.c` `part_name`). | Not now: 16 full rooms used 10.9 s of tick work a minute on one core (launch runbook). | — |
| Replication descriptors | Per-kind entries and send rules in `eg_send` (`room-everglade.coax`); townsfolk only after 0.3 m with no velocity (`EG_FOLK_STEP`). | Yes: it caused townsfolk walking in place. | #12783 |
| Client timing | Clock offset from the best of 8 pings; adaptive jitter buffer, 80 to 300 ms; prediction and replay (`everglade-net.coax`). | Yes. Done except for townsfolk's input to it. | #12783 |
| World-transfer contract | None. The Grid gate is a page navigation with `?with=` (#12716). | No: rooms are instances; no world spans processes. | — |
| Persistence boundary | HP, XP and place posted off the tick through a forked poster (`room-host.c` `vitals_post`). An entry is marked sent before the API answers, and a dropped post is only logged (`room-vitals.coax:136-141`, `room-host.c:424-430`). | Yes: XP decides levels 1 to 3. | #12793 |
| Content and protocol versioning | `WIRE_VERSION :: 2` is never checked (`everglade-wire.coax`); edits carry a world version (`e edit VERSION`). | Yes: the runbook deploys oa-room before the page, so cached pages meet a new server. | #12788 |
| Diagnostics | `/health`: tick p50 and p95, fan-out, rate-limited frames, slow sockets; a minute log with time by part and events dropped. | Yes, for the raid. | #12791 |
| Realistic test harness | The drift check under shaped networks (`coax/web/everglade-drift.coax`); fighting raid bots (`raid-check.coax`); a shaping proxy (`net-proxy.coax`). No failure injection. | Yes. | #12778, #12711 |

## The development order

| Step (section 17) | Where we are | Issue |
| --- | --- | --- |
| 1. One authoritative scene with real movement and combat | On main; the drift check still finds drift. | #12778, #12783, #12784 |
| 2. Interest management and replication budgets | One radius, no budget. | #12783, #12789 |
| 3. Realistic same-location capacity | 16 full rooms in one process measured; a raid with fighting bots runs locally. Bytes per seat in a raid are not measured. | #12711, #12791 |
| 4. Durable progression and crash-safe economics | HP and XP saved, lossy; no economy. | #12793, #12794 |
| 5. Instances, admission, reconnect, draining | Instances and placement done (#12717); admission, reconnect and draining open. | #12796, #12792, #12797 |
| 6. Recoverable world transfers | Not needed (see below). | — |
| 7. Social systems and group-aware placement | Chat runs on the API, outside the tick; block, report and kick (#12714); `?with=` is best effort. | #12714, #12795 |
| 8. Fleet expansion and compatible deploys | 6 shards; no version check, no drain. | #12788, #12797 |
| 9. Dynamic partitioning where demand requires it | Not needed (see below). | — |

## Other sections

| Area | Where we are | Matters at our scale? | Issue |
| --- | --- | --- | --- |
| Login (4) | Ticket from the API, checked at join for room, MAC, expiry, shard and kicks (`room-ticket.coax` `tk_check`, `room-relay.coax` `rl_join`). No compatibility check. | Yes, for the version check. | #12788 |
| Reconnect (4) | The page rejoins after 6 s of silence (a388b270f9); a new socket replaces a silent one (86808a350c); a return within 2 min stands where it was (d1fbd24f6f). The character leaves when its socket closes; frames from a replaced socket still reach the old seat (`rl_heard` has no kicked check). | Yes: phones change networks mid-raid. | #12792 |
| Queues and admission (4, 14) | No queue; a full answer is 503 and the page retries on a doubling wait with no jitter (`everglade-net.coax` `net_down`). No rate limit on ticket asks. | Yes, at launch: every page past 192 retries in step. | #12796 |
| Transport (9) | WebSocket over TCP with `TCP_NODELAY` (`room-host.c`). | Head-of-line blocking is real on lossy phones: `net-proxy.coax` models a lost read as a 200 ms stall. Left out as a transport change; see below. | — |
| Send queues (8) | Bounded by bytes (`OUT_MAX` 256 KB, then closed), not by age. | Yes: a slow phone collects stale positions. | #12783 |
| Interest (7) | Every player sent to every seat; one 120 m radius for the rest, scanned per seat; no hysteresis; blocks hidden by the page after it receives them. | Yes: privacy, and edge churn in a raid. | #12789 |
| Replication (8) | Per-kind rules; full snapshots on join; no acknowledged baselines; teleports not marked. | Yes. | #12783 |
| Prediction and latency (10) | Prediction, replay and eased corrections done (chapter 90); the camera follows the controls (#12777). Cast rules written and built: range and sight at start and release, heals before harm, a refusal as a reason icon ([spec](../../docs/verse/2026-11-04-everglade-cast-rules.md)). | Yes. Done. | #12790 (closed) |
| Crowds (11) | Up to 60 raiders; per-tick caps on events (96), raider casts (32) and downs (32), each counted when it drops work. No target caps. Estimated 1.1 Mbit/s per seat at full visibility (chapter 92). | Yes: the raid is the crowd. | #12783, #12791 |
| Partitioning and transfer (12) | One process owns each room. The directory keeps the last report for a room, with no generation (`migrations/0115_room_directory.sql`). | Only during a deploy's overlap. | #12797 |
| Persistence consistency (13) | Displayed positions: in memory, lossy, fine. Encounter state: lost on restart. HP and XP: newer-wins and larger-XP merges, safe to repeat, but posts can be lost. WANTED: written by the page. | Yes, for the lost and client-written rows. | #12793, #12794, #12797 |
| Economy (13) | None: no currency, inventory, trade or loot. | Not now (see below). | — |
| Shared services (14) | Chat over the API's HTTP endpoints; control settings polled every 2 s; saves through a forked poster. None of them run in the tick. | Done. | — |
| Party placement (16) | `?with=PID` joins a friend's room only while it has a seat; else the lowest room with one, silently (`room-relay.coax` `rl_with`). | Yes: friends split when a room fills. | #12795 |
| Deploys and versioning (14) | 6 shards replaced in parallel; no `SIGTERM` handling, no readiness probe; rooms' state lost (`deploy.sh`, `room-host.c`). | Yes: a deploy during a raid cuts 192 players. | #12797, #12788 |
| Observability (15) | See the capability table. Missing: bytes by type, queue age, entities per seat, join bursts, server input-to-ACK, reconnect success. | Yes. | #12791 |
| Testing (15) | Drift under shaped networks; fighting raid bots; load bots that walk and cast. | Yes. | #12778, #12711 |
| Security: message lengths and rates (15) | Page frames at most 1,024 bytes, 40 frames/s and 16 KB/s per socket, closed after 10 s over (`room-relay.coax` `rl_allow`); exact field counts and clamps on parse. | Done. | — |
| Security: allocations (15) | One allocation for all Coax state; per-socket output capped at 256 KB; each room's world allocated once. | Done. | — |
| Security: secret state (7, 15) | Blocked players and later invisible entities are sent and hidden by the page. | Yes. | #12789 |

## Left out at our scale

- **WebTransport datagrams instead of WebSocket over TCP.** TCP's
  head-of-line blocking stalls state behind a lost packet, but oa-room runs
  on Cloud Run, which takes no UDP, and the research doc (section 9) asks
  for browser support to be verified first. Newest-state replication and
  age-bounded queues (#12783) limit the cost of a stall instead.
- **A login queue with positions.** At 192 seats, a rate limit and a
  jittered retry that says how many are waiting cover the launch (#12796).
- **Idempotent operation IDs for an economy.** There is no economy. The first
  operation that moves something valuable between players gets an operation
  id, a version guard and a recorded result (section 13) in the same change.
- **World transfers and dynamic partitioning.** Rooms are bounded instances;
  no encounter crosses a process.
- **Parallel simulation.** One thread holds 16 full rooms with room to spare.
- **Time dilation under overload.** The tick has about five times headroom;
  #12791 watches it.
- **Regional failover.** One region, us-central1, serves every player; a regional outage
  is a restart, not a migration.

## Issues filed from this analysis

All are under #12718:

- #12792 Reconnect as its own protocol: seat generations, a grace hold,
  stale frames dropped.
- #12793 Room vitals posts count as saved only when the API accepts them.
- #12794 The server saves WANTED, not the page.
- #12795 An INVITE holds a seat in the inviter's room.
- #12796 Admission: rate-limit ticket asks and jitter the full-room retry.
- #12797 Drain a shard on deploy, and only send players to ready capacity.

Already tracked before this analysis: #12783 (replication), #12784 (offline
as a local server), #12788 (version handshake), #12789 (interest), #12790
(cast rules), #12791 (observability), #12778 (drift check) and #12711 (load
test).

## Since this analysis

The tables above are the snapshot at `9d2e7598ef`. On 2026-11-04 these
landed on main; the issue states are GitHub's at the time of writing.

| Issue | State | Landed |
| --- | --- | --- |
| #12783 replication | closed | Wire 3, one row for every entity, deltas against `HEARD`, priority within `WIRE_BUDGET` (edd9601102); `LIFE`, `WARP`, effect rows with `T0` and `DUR`, a socket behind (67b4f62448); old pages on the old wire until 2026-11-12 (0e96c34284). Chapters 91–94. |
| #12788 version handshake | closed | The hello carries the wire version; another is refused with `r 0 version` (67b4f62448). Wire 4 sends the heading in 0.1 mrad (3370a41ddc). Chapter 98. |
| #12789 interest | closed | `room-interest.coax`: 32 m cells, in at 110 m and out past 130 m, blocks and hidden entities withheld (b4826d83b2); every channel gated (22c492ffc5). Chapter 97. |
| #12791 observability | closed | Vitals every 10 s and `/admin/rooms` (b35943120f). Chapter 99. |
| #12792 reconnect | closed | Seat generations, a 15 s grace hold, old-socket frames dropped (257c85b06d). Chapter 98. |
| #12793 vitals receipts | closed | A vitals note is saved only when the API's receipt covers it (5016ba0be3). |
| #12794 WANTED | closed | The room reports WANTED; the page no longer writes it (5016ba0be3, 45bcccd674). |
| #12795 INVITE | closed | An INVITE holds a seat for two minutes (7c2405f986). |
| #12784 offline host | closed | The room's tick in a Web Worker behind `?offline=worker` (e8c68921ed, 2b86e8fafc). The default and deleting the page's copy are #12882. Chapter 95. |
| #12796 admission | open | Admission when every room is full (5fea647468). |
| #12797 draining | open | A deploy still cuts every socket. |
| #12778 drift check | open | Three causes removed (f54bb0f001); it doesn't hold at every profile yet. Chapter 100. |

A seat's steps are earned by the server's clock since it sat, so late ticks
don't leave its queue full (d14d704b62, chapter 89).

What remains for a shared multiplayer engine, beyond Everglade, is tracked
under the roadmap #12833: one owner per character (#12834), pinned releases
(#12839), the shared `aethyr/net` contracts (#12845), streamed cells
(#12847), checkpoints (#12848), handoff (#12850), adjacent owners (#12851),
NPC senses and navigation (#12854), model decisions (#12855), one host
lifecycle (#12856), accessibility (#12857) and a WebTransport experiment
(#12858).

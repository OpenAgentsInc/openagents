# 29. Many rooms

> **Status: partly implemented.** Multi-shard directory heartbeats and path
> routing live in [`coax/web/room/room-directory.coax`](../../coax/web/room/room-directory.coax),
> single-owner leases and store fencing live in
> [`metaverse-engine/session/control.coax`](../../metaverse-engine/session/control.coax),
> crash-safe five-step travel lives in
> [`metaverse-engine/session/travel.coax`](../../metaverse-engine/session/travel.coax)
> and [`coax/web/room/room-travel.coax`](../../coax/web/room/room-travel.coax),
> and multi-town capacity harnesses live in
> [`coax/web/room/room-fleet.coax`](../../coax/web/room/room-fleet.coax),
> exercised by [`everglade/examples/many-rooms-travel.coax`](../examples/many-rooms-travel.coax).
> Dynamic 150-player zone scaling (`0003-150-a-zone.md`) and walk-through
> cross-zone world portals remain in progress.

This chapter answers the player's question: "When a room fills up—or I move to
another room—why don't I show up in two places at once or lose my XP if a
server crashes mid-transfer?"

One room seats 16 players today (`EG_SEATS = 16`). Running an MMORPG means
spreading rooms across multiple shards (`oa-room` through `oa-room-6`), routing
each player to a shard with open capacity, and handing a live character from
one room owner to another without split-brain twins or lost progress.

## What this chapter teaches

- How shards advertise free capacity, active rooms, and draining status to the
  directory over signed heartbeats (`POST /api/playtest/room-directory`).
- Why four identities—character ID, monotonic owner generation, control
  generation, and socket attachment—stay separate so stale owners fail closed
  at the store.
- How five-step travel (`reserve`, `prepare`, `commit`, `activate`, `route`)
  survives a crash at any step without duplicating or dropping a character.
- How capacity refusal leaves the player safely playing in their source room.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Shard directory & routing | 5-second signed reports (`s`, `g`, `e`, `r`, `h`), `/api/room/sK/` shard unsharding, draining flag `d` | [`coax/web/room/room-directory.coax`](../../coax/web/room/room-directory.coax) |
| Owner lease & fencing | Monotonic `#counter` owner generations, directory-clock leases, `CONTROL_ELSEWHERE`, `CONTROL_STALE` | [`metaverse-engine/session/control.coax`](../../metaverse-engine/session/control.coax) |
| Five-step travel protocol | Durable travel record, 8-second cutover/reservation hold (`TRAVEL_HOLD_MS`), crash recovery (`travel_recover`) | [`metaverse-engine/session/travel.coax`](../../metaverse-engine/session/travel.coax) |
| Room travel integration | Handles `T SEQ CODE`, pauses casts, waits for durable vitals/checkpoint receipt, emits `g CODE`, carries auras | [`coax/web/room/room-travel.coax`](../../coax/web/room/room-travel.coax) |
| Fleet capacity harness | Runs up to 16 towns × 16 seats (256 concurrent fighting bots) with injected SIGSTOP stalls | [`coax/web/room/room-fleet.coax`](../../coax/web/room/room-fleet.coax) |

**What remains to ship:** Everglade II currently runs 16-seat rooms across 6
production shards and supports instance-to-instance travel (`T SEQ CODE`) with
full HP, XP, aura, and Concentration carry. Scaling individual zones from 16
to 150 concurrent players ([`0003-150-a-zone.md`](../../docs/decisions/0003-150-a-zone.md))
and triggering automatic seamless handoffs when walking through physical zone
portals (for example, from the forest road into a separate city zone server)
remain in progress.

## The method

### Directory heartbeats and monotonic fencing

Each shard reports its live rooms, player IDs, free simulation slots, and
draining state every `DIR_EVERY_MS` (5,000 ms), signed with `X-Room-Mac`. The
API places incoming players onto a shard with free capacity and mints a signed
seat ticket naming that shard and the character's new `owner` generation.

In `metaverse-engine/session/control.coax`, leases run solely on the
directory's clock (`CONTROL_LEASE = 300` ticks, 10 s at 30 Hz), never on room
clocks. A second room asking for a character with a live lease receives
`CONTROL_ELSEWHERE`. Once ownership transfers, `control_transfer` increments
the `#counter` generation atomically so every recipient (`ControlFence` on the
store and command gate) rejects writes from the old generation with
`CONTROL_STALE`.

### Five-step crash-safe travel

Moving character `c` from room `from` to room `to` crosses one atomic commit
point:

1. **Reserve (`travel_reserve`).** `to` verifies `up && ready && used + reserved
   < capacity` (`travel_admits`). A full room returns `TRAVEL_FULL` immediately
   while `from` stays authoritative.
2. **Prepare (`travel_prepare`).** `from` enters cutover (`cut_until = now +
   8000 ms`), refuses further commands for `c`, and writes `c`'s captured body
   (HP, XP, active effects, last accepted command) into the durable travel
   record.
3. **Commit (`travel_commit`).** One directory write transfers ownership to
   `to` at `gen + 1`. Before commit, a crash aborts cleanly; after commit,
   travel only moves forward.
4. **Activate (`travel_activate`).** `to` loads the captured body under `gen + 1`
   and fences the store so `from` can never write again.
5. **Route (`travel_route`).** The client switches to `to` (`g CODE`), and
   `from` drops its local copy.

## Worked trace: move, crash before/after commit, and full destination

```text
1. Join Room 0 (id=10)      -> gen=1; cmd seq=1 (+50 XP) -> xp=50
   Room 1 (id=20) acquire   -> CONTROL_ELSEWHERE (live lease held by Room 0)
2. Travel 0 -> 1            -> reserve OK -> prepare OK (cutover blocks seq=2)
                            -> commit OK (gen=2) -> activate OK -> route=1
   Old owner Room 0 fence   -> CONTROL_STALE (gen 1 < 2)
   Room 1 cmd seq=2 (+75)   -> xp=125
3. Crash pre-commit (1->0)  -> prepare -> crash(1) -> travel_recover(1)
                            -> TRAVEL_ABORTED in 1 step; Room 1 keeps owner
4. Crash post-commit (1->0) -> reserve -> prepare -> commit (gen=3) -> crash(0)
                            -> travel_recover(0) -> TRAVEL_DONE in 1 step; route=0
5. Destination full (0->1)  -> Room 1 used==capacity -> reserve returns TRAVEL_FULL
                            -> Room 0 cmd seq=3 (+25) succeeds -> xp=150
```

## Runnable example

[`everglade/examples/many-rooms-travel.coax`](../examples/many-rooms-travel.coax)
runs the full lease, five-step travel, pre-commit abort, post-commit roll
forward, and full-room refusal sequence directly against
[`metaverse-engine/session/travel.coax`](../../metaverse-engine/session/travel.coax)
(committed transcript
[`everglade/examples/many-rooms-travel.out`](../examples/many-rooms-travel.out)):

<!-- source: everglade/examples/many-rooms-travel.coax many-rooms-travel -->
```jai
travel_run :: () -> bool {
  w: [1] TravelWorld;
  travel_world(w[..]);
  w[0].now = 1000;

  // 1. Character 1 joins Room 0 (id 10) at gen=1; Room 1 (id 20) cannot hijack the live lease.
  joined := travel_join(w[..], 0, 1, TravelBody.{hp = 18, xp = 0});
  cmd1 := travel_command(w[..], 0, 1, 1, 50);
  hijack := control_acquire(w[0].dir[..], CharacterId.{value = 1}, w[0].rooms[1].id);
  print("joined=% gen=% xp=% refuse_elsewhere=%\n",
    joined, w[0].rooms[0].bodies[1].gen, w[0].rooms[0].bodies[1].xp, hijack.status == CONTROL_ELSEWHERE);

  // 2. Five-step travel Room 0 -> Room 1: cutover drops commands, commit advances gen to 2,
  //    old owner is fenced at the store, destination activates and continues from seq=2.
  s1 := travel_reserve(w[..], 1, 0, 1);
  s2 := travel_prepare(w[..]);
  cut_refused := !travel_command(w[..], 0, 1, 2, 10);
  s3 := travel_commit(w[..]);
  s4 := travel_activate(w[..]);
  s5 := travel_route(w[..]);
  old_fenced := control_fence(w[0].store[..], CharacterId.{value = 1}, 1) == CONTROL_STALE;
  cmd2 := travel_command(w[..], 1, 1, 2, 75);
  print("steps_ok=% cut_refused=% new_gen=% old_fenced=% route=% xp=%\n",
    s1 == TRAVEL_OK && s2 == TRAVEL_OK && s3 == TRAVEL_OK && s4 == TRAVEL_OK && s5 == TRAVEL_OK,
    cut_refused, w[0].rooms[1].bodies[1].gen, old_fenced, w[0].route[1], w[0].rooms[1].bodies[1].xp);

  // 3. Crash before commit aborts cleanly back to source; crash after commit rolls forward.
  _ := travel_reserve(w[..], 1, 1, 0);
  _ = travel_prepare(w[..]);
  travel_crash(w[..], 1);
  pre_steps := travel_recover(w[..], 1);
  pre_aborted := w[0].rec[0].status == TRAVEL_ABORTED && w[0].route[1] == 1;

  _ = travel_reserve(w[..], 1, 1, 0);
  _ = travel_prepare(w[..]);
  _ = travel_commit(w[..]);
  travel_crash(w[..], 0);
  post_steps := travel_recover(w[..], 0);
  post_done := w[0].rec[0].status == TRAVEL_DONE && w[0].route[1] == 0 && w[0].rooms[0].bodies[1].gen == 3;
  print("pre_crash_aborted=% steps=% post_crash_forward=% steps=% final_gen=%\n",
    pre_aborted, pre_steps, post_done, post_steps, w[0].rooms[0].bodies[1].gen);

  // 4. Capacity refusal: a full destination refuses reserve while the player keeps playing.
  w[0].rooms[1].used = w[0].rooms[1].capacity;
  full_res := travel_reserve(w[..], 1, 0, 1);
  still_plays := travel_command(w[..], 0, 1, 3, 25);
  print("refuse_full=% still_plays=% final_xp=%\n",
    full_res == TRAVEL_FULL, still_plays, w[0].rooms[0].bodies[1].xp);

  return joined && cmd1 && hijack.status == CONTROL_ELSEWHERE &&
         s1 == TRAVEL_OK && s2 == TRAVEL_OK && s3 == TRAVEL_OK && s4 == TRAVEL_OK && s5 == TRAVEL_OK &&
         cut_refused && old_fenced && cmd2 && pre_aborted && post_done &&
         full_res == TRAVEL_FULL && still_plays && w[0].rooms[0].bodies[1].xp == 150;
}
```

Output (`coax/bin/coax run everglade/examples/many-rooms-travel.coax`, identical in `--release`):

```text
joined=true gen=1 xp=50 refuse_elsewhere=true
steps_ok=true cut_refused=true new_gen=2 old_fenced=true route=1 xp=125
pre_crash_aborted=true steps=1 post_crash_forward=true steps=1 final_gen=3
refuse_full=true still_plays=true final_xp=150
ok=true
```

## The checks

```shell
coax/bin/coax test --library metaverse-engine/session/control.coax
coax/bin/coax test --library metaverse-engine/session/travel.coax
coax/bin/coax test --library coax/web/room/room-fleet.coax
coax/bin/coax run everglade/examples/many-rooms-travel.coax
```

## Mistakes

- **Routing before the store receipt arrived.** Sending `g CODE` immediately
  after posting vitals meant a room crash between post and commit lost the last
  seconds of HP and XP on cross-shard travel. `room-travel.coax` now gates the
  `g CODE` route on `travel_saved(posts, rev, acked)` so the destination ticket
  always sees the durable receipt.
- **Frozen cutover on client disconnect.** If a player's tab closed right after
  `prepare`, holding the cutover forever locked their character in limbo.
  `travel_cut` bounds every cutover to `TRAVEL_HOLD_MS` (8,000 ms), after which
  any pre-commit journey aborts automatically and commands resume.

## What comes next

Rooms hold live state while you play, but your account and hero have to follow
you across devices and days. [Chapter 30](ch030-accounts-and-persistence.md)
covers Nostr identities, device-bound sessions, durable character records, and
fenced checkpoints.

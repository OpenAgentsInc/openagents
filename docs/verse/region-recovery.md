# Region recovery for Metaverse rooms

Status: design, model qualification and a local rehearsal (#12853). The room
fleet runs in one region (`us-central1`) today, and nothing here changes that.
This page states what a second region would have to guarantee before it is
added, the model that checks the ownership rules, and what a failover measured
on one machine.

## Targets

| Measure | Target | Why |
| --- | --- | --- |
| Players served | Under 150 ms round trip to the room for most players | The cast rules hold at 250 ms (`room-proof.coax cast`), so one region serves North America. A second region is for players farther away or for disaster recovery, not for speed at home. |
| Recovery point (loss window) | At most the replication lag, 5 s | A failover may lose the writes the replica hadn't received: XP, HP, items and seat holds of the last seconds. |
| Recovery time | Under 10 minutes to the first seat in the new region | The arbiter waits out the old primary's lease (5 s), then promotes; the page reconnects and asks for a new ticket. Most of the time is starting rooms and moving traffic. |
| Availability | One region failing at a time | Two regions failing at once is out of scope. |

## What a failover keeps and what it loses

- One region is the primary at a time. Its store takes every write, and a
  replica in another region applies its log in order.
- One arbiter, outside the room regions, issues region epochs and promotes.
  It promotes only after the primary's lease has lapsed on the arbiter's
  clock, or when an operator forces it.
- It promotes only a replica whose build can read the store: the current
  schema or the one before.
- The new primary takes the next epoch. Every write names its epoch, and a
  store refuses a write with an older one. A primary that was cut off and
  comes back is refused by the new primary without asking anyone.
- Writes the old primary took after the replica's last entry are the loss
  window. They are dropped, never merged. The old primary comes back as a
  replica at the new epoch, its log cut back to what the new primary holds.
- Owner generations in the new region start at the epoch shifted left by 20
  bits, so every ticket or room write from the old region carries a lower
  generation and is stale at every fence (`control.coax`, the vitals fence of
  migration 0127, the item ledger of migration 0130, and the checkpoint
  receiver of migration 0131). Migration 0132 holds the store's epoch:
  `verse.region_promote(EPOCH)` takes a newer epoch and raises every
  character's owner generation to the new floor in one transaction, and
  `verse.room_owner_next` never issues below it. A store that was never
  promoted is at epoch 1 and issues generations as before.
- Parties, invites and seat holds live in the same store. They survive as
  replicated, or are lost with the window.

## Reconnect and placement

A page whose room socket closes asks for a new ticket, as it does today. After
a failover, the directory in the new region answers, so the page is placed
there. A party's placement (`verse.party_room`) uses the replicated rows; a
party whose room was lost is placed together in the next room with seats.

## Not supported

- Active-active play. A battle is simulated in one region only, and a
  character is never simulated in two. Storing data in several regions does
  not make that possible.
- Writes accepted in two regions at once. The epoch rule refuses them.
- Merging the old primary's lost window back in.

## Checks

```sh
coax/bin/coax test metaverse-engine/session/region.coax --library
coax/bin/coax mutate metaverse-engine/session/region.coax --library --root-only
```

The model's checks cover a partition and promotion after the lease, a forced
promotion while the old primary still runs, delayed replication and the loss
window it sets, the return of the former primary, stale epochs and owner
generations, a replica whose build can't read the store, and replication out
of order.

## Rehearsal

```sh
ROOM_BIN=... coax/web/api/test.sh   # or API_TESTS_ONLY=room_checks.coax; room_checks.coax region
```

`room_checks.coax region` rehearses a failover on one machine, against real
processes. The test database is region A's primary. The check:

1. Makes region B a streaming replica of A (`pg_basebackup -R`).
2. Saves a player's HP in A through the signed vitals route and waits for B to
   have it.
3. Cuts B off from A's stream. A's API keeps taking the room's writes: ten
   lines over 2 seconds, the loss window.
4. Waits out the 5-second lease, promotes B, calls `verse.region_promote(2)`,
   and starts a second API on B from the same migrations.
5. Checks that B's tickets carry owner generations above `2 << 20`, that B has
   the HP saved before the cut and none of A's later writes, and that a line
   from A's room under A's generation is refused (`f`).
6. With `ROOM_BIN`, seats the player in a room that reports to B, with B's
   ticket, and checks that the room's save is applied.
7. Brings the former primary back as a fresh copy of B. It is at epoch 2 and
   holds B's HP, not its own window's.

Measured on an M-series Mac, 2026-11-05, with local sockets:

| Measure | Result |
| --- | --- |
| Replication lag (a save in A visible in B) | 10-550 ms over three runs |
| Loss window | 10 of 10 writes made during the 5.06 s between the cut and the promotion; none before the cut |
| Promotion (`pg_ctl promote` and the epoch) | 115-132 ms |
| B's API answering health | 245-268 ms after the promotion starts |
| The first ticket from B | 265-286 ms |
| The first seat in a room that reports to B | 6.4-8.0 s, of which 2 s is the check's wait for the room process to start and most of the rest the seat client's own start |

The loss window equals the time A ran cut off before the promotion: the
partition plus the lease. Across real regions, add the replication lag
(target: at most 5 s) and the time to detect the cut. Restore time across real
regions adds starting rooms in B, DNS or load balancer changes, and pages
reconnecting; the 10-minute target leaves room for those.

### Cost and capacity assumptions

These are planning figures, not measurements. A standing replica costs about
what the primary's database instance costs, plus cross-region replication
egress for the write volume (vitals lines every few seconds per seated player,
a few hundred bytes each). Rooms in B are started on failover, not kept warm,
so B costs nothing in room servers until it is used. B's room fleet needs the
same capacity as A's, since one region serves everyone after a failover.

### What the rehearsal doesn't cover

- Real cross-region latency, DNS and load balancer failover, and the arbiter
  as a service. The check acts as the arbiter.
- Rewinding the former primary in place (`pg_rewind`). The check replaces its
  data with a fresh copy of B, which drops the window the same way.
- A deployed second region. Adding one is a production change and needs the
  owner's approval (NEEDS_OWNER.md).

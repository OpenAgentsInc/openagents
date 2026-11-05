# Region recovery for Verse rooms

Status: design and model qualification (#12853). The room fleet runs in one
region (`us-central1`) today, and nothing here changes that. This page states
what a second region would have to guarantee before it is added, and the model
that checks the ownership rules.

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
  migration 0127, and the item ledger of migration 0130).
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
coax/bin/coax test aethyr/session/region.coax --library
coax/bin/coax mutate aethyr/session/region.coax --library --root-only
```

The model's checks cover a partition and promotion after the lease, a forced
promotion while the old primary still runs, delayed replication and the loss
window it sets, the return of the former primary, stale epochs and owner
generations, a replica whose build can't read the store, and replication out
of order.

## Not yet measured

The loss window and restore time above are targets. They have not been
measured against two real regions: that needs a second region's store and
rooms, which the project does not run. The model gives the loss window in
entries for a given lag; a real measurement records it in seconds, the time to
the first seat, and the cost of a standing replica.

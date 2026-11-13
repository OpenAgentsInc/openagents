# 13. Coming back

> **Status: partly implemented.** An Everglade II room saves each seat's
> pose in its checkpoint and restores it at the next join (#13057, commit
> `540c62b879`). The room's checkpoint code is in
> [`coax/web/room/room-checkpoint.coax`](../../coax/web/room/room-checkpoint.coax)
> and the check is
> [`coax/web/room/e2-restart-check.coax`](../../coax/web/room/e2-restart-check.coax).
> The measurements below are from a local room. Production restart recovery
> also needs checkpoint tickets turned on in the API, which is an owner
> step. Your character on every device is planned in Part IX. This chapter
> names functions and has no source listings yet.

This chapter answers the player's question: "Where was I?"

A server restarts. A phone loses its signal in a lift. You close the tab and
open it tomorrow. Each time, you expect to stand where you stood. This
chapter covers who remembers what, and what happens when two memories
disagree.

## What this chapter teaches

- Which facts the browser keeps and which the server keeps.
- How a room saves its seats and gives them back after a restart.
- Why the room checks a position that a returning page claims.
- How a page finds a new room when its shard drains.

## What exists today

| Fact | Who keeps it | Where |
| --- | --- | --- |
| Whether you have a character | The browser. A new player goes to Character Create and a returning player goes to `/play`. | [Chapter 11](ch011-one-address.md) |
| Your character's look and name | The browser, sent in the hello | [Chapter 12](ch012-the-room.md) |
| Where you stood | The room's checkpoint, saved every 5 seconds, and the page's last drawn position | `room-checkpoint.coax`, `everglade/client/net.coax` |
| Who you are | Your player key and the seat ticket the API signs | [Metaverse Engine chapter 88](../../aethyr/book/ch088-a-seat-joins-with-a-signed-ticket.md) |

## The method

### Two memories of one position

When a room dies, two records of your position survive:

- **The room's checkpoint.** The room saves every seat's pose every 5
  seconds. After a restart, the saved pose can be up to 5 seconds old.
- **The page's own position.** The page knows where it last drew you. It is
  current, and a player can change it.

Neither record is good enough alone. The checkpoint is trusted and stale.
The page's position is fresh and untrusted.

### The room accepts a claim it can check

A returning page sends its position before its hello. The room accepts the
claim only when both of these hold:

1. The position is on ground where you may stand.
2. The position is within run-speed reach of the saved pose, for the time
   since the save.

Otherwise the room uses the saved pose. With no saved pose, it uses the
spawn. A position pressed against the edge of the walkable ground is drawn
back onto it and not toward the spawn.

This rule lets an honest page return to the exact spot. A page that lies
gains at most the distance it could have run anyway.

### A drain moves you to another room

A deploy stops a shard on purpose. The room saves every seat and tells each
page that it is restarting. The page waits, asks for a fresh ticket and is
placed in a room on another shard. Both rooms read the same checkpoint
store, so the new room knows where you stood.

The page's wait doubles after each failed try and has a random part, so a
thousand pages don't all return in the same instant
([Metaverse Engine chapter 98](../../aethyr/book/ch098-versions-deploys-and-reconnect.md)).

## A worked example: the room is killed mid-run

Eight bots walk for 3 seconds. The check then kills the room process
without warning.

| Measure | Result |
| --- | --- |
| The room is up again after | 302 ms |
| All 8 players are seated again within | 1,379 ms |
| Position error when the page sends its position | 0 mm |
| Position error for an older page that sends none | Up to 12.3 m |

The last row is the 5-second checkpoint by itself. At a run of about
4 m/s, a pose saved a few seconds ago can be more than 12 m behind you.

With a drain in place of a kill, 4 of 4 players sat in a second room within
1,334 ms, at most 50 mm from where they stood.

## The checks

- `coax/web/tool.sh coax/web/room/e2-restart-check.coax --players 8 --walk 3`
  kills the room mid-run and measures the return.
- The same check with `--drain` stops the room cleanly and expects every
  player in a second room.
- The same check runs an older page that sends no position. It shows what
  the checkpoint gives by itself.

The local stand-in for the API issues the same checkpoint tickets as
production and keeps checkpoints in a file store, so the local check uses
the real checkpoint path.

## Mistakes

### The snap after a restart

Before commit `540c62b879`, a room restart put every player back at the
spawn or at a stale saved pose. A player who had run down the path was
pulled back up it, sometimes by more than 10 m. The page then corrected, and
the player saw a snap.

The room now takes each seat's saved pose from its checkpoint and accepts
the page's own position when the two checks above pass. The restart check
measures the error in millimetres, and a snap fails it.

## What comes next

Part V turns from the world to the rules.
[Chapter 14](ch014-ability-scores-proficiency-and-the-d20.md) builds the
core roll that every attack, save and check uses.

# 98. Versions, deploys and reconnect

> **Status: partly implemented.** Reconnect has three working pieces: a page
> whose socket goes quiet for 6 s closes it and rejoins with a doubling wait
> (commit a388b270f9); a player's new socket replaces their silent old one,
> so no twin stands in the town (86808a350c); and a player back within
> 2 minutes stands where they were (d1fbd24f6f). The protocol version is
> defined (`WIRE_VERSION :: 2`) but nothing checks it (#12788). A deploy cuts
> every socket and loses the rooms' in-memory state (#12797). Socket, session
> and character share one lifetime (#12792).

A live game changes while people play it. A new page release meets an old
server, a new server meets cached pages, and phones lose their network in
the middle of a raid. This chapter adds the protocol around the game: a
version handshake, a deploy that overlaps safely, and reconnect as its own
protocol.

## What this chapter teaches

- A protocol version on every join, and what a mismatch does.
- A deploy order where old and new versions overlap safely.
- Draining a server instead of cutting its players.
- Reconnect: socket, session and character as three lifetimes.

## Version handshake

The page sends its wire version on join. The server refuses a mismatch with
a reason, and the page shows `A NEW VERSION IS OUT · RELOADING` and reloads
once. Without this, the launch runbook's order (oa-room before the page
release) has cached pages from the old release join the new server and read
its frames with the old decoder (#12788).

Deploys have separate artifacts with separate rollback limits (research doc,
section 14): page assets, server binaries, gameplay data, the wire schema and
the database schema. A wire change ships as:

1. A server that accepts the old and new version.
2. The page release that speaks the new version.
3. A server that refuses the old version.

## Drain, don't cut

A room's world, a raid in progress and its town live only in its process's
memory. On a shutdown signal, a shard:

1. stops taking new seats and tells the instance directory it is draining;
2. notes every seat's HP, XP and place;
3. tells its pages `RESTARTING · BACK IN A MOMENT`;
4. exits after its world events end, or at a stated limit.

The directory places no new seat on a draining shard, and a new revision
takes traffic only after its readiness check builds a room's world. Today
none of this exists: deploys happen "when few players are on" (#12797).

## Reconnect is its own protocol

A socket, an authenticated session and ownership of a character have
different lifetimes (research doc, section 4).

What works today: the page watches its socket and rejoins after silence.

<!-- source: apps/openagents-web/everglade-net.coax quiet-socket -->
```jai
// The quiet socket's watch, each frame (NET_SILENT_MS): an open socket that heard nothing for that
// long, or one still opening after NET_OPEN_MS, is closed, said, and tried again as a closed one is.
net_quiet :: (s: [] Everglade, n: [] EgNet, mono: int) {
  gap := mono - n[0].stepped_at; n[0].stepped_at = mono;
  watched := n[0].sock >= 0 && (n[0].state == NET_OPEN || n[0].state == NET_OPENING);
  if !watched || n[0].quiet_fresh || n[0].frames_in != n[0].quiet_frames || gap >= NET_STALL_MS {
    n[0].quiet_frames = n[0].frames_in; n[0].quiet_since = mono; n[0].quiet_fresh = !watched; return;
  }
  if mono - n[0].quiet_since < (ifx n[0].state == NET_OPEN then NET_SILENT_MS else NET_OPEN_MS) { return; }
  _ := pipe_close(n[0].pipe[..], n[0].sock);
  if n[0].seat >= 0 { n[0].lost = true; net_tell(s, n, "LOST THE ROOM · REJOINING"); }
  net_down(n);
}
```

On the server, a new join by the same player replaces their socket once it
has been quiet for 3 s, saving its HP, XP and place first
(`rl_replace_stale` in `room-relay.coax`), and `rv_back` puts the returning
player where they stood.

What the design adds (#12792):

- **A seat generation.** Each join starts one, and every frame carries it.
  Frames from an earlier generation are dropped. Today frames still in
  flight on a replaced socket act on the old seat, because `rl_heard` doesn't
  check for a kicked socket.
- **A grace hold.** A closed socket's character stays in the world, AWAY,
  for a stated grace (for example 15 s). A rejoin within it takes the same
  seat with its conditions, effects and cooldowns. Today the character leaves
  the moment the socket closes.
- **Resume from a snapshot.** The returning seat gets a full snapshot
  (chapter 91), which is enough at Everglade's scale; there is no event log
  to replay.

## Run it

```shell
coax/bin/coax test --library coax/web/room/room.coax --only "relay: a player's own silent" --steps 4000000000
coax/bin/coax test --library coax/web/room/room.coax --only "vitals: a player back" --steps 4000000000
```

```text
ok   #test relay: a player's own silent socket gives way to their new one (no twin in the town); one still talking stays (1 case, 3372690 steps, 4144 ms)
1 of 357 checks (--only 'relay: a player's own silent'), 1 passed, 0 failed
1 of 357 checks (--only 'vitals: a player back'), 1 passed, 0 failed
```

The live check `coax/web/room/rejoin-check.coax` drops a bot's socket and
checks where and how it comes back.

## Acceptance checks

- A page with an old wire version is refused with its reason and reloads
  once (#12788).
- A deploy over a room with bots in it brings every bot back to its room,
  where it stood, with its HP (#12797).
- A drop and return within the grace keeps the same seat and its effects; a
  late frame from the old socket is dropped and counted (#12792).

## Dependencies

Chapters 88 (seats) and 91 (snapshots).

# 98. Versions, deploys and reconnect

> **Status: partly implemented.** The hello carries the wire version: a
> room refuses any other with `r 0 version` and closes the socket, and the
> page reloads once (#12788, commit 67b4f62448); pages from before wire 3
> play on the old wire until 2026-11-12 (commit 0e96c34284). Reconnect is its
> own protocol (#12792, commit 257c85b06d): a closed socket's seat is held
> for `EG_GRACE_MS` (15 s), a rejoin takes it back with its HP, conditions,
> effects and cooldowns, and each seat has a generation, so frames from a
> replaced socket are dropped. Room vitals count as saved only when the API's
> receipt covers them (#12793, commit 5016ba0be3). Signed drain requests,
> readiness checks and receipt-gated shutdown now run in `room-host.c`;
> owner generations fence character writes separately from seat generations.

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
takes traffic only after its readiness check builds a room's world.
`room-host.c` implements waiting, telling, saving and done/failed phases.
A missing receipt fails the drain; it is not reported as saved. This
protocol preserves captured character state, not every transient world event.

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

On the server, reconnecting is its own protocol (#12792). A socket, the
player's ticket and the seat have different lifetimes:

- **A seat generation.** Each time a socket takes a seat, the seat's
  generation goes up by one, and the room's `y` frame names it
  (`y SEAT N T G`). A frame counts only while its socket is the seat's current
  socket of the current generation (`rl_current` in `room-relay.coax`). A
  frame still in flight on a replaced socket is dropped and counted (the room
  view's `old socket frames dropped`).
- **A grace hold.** When a socket closes and its ticket names the player, the
  seat is held for `EG_GRACE_MS` (15 s, `room-everglade.coax`). The character
  stays in the rules and the world, standing, AWAY once the page has been
  quiet for `WIRE_AWAY_MS`. The others hear no `l`. A kicked socket (over its
  rate limits, on another wire, or kicked by an admin) leaves at once.
- **Taking the seat back.** The player's next join within the grace takes the
  same seat (`rl_take_back`). So does a join while their old socket has been
  quiet for 3 s (a phone between networks): the old socket is closed, and its
  close leaves the seat alone. The character keeps its HP, conditions,
  effects, cooldowns, XP and place. What the server knew of the page is reset
  (`eg_rejoin`): the next `D` is a snapshot (chapter 91), and inputs count
  from the new page's first. The others hear no `j`.
- **After the grace.** The seat leaves as a closed socket's did before: its
  HP, XP and place are noted for the player, everyone hears `l SEAT`, and
  `rv_back` puts the player where they stood when they join later.

## Run it

```shell
coax/bin/coax test --library coax/web/room/room.coax --only "relay: a closed socket" --steps 4000000000
coax/bin/coax test --library coax/web/room/room.coax --only "relay: a player's own silent" --steps 4000000000
coax/bin/coax test --library coax/web/room/room.coax --only "vitals: a player back" --steps 4000000000
```

The live check `coax/web/room/rejoin-check.coax` silences a bot's socket and
brings it back, sends a late frame on the old socket, closes the new one and
rejoins within the grace, and last closes for good and waits out the grace.

## Acceptance checks

- A page with an old wire version is refused with its reason and reloads
  once (#12788).
- A deploy over a room with bots in it brings every bot back to its room,
  where it stood, with its HP (#12797).
- A drop and return within the grace keeps the same seat and its effects; a
  late frame from the old socket is dropped and counted (#12792).

## Dependencies

Chapters 88 (seats) and 91 (snapshots).

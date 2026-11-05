# 86. One server owns the scene

> **Status: partly implemented.** The Everglade room server, oa-room
> ([coax/web/room/](../../coax/web/room/room.coax)), steps every room's
> world headless at 30 Hz and starts from every declared default, checked by
> `bin/room --start-check` (#12774, commit c3e170d689). The same tick also
> runs in a Web Worker as the offline host, behind `?offline=worker`
> (chapter 95, #12784, commits e8c68921ed and 2b86e8fafc); until #12882
> makes it the default, offline play still runs the page's own copy of the
> game. The shared `aethyr/net/` module this part plans is not extracted yet
> (#12845).

Part XII builds a multiplayer engine from an empty server to the full
Everglade room. Each chapter adds one capability. This chapter adds the
first: one process that owns the shared scene. Every later chapter assumes
it.

A multiplayer game needs one place where gameplay is decided. If two
programs each decide where a creature stands, they disagree as soon as their
inputs arrive in a different order. The server runs the simulation; a client
sends what its player asks for and draws what the server says. The research
behind this part
([docs/research/mmo.md](../../docs/research/mmo.md), sections 5 and 17)
lists a headless runtime as the first foundation and "one authoritative
scene" as the first step of its development order.

## What this chapter teaches

- A server that runs the simulation without drawing anything.
- A fixed tick that owns time on the server, as chapter 1's clock owns it
  on the page.
- State that starts from declared values, never from whatever memory held.
- A check that fails the build when one field starts wrong.

## The design

The server is a loop over rooms. Each room has seats, a world and a tick
counter. One tick does four things in a fixed order:

1. Step each seat's queued inputs through the movement step (chapter 87).
2. Step the rules: casts that strike now, effects, creatures and the town.
3. Write each seat's state frame (chapters 91 and 92).
4. Send every socket's frames once.

The server never draws. It loads the collision and rules data it needs to
decide play, and none of the textures, meshes or particles that only a
client draws.

The tick rate is a declared constant. Everglade runs at 30 Hz: a step is
33.3 ms, so a command that arrives at a random time waits about 16.7 ms on
average for the next tick (research doc, section 6). If a tick takes longer
than its 33.3 ms, inputs queue behind it, and no network is fast enough to
hide that; chapter 99 measures it.

### State starts from its declarations

Coax gives every struct field a default (chapter 3). The room server is a
C host that calls Coax code, and C memory starts as whatever `malloc` or
`calloc` returned. A field whose default is 2 reads as 0 in zeroed memory.
That is how a server with a level cap of 2 sent `CAP 0` and an empty kit,
which the page drew as the full high-level hotbar (chapter 100's
counterexample).

So the host does two things before the first tick:

1. It writes every field's declared default with the compiler's generated
   `coax_default_t_RmServer`.
2. It calls `rm_init`, which sets only what a declaration can't say.

<!-- source: coax/web/room/room.coax server-start -->
```jai
// The server at its start. The host first gives every field its declared default
// (coax_default_t_RmServer, room-host.c server_new: the C host's memory is otherwise zeros, and a
// default such as the level cap's 2 would never hold), then calls this once. What it sets is what a
// declared default can't say; rm_start_check holds the started server to the declarations (#12774).
rm_init :: (v: [] RmServer) #export_c {
  for i: 0..<CONNS { v[0].conns[i] = RmConn.{}; }
  for r: 0..<RELAYS { v[0].relays[r].live = false; }
  for k: 0..<EG_ROOMS { v[0].eg[k].live = false; v[0].eg[k].relay = -1; }
  v[0].q_head = 0; v[0].q_n = 0;
  for r: 0..<ROOMS { v[0].rooms[r].live = false; v[0].rooms[r].s[0] = OrbitLeague.{}; }
}
```

`rm_start_check` compares the started server with fresh declared values,
field by field. The host's `--start-check` also starts a second server in
memory filled with `0xa5` bytes and compares the two byte for byte, so a
field that depends on prior memory fails.

## Run it

Build the server and run the start check. The build script runs the same
check and fails when it does.

```shell
coax/web/room/build.sh
coax/web/room/bin/room --start-check
```

```text
room: start check: 0 fields off their defaults, the same from any memory
```

The exit status is 0. Remove the `coax_default_t_RmServer` call from
`server_new` in `room-host.c` and the check reports how many fields are off
their defaults and exits 1.

## What exists today

- [coax/web/room/room-host.c](../../coax/web/room/room-host.c): the
  single-threaded host (TCP, WebSocket frames, the `poll` loop),
  `everglade_tick`, `server_new` and `start_check`.
- [coax/web/room/room-everglade.coax](../../coax/web/room/room-everglade.coax):
  the rooms' simulation (`rm_eg_begin`, `rm_eg_input`, `rm_eg_moved`,
  `rm_eg_end`), 16 rooms of 16 seats a process at `EG_HZ` 30.
- [coax/web/room/room-world.c](../../coax/web/room/room-world.c): each room's
  headless Everglade world, built from the page's own units with nothing
  drawn.

## What is left

- Offline play runs the page's own simulation instead of this server. Chapter
  95 runs this server in a local worker (#12784).
- The shared loop lives in `coax/web/room/` and is Everglade's. Extracting it
  to `aethyr/net/server.coax` for other realms waits on the rest of this part.

## Acceptance checks

- `bin/room --start-check` exits 0 on every build.
- A room steps the same inputs to the same state at any host frame rate
  (chapter 1's rule, on the server's tick).

## Dependencies

Chapters 1 (the fixed step), 3 (state from its defaults) and 6 (entity
identity).

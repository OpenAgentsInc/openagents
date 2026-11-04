# 95. Offline is a local server

> **Status: unimplemented — design draft.** Today offline Everglade is a
> second game: the page runs its own watch, townsfolk, spells and rules,
> and in a room it switches to the server's state, with about 65 branches on
> room state across 8 files. #12784 builds the room's simulation to Wasm and
> runs it in a Web Worker as the offline host; none of its files
> (`coax/web/room/room-worker.c`, `room-tick.c`,
> `apps/openagents-web/everglade-host.coax`) are on main yet.

Every feature of a game that has an offline mode and an online mode is
written twice, unless the offline mode is the online mode. This chapter adds
that rule: offline play runs the same server simulation, in a local host,
over the same protocol. The page is always a client.

## What this chapter teaches

- Why "offline" and "room" must be one code path.
- Running the server's simulation in a Web Worker.
- A local host that speaks the same frames as the real server.
- Where state defaults and new server steps must live so both hosts get them.

## The failure it removes

The owner played a local room on 2146dc01d9 and found five problems: the
camera snapped while running, townsfolk walked in place, Reverse Gravity
had no particles, a hold showed nothing, and a level 1 character had the
full high-level hotbar. Each had its own direct cause (chapters 86, 87, 90,
91 and 94). They share one root cause: the page ran an offline game and a
room game side by side, each feature was rewired by hand for the room, and
only movement was wired completely. A fix to one path never reached the
other.

## The design

**One simulation, two hosts.** The server's simulation (`room.coax`,
`room-everglade.coax`, the raid and rules units) is Coax. Build it twice:

- to C for oa-room, with `room-host.c` moving bytes over TCP;
- to Wasm for a Web Worker, with `room-worker.c` moving the same frames
  over `postMessage`.

**The page is always a client.** Offline, the page joins
`local:/api/room/relay/everglade/local-1`. `net_live` is true. The page sends
intents and draws state exactly as in a room; the `?raid=preview` cinematic
runs as a room event in the local host. Then the page's own offline
simulation is deleted, not kept as a fallback.

**Shared tick, shared start.** Both hosts include one tick
(`room-tick.c`). A new server step goes there, not in one host, or the
other host misses it. Both hosts start the server with its declared defaults
and `rm_init` (chapter 86), never with state set in a host's `main`.

**Measure the cost.** A phone now runs the simulation as well as the
drawing. The worker's tick time is measured on a phone-class CPU throttle,
next to the 60 FPS rule of chapter 83.

## What it builds

- `apps/openagents-web/everglade-host.coax` and
  `coax/web/room/room-worker.c`: the local host.
- `coax/web/room/room-tick.c`: the tick both hosts include.
- Deletion of the page's offline-only simulation paths, with the count of
  room branches before and after.

## Acceptance checks and oracles

- The drift check (chapter 100) compares the offline page and a room page
  probe by probe. With one simulation, every probe holds at local latency.
- The number of branches on room state in `apps/openagents-web` falls to the
  few that choose a host.
- The worker's tick fits its step on the phone throttle, and the page keeps
  60 FPS.

## Dependencies

Chapters 86 (the server), 87 (shared step and world) and 89 (intents).

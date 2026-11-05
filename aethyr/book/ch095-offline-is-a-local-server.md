# 95. Offline is a local server

> **Status: partly implemented.** The local host runs behind a switch,
> `?offline=worker` (#12784): `coax/web/room/worker.sh` builds the room
> server's units to `build/everglade-room.wasm` with
> [room-worker.c](../../coax/web/room/room-worker.c) in place of
> `room-host.c`, both hosts include one tick
> ([room-tick.c](../../coax/web/room/room-tick.c)), and
> [room-worker.js](../../coax/web/room-worker.js) runs it in a Web Worker
> behind a `local:` socket. Since #12882 an Everglade page that never sat
> and still can't reach the rooms after three tries plays on the local host
> too (`everglade-net.coax` `net_down`, `NET_LOCAL_AFTER`); `?offline=page`
> keeps the page's own game. The page's own offline simulation is not yet
> deleted, and a page plays it until that fallback (#12882).

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
`local:/api/room/relay/everglade/local-1`. The page's host (`host.js`) opens
a `local:` URL as a loopback to the worker instead of a WebSocket, with the
same text frames, so the page's Coax can't tell the two apart. `net_live` is true. The page sends
intents and draws state exactly as in a room; the `?raid=preview` cinematic
runs as a room event in the local host. Then the page's own offline
simulation is deleted, not kept as a fallback.

**Shared tick, shared start.** Both hosts include one tick
(`room-tick.c`). A new server step goes there, not in one host, or the
other host misses it. Both hosts start the server with its declared defaults
and `rm_init` (chapter 86), never with state set in a host's `main`.

**Shared math.** One tick is not enough if the two hosts round floats
differently. macOS's libm and wasi-libc give different last bits for some
sines, exponentials and powers, and the town's falls and sight checks turn
those bits into different outcomes. Both hosts link `room-math.c` in place
of the C library's functions and build with `-ffp-contract=off`, so the
native step and the wasm step compute the same bits (#12893).

**Measure the cost.** A phone now runs the simulation as well as the
drawing. The worker's tick time is measured on a phone-class CPU throttle,
next to the 60 FPS rule of chapter 83.

## What it builds

- `coax/web/room/room-worker.c` and `coax/web/room-worker.js`: the local
  host. The worker fetches the same world document the page reads, keeps
  the 30 Hz beat, and stands in for the API's control lines
  (`?levels=off`, and `?raid=preview` as a room event).
- `coax/web/room/room-tick.c`: the tick both hosts include.
- `coax/web/room/room-math.c`: the sines, exponentials and powers both hosts
  link, checked by `coax/web/room/room-math-check.sh`.
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

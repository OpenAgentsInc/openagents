# The agent clock

Issue: [#13096](https://github.com/OpenAgentsInc/openagents/issues/13096)
(phase 2 of the agent loop,
[what to adapt](../research/exact/03-what-to-adapt.md)).
Code: `coax/web/drive.coax` (the driver), `coax/web/host.js` (agent mode),
`coax/web/room-worker.js` (the offline room), `metaverse-engine/agent/pin.coax` (the
pin), `metaverse-engine/agent/drive.coax` (`quiet`).

`coax drive` opens every page in agent mode. In agent mode, simulation time
moves only when you step it. Between two operations nothing moves: no
monster walks, no cast lands and no number fades.

```sh
coax/web/tool.sh drive web apps/openagents-web /worlds/everglade-ii \
  "walk-to guard" "clock +2000" "clock settle" \
  "clock until you_x > 3 within 5000" "state you_x"
```

## The operations

| Operation | What it does |
| --- | --- |
| `clock +MS` | Steps the clock MS in whole frames, 60 a second, and at least one frame. It prints how far the clock moved. |
| `clock settle [within MS]` | Steps 100 ms at a time until the page and the offline room both report nothing scheduled. It stops after MS of the clock (10,000 by default) and prints what was still scheduled. |
| `clock until PATH [OP VALUE] [within MS]` | Steps 100 ms at a time until the state's field compares so. OP is `==`, `!=`, `<`, `>`, `<=` or `>=`. With no OP, it waits until the field isn't 0 or false. It fails after MS (`--within`, 30,000 by default). |

A world operation (`walk-to`, `face`, `use`, `mount`) steps the clock itself,
100 ms at a time, until it is done or has failed.

`clock settle` always steps once before it asks, because the page sees an
operation you just sent on its next frame.

## What "nothing scheduled" means

- **The page.** The realm answers the driver's `quiet` command
  (`metaverse-engine/agent/drive.coax` `drv_quiet`). It is busy while a world operation
  is going, or while the realm said this frame that something is scheduled
  (`drv_busy`). Everglade II says so while a damage number is rising. A realm
  adds a `drv_busy` call for anything else it wants `settle` to wait for.
- **The offline room.** The room counts the casts that have begun and not
  resolved (`coax/web/room/room-casts.coax` `rm_scheduled`).

Monsters that wander don't count as scheduled, so a world with wandering
monsters still settles.

## How the page and the room step together

The page uses the offline room (`coax/web/room/worker.coax`, built as
`everglade-room.wasm` and run by `room-worker.js`) whenever no room server
answers, which is the case under `coax drive`. In agent mode the host draws
only when the clock steps, each frame one fixed step, and it advances the room
to the same moment before each frame. This is the mechanism clips use
(`coaxialClipStep`), so a clip and an agent session step the same way.

While the page loads, before the first step, the host draws frames one
microsecond apart. Loading therefore finishes, and the clock is still within a
few milliseconds of zero when the driver takes it.

## The pin

Every agent session runs with the values in `metaverse-engine/agent/pin.coax`:

| Value | Pin |
| --- | --- |
| Seed | 13096 |
| Epoch | 2026-01-01T00:00:00Z |
| Locale | en-US |
| Time zone | UTC |

The driver hands the pin to the page before it loads. In the page,
`Date`, `Math.random` and `crypto.getRandomValues` answer from the pin, and
the date moves with the agent clock. The offline room takes its dice and its
date from the same pin. The driver also sets the browser's locale and time
zone.

To change a pinned value, change it in `metaverse-engine/agent/pin.coax` and nowhere
else.

## Limits

- The driver has one host today: `web`. The Mac and iOS hosts don't have the
  agent clock or the pin yet.
- A page that plays in a room on a server can't be stepped: the server keeps
  its own time. `coax drive` serves the app with no room server, so the page
  plays in the offline room.
- Agent mode works only on a page served from `127.0.0.1` or `localhost`.
- `clock data` (step until pending fetches answer) is not built.
- `--real-time` opens the page as a player's. Then `clock +MS` waits MS of
  real time, and `settle` and `until` refuse.

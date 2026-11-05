# Everglade session profile v4

Status: the profile Everglade rooms and pages speak today (GitHub #12844).

The profile is the text wire between an Everglade page and its room server.
Its machine-readable form is
[`proto/verse/everglade-session-v4.json`](../../proto/verse/everglade-session-v4.json).
The codec that implements it is `apps/openagents-web/everglade-wire.coax`,
which both the page and the room server load.

## Identity

- Profile `openagents.everglade.session`, version 4 (`WIRE_VERSION`).
- The oldest version a room seats is 4 (`WIRE_MIN`).
- A page also states its build pins before its hello (`P ENGINE RULES
  CONTENT`, all 1 today). A room refuses a hello whose version is outside
  `WIRE_MIN..WIRE_VERSION`, or whose pins differ from its own, with
  `r 0 version`, and the page reloads. Admission is `eg_admit` in
  `coax/web/room/room-everglade.coax`, and its checks run in the room suite.
- A seat ticket carries the character's owner generation (#12834). A room
  that holds an older generation ends the seat with `r 0 elsewhere`.

## What the profile records

For each message the profile names its direction, its head, its fields in
order, one canonical value for each field, and the exact frame those values
make. It also lists malformed frames (a field missing, or not a whole number),
the refusal words for each intent, and the features the profile does not
support.

To change the wire, raise `WIRE_VERSION` and add a new profile file. Don't
edit a published profile in place.

## Check

```sh
coax/web/tool.sh apps/openagents-web/everglade-wire-profile.coax proto/verse/everglade-session-v4.json
```

The check holds the codec to the file:

- The file's version, oldest version, pins and frame limits are the codec's
  constants.
- Each message's values, written by the codec's writer, are its frame byte
  for byte.
- Each message with a reader reads its frame back to the same values, field
  by field.
- Each malformed frame leaves its reader's `ok` unset.

The frames in the file are written by hand, so they are an expected result
independent of the codec.

## Not supported

- The wire before version 3 (`H 1`). Rooms seat it on the old wire only until
  2026-11-12, then refuse it.
- The `realm.v1` chamber messages in
  [openagents-verse-protocol.md](../verse/openagents-verse-protocol.md)
  (`ActorAppear`, `ActorUpdate`, `SpatialDelta`, transfer). Everglade rooms
  don't speak them.
- Protobuf framing. This profile is text frames only.

## Not yet covered

- The entity rows inside `D` (`s ID BACK MASK ...`, `x`, `n`) are checked by
  the codec's own tests (`WireMirror`), not by fixtures in this file.
- The relay's own frames (`y`, `j`, `l`, `t`) and the block list (`K`).

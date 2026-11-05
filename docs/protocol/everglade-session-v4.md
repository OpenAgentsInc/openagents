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

The profile also records:

- **The entity row** (`row`): the 23 fields of every replicated entity, in the
  order `MASK` numbers them. Its fixtures (`s ID BACK MASK V ...`) name the
  fields they carry; one is absolute, and one is a delta against a `base`
  with the row it makes (`expect`). `x ID` and `n SS ID LOOK CLASS FLAGS NAME`
  have fixtures too.
- **The relay's frames** (`y SEAT OTHERS CLOCK GENERATION`, `j SEAT`,
  `l SEAT`, `t SENT CLOCK`) and the page's block list (`K PID ...`, `K -`).
- **ACK and baseline fields** (`ack`): `I`'s `heard`, `D`'s `ack`, `s`'s
  `back`, and the outcome counts `o` and `O`.
- **Lifecycle and ownership fields** (`lifecycle`): the seat generation in
  `y`, a row's `life`, `warp` and `owner`, `x`, and the owner generation the
  seat ticket carries.
- **The seat ticket** (`ticket`): its format, one signed ticket with the
  fields it reads to (owner generation 12, shard 3, HP, XP and WANTED), and
  tickets that are refused: another room's, an expired one, and one whose
  owner generation was raised after signing.

To change the wire, raise `WIRE_VERSION` and add a new profile file. Don't
edit a published profile in place, except to add a message that a page or room
of the same version that doesn't know it ignores. The outcome count `o N`, the
page's `O N` and `e town` (#12902) were added that way: a room or page from
before them never sends them and drops them when it hears them.

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
- The row's field names are the codec's `WS_*` constants in order. Each row
  fixture's `MASK` names exactly its fields; written from its base it is the
  frame, and read onto that base it is the expected row.
- The relay's frames are whole numbers, as the page reads them. The room's
  writer of the same frames is checked by the room suite (`relay:` tests).
- The seat ticket reads to its expected fields through the room's own reader
  (`tk_check` in `coax/web/room/room-ticket.coax`), and each refused ticket is
  refused.
- Each named ACK, baseline and lifecycle field is a field of a message, of the
  row, or of the ticket.

The frames in the file are written by hand, and the ticket's MAC was computed
with `openssl dgst -sha256 -hmac`, so they are an expected result independent
of the codec.

The codec is checked against the field descriptions, not generated from them.
The issue allows either. A generator would add a build step to every page and
room build for a codec of about 40 messages that changes rarely.

## Not supported

- The wire before version 3 (`H 1`). Rooms seat it on the old wire only until
  2026-11-12, then refuse it.
- The `realm.v1` chamber messages in
  [openagents-verse-protocol.md](../verse/openagents-verse-protocol.md)
  (`ActorAppear`, `ActorUpdate`, `SpatialDelta`, transfer). Everglade rooms
  don't speak them.
- Protobuf framing. This profile is text frames only.

- The relay's `m SEAT TEXT`. Other games' rooms pass a player's frames on as
  sent. An Everglade room reads them as intents and passes none on.

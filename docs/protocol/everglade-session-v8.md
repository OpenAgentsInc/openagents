# Everglade session wire, version 8 (binary)

- Issue: #13278 (AAA P2.5). Tracker: #13257.
- Table and codec: `everglade/shared/wire-table.coax` (`WT`, `wt_bin`, `wt_text`).
- Profile: `proto/verse/everglade-session-v8.json`. It is written from the table; do not edit it.
- Check: `coax/web/tool.sh coax/web/room/wire8-profile.coax proto/verse/everglade-session-v8.json`
- Unit tests: `coax/bin/coax test everglade/shared/wire-table.coax`
- Previous wire: `docs/protocol/everglade-session-v7.md` (text). The rule for serving it:
  `docs/protocol/everglade-previous-wire.md`.

## What it is

One table, `WT`, has a row for every frame and every entry of the session: its opcode, its
text head on wire 7, its fields with each field's type, bits and unit, and an example. One
encoder and one decoder read that table. The profile and its golden bytes are written from
the same table by the check with `--write`.

A frame is one WebSocket binary message (a WebTransport datagram or stream record on that
transport):

1. One opcode byte: 128 plus the kind's row in `WT`. No text frame starts with a byte of 128
   or more, so a reader tells wire 7 from wire 8 by a frame's first byte.
2. The kind's fields as bits, the highest bit first.
3. Zero bits to a whole byte.

The relay's own frames (`y`, `j`, `l`, `t`: `docs/protocol/room-relay-v2.md`) and the
optional latency observation (`@1`: `docs/protocol/everglade-latency-v1.md`) are other
protocols. They stay text messages on the same socket.

## Field types

| Type | Meaning |
| --- | --- |
| `uG` | A whole number, 0 or more. Groups of G bits, the lowest group first. One bit after each group says whether another follows. |
| `iG` | A whole number of either sign. Zig-zagged (0, -1, 1, -2 as 0, 1, 2, 3), then as `uG`. |
| `bG` | G bits. |
| `w` | A word. Its length as `u5`, zero bits to a whole byte, then its bytes. |
| `n` | A name, a record's last field, spaces kept. Written as a word. |
| `t` | A number or a word. One bit (0 a number as `i7`, 1 a word), then it. |
| `?` | One bit. 1 when the fields after it are said, 0 when the record ends here. |
| `*F` | The rest of the record is field F again and again. Their count as `u4`, then each. |

## Precision

Every field is a whole number in the unit its row names. The units are the ones wire 7
wrote in decimal, so the room decides from the same values on both wires, and a page
predicts its own movement from the same inputs the room steps.

| Unit | One unit is | Used by |
| --- | --- | --- |
| `cm` | 1 cm | positions, aims |
| `cm/s` | 1 cm/s | a row's velocity |
| `crad` | 0.01 rad | a row's yaw, `A`'s yaw |
| `0.1mrad` | 0.0001 rad | an input's heading (`I`, `J1`) |
| `percent` | 1/100 of a key or stick | an input's keys and stick, -100 to 100 |
| `mm`, `mm/s`, `thousandths` | 1 mm, 1 mm/s, 1/1000 | projectiles (`F`, `a`, `d`) |
| `ms` | 1 ms | times |
| `0.1ms` | 0.1 ms | a row's CARRY (the controller's leftover step time) |
| `tick`, `ticks` | 1/30 s | the room's ticks |
| `hp`, `xp` | 1 point | Hit Points, XP |

Nothing is rounded on the way to bits: a number goes whole, in as many groups as it needs.
The rounding is where a float becomes its whole number, in `everglade-wire.coax`
(`wire_cm`, `wire_cyaw`, `wire_fine`), as before.

## The state frame

`D` is its header (`tick:u7 ack:u7 f:b3 part:u3`), zero bits to a whole byte, then its
entries. Each entry is its length (one byte under 128; else two: 128 plus the high bits, then
the low byte) and its bytes, which are a frame of their own: an opcode and fields.

Rows go as one entry, a run (kind 2): for each row a 1 bit and the row, then a 0 bit. A row
is its entity's id (`u8`), BACK (`b4`), the mask of the fields that changed, and each changed
field's change (`i4`). The mask is two levels: five bits say which runs of fields changed
(fields 0 to 4, 5 to 7, 8 to 12, 13 to 17, 18 to 22), then each changed run's own bits. The 23
fields and their units are in the profile's `row.fields`.

## Wire 7 from the same table

Wire 7's text is the table's other form. `wt_bin` turns any text frame into its binary frame
and `wt_text` turns it back, byte for byte. A frame the table has no row for, or one not
written as its row says, goes as kind 0: its words, or its bytes. So nothing is lost either
way, and a reader's own checks (a field count, a range) see the same text.

A room that speaks wire 8 serves a page on wire 7 through these two, for one release
(`everglade/server/wire-prev.coax`).

## Sizes

| Frame | Wire 7 text | Wire 8 |
| --- | --- | --- |
| `I 41 0 100 0 0 -15708 1 900` | 27 bytes | 11 bytes |
| `x 7` (an entry, with its separator or length) | 4 | 4 |
| `e hit 1002 1003 7 1` | 19 | 7 |
| `D 900 41 0 0 ` (the header) | 13 | 5 |
| A packed row of 60 bits in a run | 12 (`S`, 10 characters, `;`) | 8 |

## Measurements

See "Before and after" below once the room and the pages speak wire 8.

## Adding a kind

Add a row at the end of `WT` (a kind's number is its row and never changes; a kind that goes
keeps its row), raise `WT_KINDS`, run the check with `--write`, and commit the profile with
the code. `coax/bin/coax test everglade/shared/everglade-wire.coax --only "binary wire"`
holds every writer in that file to its row.

# Everglade session frames in binary (wire 7, `WIRE_HELLO_BINARY`)

- Issue: #13278 (AAA P2.5). Tracker: #13257.
- Table and codec: `everglade/shared/wire-table.coax` (`WT`, `wt_bin`, `wt_text`).
- Profile: `proto/verse/everglade-session-binary-v1.json`. It is written from the table; do not edit it.
- Check: `coax/web/tool.sh coax/web/room/wire-table-profile.coax proto/verse/everglade-session-binary-v1.json`
- Unit tests: `coax/bin/coax test everglade/shared/wire-table.coax`
- The same frames as text: `docs/protocol/everglade-session-v7.md`.

## What it is

One table, `WT`, has a row for every frame and every entry of the session: its opcode, its
text head on wire 7, its fields with each field's type, bits and unit, and an example. One
encoder and one decoder read that table. The profile and its golden bytes are written from
the same table by the check with `--write`.

## How a page and a room agree on it

It is not a new wire number. Wire 7 has not shipped (production's rooms speak wire 5), and the
rule is one wire raise a release, so binary frames are part of wire 7.

A page asks for them in its hello: it sets `WIRE_HELLO_BINARY` (16) in `H`'s FLAGS. The hello
itself, and the `P` and `A` before it, are text frames. So a room of any build reads them, and
a room that cannot seat the page refuses it in text the page reads (`r 0 version`).

A room that reads the flag writes that page binary frames from then on. The page writes binary
frames after its hello. Each side reads a frame by its first byte, so a text frame that crosses
the hello on the way is still read.

A page that does not set the flag is written and read as text, as before. That form goes when
the pages that speak binary and the rooms have both shipped.

## A frame

A frame is one WebSocket binary message (a WebTransport datagram or stream record on that
transport):

1. One opcode byte: 128 plus the kind's row in `WT`. Kinds are 0 to 62, so an opcode is 0x80 to
   0xBE: a UTF-8 continuation byte, which no text starts with. A reader tells text from binary
   by a frame's first byte, and a host sends a frame that starts so as a binary message. Kind 63
   (0xBF) is kept for the day the table has more than 63 kinds: the kind then follows it.
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

Rows go as one entry, a run (kind 2): the rows back to back, each ended with zero bits to a
whole byte, to the entry's end. A row
is its entity's id (`u8`), BACK (`b4`), the mask of the fields that changed, and each changed
field's change (`i4`). The mask is two levels: five bits say which runs of fields changed
(fields 0 to 4, 5 to 7, 8 to 12, 13 to 17, 18 to 22), then each changed run's own bits. The 23
fields and their units are in the profile's `row.fields`.

## Text from the same table

Wire 7's text is the table's other form. `wt_bin` turns any text frame into its binary frame
and `wt_text` turns it back, byte for byte. A frame the table has no row for, or one not
written as its row says, goes as kind 0: its words, or its bytes. So nothing is lost either
way, and a reader's own checks (a field count, a range) see the same text.

## What is written as bits, and what goes through text

For a page that reads binary:

- **The state frame is written as bits.** `wire_seat_frame_look` writes a seat's rows as runs
  and its `x`s as entries, and the `D`'s header is the table's (`everglade/server/send.coax`).
  No text is made on the way.
- **An input is read from bits.** `I` and `J1` go from the frame's bits into the seat's queue
  (`everglade/server/input.coax` `eg_heard_in`).
- **Every other frame goes through its text once.** The room's other writers (who a seat is,
  events, refusals, the world header) still write text, and each is turned to its bytes as it
  goes into the seat's frame or the out buffer. What a page says other than an input is read
  as its text by the readers that were there. These are a few frames a second a seat.
- The page and the bots turn every frame at their socket (`everglade/client/net.coax`,
  `coax/web/room/bots.coax`), so their readers are the text ones.

The room counts a frame it sent as kind 0 (`wire_any`) and a binary frame it could not read
(`wire_bad`); its checks hold both at 0.

What is left for after the pages and rooms have shipped: the text form goes. The writers and
readers then take the table's rows directly, and `wt_bin` and `wt_text` stay only as the
checks' way to write a frame by hand.

## How the room dispatches what it hears

A frame's kind is its opcode (binary) or the table's row whose head it has (text). The room
takes a frame only when its kind is one a page writes (`WT[kind].who == WT_PAGE`), and hands
it to its reader by that kind (`eg_heard_as`). The previous wire's `M` and `U` frames have no
row; `wire-prev.coax` reads them.

## The room's out buffer

A frame is written once, for one connection, into the room's out buffer
(`coax/web/room/room-relay.coax` `ob_put`) with its WebSocket header before it. The host
(`room-host.c` `relay_flush`) takes each record and puts its bytes on that connection's
socket as they are. It builds no frame and looks at no other connection. Before, a frame was
copied into a queue entry with a flag for each of 512 connections, copied out again, built
into a WebSocket frame and copied to the socket's buffer, and the host asked the room about
every connection for every frame.

## Transports and hosts

| Path | Binary |
| --- | --- |
| Browser, WebSocket (`coax/web/host.js`, `socket-queue-host.js`) | yes |
| Browser, the local room in a worker (`room-worker.js`) | yes |
| Browser, WebTransport (`room-wt.js`) | The wrapper carries it, but the page keeps text on this link: its input datagrams are text records (`metaverse-engine/net/transport.coax`), and the room reads them as text (`room-wt.coax`). |
| Apple page host (`metaverse-engine/host/apple/PageHost.m`) | yes (not compiled on the machine that wrote it) |
| The native page host's socket (`metaverse-engine/host/native-socket.coax`) | yes: a message whose first byte is 0x80 to 0xBF goes out as a binary message (`socket-outbound.coax`), and a binary message heard is handed to the page as it came, with no UTF-8 check (`socket-stream.coax`) |
| The fleet's bots (`coax/web/room/bots.coax`) | yes; `OA_BOTS_WIRE=text` or `packed` for the text forms |
| The protocol checks that sit a seat by hand (`coax/web/room/*-check.coax`, `drift-bots.coax`, `load.coax`) | no: they send no flag and speak text |

## Sizes

| Frame | Text | Binary |
| --- | --- | --- |
| `I 41 0 100 0 0 -15708 1 900` | 27 bytes | 11 bytes |
| `x 7` (an entry, with its separator or length) | 4 | 4 |
| `e hit 1002 1003 7 1` | 19 | 7 |
| `D 900 41 0 0 ` (the header) | 13 | 5 |
| A packed row of 60 bits in a run | 12 (`S`, 10 characters, `;`) | 8 |

## Measurements

To come: the 150-seat zone soak, text against binary, on one host.

## Adding a kind

Write and read it with the text codecs as before. Add a row at the end of `WT` (a kind's
number is its row and never changes; a kind that goes keeps its row), raise `WT_KINDS`, run
the check with `--write`, and commit the profile with the code. `coax/bin/coax test everglade/shared/everglade-wire.coax --only "binary wire"`
holds every writer in that file to its row.

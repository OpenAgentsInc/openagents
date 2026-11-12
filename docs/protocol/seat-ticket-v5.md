# Seat ticket v5

A seat ticket says who sits in an Everglade seat. The API signs one at
`POST /api/playtest/room-ticket/ROOM` and the page joins a room with it as
`?t=TICKET`. Version 5 (#13260) is signed with the API's private key. A room
checks it with public keys only, so nothing a room holds can sign a ticket,
the admin bit included. Versions 2, 3 and 4 remain unchanged: they are HMACs
under the shared `OA_ROOM_TICKET_KEY`, and a room still takes them while it
holds that key.

The machine-readable contract and its fixtures are
[`seat-ticket-v5.json`](../../proto/verse/seat-ticket-v5.json). The one
writer and the one reader are
[`aethyr/session/seat-ticket.coax`](../../aethyr/session/seat-ticket.coax):
the API calls `seat_ticket_v5_write` through `coax/web/api/ticket-sign-unit.c`,
and the room calls `tk5_read` through `tk_check_keys` in
`coax/web/room/room-ticket.coax`. No C builds or parses a v5 ticket.

## Format

```
v5.KID.PID.ACCT.NAME64.ROOM.SHARD.EXPIRES.HP.EXT.SAVED.SIG
```

Twelve fields joined by `.`, at most 4096 bytes in all.

| Field | Value |
| --- | --- |
| `KID` | The signing key's ID: the first 8 lowercase hex digits of its x-only public key |
| `PID` | The player's ID: 1 to 40 of `[a-z0-9-]` |
| `ACCT` | `0` a guest, `1` an account, `2` an account who is one of the realm's admins |
| `NAME64` | The display name in unpadded canonical base64url, or `-` for none. The name is 1 to 24 letters, digits, `-`, `_` and inner spaces |
| `ROOM` | The room's code: 1 to 24 of `[a-z0-9-]` |
| `SHARD` | The room shard that seats it: 1 to 99, no leading zero. The API writes 1 when it placed no shard |
| `EXPIRES` | Wall-clock milliseconds, 1 to 15 digits |
| `HP` | The saved-state field of versions 2 and 3: the HP or `-`, then optionally `x` and the XP, `w` and the WANTED level (1 to 5), `g` and the owner generation |
| `EXT` | `-`, or 1 to 8 signed `NAME=VALUE` items joined by `,` (see below) |
| `SAVED` | `0`: no character-checkpoint profile (schema 1). `-`: profile 2 with no capture yet (a first join). Otherwise the stored capture in unpadded base64url (profile 2), as version 4 carries it. With `-` or a capture, the owner generation is at least 1 |
| `SIG` | 128 lowercase hex digits: the BIP-340 Schnorr signature of every byte before `.SIG` |

The writer canonicalizes the HP field as the API always has: `x` appears
when the XP is above 0 or a WANTED level follows (`x0w2`), `w` only for 1 to
5, and `g` only for a generation above 0.

## Checking

A room holds one or two public keys (`OA_ROOM_TICKET_PUBKEYS`: one x-only key
in lowercase hex, or two joined by `,`). It takes a v5 ticket when all of
these hold:

1. `KID` names one of its keys, and `SIG` verifies under that key.
2. `ROOM` is the room asked and `SHARD` is this shard (`room-relay.coax`).
3. The ticket has not expired, and it ends no later than the API ever signs
   (10 minutes, plus a minute for clock skew).
4. Every field has its format. A field out of its format is refused even
   when the signature verifies.

With two keys, a ticket under either one seats. That is a rotation's overlap
(see [room keys](../deployment/room-keys.md)).

## Extension items

`EXT` carries signed facts that later changes add without a new version.
Names are 1 to 8 of `[a-z]`, in strictly ascending order; values are 1 to 24
of `[a-z0-9-]`. Version 5 defines `c` and `l` (below). A reader ignores a name it does
not know, so the API can sign a new item before every room reads it, and
rooms and the API deploy in either order. A room that needs an item treats a
ticket without it as its own rule decides; the format never makes it
optional or required.

### `c` and `l`: the character's class and level (#13259)

| Item | Value |
| --- | --- |
| `c` | The class the API keeps for the character (`verse.room_vitals.character_class`, migration 0160): `srd-` and 3 to 12 lowercase letters, such as `srd-fighter`. Left out when the API keeps none yet. |
| `l` | The character's level, one digit 1 to 6: the level its stored XP gives through the one table (`aethyr/rules/srd/advancement.coax`), at least the realm's starting level (`OA_ROOM_START_LEVEL`, 1 to 6, default 1). |

The API writes them with `seat_ticket_v5_class_ext`
(`aethyr/session/seat-class.coax`, through `stk_class_ext_c`). An Everglade II
room seats a ticketed player as the ticket says:

- The hello's level is never taken.
- The hello's class is taken only when the ticket has no `c` (the character's
  first seat). The room then reports that class on its signed vitals line as
  ` c=CLASS`, and the API keeps it only if it keeps none (`verse.room_class_once`).
- Once a seat sits, a hello changes neither class nor level, and Change
  character (`V`) changes only the look. A seat's level rises only when its XP
  crosses a level of the table.
- A v2, v3 or v4 ticket has no `EXT`. The room still takes the class from the
  first hello and fixes it for the seat. The level comes from the ticket's
  signed XP. These tickets are never refused for this.
- The page decides class and level only where it is the only party: a ticket
  the API signed for one of the realm's admins (ACCT 2), the local API
  stand-in's admin (`room-dev.coax`), and a room with no ticket keys (the
  page's own worker room and the checks). See `room-everglade.coax`
  `eg_page_decides`.

To add another item, document its name and value here, write it
in the API's call to `seat_ticket_v5_write` (the `ext` argument, which the
writer checks), and read it in the room with
`tk5_ext(cast(string) t.ext[0..<t.ext_n], "c")`. Add a fixture for it to the
JSON profile.

## Check

```sh
coax/bin/coax test aethyr/session/seat-ticket.coax
coax/web/tool.sh coax/web/room/seat-ticket-profile.coax proto/verse/seat-ticket-v5.json
```

The first runs the writer's and reader's checks, including forged bytes in
the signed fields and the signature. The second writes each fixture's values with the
fixture's throwaway keys and zero auxiliary randomness, compares the bytes,
reads each back with the room's keys and refuses each `refused` entry. The
fixture signatures were also verified once with an independent BIP-340
implementation (`coax/web/room/seat-ticket-oracle.py`).

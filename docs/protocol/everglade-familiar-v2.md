# Everglade II familiars and Healer's Kit v2

Since [Everglade session v7](everglade-session-v7.md), a familiar is a row
of the session wire (kind 15). This profile pins what a player asks of a
familiar or the Healer's Kit, and what the room answers. Profile:
[`everglade2-familiar-v2.json`](../../proto/verse/everglade2-familiar-v2.json).
Codec: `everglade/server/familiar-wire.coax`. The page writes the requests
and reads the answers with it, and the room reads the requests and writes
the answers with it.

## The row

A familiar that is out has one row, in its owner's slot. Its id is
`200000 + 970200 + SEAT`, where SEAT is its owner's seat.

| Field | Meaning |
| --- | --- |
| pose | Where it stands. It has no velocity: a step is one square at once |
| `owner` | Its owner as a target code, `1000 + SEAT` |
| `p1` | Its form: an index into the SRD's familiar forms |
| `hp`, `max` | Its Hit Points |
| `g` | Whose senses its owner has: 0 their own, 1 the familiar's sight, 2 its sight and hearing |

A familiar that is pocketed, released or dead has no row. When it comes
back, its row has a new `life`, so a page never draws it walking from where
it was. Its owner always hears its row. A page that blocks its owner never
does.

## Requests

A request is the session's use frame, `E SEQ KIND ID`. SEQ rises with each
request and is above 0.

| KIND | Request | ID |
| --- | --- | --- |
| 9 | Summon a companion (the Druid's Wild Companion) | `FORM + 100 * SLOT` |
| 10 | Begin the Wizard's Find Familiar | `FORM + 100 * SLOT + 10000 * SPIRIT`. SLOT 0 is the ritual. SPIRIT is 1 fey, 2 celestial, 3 fiend |
| 11 | Pocket, recall or release your familiar | 1 pocket, 2 recall, 3 release |
| 12 | Step it one square the way you face | 0 |
| 13 | Have it deliver Cure Wounds | The seat of the player to cure |
| 14 | Borrow its senses | 0 |
| 15 | Use the Healer's Kit on the nearest dying ally | 0 |

The room decides every request from its own rules and positions. A page
supplies no place, target position or count.

## Answers

An answer is an `e` entry of the `D` frame. The room sends it to the asker
alone, wherever they stand, on the reliable event class and outside the cue
budget. On WebTransport it goes out of band on the combat stream.

| Entry | Fields |
| --- | --- |
| `e fam` | `SEQ ORDER FORM`: request SEQ was done. ORDER 0 is a summons, with the FORM summoned. ORDER 1 to 3 is the lifecycle's, 4 the step, 5 the cure and 6 the senses, each with FORM 0 |
| `e fam-cast` | `SEQ STATUS`: the Wizard's cast. STATUS 1 begun, 2 complete, 3 ended without payment |
| `e kit` | `SEQ LEFT`: the kit was used. LEFT is its uses left, from 0 to 9 |

A refused request gets the session's `r SEQ familiar` or `r SEQ kit`.

Whose senses you have is not an answer. It is state, so it rides your
familiar's row (`g`).

A reader takes a request or an answer whole or not at all: a missing, extra
or out-of-range field drops it.

```sh
coax/web/tool.sh coax/web/room/familiar-profile.coax proto/verse/everglade2-familiar-v2.json
```

## Version 6

Before session v7 these were frames of their own: `M1`, `M3` to `M7` and
`U1` from the page, and `m1` to `m8` and `u1` from the room. Their profiles
are [the familiar frames](everglade-familiar-v1.md) and
[the Healer's Kit v1](everglade-healers-kit-v1.md).

A v7 room still seats a v6 page for one release (#13398). To that page it
speaks these frames, by the seat's hello: it reads its `M` and `U1` requests,
answers in `m1` to `m8` and `u1`, and sends it no familiar's row. The rules
behind a request are the same either way. Those writers and readers are
[the previous wire](everglade-previous-wire.md)
(`everglade/server/wire-prev.coax`), and they go when the wire rises again.

One rule changed with the rows. `m2` kept a familiar from a page its owner
blocks as well as from a page that blocks its owner. A row follows the row
layer's one rule: who sees a familiar is who sees its owner.

# 81. Reply with a realm

> **Status: unimplemented — design draft.** Today one realm answers a post:
> in Everglade you paste a link or type a line, a house takes the words as
> its sign, and a seeded Meteor Swarm smashes it
> ([docs/verse/replies.md](../../docs/verse/replies.md)). The flow lives in
> Everglade's own files (`everglade-reply.coax`, `coax/web/api/replies.c`,
> `migrations/0076_realm_replies.sql`), and only Everglade can do it. This
> chapter plans replies as an Aethyrforge feature that any realm can offer.
> Nothing in `aethyr/` runs yet.
>
> A realm made from a prompt now keeps the shape the words describe: "a stone
> courtyard with a fountain" makes stone walls round a paved yard with the
> fountain in the middle ([realm-builder.md](../../docs/verse/realm-builder.md),
> "The site"). A reply that makes a realm would use the same board.

A reply is a very small realm document: one realm, one place, a few words
and a seed. Its link replays the same moment for anyone who opens it. This
chapter turns Everglade's reply into a typed record that rides on the realm
document of [chapter 51](ch051-a-realm-is-a-document.md), is published like
[chapter 54](ch054-publish-a-realm.md)'s frozen records, and replays from
its seed through [chapter 5](ch005-replay-a-session-exactly.md).

## What this chapter teaches

- How to describe a reply as a bounded record: realm, place, kind, words
  and seed.
- How to keep only what the player typed, and never fetch a pasted link.
- How a seed makes the reply's event replay the same for every viewer.
- How a frame from the replay becomes the reply's preview card.

## Outline

1. **The record.** Fields from today's table `verse.replies`: `id` (12
   lowercase letters and digits), `realm`, `house` (0–15, an
   `EG_BUILDING` index), `kind` (`text` or `link`), `words` (1–140
   characters for a line, at most 280 for a link, no control characters
   and no `|`), `seed` (1 to 2,147,483,647) and `player`. The chapter
   generalizes `house` to a placement ID from the realm document.
2. **What is kept.** One line of plain characters, with runs of space
   folded to one. A link is text: the server and the page never fetch,
   unfurl or follow it, and no author data is kept. Words are escaped in
   the HTML, in `index.md`, and drawn in the realm's own stroke font.
3. **The event.** Each realm declares a reply event. Everglade's is the
   spell bar's Meteor Swarm (SRD 5e), its dice drawn from
   `verse/rules/srd5.coax` `srd_dice` with the reply's seed. The event
   reads only the record and the seed, so its outcome is fixed.
4. **The routes.** `POST /api/playtest/replies {realm, text | link,
   seed}` returns `{id, path, page}` or 400, 401, 403, 413 and 429 (40 a
   player a day, `OA_REPLIES_PER_DAY`). `GET /api/playtest/replies/<id>`
   returns `id=`, `realm=`, `house=`, `seed=`, `kind=` and `text=` lines.
5. **The page and its card.** The page is written to the site's bucket at
   `realms/<slug>/reply/<id>` with `og:` tags and `noindex`. The card is a
   1200x630 JPEG under 300 KB (`CARD_MAX`) taken from the frame of
   impact (`coax/web/card.js` `startMoment`), uploaded by its maker only.
6. **Replay on open.** The link opens the realm, places the viewer at the
   recorded spot, and replays the event a few seconds after the realm is
   in, then offers **WATCH AGAIN** and **REPLY WITH YOUR OWN**.

## What exists today

- [docs/verse/replies.md](../../docs/verse/replies.md): the flow, the kept
  fields and the routes.
- [coax/web/api/replies.c](../../coax/web/api/replies.c): `make`, `card`,
  `write_page` and `replies_route`.
- [coax/web/api/migrations/0076_realm_replies.sql](../../coax/web/api/migrations/0076_realm_replies.sql):
  the table and its checks.
- [apps/openagents-web/everglade-reply.coax](../../apps/openagents-web/everglade-reply.coax):
  the REPLY panel, the sign, the swarm and the `reply:` tests.
- [coax/web/api/test_reply_realm.py](../../coax/web/api/test_reply_realm.py):
  route checks, such as a reply with no player key or from another site.

## What it builds

- `aethyr/forge/reply.coax`: the reply record, its limits and its encoding
  as the text lines the page reads.
- A `reply` section in the realm document of chapter 51: the places a
  realm offers and the event it plays.
- A shared reply panel in the UI of [chapter 15](ch015-ui-and-the-hud.md),
  so no realm keeps a private copy.

## Acceptance checks and oracles

The chapter is done when:

- Two realms offer replies through the same module, and Everglade's
  existing `reply:` tests and `test_reply_realm.py` still pass.
- A reply opened in two browsers and natively draws the same event: the
  session digests of chapter 5 match at every step.
- Words over the limits, control characters, `|`, and a link with spaces
  or quotes are refused with the same 400 the route returns today.
- No request leaves the server or the page for a pasted link; a test
  counts outgoing requests during a reply and finds none.
- The card is the frame of impact, under 300 KB, and only its maker can
  upload it.

## Dependencies

Chapters [5](ch005-replay-a-session-exactly.md) (replay from a seed),
[6](ch006-entities-are-born-and-die.md) (stable IDs for places),
[15](ch015-ui-and-the-hud.md) (the panel and stroke font),
[51](ch051-a-realm-is-a-document.md) (the realm document) and
[54](ch054-publish-a-realm.md) (published records). Coax:
[chapter 18](../../coax/book/ch18-protocol.md) for the record at the
protocol boundary.

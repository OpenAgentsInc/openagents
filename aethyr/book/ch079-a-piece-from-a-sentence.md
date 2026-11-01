# 79. A piece from a sentence

> **Status: unimplemented — design draft.** Today two paths turn words into
> world content. The Sandbox's set pieces
> ([`verse/setpiece.coax`](../../verse/setpiece.coax),
> `POST /api/playtest/sandbox/piece` in
> [`coax/web/api/npc.c`](../../coax/web/api/npc.c)) make one piece from one
> sentence. Aethyrforge's board
> ([realm-builder.md](../../docs/verse/realm-builder.md)) makes a whole realm
> from a prompt, from the catalogue in migration `0048_realm_builder.sql`.
> They keep separate catalogues and checks. This chapter plans one catalogue
> and one selector path for both. Nothing in `aethyr/` runs yet.
>
> Since migration `0097_builder_layout.sql`, the board's selector also plans
> a place built round a middle as four typed features: `enclosure`,
> `centre`, `layout` and `path`. Each takes its kind in a field of its own,
> because the model API refuses one enum of more than about 90 values. A
> layout eval (`coax/web/realm-layout-eval.coax`) scores what is laid out
> against 14 held-out prompts: 49% before, 95% after.

You type or say what you want, such as "a flock of birds follows me", and
the realm gets it. The words never reach the engine. A model picks one entry
from a fixed catalogue under a schema, the server checks the answer, the
game checks it again, and a seeded generator lays it out. This chapter
builds that path.

## What this chapter teaches

- How a catalogue of pieces, with bounded fields, defines what words can
  make.
- How a typed selector picks from the catalogue, with no keyword matching.
- How to hold an answer to the catalogue twice, on the server and in the
  game.
- How a seed lays a piece out the same way every time.
- How to keep what was asked for but not built, so nothing is dropped.

## Outline

1. **The catalogue.** Each piece has fields with ranges and defaults. The
   Sandbox has four: `flock` (count 3–12, default 7; kind crow or bat),
   `lanterns` (count 4–16, default 8; colour; size), `grove` (count 3–20,
   default 9; kind round, pine, dead or mixed; size) and `pond` (size). The
   board's catalogue has about twenty features, from `land` to `village`
   and `tool`. Aethyr keeps one catalogue table as data, read by the server
   schema, the board labels and the game.
2. **The selector.** The server asks the model for one entry under a
   response schema whose enums are the catalogue's words. Status codes: 400
   no sentence, 422 nothing fits, 429 one at a time (three seconds between a
   player's pieces), 502 an answer that isn't a piece, 503 no answer.
   `OA_PIECE_DAILY_CAP` (2,000) applies across the server.
3. **Holding an answer twice.** A count out of range is held to its range; a
   missing count is the default. A kind, colour or size that isn't the
   piece's is the piece's default. An unknown piece is no piece (-1). The
   server does this in SQL and the game does it again in `verse_piece_read`.
4. **Layout from a seed.** `verse_piece_layout` lays a piece out in front of
   you, clear of your feet, as placements of `PLACE_FLOATS` floats each
   (mesh, position, yaw, scale, colour, glow, motion, solid). A flock's
   placements are offsets round whoever walks. The same seed gives the same
   layout. The board's realms use the same rule through
   [chapter 51](ch051-a-realm-is-a-document.md)'s `layout`.
5. **One undo step.** A piece arrives as one edit
   ([chapter 52](ch052-edit-a-realm-in-place.md)): one undo step and one
   publish.
6. **What was asked for.** The board names every thing asked for in
   `asked`, with the feature that makes it or none. A thing with no feature,
   or one a feature only stands in for (crows for owls), stays as a `want`
   line, at most 24 per realm. `coax/web/api/eval-builder.sh` scores forty
   prompts for coverage: 100% of asked things shown or listed is the rule.
7. **Setup fields.** Camera, style, ground and time of day are fixed fields
   chosen before the prompt (`POST /api/playtest/realms/pick`), and the
   model's board is held to them.

## What exists today

- [verse/setpiece.coax](../../verse/setpiece.coax): the Sandbox catalogue,
  `verse_piece_read`, `verse_piece_layout` and their checks.
- [coax/web/api/npc.c](../../coax/web/api/npc.c): the set piece route and
  the spoken realm edit route.
- [coax/web/api/migrations/0036_sandbox_pieces.sql](../../coax/web/api/migrations/0036_sandbox_pieces.sql):
  `verse.piece_catalogue()` and the record of every sentence.
- [coax/web/api/migrations/0048_realm_builder.sql](../../coax/web/api/migrations/0048_realm_builder.sql):
  `verse.builder_catalogue()`.
- [docs/verse/realm-builder.md](../../docs/verse/realm-builder.md): the
  board, setup, asked and wanted lines, and limits.

## What it builds

- `aethyr/forge/catalogue.coax`: one catalogue table with field ranges and
  defaults, generated into the server schema.
- `aethyr/forge/select.coax`: reading and holding a selector's answer.
- `aethyr/forge/piece.coax`: seeded layout of a piece as one edit.

## Acceptance checks and oracles

The chapter is done when:

- The server schema, the board labels and the game read one catalogue, and
  a check fails if any copy differs.
- Every out-of-range or unknown field in a fixture set is held to the same
  value on the server and in the game.
- The same piece and seed give byte-identical placements natively and in
  the browser.
- No route in the path matches words of the sentence, checked by review of
  the route list.
- The forty-prompt evaluation keeps 100% of asked things shown or listed.
- A piece is undone in one step and leaves the realm byte-identical to
  before.

## Dependencies

Chapters 6 (entities), 16 (budgets: 200 placed things a realm), 51 (the
realm document and `layout`), 52 (edits and undo) and 53 (the meshes a piece
uses).

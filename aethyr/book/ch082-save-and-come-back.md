# 82. Save and come back

> **Status: unimplemented — design draft.** Today
> [`apps/openagents-web/save.coax`](../../apps/openagents-web/save.coax)
> keeps a realm's save as a versioned `AESV` container with a CRC-32, in
> two slots, in the browser's IndexedDB and, for a signed-in player, on the
> server (`/api/playtest/progress`). The Garden, the Haunted House and the
> made realms load it; Fairy Forest packs its own payload
> (`verse/fairy-forest/life.coax` `ffl_save`). Each realm writes its own
> payload by hand. This chapter plans one save layer in Aethyr. Nothing in
> `aethyr/` runs yet.

A save keeps what a player made or reached, so that a reload or a later
visit brings it back. It is not a replay: a replay keeps inputs and
recomputes ([chapter 5](ch005-replay-a-session-exactly.md)), and a save
keeps a chosen part of the state. This chapter makes that part declared,
versioned and checked, and keeps the rules that `save.coax` already
enforces.

## What this chapter teaches

- How to declare which state a realm saves, and leave the rest to its
  defaults ([chapter 3](ch003-state-starts-from-its-defaults.md)).
- How the container, its checksum and its two slots protect a save from a
  broken write.
- How to migrate an old save forward, and refuse a save from a newer
  build.
- How the browser copy and the server copy agree.

## Outline

1. **The container.** Bytes as `save.coax` writes them: `"AESV"`, container
   version `SAVE_VERSION` (2), layer `SAVE_PROGRESS` (2), two reserved
   bytes, the schema as a little-endian `u32`, a varint and the world ID, a
   varint and the payload, then a CRC-32 (polynomial `0xEDB88320`) of
   everything before it.
2. **Two slots.** The record is `verse-save 1`, then the current save's
   time and container in base64, then the previous one's. A load tries the
   current save, then the previous one, so a corrupt write falls back to the
   last good save.
3. **Declared save fields.** A realm marks the fields it saves; Aethyr
   derives the payload from them in a fixed order. Entities in a save use
   the durable `EntityId`s of [chapter 6](ch006-entities-are-born-and-die.md),
   never step handles.
4. **Schemas and migrations.** Each payload has a schema number. A save
   with an older schema runs through the realm's migrations in order (today
   a bare-hex house save opens as schema 0). A save with a newer schema than
   the build knows is refused, never guessed at.
5. **Slots per realm.** The realm host names one slot per realm
   (`realms.json` `slot`), and `slot_was` reads an older slot once when the
   new one is empty, as the Haunted House does with `realm`.
6. **Browser and server.** The page keeps the record in IndexedDB; the
   server checks the same container (`container_ok` in
   `coax/web/api/player.c`, at most 16 MiB) and answers 409 when a later
   save is already there. The newer of the two wins.
7. **When to save.** A realm saves on a change and at a fixed interval (the
   Garden's `KEEP_EVERY` is 1.0 s), never inside a step, and on
   `pagehide`.

## What exists today

- [apps/openagents-web/save.coax](../../apps/openagents-web/save.coax):
  `save_pack`, `save_read`, `save_pick` and `save_record`.
- [apps/openagents-web/garden.coax](../../apps/openagents-web/garden.coax):
  the vase's save, its restore, and a test that refuses another world's
  save.
- [verse/fairy-forest/life.coax](../../verse/fairy-forest/life.coax): what
  you restored in Fairy Forest, packed by `ffl_save` and read by
  `ffl_open`.
- [coax/web/api/player.c](../../coax/web/api/player.c): the progress routes
  and `container_ok`.
- [coax/web/place-reload.ts](../../coax/web/place-reload.ts): a reload in
  each walkable realm keeps your place.

## What it builds

- `aethyr/save.coax`: the container, the two slots, schema migrations and
  the refusal of newer schemas, moved from `save.coax`.
- Derived save payloads from declared fields, with a layout fingerprint
  per schema.

## Acceptance checks and oracles

The chapter is done when:

- Every realm that saves today loads its existing saves through
  `aethyr/save.coax`, and no realm keeps its own container code.
- A save with one flipped byte, a truncated save and another world's save
  all fall back to the previous slot or the defaults, and each case has a
  test.
- A save written by schema N opens in a build with schema N + 1, and a
  save from schema N + 1 is refused by a build with schema N.
- `place-reload.ts` passes for every walkable realm.
- A save and its restore give the same digest of the saved fields, natively
  and in the browser.

## Dependencies

Chapters [3](ch003-state-starts-from-its-defaults.md) (defaults),
[5](ch005-replay-a-session-exactly.md) (digests),
[6](ch006-entities-are-born-and-die.md) (durable IDs) and
[51](ch051-a-realm-is-a-document.md) (the realm document, which a save never
replaces). Coax: R5 derivation and R16 snapshots in the
[roadmap](../../coax/ROADMAP.md), and
[chapter 18](../../coax/book/ch18-protocol.md) for versioned records.

# 81. Save and come back

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
- How the browser copy and the server copy detect a conflict by revision,
  not by clock time.

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
   `coax/web/api/player.c`, at most 16 MiB). Today it answers 409 when a
   later save is already there, and the newer of the two wins. Aethyr
   replaces "newer wins" with revisions; see
   [Conflicts are found by revision](#conflicts-are-found-by-revision).
7. **When to save.** A realm saves on a change and at a fixed interval (the
   Garden's `KEEP_EVERY` is 1.0 s), never inside a step, and on
   `pagehide`.

## Conflicts are found by revision

> **Design draft.** Not implemented. Today's rule compares save times, so a
> device whose clock runs ahead wins, and an offline edit can overwrite
> progress made on another device.

The design never orders saves by clock time. A save's time is shown to the
player and isn't used for any decision.

### Revisions

Each slot on the server has a `revision`, a `u64` that starts at 0 and that
only the server increments. Every save the client sends carries:

- `base_revision`: the server revision the client last loaded or had
  confirmed.
- `session`: a random ID chosen when the page or app starts.
- `sequence`: a counter of saves sent by that session, starting at 1.

The server accepts a save only when `base_revision` equals its current
`revision`. It then stores the save, increments `revision` and returns the
new value. Otherwise it answers 409 with its current save and revision, and
changes nothing. A save repeated with the same `session` and `sequence` is
answered with the result of the first attempt, so a retry after a lost reply
is never applied twice.

### What the client does with a 409

1. If the client has no changes since `base_revision`, it adopts the server's
   save and revision.
2. If both sides changed, the client has a conflict. A realm that declares a
   merge for its saved fields (for example, a set of collected items that
   merges by union) merges the two, then sends the result with the server's
   revision as `base_revision`.
3. A realm without a merge keeps both copies. The server copy stays current.
   The local copy is kept as a conflict copy in a separate slot, and the
   player chooses which to keep. Nothing is discarded without the player's
   choice.

### The three cases

- **Skewed clocks.** Device A's clock is an hour ahead of device B's. B saves
  revision 5 → 6. A, which loaded revision 5, saves with `base_revision` 5 and
  gets 409. A's later clock time has no effect.
- **Offline edits.** A device loads revision 5, goes offline and saves three
  times locally. Each local save keeps `base_revision` 5. Meanwhile another
  device saves revision 6. When the first device reconnects, its first send
  gets 409, and the client handles the conflict as above.
- **Two sessions at once.** Two tabs load revision 7. The first tab's save is
  accepted as revision 8. The second tab's save, with `base_revision` 7, gets
  409. The second tab adopts revision 8 if it had no changes, and otherwise
  merges or keeps a conflict copy.

### Checks for conflicts

- With a device clock set an hour ahead, its stale save is refused with 409
  and doesn't replace a save made at an earlier clock time.
- An offline device's queued saves produce exactly one conflict on
  reconnect, and the other device's progress is still present afterwards.
- Two sessions saving from the same revision produce one accepted save and
  one 409. The refused session's changes end up merged or in a conflict
  copy, never lost.
- A save resent with the same `session` and `sequence` increments the
  revision once.
- A client with no local changes adopts the server's save after a 409
  without asking the player.

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
- The checks for conflicts pass, and no decision depends on a save's clock
  time.

## Dependencies

Chapters [3](ch003-state-starts-from-its-defaults.md) (defaults),
[5](ch005-replay-a-session-exactly.md) (digests),
[6](ch006-entities-are-born-and-die.md) (durable IDs) and
[51](ch051-a-realm-is-a-document.md) (the realm document, which a save never
replaces). Coax: R5 derivation and R16 snapshots in the
[roadmap](../../coax/ROADMAP.md), and
[chapter 18](../../coax/book/ch18-protocol.md) for versioned records.

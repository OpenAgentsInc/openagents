# A world's document v1: a World saved whole

Issue: [#13412](https://github.com/OpenAgentsInc/openagents/issues/13412).
Code: `metaverse-engine/editor/world-doc.coax` (`world_save`, `world_load`).
Profile: `proto/world/doc-v1.json`, checked by
`coax/web/tool.sh metaverse-engine/tools/world-doc-profile.coax proto/world/doc-v1.json`.

A world with an ID is a document. The document is the engine's World
(`metaverse-engine/core/world.coax`) written as text, with what an editor needs
beside it.

```text
world.doc.v1 splice                     the world's ID
content 2                               the content tables' revision the kinds name rows of
head 41 9                               the edit log's head, and the number the next spawned entity gets
tick 1                                  steps the World has run
counters 6 0 5 1 1 8 0 0                the entity store's counters
own 2                                   the realm's own columns, a slot
e SLOT GEN ID FLAGS HAS F*9 I*4 OWN*N   a slot ever used (flags: 1 live, 2 waiting to be born, 4 doomed)
free SLOT ...                           the free slots, in the stack's order
spawns SLOT ...                         the spawns waiting for the next flush
digest 2199881616                       world_digest, mixed with the content, the head, the next number and the own columns
```

The f32 columns are written as their bits in hex, so a value is the same on
every host.

## Rules

- **Whole.** Every slot ever used is written: its generation, EntityId and
  flags, every column, and the realm's own columns. The free list and the
  waiting spawns are written in order. A loaded World gives the next spawn the
  same handle and ID the saved one would.
- **Deterministic.** Equal Worlds write equal bytes. A loaded World writes the
  bytes it was loaded from.
- **Checked.** `world_load` checks the document's form before it changes
  anything. A document refused there leaves the World as it was. Then it loads
  the document and compares the digest. A document that does not give its own
  digest is refused and leaves the World empty.
- **Numbers are never given twice.** `head` carries the next entity number, so
  a piece removed and brought back by an undo keeps its number, and a new
  piece never takes it.
- **The systems list is code.** A load keeps the list it finds.

## Splice

Splice's document is the game's World: the designed world's colliders and
every placed piece, with five own columns (the shape's middle and half sizes,
and the shape). The server only stores the document a fresh game builds from
the layer's pieces (`spw_build`). It refuses a document that this build would
not have written, such as one with a wall moved (`spw_open`).

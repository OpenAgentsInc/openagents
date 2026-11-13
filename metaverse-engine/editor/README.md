# metaverse-engine/editor

The Metaverse Engine's realm editor: the engine-side code that edits a realm document.
The product spec, what is on screen and what to build, is
[docs/metaverse/aethyrforge.md](../../docs/metaverse/aethyrforge.md).

- `edit.coax`: typed edits to a made realm's document, every revision path in one log:
  moves, turns, sizes and removals of entities, and slot edits for rows, tools, setup,
  lore, quests and placed things; inverses, undo, redo, replay, saves fenced by revision.
  Book chapters 51, 52 and 81.
- `doc.coax`: a document (GET .../doc) read into the model; older versions moved forward,
  newer or broken ones refused whole (the last stays).
  `coax/bin/coax test metaverse-engine/editor/doc.coax --library`
  `coax/bin/coax test metaverse-engine/editor/edit.coax --library`
- `publish.coax`: the engine a published made realm plays, kept as a manifest of its
  content-named files and pinned with the revision (`#authority`; the server's rule is
  migration 0107). Book chapter 54, #12629.
  `coax/bin/coax test metaverse-engine/editor/publish.coax --library`
- `pack.coax`: a realm's asset pack (`VPK1`) read whole or refused (budget, layout, a declared
  licence on every asset), written byte for byte as `metaverse/tools/pack_realm.py` writes it, and
  admitted into a realm's assets by content (`#authority`). Book chapter 53.
  `coax/bin/coax test metaverse-engine/editor/pack.coax --library`
  `coax/web/tool.sh metaverse-engine/tools/pack-check.coax PACK...` (the committed packs, written again)
- `authority.coax`: the three kinds of state (authored, running, durable), which operation may
  write which, and who decides each in a realm (`#authority`; INVARIANTS.md, "Authoring never
  writes durable state"; #13431). An edit, undo, content rollback or world load never writes the
  character record or the item ledger.
  `coax/bin/coax test metaverse-engine/editor/authority.coax --library`

- `world-edit.coax`: a world's edits on the wire (`world.edit.v1`, #13412): one typed edit
  (spawn, despawn, set a column, move, rotate, scale, set a content cell, shape the land), the
  batch that names its revision, the event a saved edit becomes and what the API answers. It
  carries an entity edit out on the scene's log, and a cell edit on a content document's log,
  and answers the inverse. [docs/protocol/world-edit-v1.md](../../docs/protocol/world-edit-v1.md).
  `coax/bin/coax test metaverse-engine/editor/world-edit.coax --library`
- `world-doc.coax`: a World saved and loaded whole (`world.doc.v1`): `world_save` and
  `world_load`, deterministic, with a digest check. [docs/protocol/world-doc-v1.md](../../docs/protocol/world-doc-v1.md).
  `coax/bin/coax test metaverse-engine/editor/world-doc.coax --library`
- `op-result.coax`: one result for every realm operation (`realm.result.v1`) and its refusal
  codes. [docs/protocol/realm-result-v1.md](../../docs/protocol/realm-result-v1.md).
  `coax/bin/coax test metaverse-engine/editor/op-result.coax --library`
- `world-follow.coax`: a page's place in a world: the lane it joined, the revision it stands at
  and its one request at a time. It reads the document once, then takes the lane's events in
  order. The realm carries each out on its own World.
  `coax/bin/coax test metaverse-engine/editor/world-follow.coax --library`
- `world-live.coax`: a world's dev and live revisions, and whether an update reaches a session
  now or at its next session (`#authority`). [docs/protocol/world-live-v1.md](../../docs/protocol/world-live-v1.md).
  `coax/bin/coax test metaverse-engine/editor/world-live.coax --library`

The page side lives in `apps/openagents-web/` (`builder.coax` today, `myrealm.coax` as the
canvas) and the shared edit layer in `metaverse/edit/edit.coax`.

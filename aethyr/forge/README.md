# aethyr/forge

Aethyrforge, the realm creator: the engine-side code that edits a realm document.
The product spec, what is on screen and what to build, is
[docs/verse/aethyrforge.md](../../docs/verse/aethyrforge.md).

- `edit.coax`: typed edits to a made realm's document, every revision path in one log:
  moves, turns, sizes and removals of entities, and slot edits for rows, tools, setup,
  lore, quests and placed things; inverses, undo, redo, replay, saves fenced by revision.
  Book chapters 51, 52 and 81.
- `doc.coax`: a document (GET .../doc) read into the model; older versions moved forward,
  newer or broken ones refused whole (the last stays).
  `coax/bin/coax test aethyr/forge/doc.coax --library`
  `coax/bin/coax test aethyr/forge/edit.coax --library`
- `publish.coax`: the engine a published made realm plays, kept as a manifest of its
  content-named files and pinned with the revision (`#authority`; the server's rule is
  migration 0107). Book chapter 54, #12629.
  `coax/bin/coax test aethyr/forge/publish.coax --library`

The page side lives in `apps/openagents-web/` (`builder.coax` today, `myrealm.coax` as the
canvas) and the shared edit layer in `verse/edit/edit.coax`.

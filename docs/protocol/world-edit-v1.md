# A world's edits v1: one edit, one batch, one event

Issue: [#13412](https://github.com/OpenAgentsInc/openagents/issues/13412), the
browser editor's E3 ([spec](../editor/2026-11-12-browser-editor.md)).
Code: `metaverse-engine/editor/world-edit.coax` (the one codec),
`coax/web/api/worlds.c` with `coax/web/api/world-policy.coax` (the routes and
their rules, `#authority`), `metaverse/splice/world.coax` (Splice, the first
world).
Profile: `proto/world/edit-v1.json`, checked by
`coax/web/tool.sh metaverse-engine/tools/world-edit-profile.coax proto/world/edit-v1.json`.

## An edit

An edit is one JSON object. Places are whole centimetres, turns are whole
degrees (-179 to 180) and sizes are whole percent (25 to 400). An entity is
named by its ID, 1 and up. A kind is a content ID.

| Op | Bytes | What it does |
| --- | --- | --- |
| `spawn` | `{"op":"spawn","id":0,"kind":K,"x":X,"y":Y,"z":Z,"yaw":DEG,"scale":PCT}` | Makes an entity. ID 0 asks the world to number it |
| `despawn` | `{"op":"despawn","id":ID}` | Removes one |
| `set` | `{"op":"set","id":ID,"col":COLUMN,"v":N}` | Sets a column of the World: `x y z yaw vx vy vz radius height hp hp_max kind row` |
| `move` | `{"op":"move","id":ID,"x":X,"y":Y,"z":Z}` | Moves one |
| `rotate` | `{"op":"rotate","id":ID,"yaw":DEG}` | Turns one |
| `scale` | `{"op":"scale","id":ID,"scale":PCT}` | Sizes one |
| `cell` | `{"op":"cell","table":T,"row":ID,"col":COLUMN,"v":TEXT}` | Sets a cell of a content table. Column `*` sets the whole row (`ID|CELL|...`), or drops it when `v` is empty |
| `terrain` | `{"op":"terrain","brush":B,"x":X,"z":Z,"r":R,"v":N,"mat":M}` | Shapes the land: `paint`, `raise`, `lower`, `smooth` or `flatten` |

A reader refuses any bytes but the ones the writer writes: the keys in this
order, no spaces, every number in range.

A world takes the ops its own code allows and refuses the rest in words.
Splice takes `spawn`, `despawn`, `move`, `rotate`, and a `set` of `x`, `z` or
`yaw`. It refuses a size, the land and a content cell, and says why.

## A batch

```json
{"schema":"world.edit.v1","rev":41,"do":"apply","edits":[EDIT, ...]}
```

`rev` is the dev revision the batch was made from. `do` is one of:

| `do` | Edits | What it does |
| --- | --- | --- |
| `apply` | 1 to 64 | Carries the edits out, in order. One refused edit refuses the whole batch |
| `undo` | none | Takes back the newest step that stands |
| `redo` | none | Takes the next step again |
| `original` | none | Removes every placed thing. An undo brings them back |
| `publish-now`, `publish-next` | none | Makes dev live ([dev and live](world-live-v1.md)) |

Each saved edit moves the revision by one. An undo is saved as edits of its
own (the inverses), so every reader follows it the same way.

## The routes

| Route | Answer |
| --- | --- |
| `GET /api/worlds/ID?lane=dev` | 200 and the state with the lane's document |
| `GET /api/worlds/ID` or `?lane=live` | The live revision. With `&engine=E&rules=R&content=C&wire=W` the page is given the revision its build reads, or 409 to reload. 404 until a publish |
| `POST /api/worlds/ID` | A batch. 200 and the state with the batch as taken (every spawn numbered) |
| `GET /api/worlds/ID/events?lane=L&after=N` | The lane's events after revision N, one a line, 200 at most |

The state:

```json
{"schema":"world.state.v1","id":"splice","lane":"dev","rev":43,"build":[1,1,2,1],"dev":43,"live":3,"when":"now","undo":2,"redo":1,"doc":"world.doc.v1 splice\n..."}
```

`rev` is the lane's revision. `build` is the pins of the build that reads it
(engine, rules, content, wire), so every answer names the world, the revision
and the build it touched (#13437). `undo` and `redo` count the steps that can be
taken. `doc` is the lane's [document](world-doc-v1.md) on a read; `did` is the
batch as taken on a write.

A refusal is `{"error":WORDS,"edit":I}`. `edit` names the edit of the batch
that was refused, when one was.

| Status | When |
| --- | --- |
| 400 | The body is not a batch |
| 401, 403 | Nobody is logged in; the caller is no admin; the write came from another site's page |
| 404 | No world has this ID; nothing is published yet |
| 409 | The batch names another revision than the world's; the page must reload |
| 422 | An edit may not stand (the words say why, naming the zone or the place); nothing to undo or redo |

Splice (`splice`) is written by admins only. Anyone reads it.

## The server checks with the page's code

The API links the world's own Coax and runs it on every batch: Splice's check
(`sl_check`), then the walk of the whole run against the layer as it would be
saved (`sw_replay`). The page runs the same files. So the server and the page
cannot disagree about where a piece may stand.

## Events

A saved edit is the domain event `realm-authoring.world.edited.v1`, one a
revision of the dev lane, written to `verse.world_outbox` in the transaction
that changes the document:

```json
{"id":"realm-authoring.world.edited.v1","seq":912,"world":"splice","rev":42,"edit":EDIT,"at":1762190000000}
```

A reader at revision N takes the event whose `rev` is N+1 and no other
(`we_take`). A publish is `realm-authoring.world.published.v1` on the live
lane.

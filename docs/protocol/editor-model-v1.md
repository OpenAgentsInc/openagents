# What a page tells the editor shell v1 (`editor.model.v1`)

The editor shell that `?edit` opens over an engine page is a module of its own
(`metaverse-engine/editor/shell.coax`, GitHub #13416). It keeps no world. The page's realm tells it
what the world holds in this shape, and the shell draws the list of things and the selected
thing's fields from it.

- Codec: `metaverse-engine/editor/shell-model.coax`. The realm writes with `em_begin`, `em_cols`,
  `em_kind`, `em_entity`, `em_cell` and `em_no`. The shell reads with `em_read`.
- Pinned bytes: `proto/world/model-v1.json`.
- Check: `coax/web/tool.sh metaverse-engine/tools/editor-model-profile.coax proto/world/model-v1.json`.

The model never leaves the page. The browser host (`coax/web/host.js`) copies it from the page's
module to the shell's whenever the page's count changes.

## Lines

The model is lines of text, in this order.

| Line | What it says |
| --- | --- |
| `editor.model.v1 WORLD LANE REV UNDO REDO SELECTED` | The world's ID, its lane (`dev` or `live`), the lane's revision, the steps that can be undone and redone, and the selected entity's ID (0 for none). |
| `cols NAME ...` | The World columns an edit may set on an entity (`x`, `z`, `yaw` and the others in `world.edit.v1`). The shell shows a field for each. |
| `table NAME` | The content table the kinds are rows of, when its cells may be edited. The shell then shows each cell as a field, and a changed cell is a `cell` edit that names this table. Left out, cells are only shown. |
| `k KIND NAME` | A kind: its content ID and its name. |
| `e ID KIND X Y Z YAW SCALE` | An entity: its ID on the wire, its kind, its place in whole centimetres, its turn in whole degrees and its size in whole percent. |
| `c KIND COLUMN VALUE` | A cell of a kind's content row. The shell shows the selected entity's as facts. |
| `templates NAME` | The content table templates are rows of, when the realm keeps them. The shell then saves a group of things as a row and places a template as one batch. |
| `t ID NAME` then `tp ID PARTS` | A template: its row's ID and name, then its parts (`metaverse-engine/editor/place.coax`, `pl_template_write`): each a kind and where it stands from the group's middle. |
| `all N CAP` | How many things the world holds in all, and how many it takes at most (E13, #13422). A world with more things than a model lists says so, and the shell shows "256 of 1203 things listed". Left out, the model lists every thing. |
| `kinds N` | How many kinds the world has in all, when the model names only some. The shell's Add then says how many of them it lists. |
| `local` | The world is kept on this device and has no live lane. The shell shows no Publish. |
| `land RMIN RMAX` then `m ID NAME` | The world takes `terrain` edits: the smallest and the widest brush in centimetres, then each material a brush paints (its number in the edit and its name). The shell then has a Land tool with the brush's kind, material, radius and strength. |
| `plans NAME`, `clock NAME WHEN` | The world keeps NPCs' plans by time of day (#13466, `metaverse-engine/ai/npc-plan.coax`): the content table that holds them, and the table whose row 1 holds the editor's time of day with that time (`story` is the realm's own clock, else `dawn`, `day`, `dusk` or `night`). The shell then shows a time-of-day control and an Actors list. Every change is a cell edit of those tables. |
| `a ID NAME` | An NPC with plans: its number in the realm and its name. |
| `p ID WHEN DO SPEED NOTICE ON ROUTE` | NPC ID's plan at that time of day: what it does (`stand`, `dance`, `sleep`, `guard`, `patrol`, `wander`, `follow`), its speed in centimetres a second, how far it notices in centimetres, what it does on noticing (`chase`, `call`, `flee`, `ignore`) and its route (`X Z, X Z`, `X Z R` for wander, or `foe N` for follow). These are the plan row's cells in order. |
| `as ID` | The selected NPC. The page draws its route and its notice range. Left out: none. |
| `scripts NAME` | The world runs realm scripts (#13467, [realm-script-v1.md](realm-script-v1.md)): the content table whose rows name them. The shell's Scripts pane then runs, changes and stops a script by a cell edit of that table. |
| `sk KIND NAME` | A kind of thing a script may drive: the realm's number for it (a row's `kind K`) and its name, as a person reads it. |
| `s ID NAME HASH FOR` | A script row: its place (1 to 8), its name, its module's SHA-256 and whom it drives (`kind K`, or `who N` for the NPC with that number). These are the row's cells in order (`metaverse-engine/script/rows.coax`). |
| `atonce` | The world has one lane: an edit is what people get at once (a realm's world, #13478). The shell shows no Publish. |
| `list TABLE COLUMN TITLE` | A content table whose rows are each on or off (a realm's board, the tools its visitors get): the table, the column a switch sets, and the list's title. The shell shows the list with a switch on each row. A press is a `cell` edit of that table and column, to `0` or `1`. |
| `sw ROW ON LABEL` | A row of the list on the nearest `list` line above: its ID in the table, `0` or `1`, and its name. |
| `here X Z` | The last point tapped on the ground, in whole centimetres. Left out until a point is tapped. The shell places a new thing there. |
| `no WORDS` | The last refusal, in words. The shell shows it as a notice. |

A name, a value and a refusal are each one line of printable bytes. `em_read` refuses any bytes
but the ones the writer writes: it writes what it read and compares.

## Limits

| Limit | Value |
| --- | --- |
| Entities | 256 |
| Kinds | 64 |
| Content cells | 256 |
| Bytes in a kind's name | 40 |
| Bytes in a cell's value | 80 |
| Bytes in a refusal | 240 |
| Templates | 32 |
| Bytes in a template's parts | 480 |
| Materials | 16 |
| NPCs with plans | 16 |
| Plans | 64 |
| Bytes in a route | 160 |
| Script rows | 8 |
| Kinds a script may drive | 8 |
| Bytes in a script's name | 48 |
| Lists of switches | 4 |
| Switches, in all lists | 96 |
| Bytes in a switch's name | 48 |
| Bytes in a model | 65,536 |

## What the shell sends back

The shell asks the page to act through the host, as two message kinds:

| Kind | Body | What the page does |
| --- | --- | --- |
| 520 | A `world.edit.v1` batch made from the model's revision (`docs/protocol/world-edit-v1.md`) | Sends it as its own write. The server's answer comes back in the next model: new places, or a `no` line. |
| 521 | An entity's ID, or 0 | Selects it. |
| 527 | `actor N`, or `actor 0` | Selects NPC N (or none): the page draws its route and notice range and frames it. |
| 523 | `add KIND COUNT` | Plans where COUNT things of a kind go from "here" with the placement planner (`metaverse-engine/editor/place.coax`) and sends them as one batch. |
| 522 | Words for the viewport's tools (`metaverse-engine/editor/viewport.coax`, `vp_act`): `tool play`, `tool select`, `tool move`, `tool turn`, `tool size`, `grid 10`, `grid 25`, `grid 50`, `grid 100`, `fly on`, `fly off`, `run play`, `run pause`, `step`, `sim on`, `sim off`, and for a world whose land takes edits `tool land`, `brush paint` (or `raise`, `lower`, `smooth`, `flatten`), `radius CM`, `strength PCT`, `mat N` | Picks the tool, the grid, the camera, the clock and the land's brush. Anything else changes nothing. |

## A page that wants the shell

A page's module exports three calls and takes the two messages:

- `coaxial_edit_rev(state)`: a count that changes when the model does.
- `coaxial_edit_model(state)` and `coaxial_edit_model_len(state)`: the model's bytes.

Splice is the first page (`metaverse/splice/world.coax`, `spw_model`). WorldBuilder is the
second (`apps/openagents-web/worldbuilder-world.coax`, `wbe_model`).

Two more calls are for a page whose world is the visitor's own, kept in their browser
(WorldBuilder, #13422):

- `coaxial_edit_own()`: answers 1. The host then opens the shell for whoever asks with `?edit`,
  not only for an admin, since nobody else's world can be written.
- `coaxial_edit_room_v1()`: answers 1. The host then tells the page the part of the window the
  shell's panes leave free, as message 525 (`X0 Y0 X1 Y1` in CSS pixels), whenever it changes.
  The page lays its own interface out inside it.

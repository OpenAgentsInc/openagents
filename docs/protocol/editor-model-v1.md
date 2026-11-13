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
| Bytes in a model | 32,768 |

## What the shell sends back

The shell asks the page to act through the host, as two message kinds:

| Kind | Body | What the page does |
| --- | --- | --- |
| 520 | A `world.edit.v1` batch made from the model's revision (`docs/protocol/world-edit-v1.md`) | Sends it as its own write. The server's answer comes back in the next model: new places, or a `no` line. |
| 521 | An entity's ID, or 0 | Selects it. |
| 522 | Words for the viewport's tools (`metaverse-engine/editor/viewport.coax`, `vp_act`): `tool play`, `tool select`, `tool move`, `tool turn`, `tool size`, `grid 10`, `grid 25`, `grid 50`, `grid 100`, `fly on`, `fly off`, `run play`, `run pause`, `step`, `sim on`, `sim off` | Picks the tool, the grid, the camera and the clock. Anything else changes nothing. |

## A page that wants the shell

A page's module exports three calls and takes the two messages:

- `coaxial_edit_rev(state)`: a count that changes when the model does.
- `coaxial_edit_model(state)` and `coaxial_edit_model_len(state)`: the model's bytes.

Splice is the first page (`metaverse/splice/world.coax`, `spw_model`).

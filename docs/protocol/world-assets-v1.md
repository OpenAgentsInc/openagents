# A world's imported assets v1 (`world.assets.v1`, `world.asset.import.v1`)

The browser editor imports a dropped file into a world's asset pack (GitHub
#13420, the editor's E11). Two shapes carry it. `world.assets.v1` is the list
the asset browser shows and the answer to an import. `world.asset.import.v1`
is the body of an import.

One codec writes and reads both: `metaverse-engine/editor/asset-wire.coax`.
The pinned bytes are in [proto/world/assets-v1.json](../../proto/world/assets-v1.json).
Check them with:

```sh
coax/web/tool.sh metaverse-engine/tools/assets-profile.coax proto/world/assets-v1.json
```

## The routes

| Route | What it answers |
| --- | --- |
| `GET /api/worlds/ID/assets` | The list |
| `POST /api/worlds/ID/assets` | An import. The body is `world.asset.import.v1`. The answer is the list with a `did` line, or `422` with `{"error":WORDS}` |
| `GET /api/worlds/ID/assets/pack` | The world's import pack, a VPK1 file. `404` when the world has imported nothing |

Today only the dev server answers them (`coax/web/serve.c`, `assets_local`,
through `metaverse/splice/tools/asset-local.coax`). It keeps the pack and each
dropped file by content hash under `apps/openagents-web/build/.worlds/`. The
API has no such route yet, so a deployed editor has no Assets button.

## The list

Text, a line each. Every line ends with a newline.

```text
world.assets.v1 WORLD HASH BYTES BUDGET FILE TRIANGLES SIDE
l K NAME
a KIND NAME K FILE BYTES HASH HX HZ TOP
did KIND NAME new|replaced
```

| Line | Fields |
| --- | --- |
| First | `WORLD`: the world's ID. `HASH`: the import pack's SHA-256, or `-` when the world has imported nothing. `BYTES`: the pack's size. Then the limits, which the editor shows before a file is sent: `BUDGET` (the pack's bytes at most), `FILE` (one file's bytes at most), `TRIANGLES` (one mesh's triangles at most) and `SIDE` (a texture's width or height at most, in texels) |
| `l` | A licence an import may declare: its number `K`, counting from 0, and its name |
| `a` | An asset of the pack. `KIND` is `mesh`, `texture` or `sound`. `K` is its licence. `FILE` and `HASH` are the dropped file's bytes and SHA-256. `BYTES` is what the asset takes in the pack. `HX`, `HZ` and `TOP` are a mesh's reach to each side, ahead and up, in millimetres; they are 0 for a texture and a sound |
| `did` | Only in an import's answer: the asset the import made or replaced |

The licences come first, then the assets, then at most one `did`. A reader
refuses the whole list when a line is out of order, a licence is not one the
list declares, a name is not 1 to 24 letters, digits, `-` and `_`, two assets
of a kind share a name, or the pack is larger than its budget.

## An import's body

```text
world.asset.import.v1 LICENCE NAME
```

One line, then the file's bytes as they are. `LICENCE` is the number of a
licence from the list, or `-` when none was chosen (the import is then
refused in words). `NAME` is the file's name as it was dropped: 1 to 120
printable bytes. The asset's name is made from it
(`metaverse-engine/editor/pack-import-policy.coax`, `pip_name`).

A licence is always one of the list's. Nothing in either shape carries a
licence as free text.

## What refuses an import

The policy is `metaverse-engine/editor/pack-import-policy.coax`
(`#authority`). The importer, `metaverse-engine/editor/pack-import.coax`,
says each refusal in words that name the file:

- The file is not a `.glb`, `.png`, `.wav`, `.ogg` or `.mp3` file, or is empty.
- No licence from the list was chosen.
- The file is over the file limit.
- The file is not what its name says, a mesh has more triangles than a mesh
  may, or a texture is larger than a texture may be.
- The pack already holds as many assets of that kind as it takes.
- The pack with the file would be over its budget.

The editor checks the first three before it sends anything, with the
importer's own words (`pack-import-read.coax`, `pi_early`), so a file over
the limit never leaves the browser. A refused import changes nothing: the
pack, the list and the world stay as they were.

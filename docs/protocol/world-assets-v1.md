# A world's imported assets v1 (`world.assets.v1`, `world.asset.import.v1`, `world.asset.upload.v1`)

The browser editor imports a dropped file into a world's asset pack (GitHub
#13420, the editor's E11). Three shapes carry it. `world.assets.v1` is the
list the asset browser shows and the answer to an import.
`world.asset.import.v1` is the body of an import on the dev server, which
takes the file in one request. `world.asset.upload.v1` names a file before
it is sent, on the API, which takes a file by direct upload to storage.

One codec writes and reads all three: `metaverse-engine/editor/asset-wire.coax`.
The pinned bytes are in [proto/world/assets-v1.json](../../proto/world/assets-v1.json).
Check them with:

```sh
coax/web/tool.sh metaverse-engine/tools/assets-profile.coax proto/world/assets-v1.json
```

## The routes

| Route | The dev server | The API |
| --- | --- | --- |
| `GET /api/worlds/ID/assets` | The list | The list: live's, or dev's with `?lane=dev`. With `&write=1` it is answered only to someone who may import, and anyone else gets the refusal a write gets (`401` or `403`) |
| `GET /api/worlds/ID/assets/pack` | The world's import pack, a VPK1 file. `404` when the world has imported nothing | The same, for live, or for dev with `?lane=dev`. `?live=SHA256` answers the world's pack of that hash, whatever lane names it: what a tab asks for after it is told of a new pack |
| `POST /api/worlds/ID/assets` | An import. The body is `world.asset.import.v1`. The answer is the list with a `did` line, or `422` with `{"error":WORDS}` | A file named before it is sent. The body is a `world.asset.upload.v1` ask. The answer says where the bytes go, or `422` with `{"error":WORDS}` |
| `POST /api/worlds/ID/assets/done` | None | The same ask again, after the file is sent. The answer is the list with a `did` line, or `422` with `{"error":WORDS}` |

The dev server answers through `metaverse/splice/tools/asset-local.coax`
(`coax/web/serve.c`, `assets_local`) and keeps the pack and each dropped file
by content hash under `apps/openagents-web/build/.worlds/`. The API answers
in `coax/web/api/worlds.c` and keeps the pack in Postgres and each file in
its private bucket (`migrations/0183_world_assets.sql`). Both carry an import
out with the same code, `metaverse/splice/world-assets.coax`, so the list and
every refusal are the same bytes.

On the API, who may import is who may write the world: an admin, from this
site's page or signed (`coax/web/api/world-policy.coax`, `wr_asset_write`).
On the dev server everyone on the computer is the admin.

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

## A file named before it is sent

```text
world.asset.upload.v1 LICENCE BYTES NAME
SHA256
```

Two lines, each ended by a newline. `LICENCE` and `NAME` are an import
body's. `BYTES` is the file's size. `SHA256` is the file's SHA-256, 64
lowercase hex digits. The page sends these lines twice: to ask where the
file goes, and to say the file is sent.

The answer to the ask is one line:

```text
world.asset.upload.v1 put|kept SHA256 BYTES URL
```

| Word | What the page does |
| --- | --- |
| `put` | Sends exactly `BYTES` bytes to `URL` with `PUT`, with no credentials, then says it is done. `URL` is an `http` or `https` address of at most 1,900 bytes |
| `kept` | Storage has the file already. `URL` is `-`. The page only says it is done |

`URL` is an upload the API opened in its private bucket for that size and the
file's content type (`model/gltf-binary`, `image/png`, `audio/wav`,
`audio/ogg` or `audio/mpeg`), at the object `PREFIX/worlds/ID/files/SHA256`.

When the page says the file is sent, the API reads the object back. It
imports the file only when the object's size is `BYTES` and its SHA-256 is
`SHA256` (`world-policy.coax`, `wr_asset_stored`). Otherwise it answers
`422`:

- Nothing is stored under the name: "That file did not arrive. Import it
  again."
- The object has another size: "That file did not arrive whole, so it is not
  kept. Import it again." The object is deleted.
- The object's bytes are not the ones named: "That file is not the one that
  was named, so it is not kept. Import it again." The object is deleted.

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
the limit never leaves the browser. The API checks the same three when a
file is named, before it opens an upload. A refused import changes nothing:
the pack, the list and the world stay as they were. On the API, the refused
file is also deleted from the bucket unless an earlier import kept it.

A mesh that replaces one the world has pieces of must fit where they stand.
Its kind's row is tried with the new bounds first, and the import is refused
in words when a piece would no longer fit.

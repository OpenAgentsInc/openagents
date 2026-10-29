# The realm host

Status: built, October 28 (rl-realm-host, rl-asset-packs, rl-realm-goldens). Every Verse realm on
openagents.com opens the same way, from one table. Ice Cream Man, Dead
Gears and Everglade are built on it ([realms-port.md](realms-port.md)).

A realm is four things, and nothing else is shared:

1. **its world on the galaxy** (an orbit and whether it opens),
2. **its module** (the Coax that plays and draws it, built to its own wasm),
3. **its assets** (the files its page names, fetched on entry),
4. **its save slot** (what the browser keeps for it, apart from every other realm).

All four are named in `apps/openagents-web/realms.json`. `coax/web/realms.ts`
writes `apps/openagents-web/realms.coax` (the galaxy's table, loaded by
`verse.coax`) and `realm/<slug>/index.html` (the realm's page) from it; the
build runs it first, and `bun coax/web/realms.ts --check` fails when the
written files are behind the table. Don't edit the written files.

## 1. The table entry

```json
{ "id": "house", "name": "HAUNTED HOUSE", "kind": "open", "orbit": { "r": 0.15, "a": 0, "size": 0.028 },
  "realm": { "slug": "haunted-house", "title": "Haunted House", "description": "...", "alt": "...",
             "module": "realm", "slot": "haunted-house", "slot_was": "realm", "assets": ["data:/realm/haunted-house/assets/Haunt.pb", "..."] } }
```

| Field | What |
| --- | --- |
| `id` | The world's id on the galaxy (analytics' `planet`). |
| `name` | What the galaxy writes over it: capitals, at most 40. |
| `kind` | `open` (a click opens the realm), `soon` or `locked`. An open world must have a `realm`. |
| `listed` | `false` keeps it off the galaxy unless the galaxy's URL has `?realms=all` (the fixture realm; a realm before its launch). Default `true`. |
| `orbit` | `r` the orbit's radius and `size` the planet's, both as shares of the galaxy's size (the playtest's orbits are 0.15 to 0.46); `a` its angle in radians. Pick an orbit the galaxy already draws (`ORBITS` in `verse.coax`) or one clear of the others. |
| `realm.slug` | The page: `/realm/<slug>`. |
| `realm.title`, `description`, `alt` | The page's title, its description and its link-preview text. |
| `realm.module` | The module: `apps/openagents-web/<module>.coax`, `<module>-face.c` and `<module>.wgsl`, built to `build/<module>.wasm` by a line in `build.sh`. Two realms may share a module (the page tells it which realm it is). |
| `realm.slot` | The save slot (below). Unique across realms. |
| `realm.slot_was` | A slot this realm used before, read once when its own is empty. The Haunted House kept its saves under `realm` (its module's name) before slots existed. |
| `realm.assets` | The page's `data-assets` (host.js): `pack:` (the realm's asset pack, below), or `data:`, `storage:`, `index:`, `layers:`, `sound:` and `loop:` one file each; and `particles:/particles.wgsl` for particles. |

| `realm.golden` | What the release walk asks of the page (below): `lit`, the least share of its pixels lit (the fixture 0.015, the Haunted House 0.03); `settle`, milliseconds to wait before its shot; `blocks`, how many 8 px blocks may differ from its golden. |

Each realm also needs `realm/<slug>/index.md`, written by hand: the page as
text for agents, search engines and screen readers. The generator refuses a
realm without one.

## 2. The module

A realm's module is a Coaxial app (the full list of exports is in the header
of `coax/web/host.js`). It is its own wasm, so one realm's size or a
broken build never touches another's. The smallest realm module is the
fixture (`fixture.coax`, `fixture-face.c`, 55 KB); the Haunted House
(`realm.coax`, `realm-face.c` and the engine's libraries) is the full one.

What every realm module exports:

| Export | Must | What |
| --- | --- | --- |
| `coaxial_layout`, `coaxial_state_size` | yes | The state's fingerprint and size; the host keeps the state. |
| `coaxial_max_vertices`, `coaxial_vertex_floats`, `coaxial_uniform_floats` | yes | Buffer sizes. |
| `coaxial_frame(state, vertices, uniforms, w, h, dpr, dt, still)` | yes | Steps and draws one frame; returns the vertex count. The HUD is drawn here, in Coax. |
| `coaxial_key`, `coaxial_key_up`, `coaxial_blur`, `coaxial_pointer`, `coaxial_wheel`, `coaxial_cursor` | to play | Input. A realm reads both a keyboard and two thumbs (390 px phones). |
| `coaxial_inbox`, `coaxial_inbox_cap`, `coaxial_message(state, kind, length)` | for saves | Message 4 is the slot's save, 600 the page facts (with `realm=<slug>` and `slot=<slot>`). |
| `coaxial_action`, `coaxial_outbox`, `coaxial_outbox_len`, `coaxial_action_done` | for saves | Action 4 keeps the outbox in the realm's slot; action 1 goes to a URL (`/verse/galaxy` to leave). |
| `coaxial_asset`, `coaxial_asset_ready` | with assets | Where a `data:` asset goes in memory, and that each asset arrived. |
| `coaxial_draws`, `coaxial_draw_count`, `coaxial_stores`, ... | with a 3D scene | The scene's indexed draws over its storage, index and texture assets (`realm.wgsl` `scene_vs`/`scene_fs`). |
| `coaxial_particle_cap`, `coaxial_particles`, `coaxial_particle_bytes`, `coaxial_particle_instances` | with particles | Emitters from `verse/particles.coax`, drawn by `particles.wgsl` when the page names `particles:/particles.wgsl`. |

Shared engine pieces stay in `verse/` and are linked as the Haunted House
links them (`build.sh` lists them after the face): `player.coax` walks,
`grid.coax` reads thumbs, `camera.coax` orbits, `particles.coax` emits,
`cinematic.coax` flies a camera (`realm-flight.c` shows a realm using it).
`save.coax` packs a realm's saves with a CRC and a schema number.

## Asset packs

A new realm's kit art comes as one **pack**: the meshes, their texture
layers and its sounds in a single file the page streams on entry, under a
size budget, with every asset's licence recorded. The Haunted House keeps its
separate files; Ice Cream Man, Dead Gears and Everglade use packs.

- **Manifest**, `verse/packs/<realm>.json`: the pack's name and title, its
  `budget` in bytes, the source scene (Aethyr's SCN4 `scene.bin`, packed from
  the Quaternius kits) and texture layers, `texture_size` (layers are resized
  to it, 512 by default), the `licences` (name, licence, URL each), the
  `meshes` by name and the `sounds` (name, file, licence).
- **Compiler**: `python3 verse/tools/pack_realm.py verse/packs/<realm>.json
  apps/openagents-web/realm/<slug>/assets/<realm>.pack
  apps/openagents-web/realm/<slug>/licences.md`. It refuses a pack over its
  budget, an asset without a licence, a mesh it can't find and a texture
  layer shared by two licences. A mesh takes its kit's licence from its SCN4
  kind; a layer the licence of the meshes that use it; a sound names its own.
  The format is in the compiler's header.
- **Licences**: the compiler writes `realm/<slug>/licences.md` (every
  asset, its kind and licence) and puts the same in the pack's catalog; the
  realm shows them (the fixture's panel) and links the file from its
  `index.md`. Commit both with the pack.
- **On the page**: one asset, `pack:/realm/<slug>/assets/<realm>.pack`. It
  stands for four assets in its place: 0 the catalog (`data`), 1 the
  vertices (`storage`), 2 the indices (`index`), 3 the layers (`layers`); its
  sounds are added by name for `coaxial_sounds`. While it streams the realm
  gets message 605, `LOADED TOTAL` bytes, about ten times a second; then
  `coaxial_asset_ready` for each of the four (bytes 0: it failed).
- **The catalog**, a line each: `pack NAME`, `budget BYTES`, `bytes BYTES`;
  `L K NAME|LICENCE|URL` licence K; `M NAME FIRST COUNT K X0 Y0 Z0 X1 Y1 Z1`
  a mesh (its indices into the pack's index buffer, its licence, its bounds
  in metres); `T LAYER SOURCE K` a layer; `S NAME K` a sound. A realm draws a
  mesh as `coaxial_draws` `[FIRST, COUNT, instance, 1]`. Vertices are the
  Haunted House's 48 bytes (`realm.wgsl`), the layer in the colour's top
  byte, counted in the pack's own layers. `fixture.coax` reads a catalog
  (`fixture_catalog`), and `fixture.wgsl`'s `scene_vs` draws its vertices.
- **Budget**: the fixture's is 1.5 MB. A realm's is what its world needs to
  open on a phone over a slow line; raise it in the manifest deliberately.

## 3. The save slot

The host keeps one text per slot in this browser (IndexedDB `coaxial`, store
`kept`, key = the slot; localStorage `coaxial.<slot>.kept` when IndexedDB is
refused). A realm keeps with action 4 and gets it back as message 4 at the
next visit, before the page facts. Slots never mix: the fixture keeps under
`fixture` and the Haunted House under `haunted-house`, and neither reads the
other's. A realm that also saves on the server uses its slot as the server
key (`/api/playtest/progress/<slot>/main`).

Browser storage can be refused (a private window), so a save is a
convenience: a realm must start well with nothing kept, and must refuse a
kept text that isn't its own (the fixture checks its tag; `save.coax` checks
the world name, schema and CRC).

## 4. The galaxy orbit

The galaxy (`verse.coax`) draws `realms.coax`'s worlds in order, then
players' own realms on the outer orbit. An open world is drawn lit, its
name and OPEN show under the pointer, and a click goes to its page
(`PLANET_PATH`). Without a pointer, LEFT and RIGHT step through the open
worlds it shows (the table's order, then players' realms) and ENTER opens the
one chosen. The Haunted House and Coding keep their tags. A world the
table doesn't list shows only at `/verse/galaxy?realms=all`.

## The release walk

`coax/web/goldens.ts` (run by `release.sh` on staging, then on production)
walks the landing, the galaxy and the Haunted House, then opens **every other
realm in the table from the galaxy**: `/verse/galaxy?realms=all`, RIGHT to its
world, ENTER. Each realm's page must arrive, start (`coaxial.ready`), report
no error (a pack or asset that fails says so), light at least its
`golden.lit`, and on staging match `coax/web/goldens/<slug>.png`. A realm that
is blank, broken or changed fails the release before production. A new
realm's golden: release to staging (it fails with "no golden"), then
`bun coax/web/goldens.ts https://staging.openagents.com --update`, look at
`coax/web/goldens/<slug>.png`, commit it (and only the goldens you meant to
change) and release again.

## Adding a realm

1. Its module: `<module>.coax` with checks, `<module>-face.c`, `<module>.wgsl`; a `wasm.sh` line in `build.sh`.
2. Its pack: `verse/packs/<realm>.json`, compiled with `pack_realm.py` (above).
3. Its world in `realms.json`: listed `false` until launch, assets `pack:...`.
4. `realm/<slug>/index.md`, linking its `licences.md`.
5. `bun coax/web/realms.ts`, `coax test <module>.coax`, a headless shot, and its golden (the release walk, above).
6. At launch: `listed` true (the galaxy's golden changes; approve it).

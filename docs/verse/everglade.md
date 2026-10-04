# Everglade

Status: proposed specification, October 4, 2026. Nothing in this document is
implemented yet.

Everglade is a loaded Verse zone: a forest glade with a small timber-and-plaster
workshop where a person works with a team of coding agents. It is where the
[Agent Studio](agent-studio.md) lives. AgentCraft showed the idea in a
Minecraft studio; Everglade is the same workspace built in Verse Engine from
stylized CC0 kits, and its panels are the Zeron-derived interface the studio
already specifies.

Everglade replaces the earlier plan to put the studio in a plaza building. The
studio's host coordinator, protocol, and panels are unchanged; only where they
are drawn moves.

## Asset sources

All three kits are by Quaternius, downloaded by the owner from
<https://quaternius.com> as the free Standard editions, and licensed
CC0 1.0. Each kit's `License_Standard.txt` states the license; keep a copy
beside the admitted files.

| Kit | Contents | Use in Everglade |
| --- | --- | --- |
| Stylized Nature MegaKit (Standard) | 68 static models, about 149,000 triangles in total: common, pine, twisted, and dead trees up to 19 m tall; bushes, ferns, plants, grass, clover, flowers, mushrooms; rocks, pebbles, and stepping-stone paths. 20 PNG textures, most 2048² (bark with normal maps, alpha-masked leaf and flower cards). | The glade: tree ring, undergrowth, paths, and rocks. |
| Medieval Village MegaKit (Standard) | 176 modular static models on a 2 m grid: walls (2 m × 3.12 m) in plaster, brick, and wood, doors, windows and shutters, floors, stairs, balconies, overhangs, roofs, chimneys, fences, vines, a wagon and crates. 22 PNG textures at 2048² with normal and ORM or roughness maps, plus an alpha-blended glass material. | The workshop and its yard. |
| Fantasy Props MegaKit (Standard) | Furniture and props. Fourteen are already admitted under [`assets/verse/props/quaternius/`](../../assets/verse/props/quaternius/README.md) for the summoning lair. | Station furniture. This kit is an addition to the two the owner named, proposed because neither of them has desks, shelves, or lecterns. |

Measured facts that shape the design:

- Every model is meters, Y-up, with no skins, animations, or glTF extensions.
- Foliage depends on alpha-masked, double-sided textures. Baking it to vertex
  colors, as the Ruins pack does, turns leaf cards into solid quads, so
  Everglade needs textured, alpha-tested drawing in the zone renderer.
- The two named kits carry about 88 MB of PNG source at 2048². Admitted
  textures are downscaled to at most 1024², and to 512² where a texture covers
  small or distant geometry.
- The current material path samples base color only. Normal, ORM, and
  roughness images are not admitted until a shader reads them.

## Admission and the zone pack

Admission follows the Fantasy Props precedent:

- A curated subset of each kit is admitted with an
  `openagents.verse.source-manifest.v1` manifest that pins the creator,
  license, package, and SHA-256 of every admitted file. Only models that the
  layout places are admitted.
- A Rust compiler reads the admitted sources, downscales textures, and writes
  a pinned Everglade zone pack: models, base-color textures with alpha, and
  material flags (alpha mask cutoff, double-sided, blend). The pack's digest
  and length compile into `verse`, like `PACK_SHA256` and `PACK_BYTES` for
  Ruins.
- The pack loads on entry through the Ruins loader's rules: HTTPS only, no
  redirects, exact length and digest, bounded decoding, and the
  content-addressed disk cache. Committed files are the pack and the curated
  sources, not the full kits.
- Budgets: at most 30 MB committed for sources and pack together, at most
  250,000 triangles placed, and at most 64 MB of decoded textures.

## Rendering

The zone renderer gains textured static meshes:

- Base-color texture sampling with the material's factor, alpha-mask cutoff,
  double-sided faces, and one blended pass for glass.
- Static placements merge into cells per material and upload once, as
  `imported::merge` does for the lair; there is no GPU instancing to depend on.
- The same shaders run on desktop and on phones, within the GLES 3.0 limits
  every backend requests (no storage buffers or compute).
- Everglade's atmosphere has its own colors: a green-gold day sky, warm fog,
  and lamplight inside the workshop. Amber stays the plaza's palette.

## The zone

Everglade registers as `ZoneId::Everglade` with world id `everglade-v1`,
following [Build and register a new zone](zones.md#build-and-register-a-new-zone):
its adapter, entry state, plaza arch, minimap landmark, intents, camera clamp,
mobile zone identifiers, tests, and capture example. The ground is generated in
Rust: a gentle heightfield, flat inside the clearing, rising toward the tree
ring, with grass and path textures. Its sign reads `EVERGLADE`.

## Layout

The glade is about 120 m across. The workshop stands at its center on a flat
pad, with a yard in front and a tree ring around both.

| Place | Built from | Studio station |
| --- | --- | --- |
| Approach path | Stepping-stone paths, flowers, ferns | Spawn and return |
| Yard notice board | Wooden frame, fence pieces, banners | Task Wall |
| Workshop hall | Plaster and timber walls, round-tile roof, wide windows, double doors | Desks: one workbench per seat, each with a monitor board |
| Hall gallery | Bookcases, book stands, scrolls | Library |
| Hearth corner | Cauldron, candles | Oracle |
| Yard ring | Training dummy, anvil, rock border | Proving ground |
| Lectern by the door | Book stand, lantern, banner | Podium |
| Strongroom | Chest, metal fence, ornament | Merge station |
| Bench under the trees | Bench, stools, mushrooms | Lounge |
| Wagon by the gate | Wagon, crates | Workbench (running commands) |

Placements are Rust data in a layout module, validated by tests: every
placement names an admitted model, prop bounds become navigation blockers,
every station has a reachable standing point, and no blocker covers a path.
Text that the world draws, such as the Task Wall's cards and the monitors, is
drawn by Verse on in-world boards, as the Gym draws its boards.

## The workspace

Everglade is the Agent Studio's place in the world:

- Seats are Verse agents that walk between the stations above, from the
  studio snapshot and the shared activity classifier (`atif::activity`).
- Selecting a station, or walking up to it and pressing the interact key, opens
  the matching Rust Native panel over the world through `verse::panels`:
  the console at the notice board, a seat's panel at its desk, decisions at
  the podium, and the diff review at the merge station.
- Data loads only while the player is in Everglade, as the Gym loads only
  while the player is inside.
- On phones, stations open the same panels through the hosts' native mounting
  (#10476).

## Delivery

1. Admit the curated assets and build the pinned zone pack.
2. Draw textured, alpha-tested static meshes in the zone renderer on desktop
   and phones.
3. Register the zone with generated ground, the plaza arch, and greybox
   station markers.
4. Build the glade and workshop layout with collision and station points.
5. Run the studio workspace in it (#10465).
6. Mount it on phones (#10476).

## Open questions

- Whether Fantasy Props may join the two named kits for furniture, or whether
  stations use only village and nature pieces.
- Whether Everglade later opens on the OpenAgents Grid, where portals are
  closed today.

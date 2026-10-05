# Everglade

Status: implemented on desktop, October 4, 2026; the phones observe only.
Delivery step 1 is implemented: the admitted sources under
`assets/verse/everglade/` and the pack compiler and loader in
`verse::zones::everglade_pack`. Delivery step 3 is implemented:
`ZoneId::Everglade` with generated ground and the plaza arch in
[`zones/everglade/`](../../crates/verse/src/zones/everglade/mod.rs). Delivery
step 4 is implemented: the pack loads on entry, and the layout in
[`zones/everglade/layout.rs`](../../crates/verse/src/zones/everglade/layout.rs)
places the glade, the workshop, and each station's furniture with their
blockers. Delivery step 5 is implemented on desktop in
[`zones/everglade/studio.rs`](../../crates/verse/src/zones/everglade/studio.rs)
and [`panels/studio.rs`](../../crates/verse/src/panels/studio.rs): seats walk
between stations from a live host's studio snapshots, and the panels send the
studio's intents to that host (`verse --everglade`, or `--studio-socket PATH`
for another host's control socket). `verse --studio-sim` and the
`everglade_capture` example's `studio-` views play the simulated team, a
read-only replay with no host behind it (#10572). On phones the studio panels
observe only (#10570). The [Agent Studio audit](agent-studio-audit.md) lists
what has and hasn't run end to end. Everglade also runs in a browser through
[`crates/everglade-web`](../../crates/everglade-web/README.md), without the
studio.

Everglade is a loaded Verse zone: a forest glade with a small timber-and-plaster
workshop where a person works with a team of coding agents. It is where the
[Agent Studio](agent-studio.md) lives. AgentCraft showed the idea in a
Minecraft studio; Everglade is the same workspace built in Verse Engine from
stylized CC0 kits, and its panels are the Zeron-derived interface the studio
already specifies.

![Everglade: a city for building things together](everglade-map.png)

The [illustrated map](everglade-map.svg) imagines Everglade grown into a
small city around its commons: neighborhoods for living, making,
learning, and gathering, with quiet woods at the edges.

## Inspiration

> my strong suspicion is that the optimal way to manage multiple
> intelligences is that which is closet to our primordial state: wandering
> around a small town, seeing people we know well, physically embodied
>
> the cybersynesque dashboards feel productive, but i doubt they really are

— [Will Manidis](https://x.com/WillManidis/status/2106401558565134823), replying about AgentCraft

Everglade takes that literally. The studio is a small town in a glade, the
agents are people the player knows by name and silhouette, and where an agent
stands says what it is doing. Panels open only when the player walks up to a
station and asks for detail; the default view is the place, not a dashboard.

## Placement

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
- Budgets: at most 30.5 MB committed for sources and pack together, at most
  720,000 triangles placed (300,000 before the town grew), and at most 64 MB of decoded textures.

## Rendering

The zone renderer gains textured static meshes:

- Base-color texture sampling with the material's factor, alpha-mask cutoff,
  double-sided faces, and one blended pass for glass.
- Static placements merge into cells per material and upload once, as
  `imported::merge` does for the lair; there is no GPU instancing to depend on.
- The same shaders run on desktop and on phones, within the GLES 3.0 limits
  every backend requests (no storage buffers or compute).
- Everglade's atmosphere has its own colors: a late-morning daylight sky
  (`pbr::Daylight`) from warm horizon haze to a blue zenith, a Sun in the key
  light's direction, value-noise clouds, fog that takes the sky's color along
  each view ray so distant ground meets the horizon, and lamplight inside the
  workshop. Amber stays the plaza's palette.
- The player is the ritual chamber's outfitted character (the Universal male
  Ranger that `verse_play` binds as `adventurer`), not the Grid's boxy
  avatar, and no spade companion follows in Everglade. The pack carries it as
  one skinned character with idle, walk, run, jump, backpedal, and strafe
  clips and the demolition yard's two-handed chop (`TreeChopping_Loop`),
  composed from the retained character sources under their own manifest; the movement
  state picks the clip, and the character is skinned on the CPU into a
  textured figure each frame, so GLES and WebGL2 draw it too. Its triangles
  have their own budget (40,000) and are not counted as placed.
- The studio's seats are the same character, its outfit in each seat's
  color, drawn in the player's figure. Their postures (typing on a stool at
  the desk, reading, leaning at the ring, waiting, thinking, working, and
  gesturing while they speak) are clips authored from the idle clip on the
  skeleton when the zone loads (`zones/everglade/pose.rs`), so the pack
  carries no new clips. Heads turn toward the monitor, the seat being
  spoken to, or the player.
- Ambient light is baked when the zone loads (`pbr::textured_bake`, lighting
  audit item B1): each static vertex stores how much of the sky it sees and
  one bounce of sunlight in its light channel, with alpha-tested leaf cards
  as partial occluders, so the workshop interior and the ground under the
  tree ring darken. A coarse probe grid of the same light shades the
  characters. The bake runs on a worker thread; in a browser it advances a
  little each frame at lower quality.
- The sky lights the zone (`pbr::environment`, items A2 and B2): the daylight
  sky, with its clouds at their mean cover and the lit ground below the
  horizon, is projected on the CPU into order-two spherical harmonics for
  diffuse light and a GGX-prefiltered cube for reflections, at the key's
  `sky` level. Shaded sides take the sky's blue-to-haze gradient, glossy
  surfaces reflect the sky, and reflections scale by the baked light over
  the open sky's, so covered surfaces stop reflecting it. The quality tier
  sets only the cube's size; WebGL2 samples it with an explicit level.
- Fog is exponential height fog (item A3, `HeightFog` in
  `zones::atmosphere`): it starts at 40 m, thins with height so hilltops
  stay clearer than hollows, brightens toward the Sun, and still closes in
  completely by 180 m. The renderer skips every textured cell beyond that
  distance, and cells too small to see at their distance.

## The zone

Everglade registers as `ZoneId::Everglade` with world id `verse-everglade`,
following [Build and register a new zone](zones.md#build-and-register-a-new-zone):
its adapter, entry state, plaza arch, minimap landmark, intents, camera clamp,
mobile zone identifiers, tests, and capture example. The ground is generated in
Rust: a gentle heightfield, flat inside the clearing, rising toward the tree
ring, with grass and path textures. Its sign reads `EVERGLADE`.

## Layout

The zone is about 510 m across, with a flat clearing 272 m across: sixteen
times the first glade's area. The workshop stands at its center on a flat
pad, with a yard in front, and a city grows around it after the
[illustrated map](everglade-map.svg), joined by dirt roads, inside a tree
ring about 300 m across. Every building is the workshop's own kit pieces,
one to three stories under round-tile roofs; no model was added to the
pack. The city's buildings are a table in `layout::city`, and each wall run
blocks walking as one footprint.

| District | Built from | Roads |
| --- | --- | --- |
| The Commons | Lantern Pond with reeds and stones, benches, an open bandshell | The commons walk, west of the hall |
| Main Street | Twelve shops (bakery, café, bookshop, grocer, tailor, print shop, and more), market stalls, street trees | Main Street, about 200 m long |
| Fountain Plaza | A paved plaza with a fountain and stalls, two cafés, the two-story Market Hall | Market Way |
| Creative District | The Makers' Hall, a studio, an atelier, a pottery, the Atelier Hall, the Sculpture Walk | Studio Road |
| The Foundry | The Server Barn and fab yard, a workshop, the Fab Hall, the forge, an annex | Foundry Road |
| Knowledge District | The Stacks, the Old College, the college and lecture halls, the archive, the map room, Observatory Hill's three-story tower, Reed Pond, the long meadow and its sketch cabin | Library Way |
| Stoop Lane | Ten homes and townhouses with lanterns and little gardens, the cottage | Stoop Lane, Hearth Road |
| Lantern Quarter | The Music Hall, the meeting hall, the guild and choir houses, four pubs | Hearth Road, Lantern Road |
| Brownstone Row | Six two-story brownstones with stoops, six row houses, a community garden | Brownstone Row |
| Walden Woods | The writing and code cabins, the quiet cabin, the prototype shed, the Thinking Pond, pines | The woods paths, Lantern Road |
| Fernhollow | The lookout hut by the Fern Pond, pines and ferns | The Fernhollow path |
| Gardens and orchards | Fenced beds, two orchards of young fruit trees, the beekeeper's hut | The orchard lane |

The studio's stations stay where they were:

| Place | Built from | Studio station |
| --- | --- | --- |
| Approach path | Stepping-stone paths, flowers, ferns | Spawn and return |
| Yard notice board | Wooden frame, fence pieces, banners | Task Wall |
| Workshop hall | Plaster and timber walls, round-tile roof, wide windows, double doors | Desks: one workbench per seat, each with a monitor board |
| Hall gallery | Bookcases, book stands, scrolls | Library |
| Hearth corner | Cauldron, candles | Oracle |
| Yard ring | Training dummy, anvil, rock border | Proving ground |
| Lectern by the door | Book stand, lantern, banner | Podium |
| Strongroom | Metal crate, metal fence, ornament | Merge station |
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
  (#10476). They observe only; acting from them is #10570.
- The OpenAgents app's Grid opens Everglade through a walk-in arch, with the
  shared pack loader's progress, Cancel, and Retry; see
  [the Grid's portal to Everglade](mobile.md#the-grids-portal-to-everglade).
- A shared instance runs on a chamber host (`"profile": "everglade"`) beside
  the Coder host. The host walks the seats from the studio snapshot, and every
  viewer draws them where the host places them. A viewer's NIP-HOST `world`
  right admits walking only; panels need `observe`, `operate`, and `review`.
  See [Verse networking](networking.md#plan), step 3.

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

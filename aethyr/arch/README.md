# Aethyr's architecture kit

Buildings made from parts. A part is built once, as triangles, and stood
wherever a realm wants it.

| File | What |
| --- | --- |
| `mesh.coax` | The pen that writes triangles into a buffer you own, and the shapes: `arch_box`, `arch_lathe` (a profile turned round an axis, with fluting), `arch_ring`, `arch_ellipsoid`, `arch_limb`, `arch_gem`, `arch_panel`. `arch_tint` gives the next triangles a colour of their own |
| `classical.coax` | Classical parts built from those shapes: `arch_column` (a fluted Doric column), `arch_plinth`, `arch_baluster`, `arch_rails`, `arch_entablature`, `arch_brazier`, `arch_flame`, `arch_band` |
| `place.coax` | Records that stand a part somewhere, and draws that name a part and a run of records: `arch_stand`, `arch_round` (a ring of them, with a gap for a doorway), `arch_draw` |
| `palette.coax` | The stylized materials a vertex can carry (`ARCH_M_PLASTER`, `ARCH_M_ROOF`, `ARCH_M_ASHLAR`, `ARCH_M_LEAF` and the rest), which `aethyr/render/stylized.coax` paints |
| `solid.coax` | Flat-faced solids measured in metres, so stone courses, tiles and boards come out at their true size: `arch_face`, `arch_slab`, `arch_obox`, `arch_beam`, `arch_block`, `arch_roof_plane`, and `ArchWall` with `arch_wall_face`, `arch_wall_beam`, `arch_wall_frame` for building on an upright wall |
| `village.coax` | The village library: generators for buildings, walls and street furniture ([parameters](#the-village-and-flora-library)) |
| `flora.coax` | Trees by species and season, far hills and the ground ([parameters](#the-village-and-flora-library)) |

The first user is the Golemancers' Workshop
([`everglade/workshop/temple.coax`](../../everglade/workshop/temple.coax)),
drawn by shaders written in Coax
([`everglade/workshop/gpu/temple.coax`](../../everglade/workshop/gpu/temple.coax)).

## Build a part and stand it

```coax
p: [1] ArchPen; arch_pen(p[..], mesh[..]);
s: [1] ArchScene; arch_scene(s[..], records[..], draws[..]);

p[0].material = MARBLE;
column := arch_column(p[..], 0.4, 6.0, ARCH_FLUTES);          // built once
placed := arch_round(s[..], column, 16, 7.75, 0, 0, 0, 0);    // stood sixteen times in a ring
```

- **A vertex** is 12 floats: place, normal, material, two surface coordinates
  and three spare. Triangles are a plain list, counter-clockwise from outside.
- **A record** is 16 floats: place and turn, scale, a motion and its phase, a
  tint and a value of its own.
- **A draw** is four words: first vertex, vertex count, first record, record
  count. A host draws it as one instanced call.
- **A full pen or scene stops whole** and says so (`full`). It never writes
  part of a triangle.

## The village and flora library

Each generator takes a record of sizes and colours and writes one part at the
origin, its front facing +z and the ground at y 0. A realm stands the part
with a record (`arch_stand`), which adds a place, a turn, a scale and a wash
of colour. A new variant is a change of parameters, never a new mesh.
[Painted Village](../../verse/painted/village-road.coax) is the first scene
built from it, drawn by [Aethyr's stylized shading](../render/README.md#stylized-shading).

```coax
c: ArchCottage;                      // every field has a default
c.storeys = 2; c.wall = 5.0; c.dormers = 2; c.roof_r = 0.2; c.roof_g = 0.3; c.roof_b = 0.5;
part := arch_cottage(p[..], c);      // a two-storey cottage under a blue roof
oak := arch_tree(p[..], arch_tree_spec(ARCH_OAK, ARCH_AUTUMN, 7));
```

### Buildings (`village.coax`)

| Generator | Parameters |
| --- | --- |
| `arch_cottage(p, ArchCottage)` | A timber-frame cottage. `w`, `d` its plan; `wall` the eaves' height; `base` the stone base's height; `rise` the ridge above the eaves (the roof's steepness); `over` the roof's overhang; `storeys` 1 or 2; `bay` metres between posts (the timber pattern); `braces` slanting braces in the end bays; `chimney` -1 left, 1 right, 0 none; `dormers` how many on the front slope; `porch` a lean-to this deep on the front (0 none), across `porch_from` to `porch_to` of it; `door` where the door stands along the front as a share (below 0 none); `roof_r/g/b` the tiles' colour; `plaster_r/g/b` the plaster's |
| `arch_stone_house(p, ArchStoneHouse)` | A house of dressed stone. `w`, `d`, `wall`, `rise`, `over` as the cottage's; `hip` a hipped roof (false: gables); `door` where the arched door stands (below 0 none); `windows` shuttered windows on the front; `annex` a boarded lean-to this wide on the left end (0 none); `roof_r/g/b`, `stone_r/g/b` (0: the stone's own grey), `board_r/g/b` |
| `arch_round_tower(p, ArchTower)` | A round stone tower under a cone of tiles. `r`, `h` the drum; `roof` the cone's height; `roof_r/g/b` |
| `arch_church(p, ArchChurch)` | A stone church, its gable front facing +z. `w`, `d`, `wall`, `rise` the nave; `tower` the tower's side, `tower_h` its height, `spire` the spire's; `roof_r/g/b` the slates, `spire_r/g/b`, `glass_r/g/b` the windows' light |

### Walls and street furniture (`village.coax`)

| Generator | Parameters |
| --- | --- |
| `arch_drystone(p, run, h, thick, tumble, seed)` | A dry-stone wall along +x: its length, height and depth; `tumble` which ends fall away (0 neither, 1 the start, 2 the far end, 3 both); `seed` picks the stones |
| `arch_coped_wall(p, run, h, thick, r, g, b)` | A wall of cut stone under a coping, in a colour |
| `arch_pier(p, side, h, ball)` | A square gate pier with a cap, and a stone ball when `ball` |
| `arch_lamp_post(p, h)` | An iron post with a lantern that glows |
| `arch_banner_pole(p, h, r, g, b)` | A pole flying a swallow-tailed pennant in a colour |
| `arch_signpost(p, h, boards)` | A post with that many pointed boards |
| `arch_bunting(p, a, b, sag, flags, lanterns)` | A line of little flags from point `a` to point `b`, sagging `sag`, with a hanging lantern after every third when `lanterns`. It is built where it hangs |
| `arch_stall(p, w, d, r, g, b)` | A market stall with an awning striped in a colour and cream |
| `arch_crate(p, s)` | A crate `s` across |
| `arch_window`, `arch_door`, `arch_shutter` | An opening on an `ArchWall`, for buildings of your own |
| `arch_drum`, `arch_cone`, `arch_wall_fan`, `arch_wall_arch` | A round wall, a cone of flat faces, a disc or half disc on a wall, an arch of cut stones |

### Trees, hills and the ground (`flora.coax`)

| Generator | Parameters |
| --- | --- |
| `arch_tree_spec(species, season, seed)` | An `ArchTree` for a species (`ARCH_OAK` broad and round, `ARCH_MAPLE` taller, `ARCH_POPLAR` narrow, `ARCH_PINE` a conifer) in a season (`ARCH_SUMMER`, `ARCH_AUTUMN`, `ARCH_GOLD`, `ARCH_SPRING`); the seed varies its size and shape |
| `arch_tree(p, ArchTree)` | A tree. `species`; `height` to the top of the crown; `crown` its radius; `squash` its height as a share of its width; `trunk` the trunk's radius; `clumps` lumps of leaves (a pine's tiers); `leaf_r/g/b` and `leaf2_r/g/b` two leaf colours each clump mixes; `seed` |
| `arch_clump(...)` | One faceted lump of leaves, for hedges and bushes of your own |
| `arch_peak(p, r, h, sharp, ragged, seed)` | A far hill or a mountain: its radius and height; `sharp` below 1 a dome, above 1 a peak; `ragged` how broken its outline is |
| `arch_ground(p, ArchGround, table)` | The ground as a grid over a table of heights, grass cover and paving (`ARCH_GROUND_FLOATS` floats a node); `arch_ground_at` reads the table at a place |


## Checks

```sh
coax/bin/coax test --library aethyr/arch/mesh.coax
coax/bin/coax test --library aethyr/arch/classical.coax
coax/bin/coax test --library aethyr/arch/place.coax
coax/bin/coax test --library aethyr/arch/solid.coax
coax/bin/coax test --library aethyr/arch/village.coax
coax/bin/coax test --library aethyr/arch/flora.coax
```

They check that every shape faces out, keeps to its size, and that a short
buffer is never overrun.

## What it doesn't do yet

- No textures: a shader colours a surface from its material, its position and its tint.
- No level of detail: a part has one mesh.
- The village library's buildings have no insides and no colliders yet.
- Shadows are the shader's own. The Workshop's are worked out for its columns,
  not cast from the mesh.

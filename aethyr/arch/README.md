# Aethyr's architecture kit

Buildings made from parts. A part is built once, as triangles, and stood
wherever a realm wants it.

| File | What |
| --- | --- |
| `mesh.coax` | The pen that writes triangles into a buffer you own, and the shapes: `arch_box`, `arch_lathe` (a profile turned round an axis, with fluting), `arch_ring`, `arch_ellipsoid`, `arch_limb`, `arch_gem`, `arch_panel`. `arch_tint` gives the next triangles a colour of their own. `arch_solid_box`, `arch_solid_turned` and `arch_solid_round` write a part's [solids](#solids-what-blocks-a-walker) |
| `classical.coax` | Classical parts built from those shapes: `arch_column` (a fluted Doric column), `arch_plinth`, `arch_baluster`, `arch_rails`, `arch_entablature`, `arch_brazier`, `arch_flame`, `arch_band` |
| `place.coax` | Records that stand a part somewhere, and draws that name a part and a run of records: `arch_stand`, `arch_round` (a ring of them, with a gap for a doorway), `arch_draw`. A draw also stands its part's solids in the world (`arch_scene_solids`) |
| `solids.coax` | The placed solids as Aethyr's colliders (`arch_colliders`), a walker among them (`arch_walk`, `arch_walk_among`) and a mesh that shows them (`arch_solid_mesh`) |
| `palette.coax` | The stylized materials a vertex can carry (`ARCH_M_PLASTER`, `ARCH_M_ROOF`, `ARCH_M_ASHLAR`, `ARCH_M_LEAF` and the rest), which `aethyr/render/stylized.coax` paints |
| `solid.coax` | Flat-faced solids measured in metres, so stone courses, tiles and boards come out at their true size: `arch_face`, `arch_slab`, `arch_obox`, `arch_beam`, `arch_block`, `arch_roof_plane`, and `ArchWall` with `arch_wall_face`, `arch_wall_beam`, `arch_wall_frame` for building on an upright wall |
| `village.coax` | The village library: generators for buildings, walls and street furniture ([parameters](#the-village-and-flora-library)) |
| `plaza.coax` | A town square: steps, a terrace, piers, a fountain, benches, banners, a parasol stall, barrels and jars, paving ([parameters](#a-town-square-plazacoax)) |
| `flora.coax` | Trees by species and season, bushes, far hills and the ground ([parameters](#the-village-and-flora-library)) |
| `land.coax` | Wild land: a cliff, boulders, a river, a waterfall with spray and a rainbow, a plank bridge, reeds, flowers and vines ([parameters](#wild-land-and-water-landcoax)) |

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
| `arch_cottage(p, ArchCottage)` | A timber-frame cottage. `w`, `d` its plan; `wall` the eaves' height; `base` the stone base's height; `rise` the ridge above the eaves (the roof's steepness); `over` the roof's overhang; `storeys` 1 or 2; `bay` metres between posts (the timber pattern); `braces` slanting braces in the end bays; `rails` a rail across every bay and struts in the gables (false: a calm frame of posts); `chimney` -1 left, 1 right, 0 none; `stack` the chimney's width and `stack_at` where it stands along its end (-1 back, 1 front); `dormers` how many on the front slope and `dormer_w` half a dormer's width; `porch` a lean-to this deep on the front (0 none), across `porch_from` to `porch_to` of it; `door` where the door stands along the front as a share (below 0 none); `roof_r/g/b` the tiles' colour; `plaster_r/g/b` the plaster's |
| `arch_stone_house(p, ArchStoneHouse)` | A house of dressed stone. `w`, `d`, `wall`, `rise`, `over` as the cottage's; `hip` a hipped roof (false: gables); `door` where the arched door stands (below 0 none); `windows` shuttered windows on the front; `annex` a lean-to this wide on the left end (0 none), boarded or (`annex_plaster`) plastered; `roof_r/g/b`, `stone_r/g/b` (0: the stone's own grey), `board_r/g/b` |
| `arch_round_tower(p, ArchTower)` | A round stone tower under a cone of tiles. `r`, `h` the drum; `roof` the cone's height; `roof_r/g/b` |
| `arch_church(p, ArchChurch)` | A stone church, its coped gable front facing +z: a rose window over a row of lancets, an arched door between two niches, a tower with an open belfry and its bell. `w`, `d`, `wall`, `rise` the nave; `tower` the tower's side, `tower_h` its height, `spire` the spire's; `lancets` how many on the front; `banners` banners and lanterns; `roof_r/g/b` the slates, `spire_r/g/b`, `stone_r/g/b`, `glass_r/g/b` the rose's petals |

### Walls and street furniture (`village.coax`)

| Generator | Parameters |
| --- | --- |
| `arch_drystone(p, ArchDryWall)` | A dry-stone wall along +x. `run`, `h`, `thick` its length, height and depth; `tumble` which ends fall away (0 neither, 1 the start, 2 the far end, 3 both); `stone` a stone's length (larger is chunkier); `tilt` how far stones lean; `fallen` stones lying by each tumbled end; `r/g/b`; `seed` picks the stones |
| `arch_coped_wall(p, run, h, thick, r, g, b)` | A wall of cut stone under a coping, in a colour |
| `arch_pier(p, side, h, ball)` | A square gate pier with a cap, and a stone ball when `ball` |
| `arch_lamp_post(p, h)`, `arch_lamp(p, h, s)` | An iron post with a lantern that glows, 0.34 m or `s` across; `arch_lantern(p, x, y, z, s)` is the lantern alone |
| `arch_banner_pole(p, h, r, g, b)` | A pole flying a swallow-tailed pennant in a colour |
| `arch_signpost(p, h, boards)` | A post with that many pointed boards |
| `arch_bunting(p, a, b, sag, flags, lanterns)` | A line of little flags from point `a` to point `b`, sagging `sag`, with a hanging lantern after every third when `lanterns`. It is built where it hangs |
| `arch_stall(p, ArchStall)` | A market stall. `w`, `d`; `r/g/b` and `r2/g2/b2` the awning's two stripes; `goods` what is laid out (`ARCH_GOODS_PRODUCE`, `ARCH_GOODS_POTTERY`, `ARCH_GOODS_FLOWERS`, none); `seed` |
| `arch_lancet`, `arch_banner`, `arch_wall_disc`, `arch_wall_shape` | A pointed stained light, a banner and a rose on an `ArchWall`, and any flat shape on one |
| `arch_crate(p, s)` | A crate `s` across |
| `arch_window`, `arch_door`, `arch_shutter` | An opening on an `ArchWall`, for buildings of your own |
| `arch_drum`, `arch_cone`, `arch_wall_fan`, `arch_wall_arch` | A round wall, a cone of flat faces, a disc or half disc on a wall, an arch of cut stones |

### Trees, hills and the ground (`flora.coax`)

| Generator | Parameters |
| --- | --- |
| `arch_tree_spec(species, season, seed)` | An `ArchTree` for a species (`ARCH_OAK` broad and round, `ARCH_MAPLE` taller, `ARCH_POPLAR` narrow, `ARCH_PINE` a conifer) in a season (`ARCH_SUMMER`, `ARCH_AUTUMN`, `ARCH_GOLD`, `ARCH_SPRING`); the seed varies its size and shape |
| `arch_tree(p, ArchTree)` | A tree. `species`; `height` to the top of the crown; `crown` its radius; `squash` its height as a share of its width; `trunk` the trunk's radius; `clumps` masses of leaves (a pine's tiers of boughs); `tufts` tufts of three leaves on each mass (0: bare faceted lumps); `leaf` a leaf's length as a share of its mass; `leaf_r/g/b` and `leaf2_r/g/b` two leaf colours each mass mixes; `bark_r/g/b` the bark (0: its own brown); `seed` |
| `arch_bush(p, r, h, r, g, b, seed)` | A low bush of the same leaves |
| `arch_leaf_mass`, `arch_leaves`, `arch_clump` | One mass of leaves (a dark heart under layers of pointed leaves), the leaves alone, and one bare lump, for hedges of your own |
| `arch_peak(p, r, h, sharp, ragged, seed)` | A far hill or a mountain: its radius and height; `sharp` below 1 a dome, above 1 a peak; `ragged` how broken its outline is |
| `arch_ground(p, ArchGround, table)` | The ground as a grid over a table of heights, grass cover (above 1: long grass), paving and churn (how white water runs there) (`ARCH_GROUND_FLOATS` floats a node); `arch_ground_at` reads the table at a place |


### A town square (`plaza.coax`)

| Generator | Parameters |
| --- | --- |
| `arch_steps(p, w, run, h, steps)` | A flight of stone steps `w` wide climbing `h` over `run` metres toward -z. Each step is a solid a walker climbs |
| `arch_terrace(p, w, d, h)` | A raised platform of dressed stone, paved with slabs, behind its front edge. Its top is ground |
| `arch_pedestal(p, side, h)` | A great square pier with a roundel and a cap, for a lantern or an urn |
| `arch_banner_frame(p, w, h, long, r, g, b)` | Two posts, a braced beam and a banner `long` long hung between them |
| `arch_bench(p, w)` | A seat of planks on two stone blocks |
| `arch_fountain(p, r, h)` | A round basin, a bowl on a stem, falling water and a robed statue, `h` high over all |
| `arch_parasol(p, r, h, r, g, b, seed)` | A striped market parasol over a trestle table of flowers |
| `arch_barrel(p, r, h)`, `arch_urn(p, r, h, r, g, b)`, `arch_planter(p, r, h, seed)` | A barrel, a clay jar, a stone urn with a clipped bush |
| `arch_dais(p, r, h, tiers)`, `arch_podium(p, w, d, h, tiers)` | A round and a square stepped base; each tier is a step |
| `arch_paved_ring(p, r0, r1, kerb, lift)` | A ring of setts laid in arcs inside a stone kerb |

### Wild land and water (`land.coax`)

| Generator | Parameters |
| --- | --- |
| `arch_cliff(p, ArchCliff)` | A wall of rock along x, its face toward +z: rounded buttresses of bedded stone before a mass of it. `run`, `h`, `depth`; `bulge` a buttress's radius; `beds` a bed's thickness; `hollow` how far its middle is set back (a cove); `gap`, `gap_at` a cleft left open for a waterfall; `vines` the share of buttresses hung with vines; `r/g/b`; `seed`. Nobody climbs it |
| `arch_boulder(p, rx, ry, rz, seed)` | A rounded stone, mossy on top |
| `arch_river(p, line, y)` | A river's surface along points of x, z and half width, level at `y`. Its ripples run downstream |
| `arch_pool(p, r)` | A round pool's surface |
| `arch_falls(p, w, h, out)` | A waterfall's sheet from its lip at `h`, thrown `out` forward as it falls |
| `arch_mist(p, r, h, puffs, seed)`, `arch_rainbow(p, r, wide, from, to)` | Spray, and a rainbow's arc in it |
| `arch_plank_bridge(p, ArchBridge)` | A bridge along z, its deck at height 0: `run`, `w`, `bays` between posts, `rail`, `drop` of its piles, `r/g/b` the planks, `rail_r/g/b`. The deck is ground and the rails stop a walker |
| `arch_reeds(p, r, h, blades, seed)`, `arch_flowers(p, r, tall, stems, r, g, b, seed)`, `arch_vine(p, w, drop, strands, seed)` | Reeds with cattails, a drift of flower spikes, a curtain of hanging leaves |

The river, the falls, spray and the rainbow are wet materials (`ARCH_M_WET` and
up). Draw them last, in [the stylized water pass](../render/README.md#stylized-shading).

## Solids: what blocks a walker

Every generator writes its part's solids with its triangles, so a world built
from the kit can't be walked through, and whatever is low enough to step onto
is ground.

- **A solid** is a box turned about y (`arch_solid_box`, `arch_solid_turned`)
  or an upright cylinder (`arch_solid_round`), with a foot and a top, in the
  part's own space. Give the pen somewhere to keep them with `arch_pen_solids`.
- **A generator writes solids after the geometry they cover** and matches it:
  a house is its walls up to the eaves, a tree its trunk, a stall its counter
  and posts, a dry-stone wall a run of boxes that follow its ragged top.
  Nothing blocks where nothing is drawn. Leaves, bunting and anything lower
  than 0.12 m carry no solid.
- **A draw stands them.** After `arch_scene_solids`, every `arch_draw` writes
  its part's solids where its records stand it (`ArchScene.placed`).
- **`solids.coax` walks among them** with Aethyr's walking capsule
  (`aethyr/collide.coax`: 0.3 m round, 1.8 m tall). A top up to 0.55 m above
  your feet is a step, so steps and terraces are climbed and a wall is not.

```coax
arch_pen_solids(p[..], local[..]); arch_scene_solids(s[..], local[..], placed[..]);
// ... build parts and draw them ...
feet = arch_walk_among(placed[..], s[0].pn, field[..], feet, dx, dz);
```

`arch_solid_mesh` writes the placed solids as a mesh. Painted Village draws it
when you press C, to see that what blocks you is what you see.

## Checks

```sh
coax/bin/coax test --library aethyr/arch/mesh.coax
coax/bin/coax test --library aethyr/arch/classical.coax
coax/bin/coax test --library aethyr/arch/place.coax
coax/bin/coax test --library aethyr/arch/solid.coax
coax/bin/coax test --library aethyr/arch/village.coax
coax/bin/coax test --library aethyr/arch/flora.coax
coax/bin/coax test --library aethyr/arch/plaza.coax
coax/bin/coax test --library aethyr/arch/land.coax
coax/bin/coax test --library aethyr/arch/solids.coax
```

They check that every shape faces out, keeps to its size, that a short
buffer is never overrun, and that every generator's solids stand inside what
it draws.

## What it doesn't do yet

- No textures: a shader colours a surface from its material, its position and its tint.
- No level of detail: a part has one mesh.
- The village library's buildings have no insides: a door is drawn, not opened.
- Shadows are the shader's own. The Workshop's are worked out for its columns,
  not cast from the mesh.

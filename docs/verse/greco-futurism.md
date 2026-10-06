# Greco-futurism

Status: defined October 6, 2026. The first kit and the first building, the
owner's house in Everglade, are implemented.

Greco-futurism is an architectural style for Verse: a classical temple front
reduced to its essentials, with the future carried in its surfaces and doors
rather than in its massing. This page is the style guide. Read it before you
build a Greco-futurism model, and follow the
[asset runbook](asset-runbook.md) and
[Generated models with Blender](blender-pipeline.md) for the procedure.

The style comes from four reference images the owner chose: a columned
entrance with a bronze door behind two smooth columns, the same house seen
from its lawn between large trees, a study with a coffered ceiling and an
engraved copper door, and a great room with a long sofa before a dark,
engraved double door.

## Principles

- **Reduce the temple to its essentials.** Keep the column, the
  entablature, the podium, and the stair. Drop the flutes, the volutes, the
  carved capitals, and the pediment. A Greco-futurism column is a smooth
  cylinder under a plain square capital.
- **Put the future in the surfaces.** Doors, screens, and feature walls
  carry the futurism: rectilinear circuit traces, stepped symmetric glyphs,
  fine lattices, and small panes of amber light. The massing stays
  classical and calm.
- **Stay quiet and monumental.** Large plain planes of warm stone, deep
  shadows under the entablature, and few colors. Nothing moves, smokes, or
  advertises.
- **Let nature frame the stone.** A Greco-futurism building stands among
  large trees and clipped hedges. Its stair rises between hedged planter
  walls, and trees close the view at its sides.
- **Keep it symmetric.** Every facade, door, and glyph is symmetric about
  its center line.

## Proportions

Every proportion below is a rule. Break one only when the building needs it,
and say why in the script.

| Element | Rule | In the owner's house |
| --- | --- | --- |
| Column | Height 9 to 10 diameters, shaft to capital; a giant order spans both storeys | 8.2 m tall, 0.9 m diameter, 12 smooth-shaded sides |
| Capital | A plain square abacus 1.4 diameters wide over an echinus block and a necking ring | 1.26 m abacus, 0.2 m thick |
| Column spacing | The central bay, at the door, is the widest: 4 to 5 diameters | 4.2 m between the round columns' centers |
| Entablature | A quarter to a fifth of the column height, in three equal bands: architrave, frieze, and cornice | 1.8 m: 0.6 m each |
| Cornice | Three stepped bands, each projecting further: about 0.1, 0.3, and 0.55 m | 0.12, 0.3, and 0.55 m |
| Frieze panels | Small squares, about a third of the frieze's height, spaced about 1.6 m | 0.4 m squares, 1.25 to 1.6 m apart |
| Storeys | A tall ground floor of about 4.6 m and an upper floor of about 3.6 m | 4.6 m and 3.6 m |
| Podium | 1.2 to 1.8 m above the ground, reached by the stair | 1.6 m |
| Steps | Shallow: a rise of 0.15 to 0.17 m and a run of 0.38 to 0.42 m, which the character's 0.35 m step-up climbs | 4 steps of 0.15 by 0.42 m to the forecourt, then 6 of 0.167 by 0.38 m to the portico |
| Planter walls | Knee to waist high, with a hedge 0.6 to 0.8 m tall clipped flat on top | 0.45 m and 1.1 m above their floors |
| Doors | Tall and narrow: height about twice the width, in a bronze surround | 2.5 m by 4.0 m, with a 0.6 m glyph transom |
| Lattice | A fine grid of about 0.2 m cells, with heavier bars every fourth cell | 0.2 m cells, 0.035 m and 0.09 m bars |
| Chimneys | Plain rectangular blocks, 1.5 to 2.5 m above the attic, with a cap band | Two 2.4 m blocks at the front corners, two lower ones at the back |

## Vocabulary

Each element below is a function in
[`scripts/blender/greco_futurism.py`](../../scripts/blender/greco_futurism.py)
and a kit piece in `assets/verse/generated/greco/kit/`.

| Element | Script function | What it is | Kit piece triangles |
| --- | --- | --- | ---: |
| Column | `column` | A square plinth, a base ring, the smooth shaft, a necking ring, an echinus block, and a square abacus | 114 |
| Square pier | `pier` | An anta: a square shaft with the column's plinth and capital, for the ends of a portico | 28 |
| Entablature with panel frieze | `entablature` | Architrave, a frieze of small red-brown square panels, and the stepped cornice, round a whole plan | 56 (one 4 m bay) |
| Stair and podium | `stair` | A flight of shallow steps, each a solid block showing only its front and top | 24 (6 steps) |
| Planter wall | `planter_wall` | A limestone wall with a coping band and a clipped hedge in two tiers | 40 |
| Bronze circuit door | `glyph`, `circuit_door` | A bronze leaf with copper traces in the machine glyph and amber panes, with a glyph transom | 226 (double door, frame, and transom) |
| Lattice screen wall | `lattice` | A walnut grid across an opening, in a bronze frame; light and sight pass through | 328 (3.4 by 4 m) |
| Coffered ceiling | `coffers` | Crossing beams under the ceiling slab and a stepped cove round the room | 46 (one bay) |
| Marble pilaster | `pilaster` | A flat white marble pier on a wall, with a base and a capital | 22 |
| Chimney block | `chimney` | A plain limestone block with a cap band | 32 |
| Circuit wall | `circuit_lines`, `circuit_panel` | Dark walnut paneling inscribed with faint copper circuit lines | 44 (6 by 4 m) |
| Dentils | `dentils` | A row of small marble blocks under an interior cornice | 4 each |

The props share the vocabulary:

| Prop | Script function | Triangles |
| --- | --- | ---: |
| Long low bench, with two stools | `bench_long` | 94 |
| Planter with a clipped shrub | `planter` | 64 |
| Bronze floor lamp with an amber shade | `lamp` | 40 |
| Rug with a classical border | `rug` | 50 |
| Walnut desk with a chair | `desk` | 166 |
| Long linen sofa and low table | `sofa`, `low_table` | 132 |
| Walnut bookcase with books | `bookshelf` | 176 |

### The machine glyph

The circuit relief on doors and transoms is one pattern, `glyph_segments`,
in normalized door coordinates. It holds:

- an outer border and an inner frame whose head steps up twice toward the
  center;
- a frame round two pairs of amber panes at the door's middle, like the
  lights in a vault door;
- steps that fall away from that frame toward the edges, the stepped motif
  of Art Deco;
- a spine down the center with branches that end in square pads, the
  traces of a circuit board.

Every polyline is drawn on the left half and mirrored onto the right. A
trace is a flat copper inlay 0.035 m wide and 0.02 m proud of the bronze:
low geometry rather than a texture, two triangles a segment, so it needs no
image in the pack and breaks with the door. Pads are the same inlay, and
the amber panes are shallow boxes.

## Palette

Colors are sRGB in the script and linear in the pack. Two materials sample
the Medieval Village MegaKit's admitted images, so the style adds no image to
the pack.

| Name | Material | sRGB | Linear | Used for |
| --- | --- | --- | --- | --- |
| Limestone | `GrecoLimestone` | 0.88, 0.82, 0.70 | 0.748, 0.638, 0.448 | Walls, columns, entablature, planter walls; samples the kit's plaster image through `T_Plaster_Luma` |
| Stone shade | `GrecoStoneShade` | 0.76, 0.70, 0.59 | 0.538, 0.448, 0.307 | Steps, the forecourt, plinths, the cornice's middle band, coves, and the ceiling slab |
| Marble | `GrecoMarble` | 0.91, 0.90, 0.87 | 0.807, 0.787, 0.729 | The great room's floor, pilasters, door surround, and dentils |
| Walnut | `GrecoWalnut` | The kit's wood trim image at 42 percent | Factor on `T_WoodTrim_BaseColor` | Lattice bars, the dark wall, bookcases, and furniture |
| Bronze | `GrecoBronze` | 0.24, 0.15, 0.09 | 0.047, 0.020, 0.009 | Door leaves, frames, and mullions |
| Copper | `GrecoCopper` | 0.66, 0.38, 0.20 | 0.393, 0.119, 0.033 | Circuit traces, pads, desk and lamp metal |
| Red-brown | `GrecoRedBrown` | 0.40, 0.18, 0.11 | 0.133, 0.027, 0.012 | Frieze panels, rug borders, cushions, books |
| Amber | `EmitAmber` | 1.00, 0.68, 0.30 | 1.000, 0.420, 0.073 | Door panes, the transom's slits, and the lamp shades |
| Glass | `GrecoGlass` | 0.10, 0.11, 0.12 | 0.010, 0.012, 0.013 | Upper windows |
| Hedge | `GrecoHedge` | 0.17, 0.29, 0.12 | 0.025, 0.068, 0.013 | Clipped hedges and shrubs |
| Linen | `GrecoLinen` | 0.86, 0.81, 0.71 | 0.711, 0.621, 0.462 | The sofa, books |
| Rug field | `GrecoRug` | 0.76, 0.58, 0.45 | 0.538, 0.296, 0.171 | Rug fields |
| Flame | `EmitFlame` | 1.00, 0.82, 0.50 | 1.000, 0.638, 0.214 | Candle and brazier flames |
| Cove light | `EmitCove` | 0.30, 0.22, 0.12 | 0.073, 0.040, 0.013 | The warm line under the coffers' cove, and the uplights' faces |
| Inlay | `EmitInlay` | 0.20, 0.08, 0.03 | 0.033, 0.007, 0.002 | The dark wall's circuit lines, a faint ember |

A model may use at most 16 materials, the pack's limit. The style uses 15.

## Materials and glow

- **Stone** is matte and warm. The plaster image gives it a faint mottle,
  tiled every 2 m. Don't add a normal map: the pack samples base color only.
- **Metal** is flat color. Bronze is nearly black-brown so that copper
  traces read against it.
- **Glow.** The Everglade pack carries no emission. A material whose name
  starts with `Emit` glows instead: the zone gives it a fixed luminance of
  6,000 cd/m² times its color (`scene::emits`), which reads as warm light in
  shade and at dusk without overpowering a sunlit wall. The amber panes and
  lamp shades are `EmitAmber`. In a zone without that rule, the amber color
  alone carries the light: keep the panes small and set them in dark bronze
  so they still read as lit.
- **How brightly.** An `Emit` material's linear color sets its glow:
  flames past white, the cove's line soft, the inlay a faint ember at about
  200 cd/m², which glows in a dim room and vanishes in sun.
- **Glass** is opaque and dark on upper floors, which nobody enters.
  Openings on an enterable floor carry a lattice or mullions with no glass,
  so light and sight pass through.

## Light

A Greco-futurism interior is lit like the crypt: warm pools of light in a
darker room, not even daylight. Its light comes from fixtures in the kit,
each a real point light the zone gives the stage beside a glowing `Emit`
material:

| Fixture | Script function | Light | Flickers |
| --- | --- | --- | --- |
| Candelabrum and floor candle stand | `candelabra`, `floor_candelabrum` | 6,500 cd, 5 m | Yes |
| Side table with candles | `side_table` | 6,500 cd, 5 m | Yes |
| Wall sconce | `sconce` | 7,000 cd, 6 m | Yes |
| Floor lamp | `lamp` | 6,000 cd, 6 m | No |
| Brazier | `brazier` | 22,000 cd, 9 m | Yes |
| Lantern, on a pier or a post | `wall_lantern`, `lantern_post` | 5,000 cd, 8 m | Yes |
| Uplight | `uplight` | 8,000 cd, 7 m | No |

- **Where the lights are.** The script records each fixture's light and
  each flame in the model's `footprint.json` (`lights` and `flames`), in
  the glTF frame; the zone carries them as data
  ([`layout/estate.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/estate.rs)).
- **Limits.** A stage carries at most 32 lamps, and the renderer shades a
  stage's first 8 on the low tier and its first 16 on the medium tier
  (`pbr::gpu::lamp_budget`). So the zone gives a building's lamps only
  while the player is near it (the outside's within 70 m, the inside's
  within 34 m), those on the player's side of the walls first, then the
  nearest. The owner's house has 20 fixtures, 12 of them inside.
- **Grade.** While the player is in the great room, the stage takes a
  darker, warmer, and more contrasty grade (one stop down, cool shadows
  against warm highlights, and a vignette), so the candles read as pools
  of light. Everywhere else the zone's look is unchanged.
- **Particles.** Each flame has a warm halo (`greco_candle_glow`), in the
  fx sprite pipeline ([Particle effects](particles.md)). Dust motes read
  as noise in a daylit room, so the style leaves them out.
- **Destruction.** The lights stay where the fixtures stood if the house
  breaks; they go out only with the zone.

## Scaling to Verse

### Triangle budgets

The style suits a stylized low-poly world: its surfaces are planes, and its
detail is in a few places. Spend triangles on doors, screens, and the
frieze, not on walls.

| Kind | Near level | Far level |
| --- | --- | --- |
| Kit piece or prop | under 1,000; most under 200 | none |
| House, two storeys, with an enterable ground floor | 6,000 to 9,000: two to three of the lighter town houses (`town_houses.py`, 974 to 3,254) | 15 to 20 percent of the near level |
| Landmark or hall | under 20,000, the pack's limit a model | 15 to 20 percent |

Ways to stay in budget:

- Drop every face nothing sees: the bottoms of steps and walls, the backs of
  trims against a wall, and the sides of beams that meet another beam. The
  script's `box` takes a `skip` list for this.
- Draw circuit traces and frieze panels as single flat
  faces just proud of their surface: a trace costs 2 triangles rather than 10.
- Use 12 sides for a column near and 6 far.
- Build one object per model, so its glTF has one node.

### Levels of detail

Everglade draws a model's far level beyond 80 m. A Greco-futurism far level
keeps the podium, steps, walls, columns at 6 sides, the entablature without
its panels, the attic, the chimneys, and the hedges. It replaces each
lattice, door, and window with one panel, and drops the interior, the
glyphs, and the furniture. Build it from the same script
with `FAR` set, as `town_houses.py` does.

### Cells, collision, and destruction

- **Cells.** Everglade merges static placements into 8 m cells per material.
  A large building spans several cells; that's expected.
- **Collision.** A generated building collides by its own triangles, in
  0.25 m columns (`demolition::carve`), so the steps climb, the podium holds
  a walker up, open doors and lattices behave as their geometry says, and
  the flat roof is walkable. Its `footprint.json` boxes are what navigation
  plans around: give a box to each wall run beside a doorway, each column
  and pier, each planter wall, and each large piece of furniture. Don't box
  the stair or the podium; navigation is flat, and a box across them would
  close the doorway.
- **Destruction.** The building breaks like other generated models: it's cut
  on a lattice of blocks at most 3.5 m across and 3.2 m tall, and floors
  and the roof fall when their columns go. Keep the circuit traces in the
  model, never as separate world geometry, so they break with it.
- **Smoke.** Greco-futurism chimneys don't smoke. They're part of the
  silhouette, not a sign of a hearth.

### Beside the half-timbered town

Everglade's town is half-timbered plaster under red tile. A Greco-futurism
building doesn't stand among those cottages: it's a quiet estate at the edge
of town, on open ground, framed by trees, at the end of a street or a walk.
Give it room: at least 3 m of open ground on every side of its footprint,
and keep roads and their walks clear. Face it down a street, so the street
leads the eye to its portico.

## The owner's house

`generated/greco_house` is the first building in the style: the owner's
house.

- **Where.** At the east end of Library Way in Everglade, its stair's foot
  at (109, -29), facing west down the street, with the east woods behind it
  and Observatory Hill to the south
  ([`layout/estate.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/estate.rs)).
  A walk continues Library Way to the stair.
- **Approach.** Four steps between hedge banks rise to a paved forecourt
  with a long walnut bench and two stools; six more between hedged planters
  rise to the portico on its 1.6 m podium.
- **Portico.** Two round columns flank the door, with two square piers at
  each end, under the full entablature, its architrave plain.
- **Facade.** The bronze circuit door, its two leaves swung in against the
  reveals, under a glyph transom with amber slits; a lattice screen on each
  side; and a band of dark windows between bronze mullions above.
- **Great room.** The enterable ground floor: 19 m by 13 m under a coffered
  ceiling, with marble pilasters between tall windows, a dark walnut wall
  inscribed with circuit lines, an engraved double door in a marble
  surround, two bookcases, a desk on a rug, a long sofa against the west
  wall with a low table on a second rug, a floor lamp, and two planters.
  The walk from the door to the desk stays clear, at least 2.5 m wide, so
  the desk and the engraved door are the view from the entrance. West of
  the desk, in front of the engraved-door wall, a spot stays clear and lit
  for a workstation facing into the room (`estate::WORKSTATION`).
- **Light.** A candelabrum on the desk, bronze sconces flanking the engraved
  door and the front door, tall candle stands by the workstation spot and
  in the front corners, a brazier across from
  the sofa, candles on a side table and a lamp by the sofa, a lamp by the
  desk, the cove's warm line round the coffers, and the dark wall's inlay
  glowing faintly. Outside, lanterns hang on the inner piers, lanterns on
  posts mark both flights of the stair, and uplights wash the door and its
  screens.
- **Budget.** 6,156 triangles near and 1,016 far, with its light
  fixtures.

Rebuild it and admit it:

```sh
B=/Applications/Blender.app/Contents/MacOS/Blender
$B -b --factory-startup --python scripts/blender/greco_futurism.py
python3 scripts/blender/greco_admit.py
```

`greco_admit.py` converts the house and its far level into the pack's
`generated` and `lod` sets and adds only those files to each manifest.
Review renders come from
`scripts/blender/greco_views.py -- house IN.glb OUT_DIR`.

## Planned assets

| Asset | What it is |
| --- | --- |
| Pavilion | An open square temple of four round columns under the entablature, for a garden or a lookout |
| Library | A single tall hall behind a portico, its walls lined with lattice-fronted bookcases |
| Gatehouse and gate | Two square piers with a glyph lintel, and bronze gates with the machine glyph |
| Colonnade | A covered walk of columns under a flat entablature, to join two buildings |
| Garden wall | A limestone wall with a coping, in 4 m bays, with a lattice window bay |
| Fountain | A square stepped basin with a plain stele inscribed with circuit lines |
| Stele | A standing limestone slab with an inscription and a small amber pane, as a sign or a marker |
| Bronze lantern post | A square bronze post with an amber glass head, for the stair and the walks |
| Reading chair and side table | Walnut and linen furniture for the great room |
| Second-floor interior | The upper rooms, once a staircase fits the budget |
| Dusk lighting | Lamplight in the great room and the transom, when Everglade has a dusk |

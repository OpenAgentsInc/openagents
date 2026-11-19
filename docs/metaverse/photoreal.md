# The photoreal style

Status: the drawing path is built (November 18) and SLAB is its first zone:
its realm record names `photoreal`, and `/worlds/slab?style=painterly`
shows the older look. SLAB's walls, glass, deck, stone and works are drawn
with the style's built surfaces (below, "Materials"). The style's definition is in
[art-styles.md](art-styles.md).

This page says what the style is made of, how a realm draws with it, and
how you add a mesh kind, a material and instances to SLAB without touching
the drawing code.

## What it is

A realm that names `photoreal` draws its world as meshes: a few *parts*
(a box, a column, a lamp post) each placed many thousands of times by
*records*. One pass draws all of them. The same pass draws the shadow map.

| Piece | What it does | File |
| --- | --- | --- |
| Layout | Where the frame's numbers, a record and a material row sit | `metaverse-engine/render/shade/photoreal-layout.coax` |
| Shaders | The mesh pass, shading, sky, air, film | `metaverse-engine/render/shade/photoreal.coax` |
| Frame writer | Fills the frame's numbers on the CPU each frame | `metaverse-engine/render/shade/photoreal-frame.coax` |
| Scene list | Parts and records, written so one draw covers one part | `metaverse-engine/render/shade/photoreal-scene.coax` |
| Lamps, the lamp field, near lights, lit rooms, the moon, the wind | `pr_light_add`, `pr_lamp_field`, `pr_lights_near`, `pr_lamps_level`, `pr_moon_at`, `pr_frame_wind` | `metaverse-engine/render/shade/photoreal-lights.coax` |
| Checks that hold the CPU and the shaders together | | `metaverse-engine/render/tests/photoreal-checks.coax` |

The parts themselves are made with the architecture kit's pen
(`metaverse-engine/arch/mesh.coax`, `solid.coax`, `classical.coax`,
`flora.coax`). Don't write a second mesh format.

### Light

- **Sun and sky.** `pr_frame_sky` takes the direction to the sun (a realm
  works it out from its site, month and hour) and writes the sun's colour
  after its air, the sky's light on a surface by its facing (nine
  spherical-harmonic rows), the light the ground sends back up and the
  exposure. The sky is the engine's atmosphere (`atmosphere.coax`).
- **Haze.** `PrSky.haze` adds haze to that air: 0 is a clear day, 0.15 a
  hazy evening at sea. Haze dims and reddens a low sun and pales the sky,
  most along the horizon and round the sun. A golden hour comes from the
  sun's height and the haze. There is no painted tint.
- **Sun against sky.** Under a high sun in clear air, level ground takes six
  to ten times more light from the sun than from the whole sky
  (`PR_SUN_POWER`). Under a low sun in haze the two are about equal, so
  shadows are soft and cool.
- **Cloud.** `PrSky.cover` draws the clouds, and thick cloud takes the sun:
  none of it under a broken sky (a cover up to 0.45), 85% of it by 0.9. The
  sky's light, the air's and the clouds' turn grey by the same share
  (`pr_overcast`, `pr_under_cloud`). A realm whose weather knows how much
  light comes through states it: `PrSky.sun_share` and `PrSky.sky_share`.
  The camera opens up for a darker sky most of the way (`PR_ADAPT`, 0.7),
  so a storm is darker on the display but not as dark as its light. SLAB
  takes its cover, both shares and its air's thickness from the sea's
  weather (`s[0].look.now`), so a gale is grey and dim.
- **Shadows.** One shadow map follows the view (`pr_frame_light`). Its edge
  is softened over nine taps (four on a phone). A realm may answer for
  points beyond the map: SLAB traces its own plan there.
- **Ambient occlusion.** A realm lays a grid over its ground, fills it with
  the height of what stands in each cell, and `pr_field_build` turns that
  into how much sky each cell sees. Streets between towers are darker than
  open plazas, and walls take the answer of the ground at their foot.
- **Contact.** A post pass darkens what the depth buffer shows is hidden
  close by (`pr_post`, `pr_contact`): the sky's share of a pixel's light by
  how much of the sky the geometry round it hides (12 taps in a disc), and
  the sun's share by a short march toward the sun (8 steps) for the shadows
  the shadow map is too coarse to catch. Both reach farther with distance
  (the disc is about 1.2% of the distance across, 0.8 m to 24 m), so a
  tower's foot sits on the deck from the opening camera and a bench's from
  walking height. The surfaces hand the pass their normal and the sun's
  share of their light in the second target (`pr_lit_out`,
  `pr_sun_share`); the sky, the sea and traced views mark themselves for
  none (`pr_lit_plain`). See [Contact](#contact).
- **Reflections.** A smooth surface mirrors the sky (`pr_environment`). A
  realm may put more in the mirror: SLAB's glass and pools also mirror the
  platform's own buildings on a desktop.
- **Air.** `pr_air` fades a surface into the air's light with distance. The
  air is thick at sea level and thins with height.
- **Film.** `pr_out` exposes the light, runs it through the engine's photo
  response (`film_photo` in `filmic.coax`) and encodes it for the display.
  The photo response is the engine's film from middle grey up. Below middle
  grey it has a camera's gentler toe: the film shows a hundredth of white a
  sixth as bright, which turns a hazy sea or a shaded wall near black, and
  the photo response shows it about half as bright. `film_photo_light` gives
  back the light behind a dark display grey.
- **Exposure.** `PrSky.exposure` scales the engine's choice, which shows pale
  level ground as a pale grey. SLAB sets 1.3 (`PHOTO_EXPOSURE`), so its
  cream wall square to a low sun sits near the top of the display without
  losing its colour in the film's shoulder, as in the concept picture.
- **White balance.** `PrSky.warm` is the camera's white balance: 0 leaves
  light its own colour, and a grey keeps its brightness. It is held to 0.2
  either way. SLAB sets 0.07 under a high sun and up to 0.14 under a low
  one, less under cloud, with a haze of 0.22 (a marine morning). Keep it
  small: most of the golden hour comes from the sun and the haze.
- **Twilight balance.** `pr_frame_balance(u, base, share)`, called after
  `pr_frame_sky`, is the camera's automatic white balance for a cool light
  (`pr_balance`). With the sun just under the horizon, only the sky's
  twice-scattered blue lights the scene: about 12 times as much blue as red,
  where measured twilight daylight has two to three times. Unbalanced, the
  camera opens all the way for that dim light and the whole picture turns a
  saturated blue. `share` removes that share of the cast past a clear day's
  sky (blue to green above 1.3, red to green under 0.8), so a day, a
  sunset's warm light and deep night are unchanged. A realm that does not
  call it gets no balance. SLAB sets 0.85 (`PHOTO_BALANCE`), so its sea at
  dawn and dusk, and the build view's first tow before sunrise, stay
  grey-blue.
- **Night sky.** After the sun sets, `pr_dome` draws stars (fixed to the
  sky, fading in as the sky darkens) and the moon at its place and phase for
  the date (`pr_moon_at`: a low-precision lunar ephemeris, good to about a
  degree). The moon lights the scene as a second, much weaker sun when it is
  up. SLAB's months are mid-month dates in 2026, and several of them fall
  near a new moon, so the moon is often a thin crescent or not up.
- **Lamps.** A realm places lamps with `photo_light(ph, x, y, z, high, r, g,
  b, climb)`: a light `high` metres over the ground, in linear (r, g, b),
  that lights walls up to the height `climb`. All of them bake into a lamp
  field (4 floats a cell on the ambient-occlusion grid: SLAB's is 250 × 150
  cells of 4 m), so a street of lamps costs one texture read. The field
  lights only ground inside it and over its ground's height: a hull wall
  below the deck takes none. On a desktop the twelve lamps nearest the eye
  are also near lights (`PR_NEAR_LIGHTS`, from `pr_lights_near`), each with a GGX highlight, so a
  wet-looking pool shines under each post. `pr_lamps_level` turns every
  lamp on as the daylight fails (0 by day, 1 by dusk). A lamp's glow is a
  record with `PR_LAMP` (or `PR_BEACON`, which flashes): dark by day, and
  never drawn under a pixel or two, so a far street is a row of lights.
  Limits: 3,072 lamps (`PHOTO_LIGHTS`); no shadows from lamps; the sea does
  not mirror them yet.
- **Lit rooms.** The glass patterns light a share of the rooms behind
  their panes at night (`pr_room` in `photoreal-surfaces.coax`; the share is
  `PR_ROOMS`, from `pr_rooms_by_hour`). SLAB's painted towers follow the
  same share.

### Shading

`pr_lit` is a physically based surface: GGX highlights, Fresnel, and energy
kept (what a surface mirrors it does not also scatter). A material gives it
a colour, a roughness, how much metal, a pattern and a glow.

### Wind

`PR_WIND` (`pr_frame_wind`) holds the wind's direction and strength for the
frame. A record with `PR_SWAY` bends with it: the higher a vertex stands in
its part the more it moves, in gusts of a few seconds, and leaves flutter a
little on their own, so a crown shivers while its trunk leans.

**Leaves that fold.** `pr_part_fold` (`photoreal-scene.coax`) marks a part's
leaf vertices (a vertex's u at −8 or below). They stand as made near the
eye, sink into the crown with distance and are gone past
`pr_fold_reach`, where the solid crown under them (`arch_tree_hearts`)
stands alone. A phone never draws them. This is how SLAB's groves keep
their many leaf cards to the trees near the eye.

### The phone tier

`PrSky.reduced` (a phone) asks for four shadow taps, no clouds in the dome,
no fine normal detail, and contact from six taps with no march toward the
sun. `pr_list_draws(l, true)` leaves out records
marked fine (`pr_fine`). The host draws a phone's shadow map at half the
size.

### The picture's detail on a slow device

SLAB runs the engine's graphics governor
(`metaverse-engine/core/quality.coax` through `quality-page.coax`), wired in
`apps/openagents-web/slab-quality.coax`. In Automatic, when the frames' long
average runs over 18.7 ms (about 53 frames a second) for half a second, it
steps down the engine's ladder of twelve rungs; after five seconds on time it
steps up one rung, and a rung that fails again waits twice as long. What
SLAB draws on each rung:

| Rungs | Pixels drawn per CSS pixel | Contact | Content and sky |
| --- | --- | --- | --- |
| 11 (Full) | the device's (up to 3) | on | full |
| 10 | 2 | on | full |
| 3 to 9 | 1.25 to 1.5 | on | full |
| 2 (Lighter) | 1 | off | full |
| 0 and 1 | 0.75, 0.85 | off | the phone tier's |

The host draws the fewer pixels and the browser scales the picture up. A
device with no rung kept from an earlier visit starts no lower than rung 8
(what a phone drew before: 1.5 CSS pixels with contact), and the measured
start in its first 1.5 seconds steps it down as far as its frames say
(`qp_floor`). When Automatic draws below Full, Look says "We lowered the
picture's detail to keep it smooth on this device." under a Picture detail
choice: Automatic, Full or Lighter (kept in the browser as `verse.quality`).

For tests: `?tier=0..11` (or `full`, `lighter`) holds one rung (`qp_pin`); a
fixed moment (`?t=`) draws Full unless `?tier=auto`; `?slowgpu=K` makes the
governor see every frame K times longer, and message 7913 with the text `K`
changes it while the page runs. `globalThis.coaxial.quality(field)` reads the
rung (0), the mode (1), the pixel budget in thousands (2), contact (3), the
phone content (4), whether Look says it is lowered (5), the steps (6) and the
long average in tenths of a millisecond (7).

The background triangle (`slab_bg_fs`) stops at once on a pixel the clipmap
sea will cover (photoreal meshes on, the eye above the water, no traced
walker, no dry casting cells, the ray down onto the sea within half the
skirt's reach): the sea's opaque surface draws over it, so the sky, the smoke
and the air there were wasted work. It saves about a third of the main pass
(results in `docs/research/ocean/slab-world-spec.md`, section 12).

## Materials

A material names a pattern (`PR_PATTERN_*`, `photoreal-layout.coax`). The
patterns for built things are in
`metaverse-engine/render/shade/photoreal-surfaces.coax`. Each is written to
hold at three distances: a kilometre off, at the face of a wall, and at a
walker's height. A line, a joint or a hole finer than a pixel fades to its
share of the pixel (`pr_line`, `pr_band`) or goes (`pr_resolve`), so
nothing shimmers when the camera moves.

| Pattern | Size (`PR_M_SCALE`) | What it draws |
| --- | --- | --- |
| `PR_PATTERN_CONCRETE` 1 | | cloudy pours and a fine grain |
| `PR_PATTERN_GLASS` 2 | a floor's height (3.9 m) | unitized curtain wall: a spandrel at each floor, a mullion every 1.5 m, each pane bowed a little so what it mirrors bends and breaks at the mullions, lit rooms at night |
| `PR_PATTERN_PAVING` 3 | a slab's side (1.2 m) | slabs in mortar joints with a chamfer's shade; under 1 m laid half off row to row and 1.5 times as long, 1.5 m and over twice as long; each slab its own tone, warmth and tilt; grain, aggregate, stains and wear (`pr_paving`) |
| `PR_PATTERN_ASPHALT` 4 | | a road's grit and wear |
| `PR_PATTERN_STONE` 7 | a course (0.6 m); two rows | dressed stone: blocks set half along, veins, hanging streaks, grime at the foot; flutes round a column |
| `PR_PATTERN_PANELS` 8 | a rib (0.3 m) | ribbed metal sheet |
| `PR_PATTERN_POURED` 12 | a lift (3 m); two rows | a wall cast in place: lifts, form panels, tie holes, construction joints, a seam, openings and their streaks, a waterline |
| `PR_PATTERN_RIBBON` 13 | a floor's height | ribbon windows between pale metal spandrels, lit rooms |
| `PR_PATTERN_FINS` 14 | a floor's height | glass behind pale fins every 1.2 m, lit rooms |
| `PR_PATTERN_PAINTED` 15 | a plate's course (2.4 m) | painted steel plate: welds, chalky paint, rust runs from the seams, grime at the foot |

Patterns 10 and 11 are kept for the lights' and gardens' patterns.

**Two rows.** A pattern marked "two rows" reads eight more numbers from the
row after its material's (`PR_M_MORE`). Make it with
`photo_material2(ph, r, g, b, rough, metal, pattern, scale, glow, more)`; it
takes two rows and answers the first.

- `PR_PATTERN_POURED`: the form panel's width, the bay between construction
  joints, the seam's height over the part's foot (0: none), the spacing of
  openings, the two rows of openings' heights over the foot (0: no row), how
  much the pours' tone varies, and how much lighter the wall over the seam
  is. Its growth and salt bands sit at the frame's sea level
  (`PR_HAZE + 2`).
- `PR_PATTERN_STONE`: flutes round a column (0: a plain face), and how
  weathered it is (0 to 1).

**Wet.** `pr_wet(s, wet, puddle, flat)` makes any surface wet: porous stuff
darkens and turns glossy, and standing water is a flat mirror. SLAB wets its
walls where the live sea reaches and its deck after rain.

**Lit rooms.** The glass patterns light a share of their rooms at night
(`pr_room`): whole floors busier or quieter together, each room warm or cool,
brightest under its ceiling, and far off the field's average. The share is
the frame's `PR_ROOMS`: write it with `pr_frame_rooms(u, b, share)` after
`pr_frame_sky`. `pr_rooms_by_hour(hour, day)` gives a city's share: about
two in five in the evening, an eighth from 2 to 5 in the morning.

### SLAB's materials

SLAB makes its materials first, in a fixed order, so the shader can name
them: `apps/openagents-web/slab-photo-rows.coax` lists the rows, and content
sets' materials follow `SLAB_M_FIRST_FREE` (24). SLAB uses 23 rows of the 62:
four facades for the towers, steel and the bridge decks, the poured wall
(two rows), asphalt, pavers, plaza slabs, the promenade, kerbs, marble,
fluted marble and limestone (two rows each), painted steel, cladding,
corrugated sheet and the stacks' concrete.

- `photo_kind_material` picks a plan record's material by its kind. Rows
  `SLAB_M_TINTED_FIRST` to `SLAB_M_TINTED_LAST` take their colour times the
  plan's paint for the kind (`slab_paint`), so a container keeps its colour
  and a stack its red bands. A temple, a round temple and the statues are
  the exception: their stone is untinted, since the paint fakes columns and a
  dim cella that their real parts and the light now draw.
- A temple's columns are `photo_shaft`: a smooth lathe with a plain base,
  a shaft that narrows by a sixth with a slight swell, and a flared capital
  under its slab; the fluted marble cuts the flutes, and its weather marks run
  round a column by arc length so they don't break at the facing's diagonals.
- A mirror on the platform (`slab_photo_mirror`) shows another tower as dark
  glass with the sky in it and its floors' faint lines, not the plan's
  painted pane grid, which a reflection magnifies into blocks.
- The hull's top takes the deck's zones in the shader (`slab_deck_at`): a
  10 m carriageway on each street of the 100 m grid, kerbs from 5 to 5.3 m,
  0.6 m pavers to 10 m, 1.2 m slabs between, the 1.5 m promenade within
  20 m of the edge. `slab_deck_street` paints edge lines, the gutter's shade
  and the tyres' tracks. The detail set (`slab-content-city.coax`) draws the
  kerbs, gutters, centre dashes and crossings as parts near the eye, on the
  same lines.
- The hull's walls: 3 m lifts in 2.5 m forms, a joint at each 50 m bulkhead,
  the seam 50 m over the keel where the watertight hull meets the lighter
  wall on its columns, scuppers every 25 m with rust and water streaks under
  them, marine growth and a salt band at the waterline, and a wet band that
  follows the live sea (`slab_sea_height`) as high as its waves run.

## Add things to SLAB

SLAB's platform comes from its plan (`metaverse/slab-plan.coax`).
`apps/openagents-web/slab-photoreal.coax` turns each plan record into
placed parts. Everything else that stands on the platform is a *content
set*: windows and railings, people, vehicles, ships, lights, the
construction view's objects.

A content set is one new file. It joins the world in
`apps/openagents-web/slab-content.coax`, in three calls:

```
slab_content_make(ph, pen)     // once: make parts and materials
slab_content_place(ph, l, w)   // on every rebuild: place what stands still
slab_content_move(ph, l, w)    // every frame: place what moves
```

To add a set:

1. Write your file, for example `apps/openagents-web/slab-lamps.coax`.
2. In `slab-content.coax`, `#load` it, give `SlabContent` a field for what
   your set keeps, and call your three procedures from the three above.

You don't edit `slab-photoreal.coax`, the shaders or `slab-face.c`.

### A mesh kind

Make the part with the pen and hand it to `photo_part`. Keep the number it
answers.

```
SlabLamps :: struct { post: int; lamp: int; metal: int; glass: int; }

slab_lamps_make :: (ph: [] SlabPhoto, c: [] SlabLamps, p: [] ArchPen) {
  from := p[0].n;
  arch_limb(p, 0, 0, 0, 0, 5.5, 0, 0.09, 0.06, 8);   // a post 5.5 m tall
  c[0].post = photo_part(ph, arch_part(p, from));
  from = p[0].n;
  arch_box(p, 0, 5.6, 0, 0.25, 0.1, 0.25);
  c[0].lamp = photo_part(ph, arch_part(p, from));
  ...
}
```

Model a part in metres, standing on y = 0, centred on x and z. A part that
many things share at different sizes (a box, a drum) is better made one
metre across and stretched by its records: SLAB already has `ph[0].box`,
`ph[0].drum` (40 sides), `ph[0].post` (12 sides), `ph[0].dome` and
`ph[0].gable`.

Limits, all shared by every set: 72,000 vertices (`PHOTO_VERTICES`), 128
parts (`PR_PARTS`). When something doesn't fit it is left out and
`ph[0].full` is set; the check in `slab-photoreal.coax` fails on it.

### A material

```
  c[0].metal = photo_material(ph, 0.10, 0.10, 0.11, 0.45, 1, PR_PATTERN_PLAIN, 0, 0);
  c[0].glass = photo_material(ph, 1.00, 0.86, 0.60, 0.30, 0, PR_PATTERN_PLAIN, 0, 0);
```

The arguments are the colour as it should look on a display (red, green,
blue), the roughness (0 a mirror to 1 chalk), the metal (0 or 1), a pattern
(`PR_PATTERN_*` in `photoreal-layout.coax`), the pattern's size in metres,
and the glow. `photo_glow(ph, material, glow)` changes a material's glow
later, for a lamp that comes on at dusk.

The table holds 62 materials (row 0 is the default and row 63 means "the
plan's own paint"); SLAB's own take the first 23 (above).

A part takes a material in one of two ways:

- From its vertices: call `arch_material(p, material, 0, 0, 0)` before you
  write them. One part can then carry several materials.
- From its record: `pr_made_of(l, r, material)` gives the whole placed part
  one material.

### Instances

Place parts in `slab_content_place`. It runs twice on every rebuild: the
first run counts, the second writes. Place the same things both times.

```
slab_lamps_place :: (ph: [] SlabPhoto, c: [] SlabLamps, l: [] PrList, w: SlabWorld) {
  pr_fine(l, true);                       // a phone leaves these out
  for i: 0..<200 {
    x := -480 + cast(f32) i * 4.8;
    r := pr_put(l, c[0].post, x, SLAB_DECK, 146, 0, 1);
    pr_made_of(l, r, c[0].metal);
    r = pr_put(l, c[0].lamp, x, SLAB_DECK, 146, 0, 1);
    pr_made_of(l, r, c[0].glass);
  }
  pr_fine(l, false);
}
```

`pr_put(l, part, x, y, z, yaw, scale)` places one part and answers its
record. Positions are in the hull's frame (the plan's), in metres: x along
the platform, z across it, y up, the prime deck at `SLAB_DECK`. The shader
moves everything with the hull. What a record can also say:

| Call | Meaning |
| --- | --- |
| `pr_stretch(l, r, sx, sy, sz)` | sizes along the part's x, y, z in place of the one scale |
| `pr_tint(l, r, red, green, blue)` | multiplies the material's colour |
| `pr_made_of(l, r, material)` | one material for the whole part |
| `pr_motion(l, r, PR_SWAY, phase)` | bends in the wind (`PR_BOB`: rises and falls) |
| `pr_laid(l, r)` | lies flat on a surface (a road marking): drawn a hair nearer so the two never flicker |
| `pr_free(l, r)` | doesn't ride the platform (a ship beside it): its position is in the world's frame |
| `pr_near(l, r, metres)` | a small thing: drawn only within that distance of the eye (three fifths of it on a phone) |
| `pr_lean(l, r, pitch, roll)` | stands the part rolled about its own x, then pitched about its own z, then turned by its yaw (a ship on a wave, a bone of a walking figure); it takes the place of a motion. |
| `pr_span(l, part, ax, ay, az, bx, by, bz, across)` | a part made 1 m long along +y (a bone, a rod), stretched from a to b and `across` thick |
| `pr_span_in(l, part, ox, oy, oz, yaw, ax, ..., bz, wide, deep)` | the same, with a and b given in a frame at o turned by `yaw` (a limb of a figure that faces some way) |
| `pr_value(l, r, v)` | the realm's own number; SLAB's plan parts carry their record's number here |
| `pr_box(l, ph[0].box, x0, y0, z0, x1, y1, z1, material)` | a box between two corners |

`SlabWorld` (`slab-content.coax`) tells a set what it may read: the plan's
records, the layout, whether the construction view is on and its day, the
tier, the time, the hour and how far into the night it is.

The list is made again only when the plan's records change (a layout, a
step of the construction view). What moves every frame goes in
`slab_content_move`: people, vehicles, ships. Those records are sent to
the GPU every frame, so keep them to what really moves.

Give a small or thin thing (a rail, a lamp post, a ladder) a distance with
`pr_near`: about where it is two pixels across. A 0.2 m post is two pixels
at roughly 90 m on a 1440 × 900 view, so 130 m is a fair distance for it.
Past the distance the vertex shader shrinks the part to a point at its
record's place, in both the main pass and the shadow pass, over the last
fifth of the distance. The part is gone before it can shimmer, and you never
see it go. The shrink is toward the record's place, so put that place inside
whatever the part is fixed to (a walkway's is the middle of its stack). A
shrunk part still costs its vertices, so `pr_near` is for how a thing looks,
and `pr_fine` is for what a phone pays. The distance shares the record's
float with `pr_free` (`PR_R_NEAR` in `photoreal-layout.coax`): call them in
either order.

Limits: 16,384 records in all (`PHOTO_RECORDS`).

### The detail set

`apps/openagents-web/slab-content-city.coax` places SLAB's built detail
from the engine's street and works kits (`metaverse-engine/arch/city.coax`,
`works.coax`). Every piece follows a rule over the plan's records as they
are this rebuild, so a resized hull keeps its detail. It reads the deck's
bounds, height and dock basin from the hull's own records (`city_deck`) and
keeps clear of every building (`city_open`). The parapet is the hull's wall
concrete (`SLAB_M_WALL`), kerbs are granite (`SLAB_M_KERB`), and temple detail
is its temple's marble or limestone; the set's own six rows (metal, dark,
glass, lamp, stone, paint) and ribbed sheet are for its fittings. It places on the prime layout
only, and nothing in the construction view.

| Rule | What it places | Records |
| --- | --- | --- |
| The edge | A parapet and handrail round the wall's top and the basin, a drain channel, lamps every 36 m, benches, lift and stair heads | 466 |
| The dock | Four gantry cranes on the basin's rims, a landing stage, a gangway and a door, a lift shaft, bollards, fenders | 29 |
| Streets | On the deck paint's 100 m grid, where nothing stands: kerbs and gutters, centre dashes, drain covers, lamps on both sides, benches, zebra crossings, bollards and blank sign posts at the corners | 1,269 |
| Blocks | Towers: a roof parapet, roof plant round a lift overrun, masts on the tallest, a plinth, piers and a fascia at the foot, canopies over doors that open onto clear deck, and fins, belts or stone corners by a seeded style. The medical and civic blocks: a surround round each painted window, a door, roof plant. Data halls: fans in cowls over the roof's painted ones, louvres and doors, and a cooling yard of chillers and headers on the clearest side | 1,160 |
| Works | Process halls (turbine halls, heat exchangers, industry, desalination): a pipe rack down the clearest long side with drops into the wall and valve stations, louvres and doors, a stair tower, roof plant. Stacks and tall columns: ring walks, a caged ladder, a flue duct to the nearest hall. Tanks: a railed rim walk, a ladder, a valve station and pipes between neighbours. Moonpools: a rim walk and a gantry. The cooling plant: fans in cowls and louvres. The mining yard: a fan bank and an intake louvre on each container, a cable duct and switchgear. The desalination plant: reverse-osmosis trains and its sea pipes over the promenade and down the wall | 791 |
| Temples | For each temple and colonnade, in its own marble or limestone: a flight of steps up each front, a cornice over the entablature, triglyphs along the frieze, and the pediment's trim | 852 |

The lamps' lens material is `slab_city_lamp(ph)`. The night set lights it,
and each post calls `night_post` for its light.

### The gardens

`apps/openagents-web/slab-content-green.coax` plants the plan's garden
records and fills the water:

| Rule | What it places |
| --- | --- |
| Groves and avenues | Stone pines, cypresses and olives from the engine's layered trees (`metaverse-engine/arch/flora.coax`), each swaying in the wind; hedges, shrubs and flower beds; a gravel court |
| The canal and pools | A still-water sheet over a dark floor (`PR_PATTERN_WATER`: the sky and the temples mirrored, a faint caustic net), inside a stone rim |
| The fountain | Its bowls, and a jet: `green_jet` finds the fountain's record and the water set throws a burst every 0.25 s at 7 m/s into the spray ring, so the jet stands about 2 m and falls back in the wind |

### The night set

`apps/openagents-web/slab-content-night.coax` lights the platform by rule
over the plan:

| Rule | What it places |
| --- | --- |
| Street lamps | A warm light at each of content-city's lamp posts (`night_post`) |
| Temples | Floodlights round every temple and tholos, 4 m out, washing the columns |
| The fountain | Four lamps in the basin, and the jet lit from below |
| Works | Cold-white work lights on 16 m masts over the plant and the yards (`night_work_light`, for a construction set too) |
| Aviation | Red beacons on the stacks' rims and on the corners of towers over 60 m, white flashing beacons at the hull's corners |

`night_lamp(ph, l, x, y, z, high, r, g, b, climb)` puts a light and its glow
ball at a post another set stood. `NIGHT_STREETS` true makes the set stand
its own posts on a 25 m grid of the open deck instead.

### The construction view

`apps/openagents-web/slab-build-draw.coax` is the content set of SLAB's
build view (`?view=build`). It draws every body of the build simulation's
view of the day (`metaverse/slab-build-view.coax`) as moving records, with
`pr_free` so they stand in the world's frame, not the hull's: the yard's
apron, cells, gates, cranes, trucks and lights; caissons as far as they are
cast; tow pieces split into their caissons; tugs and people from
`traffic.coax`, barges, cranes, slipform decks, bars and mats from the
engine's site kit (`metaverse-engine/arch/site.coax`); the lines as short
segments; and the upper structure bay by bay. The city comes from the
plan's own records as each module lands. The drawing keeps a floating
origin, fixed for each shot of the film at its subject, so the yard 87 km
from the berth draws at full float precision. The build view draws meshes
whatever the realm's style.

### Things that move

The engine has the kinds a living place needs, so a realm doesn't draw its
own:

| You need | Use | Where |
| --- | --- | --- |
| A ship, a truck, a forklift, a cart, a helicopter and its rotors, a person at a distance, a bird | `traffic_ship(pen, look, TRAFFIC_SUPPLY)` and the rest: each answers a part, front along +x, standing on y 0 (a ship's waterline) | `metaverse-engine/arch/traffic.coax` |
| A ship that rides the sea | Sample the sea's height at the nine places `traffic_ride_at` gives, pass them to `traffic_ride`, and give its heave to the record's height and its pitch and roll to `pr_lean` | `metaverse-engine/arch/traffic.coax` |
| A walking figure up close | `pr_figure`: the engine's person skeleton and walk (`anim/figure.coax`), drawn as 21 records, one a bone | `metaverse-engine/render/shade/photoreal-life.coax` |
| A flag in the wind, a flying bird, a turning rotor | `pr_flag` (12 records), `pr_bird` (3), `pr_rotors` (2) | `metaverse-engine/render/shade/photoreal-life.coax` |
| Where a mover is at a time, with no state kept | `tt_run` on a lane with a wait at each point, then `tt_place` and `tt_heading` | `metaverse-engine/world/timetable.coax` |

A timetable is a function of the time alone: every visitor sees the same
mover in the same place, and a picture at a fixed time is repeatable.
Movers do not see each other, so space them in time.

SLAB's set is `apps/openagents-web/slab-content-life.coax` (issue #13687).
Its routes come from the plan's records by rule, on each rebuild: two
promenade loops inside the parapet, sidewalk runs down every street where
the deck is clear for 60 m or more, street loops that stop at every
crossing, and forklift loops round the works. `slab_life_frame` works out
where everything is once a frame, before the moving records are written,
because the list runs twice. It draws, at most:

| What | Desktop | Phone |
| --- | --- | --- |
| People walking, as one record each | 480 | 190 |
| People standing at the parapet | 48 | 20 |
| Of those, whole walking figures (`pr_figure`) within 70 m | 48 | 14 |
| Trucks and carts | 22 | 10 |
| Forklifts | 6 | 3 |
| Seabirds | 24 | 8 |
| Flags | 6 | 6 |
| Ships (supply ship, tug, bulk carrier) and the helicopter | 4 | 4 |

Ships are built in `metaverse-engine/arch/traffic.coax` with rounded hull
sections, a red bottom and black boot-top, a white house with rows of
windows, rails, a mast with radar, banded funnels, cranes with wires,
lifeboats and anchors. Their lights, and the vehicles' and the
helicopter's, share one lamp material whose glow follows the sun. The
promenade statues are `pr_figure` at three times life size in marble.

People beyond 1.7 km are not drawn. In walk mode under the photoreal style
the visitor is one of these figures, and the shader's own walker steps
aside. Clicks on a ship, a vehicle or the helicopter select codes -10 to
-16 and open a card in the cards' one pattern.

A leaning record is a metre a metre unless it has a stretch: its scale's
float holds the roll. The shader leans its normal with it.

### Check it

```
coax/bin/coax test apps/openagents-web/slab.coax --only "photoreal"
coax/remote/remote.sh run -- coax/web/wasm.sh apps/openagents-web/slab.coax apps/openagents-web/slab-face.c apps/openagents-web/build/slab.wasm
coax/web/tool.sh shot "http://127.0.0.1:PORT/worlds/slab?t=20&hud=0" out.png 1440x900 4000
```

Look at it from the opening camera, at walking height (`&mode=walk`) and at
390 × 844 (`&tier=phone`). `&style=painterly` shows the older look beside
it.

Two build traps:

- **A shader change needs both WGSL files.** `slab-fx.wgsl` (the spray)
  loads the sea's shader, which loads the photoreal one. After a change to
  `gpu/slab.coax`, `gpu/slab-sea.coax`, `gpu/slab-photoreal.coax` or the
  engine's `photoreal.coax`, regenerate every stale module and check:

  ```
  coax/web/tool.sh coax/web/wgsl-module.coax --site
  coax/web/tool.sh coax/web/wgsl-module.coax --site --check
  ```

- **A black page with no error is a stale frame plan.** SLAB draws its
  meshes and then its sea in one pass, which the host's frame plan has
  known since November 18 (commit 08dd0c5f85). A worktree whose
  `apps/openagents-web/build/frame-plan.wasm` is older draws SLAB black.
  Rebuild it:

  ```
  coax/remote/remote.sh run -- coax/web/wasm.sh metaverse-engine/host/frame-plan-carrier.coax coax/web/frame-plan-face.c apps/openagents-web/build/frame-plan.wasm
  ```

## What SLAB keeps traced

- The view from under the sea. The meshes aren't drawn there. The trace
  draws the platform under the same sun and sky (`slab_photo_lit`): its
  paint, the platform's own shadow traced, and the sky's light by facing.
  Whoever walks the deck is lit the same way.
- What the section shows under the sea. In the section the meshes draw
  what the cut leaves, with the same materials as the normal view. The
  vertex shader cuts them (`slab_photo_cut_point`, `slab_photo_cut_gone`
  in `gpu/slab-photoreal.coax`): a part wholly on the eye's side of the cut
  goes, and a part the cut crosses is pressed flat onto the cut plane, so
  its faces toward the eye become the cut face. `slab_scene_fs` paints that
  face as the trace's drawing (`slab_section`, lit by `slab_photo_lit`).
  The trace draws only what lies under the sea, which the slot shows
  through its water. The cut is made on the vertices, not by discarding
  fragments, so the mesh pass keeps the GPU's early depth test.
- Rain. The sea's rain streaks are drawn over the meshes too.
- Picking. A click is still tested against the plan's records on the CPU.
- The sea. It reads the same sky (below).

## What the water reads

In photoreal the sea's surface is lit and shown as the meshes are. Its
optics (`apps/openagents-web/gpu/slab-sea.coax`) read the frame's sun and
sky in light's own scale and leave through the frame's film:

| The sea's call | In photoreal it answers with |
| --- | --- |
| `slab_sea_top_sun(u)` | the frame's sun on a card facing it (`PR_SUN_LIGHT`) |
| `slab_sea_top_skylight(u)` | the sky's light on a card facing up (`pr_sky_light`) |
| `slab_sea_top_sky(r, cap, u)` | the sky the surface mirrors along `r` (`pr_sky`: the air under the frame's cloud, without the sun's disc) |
| `slab_sea_top_air(c, d, dist, u)` | the frame's air over `dist` metres along `d`, mixed in as light (`pr_air_through`, `pr_air_far`) |
| `slab_sea_top_out(c, u)` | `pr_out`: exposure, film, white balance |

The open sea, its foam and the hull's white water all go through these, so
a wall and the water at its foot share one sun, sky, air, exposure and
film. `slab_haze` adds no air of its own over the sea in photoreal
(`slab_photo_sea_haze`).

The view from under the sea and the section's slot are in the frame's light
too. The daylight that enters the water is the frame's sun and sky
(`slab_sea_daylight` reads `slab_sea_top_sun` and `slab_sea_top_skylight`),
the hull under the water is handed to the water as light
(`slab_photo_light`, not `slab_photo_lit`'s display colour), and
`slab_sea_under_out` takes the result through the frame's film. Under the
sea the camera meters the water's own light round the eye, as it meters
pale ground in the air, and opens up most of the way (the 0.7 of
`PR_ADAPT`), so the hull's bottom, the pipes and the lines read at 45 m.
An eye above the water (the section's slot) keeps the air's exposure: the
slot is the sea round it, seen into. Elsewhere `slab_sky`, `slab_haze` and
`slab_grade` (`apps/openagents-web/gpu/slab.coax`) answer with the
physical sky, air and film, and the uniforms `P_SUNCOL`, `P_ZENITH` and
`P_HORIZON` hold the sun's and the sky's colours through the same film. The
painted golden grade (`P_SUNHULL + 3`) is 0.

The weather is the sea's (`slab-sea.coax`, `SeaWeather`). `slab_photo_frame`
reads its cloud, its shares of sun and sky, its haze length and its
lightning, and hands them to `pr_frame_sky`. Two things it adds from storm
light: under a full cover no sun's disc shows (the sun's share falls with
the cover's square, to 5% of the weather's own at full cover), and rain
cloud is deep convective cloud, which passes a third to a tenth of a stratus
layer's light (the sky's share falls by up to 65% with the rain). A gale is
grey; a storm or a hurricane is dim, with no lit faces and soft shadow.
Nothing corrects the frame afterwards.

Anything else that wants light's own scale reads the frame's block at
`SLAB_PHOTO` (`apps/openagents-web/gpu/slab-photoreal.coax`):

| Call | Answers |
| --- | --- |
| `pr_dome(rd, soft, u, SLAB_PHOTO)` | the sky along `rd`: air, sun, cloud |
| `pr_sky(rd, u, SLAB_PHOTO)` | the same without the sun's disc or cloud (a blurred reflection) |
| `pr_sky_light(n, u, SLAB_PHOTO)` | the sky's light on a surface facing `n` |
| `u[SLAB_PHOTO + PR_SUN ..]`, `u[SLAB_PHOTO + PR_SUN_LIGHT ..]` | toward the sun; its light on a surface square to it |
| `pr_air(c, world, u, SLAB_PHOTO)` | `c` after the air between it and the eye |
| `pr_out(c, u, SLAB_PHOTO)` | light as the display's colour |

`slab_photo(u)` says whether the frame is photoreal.

## Contact

The post pass is the host's (`coax/web/host.js`, "The post pass"): a page
whose shader module has `post_fs` draws into two targets, and `post_fs`
draws the screen from them, the depth buffer and the bloom.

- Every fragment entry that draws the frame returns `PrLit`: `light` (the
  display's colour, as `pr_out` gives it) and `aux` (`pr_lit_out(c, n,
  share)` for a lit surface; `pr_lit_plain(c)` for the sky, the sea and
  anything traced).
- `post_fs` calls `pr_post(light, aux, x, y, u, base, depth)`.
- The module also needs a `bloom_fs`. SLAB asks for none of the five bloom
  passes (`coaxial_bloom_passes_v1` in `slab-face.c` answers 0), and its
  `slab_bloom_fs` only copies its source.
- Bind the post pass's six textures in group 1 (`post_light` to
  `post_lin`, `apps/openagents-web/gpu/slab.bindings`).

`/worlds/slab?contact=0` leaves contact out, to compare it and to cost it.
On a laptop's GPU (Apple M-series, headless, the machine under heavy load)
contact costs about 0.1 to 0.3 ms a frame at 1440 × 900 and about 0.1 to
0.2 ms on the phone tier at 390 × 844 and a device pixel ratio of 3.

## A realm of its own

A realm that wants the style without SLAB's files:

1. In its shaders, load `photoreal.coax` and write `scene_vs`, `scene_fs`,
   `shadow_vs` and `shadow_fs` over `pr_stand`, `pr_surface`, `pr_shadow`,
   `pr_field`, `pr_environment`, `pr_lit`, `pr_air` and `pr_out`
   (`apps/openagents-web/gpu/slab-photoreal.coax` is the example). Call
   `pr_surface` first in the fragment shader: it takes screen derivatives.
2. Bind the uniforms, then four stores in group 1 (the vertices, the
   records, the materials, the sky field), then the shadow map and its
   sampler in group 2 (`apps/openagents-web/gpu/slab.bindings`). Don't name
   a store `mesh` or `frame`: the host gives those names an older layout.
3. Keep `PR_FLOATS` floats for the frame's block anywhere in its uniforms.
   Each frame call `pr_frame_view`, `pr_frame_pose` (or `pr_frame_no_pose`),
   `pr_frame_sky`, `pr_frame_light` and `pr_frame_field` (or
   `pr_frame_no_field`).
4. For contact, add the post pass ([Contact](#contact)).
5. Build its list with `pr_list`, `pr_part`, `pr_list_count`,
   `pr_list_fill` and `pr_list_draws`, and export the draws and stores from
   its face (`apps/openagents-web/slab-face.c`).

The CPU side can't load the shader modules (their vector types collide with
a page's own). So the frame writer carries the sky and film in scalars, and
`photoreal-checks.coax` holds the two to the same numbers.

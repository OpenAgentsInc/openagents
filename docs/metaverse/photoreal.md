# The photoreal style

Status: the drawing path is built (November 18) and SLAB is its first zone
(`/worlds/slab?style=photoreal`). Materials for SLAB's concrete, glass and
deck are the next step. The style's definition is in
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
- **Shadows.** One shadow map follows the view (`pr_frame_light`). Its edge
  is softened over nine taps (four on a phone). A realm may answer for
  points beyond the map: SLAB traces its own plan there.
- **Ambient occlusion.** A realm lays a grid over its ground, fills it with
  the height of what stands in each cell, and `pr_field_build` turns that
  into how much sky each cell sees. Streets between towers are darker than
  open plazas, and walls take the answer of the ground at their foot.
- **Reflections.** A smooth surface mirrors the sky (`pr_environment`). A
  realm may put more in the mirror: SLAB's glass and pools also mirror the
  platform's own buildings on a desktop.
- **Air.** `pr_air` fades a surface into the air's light with distance. The
  air is thick at sea level and thins with height.
- **Film.** `pr_out` exposes the light, runs it through the engine's film
  response (`filmic.coax`) and encodes it for the display.

### Shading

`pr_lit` is a physically based surface: GGX highlights, Fresnel, and energy
kept (what a surface mirrors it does not also scatter). A material gives it
a colour, a roughness, how much metal, a pattern and a glow.

### The phone tier

`PrSky.reduced` (a phone) asks for four shadow taps, no clouds in the dome,
and no fine normal detail. `pr_list_draws(l, true)` leaves out records
marked fine (`pr_fine`). The host draws a phone's shadow map at half the
size.

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

Limits, all shared by every set: 40,000 vertices (`PHOTO_VERTICES`), 128
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

The table holds 62 materials for content (row 0 is the default and row 63
means "the plan's own paint").

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
| `pr_value(l, r, v)` | the realm's own number; SLAB's plan parts carry their record's number here |
| `pr_box(l, ph[0].box, x0, y0, z0, x1, y1, z1, material)` | a box between two corners |

`SlabWorld` (`slab-content.coax`) tells a set what it may read: the plan's
records, the layout, whether the construction view is on and its day, the
tier, the time, the hour and how far into the night it is.

The list is made again only when the plan's records change (a layout, a
step of the construction view). What moves every frame goes in
`slab_content_move`: people, vehicles, ships. Those records are sent to
the GPU every frame, so keep them to what really moves.

Limits: 16,384 records in all (`PHOTO_RECORDS`).

### Check it

```
coax/bin/coax test apps/openagents-web/slab.coax --only "photoreal"
coax/remote/remote.sh run -- coax/web/wasm.sh apps/openagents-web/slab.coax apps/openagents-web/slab-face.c apps/openagents-web/build/slab.wasm
coax/web/tool.sh shot "http://127.0.0.1:PORT/worlds/slab?t=20&hud=0&style=photoreal" out.png 1440x900 4000
```

Look at it from the opening camera, at walking height (`&mode=walk`) and at
390 × 844 (`&tier=phone`).

## What SLAB keeps traced

- The section and the view from under the sea. The meshes aren't drawn
  there; the trace draws the platform under the same sky.
- Picking. A click is still tested against the plan's records on the CPU.
- The sea. It reads the same sky (below).

## What the water reads

SLAB's sea is written for colours as a display shows them. In photoreal
`slab_sky`, `slab_haze` and `slab_grade` (`apps/openagents-web/gpu/slab.coax`)
already answer with the physical sky, air and film, and the uniforms
`P_SUNCOL`, `P_ZENITH` and `P_HORIZON` hold the sun's and the sky's
colours through the same film. The painted golden grade (`P_SUNHULL + 3`)
is 0.

Water that wants light's own scale reads the frame's block at
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
4. Build its list with `pr_list`, `pr_part`, `pr_list_count`,
   `pr_list_fill` and `pr_list_draws`, and export the draws and stores from
   its face (`apps/openagents-web/slab-face.c`).

The CPU side can't load the shader modules (their vector types collide with
a page's own). So the frame writer carries the sky and film in scalars, and
`photoreal-checks.coax` holds the two to the same numbers.

# Verse art styles

Status: specification, October 29. Nothing here is built yet; the work is the
Verse project's "Art styles" initiative (`apps/openagents-web/projects/verse/work/art-styles.json`)
and Planetary Villa's "Hecate's Cemetery in painterly" milestone.

The owner's rule: every zone on the Verse is drawn in one of a few art
styles. For now there are three: **lowpoly**, **painterly** and **photoreal**
(as close to photorealistic as we can get). A zone names its style; the
engine owns what the style means. Realms declare a style; they don't
hand-tune shading numbers.

Painterly is the style Kiki wants for Planetary Villa and Hecate's
Cemetery, and the owner wants it for Everglade too. This document defines
all three styles, defines painterly in full, and audits both realms against it.

## The three styles

| | lowpoly | painterly | photoreal |
| --- | --- | --- | --- |
| Geometry | facets read as facets | soft shapes; silhouettes matter, small detail doesn't | as detailed as the budget allows |
| Shading | flat or near-flat per face, one smooth light term | soft toon: 3 softly blended light bands, warm rim | physically based (GGX, energy conserving) |
| Textures | flat colours, no texture or a palette strip | flat, hand-painted, broad colour, no fine grain, no normal maps | high resolution with normal and roughness maps |
| Palette | free | saturated but softened; shadows blue/violet | measured albedos |
| Shadows | blob shadows under movers | real shadows with soft edges, tinted | real shadows, contact shadows, AO |
| Atmosphere | light distance fog | heavy blue-cyan distance haze, painted gradient sky, soft clouds, light shafts | physical sky, volumetric fog |
| Foliage | cones and spheres | blob-shaded canopies, dense wind-swept grass | detailed foliage |
| Post | little or none | gentle bloom, soft tone curve, lifted blacks | tonemap, bloom, colour grade |
| Phones | full | reduced tier (below) | reduced tier |

Lowpoly is what most realms draw today. Photoreal is defined here so zones
can name it. Its engine work isn't planned yet and gets its own initiative
when a zone needs it.

## The style field

`apps/openagents-web/realms.json`: each realm gets a required `style`, one
of `lowpoly`, `painterly`, `photoreal`. A realm whose parts differ (an
interior, a dungeon under a city) lists them in `zones`, each with an `id`
and a `style`; a zone without one takes the realm's.

```json
"realm": { "slug": "everglade", "module": "everglade", "style": "painterly",
           "clock": "cycle", "zones": [{ "id": "crypt", "style": "painterly" }], ... }
```

- `coax/web/realms.ts --check` refuses a realm without a style, an unknown
  style, a zone id that appears twice, and a painterly or photoreal realm
  whose `<module>.wgsl` lacks the style block that `wgsl.py` generates (below).
- The style goes to the module in the page facts (message 600:
  `style=painterly`, and `zone.<id>=<style>` for each zone) and to the page
  as `data-style`. The realm's `index.md` names it.
- `clock`: how the realm's time of day runs. `cycle` (the day turns, 48 real
  minutes a day by default), `fixed:<t>` (always t, 0 to 1, 0 midnight,
  0.5 noon) or `real` (the visitor's local time). Hecate's Cemetery is
  `fixed:0.0` (night, always). Goldens and style checks always pin it
  (`?clock=0.5`), so a shot doesn't depend on when it was taken.
- Existing realms are tagged when the field lands: Haunted House, Dead
  Gears, Ice Cream Man, the fixture, the Grid, the Sandbox and the drops
  `lowpoly`; Hecate's Cemetery and Everglade `painterly`. Each stays as it
  draws today until its adoption task changes it.

## Presets

Each style is an engine preset: one Coax record in `verse/style-preset.coax`
(`StylePreset`) holding every number this document gives, keyed by time of
day where it changes (built October 29, as-presets; the block's layout is the
`SU_*` constants there). A realm reads its preset with
`verse_style_preset(style, tier)`, evaluates it with
`verse_style_at(preset, clock)` each frame and copies the result into its
uniforms as one block (`STYLE_FLOATS`, 32 vec4f). The shading functions
(bands, rim, shadow tint, haze, sky, grass wind, foliage normal, tone) are
Coax in `verse/style.coax` (a file of their own, because `wgsl.py` lowers
only the Metal scalar profile, which has no records), written in the subset `coax/web/wgsl.py` lowers, and
emitted into each realm's WGSL between
`// ---- generated: verse/style.coax` markers, the way `verse/look.coax`'s
outline functions are today. One source, every realm, with checks:
`#test`s pin the band edges, the haze curve, the clock interpolation and
the phone tier's numbers.

What a realm still decides: its geometry, its albedo colours (inside the
palette rules), where its key light points (sun or moon), what glows, and
its clock. Nothing else: no realm-local fog constants, ambient colours or
tone curves. A realm that needs a different look asks for a preset change
in `verse/style.coax`, where every realm in that style gets it.

## Painterly

The reference is the look of *The Legend of Zelda: Breath of the Wild*,
described in words (no reference images are committed): believable light
and space, painted with broad strokes and little fine detail. A sunlit
meadow reads as three flat-ish values of green with a warm edge where the
grass meets the sky. Hills behind it step back in paler, bluer layers, like
painted flats on a stage. Shadows are cool blue rather than dark, and nothing
is outlined in black.

All colours below are linear RGB (what the shader computes in) unless they
say sRGB.

### 1. Shading: soft toon bands

The diffuse term is wrapped, then split into bands:

```
w      = saturate((dot(N, L) + 0.25) / 1.25)     // wrapped Lambert
w     *= shadow                                  // the shadow map's soft term, before banding
band   = 0.55 * soft_step(0.20, w)               // shadow -> mid
       + 0.45 * soft_step(0.58, w)               // mid -> lit
soft_step(e, x) = smoothstep(e - s, e + s, x)
```

- **Three bands**: shadow (0), mid (0.55), lit (1.0). Edges at w = 0.20 and
  0.58, softness s = 0.06 (the blend spans 0.12 of w). Two bands (one edge at
  0.40, s = 0.08) for characters (below).
- **Colour**: `mix(cool(albedo * shadow_tint * ambient), albedo * (key_colour
  * key_strength + ambient), band)`. The shadow band is never black: it's the
  albedo under the tinted ambient, cooled (section 3).
- **No specular** on matte surfaces. Wet, metal or glass surfaces get one
  hard-edged highlight: `step(0.92, dot(R, L))` at 0.35 strength, so it
  reads as a dab of paint rather than a gloss.
- **Normals**: smooth vertex normals, never the per-face facet the lowpoly
  shaders derive from `dpdx`/`dpdy`. Ground normals come from the height
  field blurred over 3 cells, so the terrain's facets don't show.

### 2. Rim light

A warm rim along silhouettes, on the side the key light comes from:

```
rim = smoothstep(0.38, 0.12, dot(N, V)) * (0.35 + 0.65 * saturate(dot(N, L) + 0.3)) * rim_strength
colour += rim * rim_colour
```

- Width: the rim starts where the surface turns 68° from the eye
  (N·V = 0.38) and is full at 83° (0.12). On a 1.8 m figure 8 m away that
  is 2 to 4 px at 1440×900.
- Strength 0.30 for the world and 0.45 for characters. The colour is the key
  light's, warmed: by day (1.00, 0.84, 0.60), at dusk and dawn (1.00, 0.70,
  0.45), and at night the moon's silver (0.70, 0.78, 0.95) at 0.6 of the
  strength.
- It is a light term, so it fogs with the surface and never draws over the sky.

### 3. Shadows and ambient

- **Real shadows**: the host's shadow map (`host.js shadowLayout`: 2048²
  desktop, 1024² phone) from the key light, filtered soft: 9-tap Poisson PCF
  with a 2.5-texel radius on desktop, 4 taps on a phone. The term multiplies
  `w` before banding, so a cast shadow falls into the shadow band with the
  same soft edge as the terminator. A realm of meshes draws its draws into it
  with `shadow_vs`/`shadow_fs`; a traced scene (the style fixture) draws its
  shapes with `caster_vs` (`coaxial_caster_vertices`/`_instances`) and reads
  the map in `bg_fs`. The map covers the eye's near 60 m (40 m on a phone),
  its middle snapped to whole texels, its depth twice the reach either way
  (`verse_style_shadow_reach`, `_snap`, `_across`, `_depth`, `_tap`).
- **The rim is the key's light**: a cast shadow takes it too (the rim term
  is multiplied by the shadow map's term).
- **Blob shadows stay** under movers (the player, pets, NPCs) as contact
  shadows at 0.35 strength, under the cast shadow.
- **Tint**: shadows take `shadow_tint`, which is blue or violet by
  construction: by day (0.52, 0.58, 0.86), at dusk (0.58, 0.48, 0.82), and at
  night (0.36, 0.42, 0.70). The blue channel is always the largest and no
  channel is below 0.35.
- **Cool, warm albedos too** (decided October 29): albedo × tint × ambient
  alone keeps a warm surface warm in shadow (the fixture's olive ground stays
  blue-smallest), so the shadow band's colour then takes the tint's hue once
  more and keeps its luminance:

  ```
  s    = albedo * shadow_tint * ambient
  m    = s * pow(shadow_tint / luma(shadow_tint), cool)    // cool = 1.5
  cool(s) = m * luma(s) / luma(m)
  ```

  A multiply, not a mix: mixing a warm colour toward blue passes through
  grey, while this turns olive ground teal and brown ground slate, still
  ground, never grey or black. `cool` is the preset's (`SU_SHADOW.w`, 1.5 for
  painterly, 0 for lowpoly), in `verse/style.coax` `verse_style_cool` and
  `verse_style_keep_value`.
- **Ambient** is a hemisphere, sky colour above and a warm bounce below:
  by day sky (0.55, 0.66, 0.84) and ground (0.45, 0.40, 0.30), at 0.55 of the
  key; at night sky (0.14, 0.18, 0.34) and ground (0.06, 0.06, 0.08).
- **Never muddy**: lit geometry's encoded (sRGB) luminance is at least 0.10
  by day and 0.06 at night anywhere but a deliberate black (Hecate's shadow
  visitor), and the tone curve lifts blacks (section 9).

### 4. Atmosphere: haze

The signature. Distance haze fades the far world to blue-cyan in steps, so
ridges stack like painted flats.

```
f_dist   = 1 - exp(-density * max(d - start, 0))
f_height = exp(-max(y - base_y, 0) / falloff)          // thinner higher up
f        = min(f_dist * mix(1.0, f_height, 0.6), cap)
hue      = haze_colour * mix(teal, 1, smoothstep(0.15, cap, f))   // teal = (0.70, 1.00, 0.84)
haze     = mix(hue, key_colour_in_haze, pow(saturate(dot(V, L)), 8) * 0.45)
colour   = mix(colour, haze, f)
```

| | day | dusk/dawn | night |
| --- | --- | --- | --- |
| `start` | 20 m | 20 m | 12 m |
| `density` (desktop) | 0.011 /m | 0.013 /m | 0.016 /m |
| `cap` | 0.88 | 0.88 | 0.85 |
| `falloff` | 60 m | 50 m | 40 m |
| `haze_colour` | (0.56, 0.74, 0.90) | (0.78, 0.66, 0.74) | (0.10, 0.14, 0.27) |
| key light in haze | (1.00, 0.90, 0.72) | (1.00, 0.62, 0.40) | (0.40, 0.48, 0.70) |

The haze is coloured, never grey or white: where it is thin (the middle
distance) its hue leans teal, and it turns to `haze_colour`'s blue as it
thickens toward its cap, so the near world stays crisp and saturated, the
middle distance goes teal and the far ridges blue, as Breath of the Wild's
valleys do (`verse/style.coax` `verse_style_haze_hue`, added October 29).

At 100 m by day the haze is 0.58, at 200 m 0.86, and it caps at 0.88 so
far ridges still show as silhouettes. `base_y` is the realm's ground level
at the eye. A realm with a short view (a walled city) still gets the same
curve. Its far trees and hills are what show the layers, so it needs
something beyond 80 m in most views.

Haze applies identically to every surface, the water, the grass and the
particles. The sky's horizon colour equals `haze_colour`, so the world meets
the sky without a seam.

### 5. Sky

- **Gradient**: zenith to horizon by `pow(up, 0.55)` over the view ray's
  elevation (from the ray, not the screen row, so it stays put as the camera
  pitches), plus a 6° band at the horizon lifted 15% toward `haze_colour`.
  By day the zenith is (0.24, 0.48, 0.86) and the horizon equals the haze.
  At dusk the zenith is (0.20, 0.24, 0.52) and the horizon (0.95, 0.62,
  0.48). At night the zenith is (0.012, 0.018, 0.05) and the horizon equals
  the haze.
- **Sun or moon**: a disc (0.53°, drawn at 2° for readability) with a two-term
  halo, `exp(-k(1-cosθ))` with k = 900 and 60, the wide term coloured like
  the key light in haze.
- **Clouds**: one dome layer at 1,500 m. The shape is 2 octaves of value noise
  at 1/2,000 and 1/700 per metre, cut by `smoothstep(0.48, 0.62, n)`. They
  are shaded in two tones, the lit top (1.0, 0.98, 0.95) by day and the
  underside the shadow tint times 1.4, blended by the noise gradient toward
  the sun. They drift 3 m/s. Cover per preset is 0 to 0.6, default 0.35.
  Phones use one octave. A realm's preset can set cover 0 (Hecate's until Kiki
  answers whether there are clouds).
- **Night**: stars (as Hecate's draws them now) fade in below a sun
  elevation of −6°.

### 6. Bloom and light shafts

- **Bloom** (the host's post pass, `post_fs`/`bloom_fs`): luminance threshold
  0.85 after exposure, soft knee 0.25, mixed back at 0.30 desktop and 0.22
  phone. Emissive materials (Hecate's dog, glowing flowers) write their bloom
  share directly, as now.
- **Light shafts** (built October 29, as-light-shafts): a screen-space radial
  blur toward the key light's screen position, in the post pass. Its source is
  the sky's glow round the light, `exp(-24 (1 - cos θ))` (half at 14°, a
  tenth at 25°), which the scene writes into the post pass's aux target: the
  sky passes all of it, a surface the share the haze has taken of it (the lit
  air in front of a far ridge glows; a near trunk or crown blocks), so light
  streams past the near world's edges and the near world casts dark streaks
  into it. From each pixel the pass marches toward the light, desktop 24
  taps, a phone 12, tap i weighted `0.94^i`, starting a hashed fraction of a
  step in so few taps don't step; the sum over the taps' total weight, times
  the weight (0.44 by day, 0.32 at dusk and dawn, 0.20 at night: the first
  numbers here, 0.22 and 0.10, didn't read), is added to the light in the key
  light's in-haze colour times its strength. Shafts are full while the light is
  in the frame and fade out over 0.35 of the frame past its edge
  (`verse_style_shaft_source`, `_fade`, `_tap`, `_total`, `verse_style_shaft`).
  The fixture's `?sun=AZ&sunup=EL` (degrees) puts the sun behind its tree for
  the golden `fixture-painterly-shafts`; its post pass costs 0.17 ms more with
  the sun in view at 1440×900 on an M-series Mac.

### 7. Foliage: blob canopies

- **Canopy normals**: a canopy's (leaf mesh's or leaf primitive's) normal is
  `normalize(mix(mesh_n, normalize(p - c), 0.85))`, where `c` is the
  canopy's centre, from the pack catalog's bounds (`M` line: the middle of
  X0..X1, Y0..Y1, Z0..Z1, moved by the instance's transform) or the
  primitive's own centre. A cypress or pine uses an ellipsoid (its bounds'
  half-sizes) instead of a sphere. The canopy then shades as one soft blob
  in three bands; leaf cards keep their alpha cut, so the edge stays broken.
- **No per-leaf noise**: leaf textures pass the texture rules (section 8),
  and the shader adds no high-frequency noise to leaves.
- **Sway**: canopy vertices move by the grass wind (below) at 0.05 m times
  their height above the trunk's base over the tree's height, so a canopy
  breathes and the trunk stays still.
- **Far trees**: the cones past the kit's range shade with the same ellipsoid
  normal and colour as the kit tree they stand for, so the swap doesn't
  show through the haze.
- **Leafy gaps**: a crown is open in small holes (the preset's 0.36 of its
  leaf noise, fewer through its thick middle than at its edge), so the sky
  shows through and the light shafts stream through as streaks.
- **Built** (October 29, as-blob-foliage): `verse/style.coax` holds
  `verse_style_canopy_axis` (an ellipsoid's gradient, a sphere's when the half
  sizes match), `_canopy_normal` (the 0.85 mix), `_canopy_sway` (0.05 m at
  the top, by the grass's wind) and `_canopy_open` (the gaps); the preset's
  `SU_FOLIAGE` row carries the mix, the sway and the gap share.
  `pack_realm.py` ends each `_Leaves` mesh's `M` line with
  `canopy sphere|ellipsoid R G B` (a pine or cypress is an ellipsoid; the
  colour is the leaves' mean, linear thousandths) and writes each canopy
  vertex's blob direction and height share into its spare vertex word.
  Everglade's kit leaves shade and sway by it, and its far cones take the
  same blob normal and their kit's colour from the catalog, with no per-leaf
  noise. The style fixture's traced tree sways and has the gaps.

### 8. Grass: dense, wind-swept

- **Blades**: one tapered triangle each, 0.35 to 0.60 m tall and 0.06 m wide at
  the base. The GPU instances them from a hash of their cell, with no buffer of
  blades. Desktop draws 24 blades per m² out to 32 m; a phone draws 8 per m²
  out to 18 m. Past the radius the blades shorten to nothing over 6 m, so
  there's no edge.
- **Colour**: the ground's painted colour under the blade, the tip 15%
  lighter and warmer (a brush stroke's lifted end), with a per-blade hue
  jitter of ±4%. The blade's normal is the ground's up, so a field shades
  as one surface in bands, not as noise.
- **Wind**: bend angle `θ = A * (0.6 * sin(k·x − ω t) + 0.4 * gust(x, t))`.
  A = 0.30 rad. The wave has wavelength 12 m and speed 3 m/s along the
  preset's wind direction. `gust` is value noise at 1/25 per metre, moving
  with the wave. The bend grows with the square of height along the
  blade. Bright bands roll across a field as the waves pass: the bent
  blades' normals tilt toward the light by up to 20°.
- **Parting**: blades within 0.7 m of a mover (the player, pets, NPCs, up to
  8 from a uniform list) lean away from it.
- **Where**: the realm gives a mask: no grass on paths, streets, paved
  squares, floors, water, under buildings or in plots it marks (a uniform
  of capsules and the path distance the shaders already compute).
- **Still frames**: under `still` (goldens, reduced motion) the wind is
  frozen at t = 0. The shot is the same every time.
- **Budget**: grass costs at most 2.5 ms of a desktop frame and 3 ms of a
  phone's. If the realm's frame budget is missed, the phone tier halves the
  density before the radius.
- **Built** (October 29, as-wind-grass): `verse/style.coax` holds the wind
  (`verse_style_wind`, `_gust`, `_bend`, shared with the foliage's sway), the
  blade numbers (`_blade_height`, `_grass_fade`, `_grass_blades`), the mask
  (`_grass_clear`: none on a masked shape, all 0.25 m clear of it;
  `_segment` for a capsule or a path), the parting (`_part`), the tilt, the
  tip and the jitter. host.js draws a WGSL's `grass_vs`/`grass_fs` right
  after `bg_fs`, `coaxial_grass_instances(state)` triangles, depth-tested; a
  traced `bg_fs` that writes `@builtin(frag_depth)` hides the blades behind
  what it traced. A realm draws the half disc ahead of the eye: a square two
  radii across and one deep (49,152 blades on desktop, 5,184 on a phone). The
  blade's root is the meadow's colour in the blades' shade (0.78 of it), as
  is the soil between them, and the rim skips blades (a field seen edge on
  would be all rim). The style fixture's ground is now a meadow with a dirt
  path behind the shapes and an earth bank round the pond; its main pass
  costs 0.17 ms more at 1440×900 and 0.10 ms more on a 390×844 phone canvas
  (M-series, headless).

### 9. Palette and tone

- **Albedo rules** (checked on packs and on a realm's constant colours):
  HSV saturation 0.20 to 0.70, value 0.18 to 0.92. Greens lean yellow
  (hue 75° to 130°), earth leans ochre (25° to 45°). No pure black or white
  albedo except what glows and Hecate's shadow visitor.
- **Tone**: `1 - exp(-c * exposure)` (the curve Hecate's uses) with exposure
  1.6 by day and 2.4 at night. Then saturation goes up 1.10 around luminance,
  and blacks are lifted 0.035 (`c = 0.035 + 0.965 * c`) before encoding.
  Every painterly realm gets a post pass (`post_fs`) for this.
- **Outlines**: none in ink. Painterly separates shapes by value and colour.
  The outline pass (`verse/look.coax`) is allowed only in its light form, a
  pale edge brighter than the surface it borders, at no more than 0.35
  strength, fading with haze. Hecate's moonlit edges are this form (below).
  A dark outline fails the style check.

### 10. Textures and packs

- **Pack style**: `verse/packs/<realm>.json` gets `"style"`. `pack_realm.py`
  writes `style NAME` into the catalog and checks the layers against it.
- **Painterly layers**:
  - at most 512 texels a side; 256 is the default for props and characters.
  - no normal, roughness or detail maps (the pack format carries none; a
    manifest that names one is refused).
  - fine-detail energy at most 0.035: the RMS of a layer's luminance minus its
    own 4×-down-then-up copy, over opaque texels, in sRGB 0..1. This is what
    "almost no small-scale grain" means as a number.
  - mean saturation and value inside the albedo rules (section 9).
- **The paint pass**: a layer that fails can be painted at pack time instead
  of hand-repainted. `"paint": true` on the layer (or the pack) runs a
  Kuwahara filter (radius 4 at 512, 2 at 256), then quantizes to 24
  colours per layer and pulls saturation into range. The result must pass the
  checks; the original stays the source.
- **Procedural surfaces** (ground, paths, stone in the shaders) follow the
  same rule. Colour noise is broad, at most 0.25 cycles a metre with ±8%
  amplitude. The fine noise terms (2 to 13 cycles a metre in both realms
  today) are removed.
- **The release check**: `realms.ts --check` (and the release walk) refuses
  a realm whose pack's `style` differs from the realm's, or which has no style
  when the realm's is painterly or photoreal. A lowpoly pack in a painterly
  realm is a refusal, not a warning.

### 11. Characters

- **Shading**: two bands (edge 0.40, softness 0.08), the character rim
  (0.45), and the tinted shadow. They get no specular, and the cast shadow
  stays on them (from the map).
- **Skin and cloth flat**: textures sample 2 mip levels coarser (LOD bias
  +2 at 512 texels), so stitching, pores and fabric grain go. Layers pass
  the pack's texture rules (the Ranger's outfit is painted at pack time if
  needed).
- **Proportions and silhouette**: stylized (heads 1/6 to 1/7 of height,
  hands and feet slightly large); every character reads by its silhouette and
  two or three flat colour masses at 20 m. Grey boxes with sphere heads are
  placeholders and fail the style check where they show.
- **What glows stays glowing**: emissive materials skip the bands. Hecate's
  dog stays white light, and the shadow visitor stays black with its fuzzy
  edge. Those are character designs, not lighting.

### 12. Water: stylized preset

A `painted` preset in `verse/water-look.coax` beside the seven physical
ones. The physics (W1), the mesh and the swimmer don't change, only the look:

- **Colour bands by depth** in place of Beer-Lambert: shallow (0.30, 0.72,
  0.74) under 0.4 m, mid (0.10, 0.45, 0.55) to 1.5 m, deep (0.05, 0.22, 0.36)
  below. Each edge is soft over 0.15 m.
- **Sky reflection**: the sky's gradient colour at the reflected ray, with
  Fresnel kept (Schlick), with no roughness blur.
- **Sparkles** in place of GGX glints: on a 0.15 m grid, a cell sparkles when
  its hashed normal's reflection is within 4° of the sun (or moon). It draws as
  a 2 to 3 px point at (1.0, 0.97, 0.88) and twinkles over 0.3 s. A phone
  uses a 0.25 m grid.
- **Foam**: the shore band as a clean 0.25 m white line with a broken inner
  edge. Ripples and the wake draw as white rings (W6's field, drawn flat).
- Haze and tone as every surface.

### 13. Light through the day

`verse_style_at(preset, clock)` interpolates the keys linearly in the clock
(0 midnight, 0.25 dawn, 0.5 noon, 0.75 dusk). The sun's elevation follows
the clock (−20° at midnight to 60° at noon on the preset's azimuth), and its
azimuth swings a quarter turn either side of it, so it rises on one side and
sets on the other. `verse_style_clock` reads the page's facts (`clock`,
`now`, `day`) and `verse_style_clock_at` runs it: a `cycle` day is 48 real
minutes and the same for every visitor (it counts from 1970), `real` is the
visitor's local time, and a realm with its own moon puts it in with
`verse_style_own_moon`. Below
−6° the moon is the key, with key strength 0.35 and its own colour (0.62,
0.70, 0.95). Key colours: by day (1.00, 0.95, 0.85) at strength 1.0, and at
dusk or dawn (1.00, 0.72, 0.48) at 0.75.

A realm with a real moon (Hecate's) keeps its own moon direction and phase.
The preset gives colours and strengths, and the realm gives where the light is.

### 14. The phone tier

`verse_style_preset(style, PHONE)` (a coarse pointer, or a canvas under
600 css px wide):

| | desktop | phone |
| --- | --- | --- |
| shadow map, PCF taps | 2048², 9 | 1024², 4 |
| grass | 24/m² to 32 m | 8/m² to 18 m |
| clouds | 2 octaves | 1 octave |
| light shafts | 24 samples | 12 samples |
| bloom mix | 0.30 | 0.22 |
| haze density | ×1.0 | ×1.25 (hides the nearer draw distance) |
| sparkle grid | 0.15 m | 0.25 m |

Bands, rim, tint, palette and tone are the same on both: a phone looks like
the same painting, with less in it.

### 15. Judging "looks painterly"

Two checks, both headless (`bun coax/web/shot.ts`, never a visible browser):

1. **The style check**, `coax/web/style-check.ts URL --style painterly`.
   It shoots each realm's named views (`realms.json` `style_views`, e.g.
   `#road`, `#river`, `#above`) at 1440×900 and 390×844 with the clock
   pinned and `still`, masks the HUD plates, and measures:

   | Measure | Painterly passes when |
   | --- | --- |
   | Fine detail: RMS of luminance minus its 5×5 box blur | ≤ 0.035 |
   | Dark ink: share of pixels darker than 0.4× their 3×3 neighbourhood mean | ≤ 0.2% |
   | Near black: share with sRGB luminance < 0.03 | ≤ 0.5% by day, ≤ 2% at night |
   | Shadow hue: per surface (the `?probe=shadow` mask: its id, the shadow band, surfaces the haze has taken under 0.35 of), its mean colour in the shadow band against the same surface lit | blue share b/(r+g+b) at least 0.02 higher; HSV saturation at least 0.05 (never grey) |
   | Saturation: mean HSV saturation of scene pixels | 0.22 to 0.60 |
   | Haze: the band of rows just under the horizon, mean colour | within ΔE 12 of the preset's haze; luminance spread ≤ 0.08 |
   | Bands: luminance histogram of the largest lit object (`?probe` id) | at least two peaks, 0.15 apart |

   A realm draws the shadow mask for the shadow-hue row at its address plus
   `&probe=shadow`: red where a surface is in the shadow band, green its id
   over 16, blue on every surface measured (the style fixture draws it).
   It runs in the release walk for every painterly realm; a failure stops
   the release like a golden does. Lowpoly and photoreal get their own
   rows when they get presets.
2. **The side-by-side**: the check writes `style-<slug>.png`, the realm's
   views beside the **style fixture**. The fixture realm drawn in the same
   preset (`/realms/fixture?style=painterly`): a sphere and a cube in the
   three bands with rim and tinted shadow, a blob tree, a patch of grass in
   the wind, a stylized pond, a figure, and three ridges stepping back into
   the haze, at noon, dusk and night. The owner (and Kiki for her realms)
   judge the look on that sheet; their approval is what moves a realm's
   golden.

The fixture's painterly scene is each engine task's demo, and its golden
(one per clock key) is how an engine change shows up for review.

## Audit: Hecate's Cemetery

`/realms/hecates-cemetery`: `apps/openagents-web/villa.coax` (with
`villafig`, `villapet`, `villaplay`, `villaroom`, `villatips`),
`villa-face.c`, `villa.wgsl`. Kiki's design is
`verse/planetary-villa/README.md`. Everything is drawn from five primitive
shapes (box, sphere, cylinder, cone, ball) with a colour each; there is no
pack and no texture, only `Villa.indices` and the particle shader. Shots
from production, October 29: desktop 1440×900 and phone 390×844.

| Painterly element | What it draws today | Gap |
| --- | --- | --- |
| Style | No style field; the code calls itself "low-poly shapes, each face lit flat" | Tag `painterly`; the README's "almost low-poly" becomes painterly at night (Kiki's word confirms it) |
| Shading | Per-face facet normal from `dpdx`/`dpdy`, one Lambert term from the moon, so the low-poly faceting shows on every cone and the terrain | Smooth normals, 3 bands, no facets |
| Rim | Only on the shadow visitor (a thin moonlit rim) | Rim on everything, moon-silver at night |
| Shadows | 16 blob shadows on the ground; no cast shadows; shadowed albedo darkens toward black | Shadow map from the moon (soft), blue/violet tint, blobs kept as contact shadows |
| Ambient | (0.035, 0.04, 0.05) to (0.07, 0.09, 0.15), very low; the golden only asks 1.5% of pixels lit | Night preset ambient; readable everywhere but the visitor; golden `lit` rises |
| Haze | `exp` fog from 18 m at 0.022/m to near-black navy (0.022, 0.03, 0.06), capped 0.92 | Night haze (0.10, 0.14, 0.27) from 12 m, cap 0.85; the hills behind the wall step back in blue layers rather than going black |
| Sky | Night gradient (near black), two star layers, the moon in its real phase with halo; space for the flight | Keep the moon and stars; the horizon meets the haze colour; clouds off until Kiki answers the open question |
| Bloom and shafts | Post pass with bloom on what glows (dog, flowers, the moon) and an exp tone curve | Threshold and mix to the preset; moon shafts through the cypresses at night strength; black lift |
| Outlines | Pale moonlit outlines on every shape (strength 12 of 15) and path edges, mixed at 0.55 | Keep only the light form at ≤ 0.35 and fade with haze; Kiki asked for "some outlines", so this is her sign-off |
| Foliage | Cypresses are cones, round trees spheres, flat-lit per face; hedges are boxes | Ellipsoid/blob normals; hedges as soft rounded masses; canopy sway |
| Grass | None; the ground is a flat-lit height field with broad and fine noise | Moonlit grass in the field and the garden beds, wind-swept; none on paths or plots |
| Textures | None (procedural colour). Ground noise at 2.3 cycles/m and path grit at 4 and 11 cycles/m: fine grain | Broad noise only; path as a painted band |
| Palette | Very dark greens (0.07–0.17), grey stone; little saturation | Night palette inside the albedo rules: blue-green grass, warm grey stone, violet shadows |
| Characters | You: a black, fuzzy-edged figure built from balls and limbs (Kiki's design). The dog glows white and blooms. Other visitors the same | Keep both designs (exempt: they're characters, not lighting); give the visitor's body the soft silhouette rather than visible ball joints |
| Water | The fountain's water: `LOOK_WATER`, a noise ripple and a moon highlight `pow(…, 60)` | Painted water preset (bands, moon sparkles, clean foam ring) |
| Phone | Same shader, same costs | Phone tier |

Biggest gaps: the faceted single-term moonlight (no bands, no rim), black
shadows and near-black haze, no grass, and no real shadows. The look Kiki
described (dim, moonlit, outlined) maps onto painterly's night preset; the
two places it bends are brightness (painterly never goes muddy) and outlines
(light, not ink). Both are her call, in her realm's adoption tasks.

## Audit: Everglade

`/realms/everglade`: `apps/openagents-web/everglade.coax` (with
`everglade-edit`, `everglade-voice`), `everglade-face.c`, `everglade.wgsl`,
`verse/everglade/*` (plan, people, quests, ranger, river), the pack
`verse/packs/everglade.json` (Quaternius Stylized Nature, Medieval Village
and Fantasy Props kits; `texture_size` 512; 6.0 MB of a 6.75 MB budget), and
the river on `verse/water.coax` and `verse/water-look.coax` (W2). Shots
from production, October 29: the road in, the river crossing and the deep
wood at 1440×900, and the road in and the commons at 390×844.

| Painterly element | What it draws today | Gap |
| --- | --- | --- |
| Style | No style field; the doc calls it "the realm's own lighting (sun, sky, fog)" | Tag `painterly`; the pack declares it |
| Shading | Smooth Lambert from a fixed sun (0.45, 0.8, 0.32) × 0.9 plus a hemisphere sky at 0.62: soft but unbanded | 3 bands, warm rim, tinted shadow |
| Shadows | 16 blob shadows (the Ranger's shows); no cast shadows from trees, buildings or the palisade | Shadow map from the sun; tree and building shadows on the streets |
| Light | One fixed noon-ish sun; no day or night | `clock: cycle`; dawn, day, dusk, night presets; lamps and windows at night (realm task) |
| Haze | `exp` fog from 40 m (170 m from above) at 0.012/m to `HORIZON` (0.80, 0.86, 0.90), a pale grey that bleaches the gatehouse and far town to white | Blue-cyan haze, stacked ridges; something beyond 80 m to stack (hills or far forest rising past the trees) |
| Sky | A screen-space two-colour gradient (horizon to zenith by screen row), no sun, no clouds | Ray-based gradient, sun with halo, clouds, the night sky |
| Bloom and shafts | No post pass at all (clamp and gamma); no bloom, no tone curve | A post pass: tone, black lift, bloom; sun shafts through the canopy |
| Outlines | None | None (correct) |
| Foliage | Kit trees (Pine_4/5, CommonTree_3/4) with alpha-cut leaf cards, both sides lit by the mesh normal, so leaves read as many small facets; far trees are flat cones | Blob canopy normals from the catalog bounds; far cones shaded to match; sway |
| Grass | None: the meadow and the 6 m apron are a noise-tinted ground (broad 0.13 and fine 2.7 cycles/m) | Dense wind-swept grass on the apron, the clearings and the glade; none on streets, the commons or the river |
| Textures | The pack's layers at 512, Quaternius's flat-coloured style (close already); the ground, streets and squares get procedural fine grit (5, 9 and 13 cycles/m) | Layers through the painterly checks (painted at pack time where they fail); procedural fine noise removed |
| Palette | Saturated yellow-green ground (0.24–0.58), warm roofs, grey rocks; no blue in the shadows | Inside the albedo rules mostly; the shadow tint and haze carry the blue |
| Characters | You: the Ranger, 31,396 triangles, 75 joints, textures at 512/256, lit like the world (smooth Lambert). The people: grey-box figures (boxes with sphere heads) in their trade's colour; the cats small coloured shapes. The waystone and some props are plain boxes | The Ranger with character shading (2 bands, rim, coarse mips). The people as aethyr figure parts with the `Peasant` and `Ranger` outfits (already planned in the doc) in painterly shading; the cats as `Custom_Cat`. Boxes that stand for props replaced by kit pieces |
| Water | The river on W2's physically based shader: Gerstner mesh, Fresnel, roughness-blurred sky, Beer-Lambert absorption, GGX glints, foam; fogs to `HORIZON` | The `painted` water preset: depth bands, sparkles, clean foam, haze; physics unchanged |
| Bridge | A plain tan slab with no planks or rails (visible at the river crossing) | Kit planks and rails as the doc describes (`Floor_WoodDark`, `Prop_WoodenFence_Single`) |
| Phone | Fewer kit trees (40 per kind within 42 m), cones past that; the whole frame under 700,000 triangles | Phone tier for grass, shadows, shafts; keep the triangle budget |

Biggest gaps: no post pass at all, a pale grey haze instead of blue, no
grass, no cast shadows or day/night, unbanded shading, the people still grey
boxes, and a physically based river in a painted world.

## Order of the work

1. The style field and presets, and the style fixture, which everything else
   is shown on.
2. Shading (bands, rim, tinted shadows on the shadow map) and tone/haze/sky.
3. Foliage, grass, bloom and shafts, water, characters, textures and the
   pack check.
4. The phone tier and the style check in the release walk.
5. Each realm adopts painterly as the pieces land: Everglade (Verse project)
   and Hecate's Cemetery (Planetary Villa project, with Kiki's sign-off).

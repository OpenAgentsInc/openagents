# 13. Light and shadow

> **Status: partly implemented.** The hemisphere ambient, the shadow map's
> reach, snap, place, depth and tap count, the contact shadow and the haze
> moved from `verse/style.coax` into
> [aethyr/render/light.coax](../render/light.coax), with every realm's WGSL
> byte-identical. The module adds the key light from the clock, the shadow
> map's space with its snapped middle, and a bounded list of local lights.
> Realms still build their light in their own shaders; the shadow pass, tone
> mapping and the filter's tap positions stay where they are (see
> [Still to do](#still-to-do)).

Light is where realms look most different and are most alike underneath:
one key light (the sun or the moon), a sky and ground ambient, a shadow map
around the player, a little contact darkening and distance haze. Aethyr
computes these from the realm's clock and hands every material the same
numbers.

## What this chapter teaches

- A key light from a clock: its direction through the day, and the moon
  taking over while the sun is down.
- A shadow map that doesn't crawl: its middle moves in whole texels.
- Haze as a function of distance and height.
- Local lights as a bounded list that refuses past its size.

## The key light from the clock

<!-- source: aethyr/render/light.coax key -->
```jai
// The key light at `clock` (0 midnight, 0.25 dawn, 0.5 noon, 0.75 dusk; it wraps). The sun's
// elevation runs from `low` at midnight to `high` at noon. Its azimuth turns half a turn through
// the day, an eighth of a turn either side of noon's `azimuth` at dawn and dusk. While the sun is
// below `moon_below` the moon leads, opposite it, so exactly one of the two is the key. (x, y, z)
// points toward the light.
LightKey :: struct { x: f32; y: f32; z: f32; moon: bool; elevation: f32; }
light_key :: (low: f32, high: f32, azimuth: f32, moon_below: f32, clock: f32) -> LightKey {
  e := 0.5 * (low + high) - 0.5 * (high - low) * cos(6.2831853 * clock);
  az := azimuth + (clock - floor(clock) - 0.5) * 3.14159265;
  k := LightKey.{x = cos(e) * sin(az), y = sin(e), z = cos(e) * cos(az), moon = e < moon_below, elevation = e};
  if k.moon { k.x = -k.x; k.y = -k.y; k.z = -k.z; }
  return k;
}
```

The clock runs from 0 at midnight through 0.25 at dawn, 0.5 at noon and
0.75 at dusk, and wraps. The sun's elevation is a cosine between the
preset's `low` at midnight and `high` at noon. Below `moon_below` the moon
leads, opposite the sun, so exactly one of the two is the key at any time.
`light_key` uses the same arithmetic, in the same order, as the realms'
uniform writer, `verse_style_at` in `verse/style-preset.coax`, which puts
the direction into the block's `SU_KEY_DIR` words.

## A shadow map that holds still

The shadow map covers a square of `reach` metres each way around the eye,
60 m on a desktop and 40 m on a phone, in a map of `size` texels. If its
middle followed the eye exactly, every texel would move a little each frame
and shadow edges would crawl. Instead the middle snaps to whole texels:

<!-- source: aethyr/render/light.coax space -->
```jai
// The shadow map's space for a key light toward (lx, ly, lz): `ax` across it, level with the
// ground, `ay` across it the other way, and the map's middle (mx along ax, my along ay) at the
// eye's feet, snapped to whole texels of a map `size` texels square that reaches `reach` metres
// each way. The realms' shaders build the same space (gpu/fixture.coax fixture_light_space).
LightSpace :: struct {
  axx: f32; axz: f32;            // ax is level: its y is 0
  ayx: f32; ayy: f32; ayz: f32;
  mx: f32; my: f32; reach: f32; texel: f32;
}
light_space :: (lx: f32, ly: f32, lz: f32, eye_x: f32, eye_z: f32, reach: f32, size: f32) -> LightSpace {
  s: LightSpace;
  s.axx = 1;
  if abs(ly) < 0.999 {   // cross((0, 1, 0), l) made unit; straight overhead, +x will do
    n := sqrt(lz * lz + lx * lx);
    s.axx = lz / n; s.axz = -lx / n;
  }
  s.ayx = ly * s.axz; s.ayy = lz * s.axx - lx * s.axz; s.ayz = -ly * s.axx;   // cross(l, ax)
  s.mx = verse_style_shadow_snap(eye_x * s.axx + eye_z * s.axz, reach, size);
  s.my = verse_style_shadow_snap(eye_x * s.ayx + eye_z * s.ayz, reach, size);
  s.reach = reach;
  s.texel = 2 * reach / st_max(size, 1);
  return s;
}
// Where a world point falls on the map, in texels from the map's corner, along ax (`which` 0)
// or ay (`which` 1). The map's middle moves only in whole texels, so a point that holds still
// keeps the same fraction of a texel however the eye moves: shadow edges don't crawl.
light_map_texel :: (s: LightSpace, x: f32, y: f32, z: f32, which: int) -> f32 {
  along := ifx which == 0 then x * s.axx + z * s.axz - s.mx else x * s.ayx + y * s.ayy + z * s.ayz - s.my;
  return (verse_style_shadow_across(along, s.reach) * 0.5 + 0.5) * (2 * s.reach / s.texel);
}
```

`ax` is level with the ground and square to the light, and `ay` is square
to both. A point that holds still keeps the same fraction of a texel however
the eye moves, because the map under it moves only in whole texels.
`fixture_light_space` in the fixture realm's shader builds the same space on
the GPU. The haze is `verse_style_haze`: exponential past a start distance,
thinner higher up, and capped, so a far ridge keeps some contrast.

## Local lights

<!-- source: aethyr/render/light.coax local -->
```jai
LIGHT_LOCAL_MAX :: 16;   // local lights one frame may hold
// A lantern or a spell: where it is, how far it reaches, and its colour (linear, 0 or more).
LocalLight :: struct { x: f32; y: f32; z: f32; radius: f32; r: f32; g: f32; b: f32; }
LocalLights :: struct { lights: [LIGHT_LOCAL_MAX] LocalLight; count: int; refused: int; }

// A local light's share at distance d of its radius r: (1 - d/r)^2, none at r and past it
// (verse/haunt-light.coax verse_haunt_attenuate, the Haunted House's).
light_falloff :: (d: f32, r: f32) -> f32 {
  q := st_max(0, 1 - d / st_max(r, 0.01));
  return q * q;
}

// Adds a light to this frame's list, or refuses it whole: past LIGHT_LOCAL_MAX, a radius that
// isn't above 0, or a colour below 0. A refusal is counted.
light_local_add :: (s: [] LocalLights, l: LocalLight) -> bool {
  if s[0].count >= LIGHT_LOCAL_MAX || !(l.radius > 0) || !(l.r >= 0 && l.g >= 0 && l.b >= 0) {
    s[0].refused += 1;
    return false;
  }
  s[0].lights[s[0].count] = l;
  s[0].count += 1;
  return true;
}

// The light every local light puts on the point (x, y, z), summed.
light_local_at :: (s: LocalLights, x: f32, y: f32, z: f32) -> MatRgb {
  c := mat_rgb(0, 0, 0);
  for i: 0..<s.count {
    l := s.lights[i];
    dx := x - l.x; dy := y - l.y; dz := z - l.z;
    k := light_falloff(sqrt(dx * dx + dy * dy + dz * dz), l.radius);
    c.r += l.r * k; c.g += l.g * k; c.b += l.b * k;
  }
  return c;
}
```

A lantern or a spell is a point, a radius and a colour. Its share falls off
as (1 − d/r)², the Haunted House's `verse_haunt_attenuate`, and ends at its
radius. A frame holds at most 16. `light_local_add` takes a light whole or
refuses it: past 16, with a radius that isn't above 0, or with a colour below
0. Each refusal is counted.

## A worked example

Work these out by hand first:

1. **The key at noon.** The painterly preset's sun runs from −20° at
   midnight to 60° at noon, with noon's azimuth 0.6 rad. At noon the
   elevation is ½(−20 + 60) − ½(60 + 20)·cos π = 20 + 40 = 60°. The
   direction is (cos 60° sin 0.6, sin 60°, cos 60° cos 0.6) =
   (0.5 · 0.5646, 0.8660, 0.5 · 0.8253) = (0.2823, 0.8660, 0.4127).
2. **The key at midnight.** The elevation is 20 − 40 = −20°, below −6°, so
   the moon leads, opposite the sun. Its elevation is +20°.
3. **One texel.** A map of 2048 texels reaching 60 m each way has texels
   of 120 / 2048 = 0.05859375 m.
4. **The middle at 1 m.** With the light straight overhead, `ax` is +x. The
   eye at 1.00 m is 1 / 0.05859375 = 17.07 texels from the origin, which
   rounds to 17, so the middle is 17 · 0.05859375 = 0.99609375 m.
5. **A step that stays.** At 1.02 m the eye is 17.41 texels out. That still
   rounds to 17, and the middle doesn't move.
6. **A step that moves.** At 1.03 m it is 17.58 texels out, which rounds to
   18. The middle moves one whole texel, to 1.0546875 m.
7. **A post that holds still.** A post at 2.5 m is
   (2.5 − 0.99609375) / 0.05859375 = 25.667 texels past the middle, on texel
   1024 + 25.667 = 1049.667. After the middle moves, it is 24.667 past it,
   on texel 1048.667: one texel over, with the same two-thirds of a texel.
8. **A lantern 1 m away.** (1 − 1/4)² = 0.5625 of its colour.
9. **Haze at 100 m.** 1 − e^(−0.011 · 80) = 1 − 0.4148 = 0.5852.

The example [aethyr/examples/light-a-frame.coax](../examples/light-a-frame.coax)
runs these. It also writes the painterly preset's uniform block with
`verse_style_at` at each clock key and compares its key direction with
`light_key`'s, bit for bit:

<!-- source: aethyr/examples/light-a-frame.coax example -->
```jai
main :: () {
  // 1. The key light at the four clock keys, from the painterly preset's sun.
  p := verse_style_preset(STYLE_PAINTERLY, STYLE_DESKTOP);
  u: [STYLE_FLOATS] f32;
  same := true;
  for i: 0..<4 {
    clock := cast(f32) i * 0.25;
    k := light_key(p.sun_low, p.sun_high, p.sun_azimuth, p.moon_below, clock);
    print("clock %: % at elevation % deg, toward % % %\n", clock, ifx k.moon then "moon" else "sun", r4(k.elevation / DEG), r4(k.x), r4(k.y), r4(k.z));
    if !verse_style_at(p, clock, u[..]) { print("no preset\n"); return; }
    moon := ifx k.moon then 1.0f32 else 0.0f32;
    same = same && u[SU_KEY_DIR] == k.x && u[SU_KEY_DIR + 1] == k.y && u[SU_KEY_DIR + 2] == k.z && u[SU_KEY_DIR + 3] == moon;
  }
  print("the realms' uniform block holds the same key, bit for bit: %\n", same);

  // 2. A shadow map 2048 texels square reaching 60 m, the light straight overhead.
  s := light_space(0, 1, 0, 1.0, 0, 60, 2048);
  print("texel % m; eye at 1.00 m: middle %\n", s.texel, s.mx);
  for x: [3] f32.[1.02, 1.03, 1.08] {
    t := light_space(0, 1, 0, x, 0, 60, 2048);
    print("eye at % m: middle %, the post at 2.5 m on texel %\n", x, t.mx, r4(light_map_texel(t, 2.5, 0, 0, 0)));
  }

  // 3. Local lights: a lantern of radius 4 m, then fifteen more and one too many.
  lights: [1] LocalLights;
  lamp := LocalLight.{x = 0, y = 2, z = 0, radius = 4, r = 1, g = 0.8, b = 0.5};
  added := light_local_add(lights[..], lamp);
  c := light_local_at(lights[0], 1, 2, 0);
  print("lantern added %: 1 m away it gives % % %\n", added, c.r, c.g, c.b);
  for i: 1..<LIGHT_LOCAL_MAX { added = added && light_local_add(lights[..], lamp); }
  last := light_local_add(lights[..], lamp);
  dark := LocalLight.{x = 0, y = 0, z = 0, radius = 0, r = 1, g = 1, b = 1};
  print("% lights: the 17th added %; refused %\n", lights[0].count, last, lights[0].refused);
  lights2: [1] LocalLights;
  print("a light of radius 0 added %\n", light_local_add(lights2[..], dark));

  // 4. The day's haze: none to 20 m, then thicker, capped at 0.88.
  for d: [4] f32.[20, 100, 200, 5000] {
    print("haze at % m: %\n", d, r4(verse_style_haze(d, 0, 0, 20, 0.011, 0.88, 60)));
  }
}
```

```shell
coax/bin/coax run aethyr/examples/light-a-frame.coax
```

```text
clock 0.0: moon at elevation -20.0 deg, toward 0.7756 0.342 -0.5306
clock 0.25: sun at elevation 20.0 deg, toward -0.1732 0.342 0.9236
clock 0.5: sun at elevation 60.0 deg, toward 0.2823 0.866 0.4127
clock 0.75: sun at elevation 20.0 deg, toward 0.9236 0.342 0.1732
the realms' uniform block holds the same key, bit for bit: true
texel 0.05859375 m; eye at 1.00 m: middle 0.99609375
eye at 1.02 m: middle 0.99609375, the post at 2.5 m on texel 1049.6666
eye at 1.03 m: middle 1.0546875, the post at 2.5 m on texel 1048.6666
eye at 1.08 m: middle 1.0546875, the post at 2.5 m on texel 1048.6666
lantern added true: 1 m away it gives 0.5625 0.45000002 0.28125
16 lights: the 17th added false; refused 1
a light of radius 0 added false
haze at 20.0 m: 0.0
haze at 100.0 m: 0.5852
haze at 200.0 m: 0.8619
haze at 5000.0 m: 0.88
```

The 17th light and the light of radius 0 are the broken cases: each is
refused whole, and the frame's list keeps what it had. The elevations and
directions are printed to four places, because `sin`, `cos` and `exp` come
from each platform's maths library.

## Checks

```shell
coax/bin/coax test aethyr/render/light.coax
coax/bin/coax mutate aethyr/render/light.coax --root-only
coax/bin/coax test verse/style.coax
```

`aethyr/render/light.coax` has 21 checks of its own (it loads
`material.coax`, whose 17 also run). The seven that moved with their
functions check the map's reach and snap, its depth, the contact shadow and
the haze. The new checks cover:

- the hemisphere ambient straight down, straight up, level and scaled;
- the key at the four clock keys, a unit direction through the day, the
  moon leading exactly while the sun is below `moon_below`, the azimuth
  wrapping bit for bit, and the moon as the sun turned about;
- the light's space square to the light, straight overhead, at the limit
  of 0.999, and its middle worked from the eye's feet along both axes;
- the middle moving only in whole texels over a 3 m walk, with a still
  point keeping its fraction of a texel;
- each phone and channel switch at its own edge, and haze over raised
  ground;
- local lights: the falloff, the list's limit, each malformed light, and a
  light reaching through all three axes.

`coax mutate --root-only` kills all 160 mutants. Two places stay in
`verse/style.coax` because they can't pass this gate unchanged:
`verse_style_shadow_tap` writes 0 to taps that are already 0, and
`verse_style_shadow_radius` adds `0 * on_phone`. Both are mutants that no
check can tell apart. Removing the redundant code would change every
realm's WGSL, so it waits for a change that regenerates them.

## Still to do

- **One lighting record per frame.** Realms still read the key, ambient,
  fog and shadow numbers from the preset's uniform block and build the
  light's space in their own shaders (`fixture_light_space` in the fixture,
  the same code in Everglade). One record, filled once from the clock and
  read by every material, replaces those copies.
- **The shadow pass.** Drawing the shadow map first in the scene pass, with
  its snapped projection and filter taps, is `aethyr/render/shadow.coax`.
  The tap positions and radius move with it.
- **Tone mapping and encoding.** `verse_style_tone` and the Metal path's
  `verse/light.coax` stay in `verse/`.
- **Local lights in a realm.** No realm draws from the bounded list yet;
  the Haunted House uses the same falloff in its own shader. Counting them
  against [chapter 16](ch016-budgets-that-refuse.md)'s budgets is still to
  do.
- **CPU and GPU agreement.** Shading agreement over a recorded frame is
  chapter 11's Grid profile. A frame from a lit, shadowed realm is not yet
  recorded.

## Dependencies

Chapters [11](ch011-the-wgsl-renderer.md) and
[12](ch012-materials.md), and chapter [1](ch001-a-frame-has-a-fixed-step.md)'s
clock. From Coax: [chapter 17](../../coax/book/ch17-gpu.md), GPU interfaces.

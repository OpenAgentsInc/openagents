# 6. The glade's light

> **Status: partly implemented.** The light is one shared profile in
> [`metaverse-engine/world/environment.coax`](../../metaverse-engine/world/environment.coax)
> (`env_glade_profile`) and Everglade II's places in
> [`everglade/world/environment.coax`](../../everglade/world/environment.coax)
> (`e2_environment_set`). Phase 1 of the
> [lighting plan](../../docs/metaverse/lighting/README.md) is done for the
> browser. The code moves into `everglade/world/` later, so this chapter names
> functions and has no source listings yet.

This chapter answers the player's question: "Why does it get darker under the
trees, and why doesn't the light jump when I walk?"

Character Create's glade is the look of the whole game. Its light is the
default for every realm in the stylized Quaternius art style. A place that
wants a different light, such as the deep woods, says only how it differs from
the glade.

## What this chapter teaches

- How one lighting record becomes the default for an art style.
- How places change the light as small deltas, with no visible step.
- How tree crowns darken the ground through a baked occlusion.
- How shadows stay fixed to the land as you walk.

## What exists today

| Piece | Where |
| --- | --- |
| The glade profile: sun, sky, ground bounce, haze, film | `env_glade_profile` and `ENV_GLADE_*` in `metaverse-engine/world/environment.coax` |
| Places as regions, admitted whole and resolved by position | [`metaverse-engine/world/environment-region.coax`](../../metaverse-engine/world/environment-region.coax) (`env_region_index_replace`, `env_resolve_camera`) |
| Everglade II's places | `e2_environment_set`, `e2_env_sunlit_profile`, `e2_env_canopy_profile` in `everglade/world/environment.coax` |
| The canopy's baked occlusion | `e2g_bake`, `e2g_occluded`, `e2g_ao_of` in [`everglade/world/ground.coax`](../../everglade/world/ground.coax), on [`metaverse-engine/render/bake.coax`](../../metaverse-engine/render/bake.coax) |
| Shadow cascades | [`metaverse-engine/render/cascades.coax`](../../metaverse-engine/render/cascades.coax) |

## The method

### One default, and deltas on it

The glade profile holds the reference image's numbers
([chapter 1](ch001-a-reference-image.md)): a 4500 K sun at 2.15, a cool sky
fill, a warm ground bounce, air that is clear to 30 m and hazy to 65 m, and a
film response at exposure 1.18.

Everglade II has three places:

- **The open glade.** The map's default, the glade profile itself. It holds
  wherever no region reaches: the glade, the clearing and the far ridges. So
  Character Create's glade and the world are one light, with no seam between
  them.
- **The sunlit path.** The first stretch out of the glade, from z = -42 to
  z = -270, full from -64 to -248. The sky light is a little greener, the
  ground bounce a little less, the sun a touch weaker and the air a touch
  thicker.
- **The deep canopy.** The middle of the path, from z = -235 to z = -395, full
  from -265 to -365 and gone by -425. The sky light and bounce are cooler and
  dimmer, the sun is weaker and the haze is denser and lower.

A delta may change only the key light's strength, the ambient light and the
haze. The film, exposure, saturation, sky and clouds stay the glade's, so the
whole world still reads as the glade.

Each region is a prism that follows the path: four samples of `e2_road_x` on
each side, 70 m out (`E2_ENV_HALF`). The region index is admitted whole
(`e2_env_admit`), or refused whole.

### The canopy's occlusion

The crowns of the trees are baked onto the ground tiles as ambient occlusion.
Each crown is a sphere. A crown of radius r whose centre is D away, in
direction d, hides this share of the cosine-weighted sky at a point with normal
n:

```text
hidden = (r / D)² × max(0, n · d)
AO     = clamp(1 − Σ hidden, 0.6, 1)
```

The bake is keyed by the tile's revision, the crowns' digest and the realm's
variant. A bake for other land or other crowns refuses, and the live bake
stays. Over the last 6 m inside a tile's edge, AO eases to open sky, so leaving
a tile is no step in light.

Your character takes the AO at your middle on its indirect light only, never
on the sun. You darken under the canopy and stay lit in a sunbeam. The ground
takes a quarter of it (`E2G_GROUND_AO_SHARE` is 0.25).

### Shadows that stay put

The sun's shadows are four cascade tiles in a 2048 atlas, reaching 24, 60, 150
and 360 m round you (two tiles, to 20 and 75 m, on a phone). Each tile's middle
snaps to whole texels in the light's space and moves only in steps of 8 to
120 m. So a shadow edge doesn't crawl as you walk.

## A worked example: one crown overhead

At scale 1, a common tree's crown proxy has its centre 5.0 m up and a radius
of 2.6 m (`E2G_CROWN`, scaled by the piece's own scales in `e2g_crowns`). Stand
right under it on flat ground, so n · d = 1:

```text
hidden = (2.6 / 5.0)² × 1 = 0.27
AO     = 1 − 0.27 = 0.73
```

Under one crown, the ground sees about 73% of the sky. Under a dense canopy the sum
passes 0.4, and the AO stops at its floor of 0.6 (`E2G_AO_FLOOR`), so the
deepest woods never go black.

Now the regions. The deep canopy's band is 60 m long along the path. The
steepest change a region makes is 1.5 divided by its band, per metre:

```text
1.5 / 60 m = 0.025 per metre
```

So walking one metre changes at most 2.5% of the canopy's delta. You see the
woods darken over a minute of walking, never in one step.

## The checks

In `everglade/world/environment.coax`:

- `#test "Everglade II's set is admitted whole: the glade the default, the
  path's two deltas"`.
- `#test "the arrival is Character Create's glade exactly, and so is the
  clearing"`.
- `#test "the deltas are the glade's look: only the key's strength, the ambient
  and the haze change"`.
- `#test "walking the whole path, the light changes continuously: nothing
  steps"`.

In `everglade/world/ground.coax`:

- `#test "the canopy AO matches an independent sampling of the sky, and the bake
  publishes onto its key and state only"`.

In `everglade/client/client.coax`:

- `#test "every frame writes the camera's environment: the glade's noon in the
  glade, the canopy's in the woods"`.
- `#test "Everglade II's shadow tiles: snapped to the light's texels, redrawn
  only past a step, the near one every frame"`.

The page reports the bake's status, the AO at you and its crowns as
`e2_probe` 9, 10 and 11. Receipts are in
[`docs/verification/aethyr-lighting-p1-2026-11-08/`](../../docs/verification/aethyr-lighting-p1-2026-11-08/).

## Mistakes

- **The woods twice as dark.** The ground already darkens under the woods in
  its own shading. When the full bake went on top (#13052), the path through
  the woods came out 15% to 30% darker than before. The ground now takes a
  quarter of the bake's AO. Measured against the frames before, the result is
  within 4% at the darkest spots under the crowns and within 2% elsewhere.
- **A light with a seam.** If the glade and the world had separate lights, the
  hand-over from Character Create would show a step. The open glade is the
  map's default instead, and the arrival test asserts that it is exactly the
  glade's light.

## What comes next

[Chapter 7](ch007-character-create-in-the-world.md) makes Character Create a
place in this light that you walk out of.

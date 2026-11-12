# 1. A reference image

> **Status: partly implemented.** The numbers this chapter takes from the
> reference image are in Aethyr's shared glade profile
> ([`aethyr/world/environment.coax`](../../aethyr/world/environment.coax),
> `env_glade_profile`) and Everglade II's land
> ([`verse/everglade2/forest.coax`](../../verse/everglade2/forest.coax)). The
> code still lives outside `everglade/`, so this chapter names functions and
> has no source listings yet.

Everglade is an MMORPG on the D&D 5E SRD 5.2.1. Before you write a rule or a
spell, you decide what the world looks like. This chapter answers the first
question a player asks when the page opens: "Where am I?"

You answer it with one picture. Everglade's picture is a Quaternius forest
scene in the stylized art style of the Quaternius Stylized Nature kit: warm
autumn light through a canopy, a dirt path and gold crowns among green ones.
Every later chapter compares its result with that picture.

## What this chapter teaches

- How to turn one picture into numbers: light, colour, density and scale.
- Where each number lives in the code.
- How to check that the world still matches the picture after each change.

## What exists today

The reference and the captures that compare with it are in
[`docs/verification/character-glade-reference-light-2026-11-07/`](../../docs/verification/character-glade-reference-light-2026-11-07/).
`reference-compare.jpg` and `reference-compare-2.jpg` put the reference next to
Character Create's glade.

The numbers taken from the picture live in three places:

| What | Where | Names |
| --- | --- | --- |
| Light and air | `aethyr/world/environment.coax` | `ENV_GLADE_*`, `env_glade_profile` |
| Ground colour and autumn | `verse/everglade2/forest.coax` | `e2_autumn`, `e2_gold` |
| Density and scale | `verse/everglade2/plant.coax`, `verse/everglade2/forest.coax` | `E2_TREE_STEP`, `e2_forest`, `E2_CLEAR_R` |

The [lighting plan](../../docs/verse/lighting/README.md) records the owner's
direction: the Character Create glade is the look, and every realm in the art
style starts from its light.

## The method

Look at the picture and write down four kinds of numbers. Each number becomes a
constant or a pure function that the CPU and the shaders share.

1. **Light.** Find the sun's colour, strength and direction, and the colour of
   the sky light and the ground bounce. The reference has a warm sun under a
   cool sky, so the key light is 4500 K under a 6500 K white balance at 2.15
   (`ENV_GLADE_KELVIN`, `ENV_GLADE_WHITE`, `ENV_GLADE_INTENSITY`). The sky fill
   is cool (`ENV_GLADE_UPPER_*`) and the ground bounce is warm
   (`ENV_GLADE_LOWER_*`).
2. **Air.** Find how far you see clearly and where the haze starts. The
   reference is clear near the camera and hazy at the tree line, so the air is
   clear to 30 m and thickens to 65 m (`ENV_GLADE_FOG_NEAR`,
   `ENV_GLADE_FOG_FAR`). The haze scatters forward toward the sun
   (`ENV_GLADE_FOG_PHASE_G`, 0.62).
3. **Colour.** Find how much of the land is gold and where. The reference has
   gold patches among green, not a gold forest. `e2_autumn` colours the tree
   crowns with broad patches from value noise. `e2_gold` gives the ground and
   grass a smaller share of the same patches (0.3 of it outside the glade), so
   the forest floor stays mostly green.
4. **Density and scale.** Count trees and measure the path. The reference shows
   trunks close together beside a path about a person and a half wide. The
   planting puts trees on a jittered 2.6 m grid (`E2_TREE_STEP`), and the first
   trunks stand 2.6 m from the path's middle.

A number that you can't find in the picture is a guess. Mark it as one in the
source comment, so a later change can replace it.

## A worked example: how dense the woods are

`e2_plant_cell` plants one 16 m planting cell (`E2_CELL`). It tries a tree on
each point of a grid with a 2.6 m step:

```text
steps per side = int(16 / 2.6) = int(6.15) = 6
tree points    = 6 × 6 = 36 per 16 m cell
```

Each point keeps its tree when a hash is at most `0.97 × e2_forest(x, z)`. Where
the woods are thickest, `e2_forest` is about 1, so about 35 of the 36 points
hold a tree:

```text
cell area      = 16 × 16 = 256 m²
trees          ≈ 35
area per tree  ≈ 256 / 35 ≈ 7.3 m²
```

That is one trunk for each square about 2.7 m on a side. The reference shows
the same spacing: you can't see far between the trunks.

The scale check is a walk. The path is about 440 m long, and you jog at
4.25 m/s (`E2_RUN_SPEED` in
[`everglade/shared/move.coax`](../shared/move.coax)):

```text
440 m / 4.25 m/s ≈ 104 s
```

A walk of under two minutes from the glade to the clearing is long enough to
feel like a journey and short enough to repeat while testing.

## The checks

The picture itself isn't a test. These checks hold the numbers that came from
it:

- `verse/everglade2/environment.coax`, `#test "the arrival is Character
  Create's glade exactly, and so is the clearing"`: the glade and the clearing
  resolve to the glade profile.
- `verse/everglade2/environment.coax`, `#test "the deltas are the glade's look:
  only the key's strength, the ambient and the haze change"`: a place on the
  path may change only those values.
- `verse/everglade2/forest.coax`, `#test "the ground keeps the glade's gold and
  stays mostly green outside it"`: outside the glade, the ground's gold stays at
  0.31 or less.
- The lighting plan's identity rule: after each lighting change, Character
  Create's glade must render the same as before, within a declared bound.

Run the land and light checks:

```sh
coax/build.sh
coax/bin/coax test --library verse/everglade2/forest.coax
coax/bin/coax test verse/everglade2/environment.coax
```

## Mistakes

- **Copies of the numbers.** The glade's light first lived as constants in an
  app file (`apps/openagents-web/gpu/character-glade-light.coax`). Another
  realm that wanted the look had to copy them, and the copies could drift
  apart. Since #13060 the numbers live once in `aethyr/world/environment.coax`,
  and the app's `cg_*` functions only delegate to them.
- **A number with no source.** If you can't point to where a number came from
  in the picture, you can't tell later whether a change made the world better
  or worse. Keep the comparison captures next to the reference, as the
  verification folder above does.

## What comes next

[Chapter 2](ch002-what-the-prototypes-taught.md) looks at what the earlier
prototypes settled before Everglade II started from this picture.

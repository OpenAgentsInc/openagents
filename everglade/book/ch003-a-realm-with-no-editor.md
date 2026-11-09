# 3. A realm with no editor

> **Status: partly implemented.** Everglade II builds and runs from text today.
> Its files are spread across `apps/openagents-web/`, `verse/everglade2/` and
> `verse/tools/assets/`. They move into `everglade/` after the work in progress
> lands, one directory per commit. Until then this chapter names files and has
> no source listings.

You build Everglade with no level editor. Every tree, path, rule and light is a
Coax function in a text file. This chapter answers the builder's question:
"What files make a realm, and how do I know the realm still works after I
change one?"

## What this chapter teaches

- The files that make up Everglade II, and what each one owns.
- How to build the page and the shaders from the command line.
- How to check a change: tests in the source, then a headless capture.

## A realm's files

| What | File | Owns |
| --- | --- | --- |
| The registry entry | [`apps/openagents-web/realms.json`](../../apps/openagents-web/realms.json), entry `everglade2` | Its address, title, module, assets, golden settings and domain |
| The page | [`apps/openagents-web/everglade2.coax`](../../apps/openagents-web/everglade2.coax) | The frame, the camera, drawing, the class bar and the room's view |
| The host face | `apps/openagents-web/everglade2-face.c` | The exports the browser host calls, such as `coaxial_frame` |
| Movement | [`apps/openagents-web/everglade2-move.coax`](../../apps/openagents-web/everglade2-move.coax) | Where you may stand, the pace, trunks and rock hulls |
| Character Create | [`apps/openagents-web/everglade2-create.coax`](../../apps/openagents-web/everglade2-create.coax) | The studio, the arrival and the run into the world |
| The GPU passes | [`apps/openagents-web/gpu/everglade2.coax`](../../apps/openagents-web/gpu/everglade2.coax) | The ground, kit pieces, grass and characters, in Coax compiled to WGSL |
| The land | [`verse/everglade2/forest.coax`](../../verse/everglade2/forest.coax) | Height, path, woods, clearing and colour, as pure functions |
| The planting | [`verse/everglade2/plant.coax`](../../verse/everglade2/plant.coax) | Which kit piece stands where |
| The ground tiles | [`verse/everglade2/ground.coax`](../../verse/everglade2/ground.coax) | The admitted terrain tiles and their baked light |
| The light | [`verse/everglade2/environment.coax`](../../verse/everglade2/environment.coax) | The places' light as an environment set |
| The pack tool | [`verse/tools/assets/everglade2_pack.coax`](../../verse/tools/assets/everglade2_pack.coax) | The asset pack the page loads |

The land functions are the centre of the design. The pack tool, the planting,
the walk and the shaders all call the same `e2_height`, `e2_path` and
`e2_forest`. If you change the path, every consumer sees the same new path.

## The method

You change a realm in four steps:

1. Edit a function in a text file.
2. Run the tests in that file. Each file carries its own `#test` blocks.
3. Build the page and the shaders.
4. Open the page in a headless browser and read what it reports.

### Build

Build the compiler once, then the page and its shader module:

```sh
coax/build.sh
cd apps/openagents-web
../../coax/web/wasm.sh everglade2.coax everglade2-face.c build/everglade2.wasm \
  everglade-step.coax ../../aethyr/sticks.coax
cd ../..
coax/web/tool.sh coax/web/wgsl-module.coax apps/openagents-web/gpu/everglade2.coax \
  apps/openagents-web/everglade2.wgsl apps/openagents-web/gpu/everglade2.module
```

The pack changes less often. Its build command, and the step that adds the
characters' material maps, are in
[`docs/verse/everglade2.md`](../../docs/verse/everglade2.md#the-pack).

### Serve

Serve the app statically, without the whole site's build:

```sh
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:8863 --static --cache 0
```

Open `http://127.0.0.1:8863/worlds/everglade2`. Two query parameters help when
you test: `?at=300` starts you 300 m down the path, and `?walk` walks you down
the path on your own.

### Check in the source

Tests sit next to the code that they check. Run one file's tests:

```sh
coax/bin/coax test --library verse/everglade2/forest.coax
coax/bin/coax test --library verse/everglade2/plant.coax
coax/bin/coax test --library apps/openagents-web/everglade2-move.coax
coax/bin/coax test --library apps/openagents-web/everglade2-walk-test.coax
```

A test states a fact about the world in words, then asserts it. For example,
`forest.coax` has `#test "the path winds and never climbs steeper than one in
five"`, and `plant.coax` has `#test "the woods are planted, dense beside the
path, none on it or in the clearing"`.

### Check in a headless browser

Some facts only show in a real frame: load time, frame time, GPU time and
memory. A capture tool opens the page in headless Chrome, drives it and writes
what it measured:

```sh
coax/web/tool.sh coax/web/everglade2-terrain-check.coax http://127.0.0.1:8863 \
  /tmp/terrain.tsv --size 2400x1400 --dpr 2
```

`everglade2-terrain-check.coax` walks from the glade, from 120 m down the path
and across the ground's seam at 360 m. It passes only when the ground loads from
the pack with no refusal and no memory growth. Its receipt is in
[`docs/verification/everglade2-terrain-2026-11-08/`](../../docs/verification/everglade2-terrain-2026-11-08/).

The page reports its state through `e2_probe` in `everglade2.coax`. Each probe
number is one fact, such as the ground's route (probe 90) or the canopy light
at your position (probe 10). A capture tool reads probes, not pixels, wherever
it can.

The golden settings in `realms.json` (`"golden": { "lit": 0.01, "settle":
3000, "blocks": 6 }`) let the release compare each page with a stored image.
`coax/web/golden-check.sh --commit HASH` names the pages a commit may have
changed.

## A worked example: one change, end to end

Suppose you want the path to wind less. The path's middle is `e2_road_x(z)` in
`forest.coax`. Past z = -75 its wind is a sum of three sines:

```text
wind = 15 sin(0.022 t) + 6 sin(0.051 t + 1.3) − 6 sin(1.3) + 2 sin(0.12 t),   t = z + 30
```

If you halve the first term to 7.5, these checks decide whether the change is
safe:

1. `forest.coax`, `"the path winds and never climbs steeper than one in five"`,
   asserts that the path still swings more than 25 m from side to side. A
   path that is too straight fails it.
2. `plant.coax` replants the woods from the new path and asserts that no piece
   stands on it.
3. `everglade2-walk-test.coax` runs the whole path through the real controller
   and asserts that nothing holds you up.
4. The ground tiles are solid along a ribbon round the path (`e2g_solid` and
   `e2g_ribbon_x` in `ground.coax`), so they follow the new path once you
   rebuild the pack. The terrain check then confirms that the page loads the
   ground from the pack with no refusal.

You never open an editor, and every claim about the new path is checked.

## Mistakes

- **A check that needs the whole site.** A plain `coax serve` without
  `--static` runs the whole site's `build.sh` before it answers. Serve
  statically while you work on one realm.
- **A fact read from pixels.** A screenshot can't tell you which route the
  ground loaded from. Add a probe to `e2_probe` and read the number.

## What comes next

Part II builds the glade. [Chapter 4](ch004-ground-you-can-stand-on.md) starts
with the ground.

# Everglade Experimental: local NWN biome sections

Verified local specimen integration, **2026-11-08**. Actual muted headless Chrome
captures on hardware Apple Metal 3 show all three terrain sections; the final
forest view includes full-height standing trees. Public realm assets are unchanged.

The fixture adds three 20×20 m sections of native Neverwinter Nights terrain to
the existing Experimental meadow. Each section is a contiguous 2×2 selection of
10 m area tiles, with the source area's model references and quarter-turn
orientations. The sections use HotU's **Peaceful Clearing** (Forest `ttf01`, area grid 4,2),
the original campaign's **South Road - Farmland** (Rural `ttr01`, grid 7,8) and
SoU's **Oasis of the Green Palm** (Desert `ttd01`, grid 9,0). Three 1024² atlases
retain original painted pixel resolutions and alpha; UV-boundary subdivision
preserves native surface geometry.

## Sites

| Local view | Center X,Z | Footprint |
| --- | --- | --- |
| A | 0, 73 | X −10…10, Z 63…83 |
| B | 30, 74 | X 20…40, Z 64…84 |
| C | 71, 72 | X 61…81, Z 62…82 |

The 1,200 m² exhibit covers approximately 3.7% of the 180×180 m meadow. The
underlying existing terrain is retained; imported geometry is lifted by 0.025 m
relative to its decoded native coordinates. The base meadow at these sites is
flat at approximately Y = −0.035 m. Its collision/walking surface is not replaced
by imported NWN collision.

A scan of all 2,353,222 base static environment triangles finds zero non-ground
triangle bounding-box intersections with any of these rectangles. No existing
village, Nature display, prop, tree or rock needs removal. The earlier proposed
northwestern display area was rejected because it contains existing Nature
specimens. The source-backed Aribeth and ten-pair comparison gallery remain in
the fixture.

## Grass masking

Only grass seed centers inside each footprint plus a 0.5 m margin are retired:
221 in A, 203 in B and 202 in C, for **626 of 52,872** original seed records. Their
Y coordinate becomes −1000 m. Nonzero blade sizes, seed counts, stream origins,
and every outside seed byte remain unchanged. The maximum measured blade
footprint including modeled wind is about 0.275 m, below the masking margin.

Do not zero a seed's sizes: `cg_instanced_vs` divides its normal by those scales.
The existing ground vertices are retained beneath the overlays. These changes
are a bounded local visual experiment, not a new terrain or plant rendering
contract.

## Local URLs

The verified views use the existing local server and private asset route:

- [All three sections](http://127.0.0.1:8949/worlds/everglade-experimental?biome=0).
- [Section A](http://127.0.0.1:8949/worlds/everglade-experimental?biome=1).
- [Section B](http://127.0.0.1:8949/worlds/everglade-experimental?biome=2).
- [Section C](http://127.0.0.1:8949/worlds/everglade-experimental?biome=3).

The private `aribeth-local` asset-path target points to
`~/ore-nwn-private/biomes/scene-v2/`. The URL and pack switch are local-only; this document does not assert a
public deployment or commit the extracted NWN payloads.

## Sources and limits

- Realm: `apps/openagents-web/realms/everglade-experimental/`, module `fivelab`.
- Source geometry/seed recipe: `verse/tools/assets/character_glade_pack.coax`.
- Grass/environment shader: `everglade/scene/gpu/character-glade.coax`.
- Existing terrain placement: `verse/experimental-terrain.coax`.
- Private import recipe in the companion ore repository:
  `tools/nwn_biomes.py`; native decoder: `tools/nwn_model.py`.
- Source audit and metadata in ore:
  `docs/neverwinter-nights/biomes.md` and
  `docs/neverwinter-nights/biomes/clearance.json`.
- Raw source data and derived pack stay under
  `~/ore-nwn-private/biomes/`, outside this Git repository.

Only static native terrain and selected primary painted textures are in scope.
Native lighting, weather, animated materials, secondary lightmaps, imported
collision and scene/gameplay parity are not established. The existing shared
painterly profile remains unchanged. The emitted pack retains all 271 prior draw
streams and 113 prior image/provider entries, adds 12,425 vertices and 4,335
triangles, and uses three new image layers. Pack/source/capture hashes and full
validation are retained in ore's `docs/neverwinter-nights/biomes/`.

## Standing trees

The initial clearing contained native groundcover and a stump. Its corrected
preview adds full-height original `plc_m08`, `plc_m09`, `plc_m08` placeables, about
7.85/7.48/7.85 metres tall, with original trunk/branches/foliage textures. Their
three new preview placements are explicitly separate from the original authored
2×2 terrain block. The forest camera pulls back to show their canopies.
Nine small named `TTF01_black` helper triangles at Y20 are omitted from the
preview to remove floating artifacts; original source exports and full atlases
retain them. The lower Y3 mesh is retained. This does not port the native
engine's helper-plane visibility or tree animation behavior.

## Implementation and checks

`verse/experimental-biomes.coax` owns the three positions and titles.
`charactercreator.coax` reads a positive `glade/nwn-biomes` catalog marker before
`fivelab.coax` projects labels through the existing Coax HUD. Local development
URLs position the camera; the public scene does not gain empty biome labels.
The Coax biome checks and release Wasm build pass under the existing 2,602,000
byte limit; tools-check reports zero new tools without a reason. Ore's native
atlas/terrain checks and independent actual pixel/area/preservation checks pass.

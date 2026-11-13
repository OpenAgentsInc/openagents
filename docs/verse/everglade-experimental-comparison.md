# Ten private NWN comparison pairs

The local Everglade Experimental specimen pack now includes ten labeled pairs:
barrel, chest, chair, table, brazier/torch, pillar, tree, rock, longsword and imp.
NWN is on each pair's left; the pinned Quaternius asset used by the existing
realm is on its right. Both have the same maximum displayed bounding span.
They are static visual specimens, with their source mesh topology and textures.

Open http://127.0.0.1:8949/worlds/everglade-experimental?compare=0 for the overview,
or use `compare=1` through `compare=10` for a particular pair. These camera hooks
run only on the existing local development path. The twenty stands occupy
centers X=18,24.5,31,37.5,44 at Z=8 and Z=16, clear of the original showcases.
The local camera hook closes the class panel to keep the exhibit visible.

`metaverse/experimental-comparison.coax` owns station positions and titles;
`fivelab.coax` draws projected labels through the existing Coax HUD.
`charactercreator.coax` enables labels only after reading a positive-count
`glade/nwn-comparison` catalog marker. The public pack lacks this marker.

Research, conversion tools, source identities, exact counterpart registry pins,
original-draw preservation checks and reproduction commands live in the ore
repository at `docs/neverwinter-nights/comparison10.md`. The shared registry
originals remain under `~/.openagents/assets/`. The private pack lives at
`~/ore-nwn-private/comparison10/gallery-v2/glade.pack.gz` and is served by the
existing private page root `~/ore-nwn-private/aribeth/world-web/`.

The append retains Aribeth and every prior scene draw. It adds 34,044 vertices,
30,569 triangles (including twenty neutral plinths) and 19 texture layers.
The actual NWN source assets and converted pack remain outside Git. The public
realm pack is unchanged. Source animation, native NWN lighting, normal/ORM
material features and emitter simulation are outside this static exhibit.

Verified on 2026-11-08 in muted headless Chrome using hardware Apple Metal 3:
all ten pairs are visible in the overview and both imps are visible in the
close-up. The final Wasm is 2,595,667 bytes under the 2,602,000-byte limit.
Catalog-marker and local-camera checks pass, as do tools-check and the ore
converter suites (14 + 8 + 6 checks). The ore comparison folder retains hashes
of both captures, the pack, converters and exact build result.

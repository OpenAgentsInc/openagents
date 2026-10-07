# The coast

Status: specification, 2026-10-06. Nothing in this document is implemented.
The owner approved an ocean or coastal zone on 2026-10-06 and asked for a
specification only for now. It is tracked by
[#10796](https://github.com/OpenAgentsInc/openagents/issues/10796), which is
blocked by the water system's spectral waves (W4,
[#10776](https://github.com/OpenAgentsInc/openagents/issues/10776)) and its
clipmap ocean and streaming (W10,
[#10782](https://github.com/OpenAgentsInc/openagents/issues/10782)).

The coast is a loaded zone where Verse's ocean is the main attraction: a
temperate bay with a sand beach and surf, sea cliffs, a harbor with
rowboats, tide pools, a lighthouse, a reef and a wreck to dive, and islands
offshore. Every water capability and rule it uses is specified in
[Water](water.md); this document says what the zone adds.

## Contents

- [Identity and entry](#identity-and-entry)
- [Layout](#layout)
- [Water](#water)
- [Life and light](#life-and-light)
- [Streaming and budgets per tier](#streaming-and-budgets-per-tier)
- [Multiplayer](#multiplayer)
- [Assets](#assets)
- [Phases and estimates](#phases-and-estimates)
- [Open questions](#open-questions)

## Identity and entry

- **Zone.** `ZoneId::Coast`, serialized `coast`, world ID `coast-v1`, arch
  sign `COAST`, following the steps in
  [Build and register a new zone](zones.md#build-and-register-a-new-zone).
  "The coast" is a working name.
- **From the Grid.** The plaza's west arch, free since the Ruins zone was
  removed on 2026-10-05, becomes the coast's arch. It loads the zone at the
  arrival terrace above the beach.
- **From Everglade.** A trail gate where Glade Run leaves Walden Woods, at
  the stream's last point (`layout::STREAM`, about `[-9, -128]`), loads the
  coast at the estuary, where the same stream reaches the sea. Verse has no
  zone-to-zone transition today: `Intent::Return` always restores the saved
  plaza pose. Phase C5 adds one, keeping the plaza pose saved when the
  player first left the plaza. Until C5, the plaza arch is the only way in.
- **Return.** A return portal on the arrival terrace goes back to the
  plaza; after C5, the estuary gate goes back to Everglade's trail gate.

## Layout

The playable area is a square of 1.2 km (`half_extent` 600 m), with land to
the northeast and open sea to the south and west. Beyond 600 m the ocean
draws to the horizon on the clipmap but isn't playable.

| Place | What it is |
| --- | --- |
| Arrival terrace | A paved overlook on the dunes above the beach, with the return portal and a view of the whole bay. |
| Driftwood Beach | A crescent of sand about 500 m long and 40 to 80 m wide. Its bed slopes about 1 in 30 below the water, so the swell shoals and breaks (V7) into surf with shore foam (S9). Driftwood, dune fencing, and beach grass. |
| The estuary | Where the stream from Everglade meets the beach: a river body whose flow runs into the surf, a reed marsh (B7), and Everglade's trail gate after C5. |
| The harbor | At the beach's north end, behind a stone breakwater about 150 m long: piers on pilings, a boathouse, moorings and buoys, six boardable rowboats, and two larger moored boats as props. A baked shelter mask cuts the swell inside the breakwater to a tenth. |
| The headland and the cliffs | Sea cliffs 35 to 45 m high along the west, with a sea cave, a rock arch, sea stacks, and footpaths to the top. |
| The lighthouse | A 24 m tower on the headland, with a gallery to walk around, a lamp room, and a keeper's cottage. |
| Tide pools | A rock shelf below the headland that the tide covers and uncovers, with pools 0.2 to 0.8 m deep. |
| The reef and the wreck | Rocky reef with kelp off the cliffs, 6 to 15 m deep, and a sunken sloop 12 m down at the harbor's approach. |
| Gull Island | About 250 m offshore and 120 m across: a rocky shore, a cove beach, and a grassy top with nesting gulls. Reached by rowboat or by a long swim. |
| The sandbar and Tern Islet | A sandbar from the beach's south end to a small islet. It is dry at low tide and under water at high tide, so a player who lingers swims back. |
| Far isles | Low island silhouettes on the horizon, drawn as distant proxies and never playable. |

## Water

- **Bodies.** One ocean body (B4) with sea water's density of 1,025
  kg/m³: Gerstner gameplay swell (V1), spectral detail from W4 (V2), the
  geometry clipmap from W10 (M2), and shoaling surf (V7). The estuary is a
  river body (B2) that flows into the ocean; the marsh is a marsh body
  (B7); each tide pool is a pool body (B5) joined to the ocean while the
  tide covers it.
- **Tide (ours).** Sea level is the mean level plus 1.2 m × sin(2π t / T),
  with a game-scale period T of 24 minutes: a real tide's 12 hours and 25
  minutes is too slow to see in a session. It is a pure function of the
  world tick and the zone seed, like the swell, so it needs no replicated
  state. The sandbar is dry when the tide is more than 0.6 m below the
  mean; the tide pools separate from the sea when it is more than 0.3 m
  below.
- **Presets.** A temperate coastal look from a coastal Jerlov water type
  for the bay, a murkier harbor, and a silty, brackish estuary (M4). Storm
  weather raises the sea state through the wave controls (V3).
- **Swimming and diving.** The rules in
  [Swimming and wading](water.md#swimming-and-wading) and
  [Breath and suffocation](water.md#breath-and-suffocation). The reef and
  the wreck sit deeper than one breath carries most characters there and
  back, so Water Breathing matters. Caustics (W7) light the reef and the
  wreck. The water is not frigid.
- **Bounds (ours).** Past the playable edge, an offshore current (P3) pushes
  swimmers and boats back inside, so there is no invisible wall at sea.
- **Spells.** Every rule in [Spells and water](water.md#spells-and-water)
  applies. The bay is deep enough for Control Water's whirlpool beyond its
  7.5 m contour; a flood cast on the open sea makes the SRD's 20-foot wave,
  which carries rowboats and may capsize them; freezing on swell forms
  floes.
- **Boats.** The rowboat from W6 ([Rowboats](water.md#rowboats)), on real
  swell. A boat can reach Gull Island; storm surf can capsize it.
- **Weather.** The coast's climate is wetter and windier than Everglade's:
  more fog, rain, and storms ([Weather](water.md#weather)). In Storm, Call
  Lightning deals its extra 1d10.

## Life and light

- **Wildlife.** Gulls over the beach and the island, crabs in the tide
  pools, fish schools on the reef, and seals on Gull Island's rocks. All of
  it is visual and seeded on each client, like Everglade's ducks.
- **The lighthouse lamp.** A sweeping beam that turns on at dusk and in fog,
  reflected on the water by the planar mirror and screen-space reflection
  (S3, S4) where the tier has them, with a fog bell when the weather is Fog.
- **No enemies.** The coast has no combat encounters in this
  specification. Sea creatures to fight would follow the
  [combat model](combat-model.md) in a later issue.

## Streaming and budgets per tier

The terrain, including the sea bed's bathymetry, is generated in Rust, as
Everglade's ground is. Static models come from a pinned coast pack and
stream by 8 m cells near the camera, with proxies for distant cells (the
cliffs, the island, and the far isles), as
[Rendering at scale](rendering-scale.md) plans for Everglade. The ocean draws
on its clipmap and doesn't stream by cell; its textures count toward the
zone's resident bytes.

These are targets to measure in phase C6, not results. Over budget, a tier
draws less and never fails.

| Budget | Low (WebGL2, GLES 3.0) | Medium (phones, WebGPU) | High (desktop) |
| --- | --- | --- | --- |
| Resident GPU bytes for the zone | ≤ 160 MiB, Everglade's budget | ≤ 160 MiB | ≤ 224 MiB, the renderer's bound |
| Of which water (from [Water](water.md#budgets-per-tier)) | ≤ 8 MiB | ≤ 32 MiB | ≤ 96 MiB |
| Fog end and draw distance | 600 m | 1.2 km | 2 km, the atmosphere's validated maximum |
| Clipmap rings | 3 | 4 | 5 |
| Spectral cascades | A baked looping tile | 2 × 64² | 3 × 128² |
| Triangles drawn a frame | ≤ 250,000 | ≤ 400,000 | ≤ 600,000 |
| Radius of cells at their near level | 64 m | 128 m | 256 m |
| Rowboats, floes, and debris simulated by the host | Within water's 32 buoyant bodies | 64 | 128 |
| Wildlife instances | 32 | 128 | 384 |
| Other players drawn | 16 | 32 | 64 |
| Compressed pack download | ≤ 48 MiB | ≤ 48 MiB | ≤ 48 MiB |

Phones run Medium; a browser runs Medium on WebGPU and Low on WebGL2.

## Multiplayer

The coast follows the water system's split between simulated and visual
state ([The surface model and determinism](water.md#the-surface-model-and-determinism))
and Verse's host authority with snapshots ([Networking](networking.md)).

- **No new stream for the sea.** The swell, the tide, the weather schedule,
  and rising water are functions of the world tick and the zone seed, so
  every client computes them.
- **Rowboats** are host-simulated buoyant bodies whose poses replicate as
  NIP-MV shared bodies; the rower's inputs are intents, and seated
  passengers publish no pose of their own.
- **Swimmers** publish poses as players do today. Other clients need each
  swimmer's medium (wading, swimming, or diving) to pick its animation.
  Carrying it in entity state is an additive change to
  [NIP-MV](../../nips/openagents/NIP-MV.md), reviewed under the Nostr skill
  before C6.
- **Spell events** (ice, a flood, a trench, a whirlpool, a redirected
  current) are host events with a start tick and a duration, included in
  the snapshot a late joiner receives.
- **Interest.** Pose frames and entity states carry NIP-MV's cell tags, and
  a client subscribes to the cells within its draw distance, so a swimmer
  at Gull Island doesn't receive the harbor's traffic at full rate.
- **Local only.** Spectral detail, foam, spray, ripples, cosmetic floes,
  and wildlife stay on each client, seeded where they must match.

## Assets

Every model comes from a script under `scripts/blender/`, following the
[Blender pipeline](blender-pipeline.md): staged in
`assets/verse/generated/`, recorded in its `PROVENANCE.md`, and admitted into
a pinned coast pack with levels of detail. The headland's grass and pines
reuse the Stylized Nature MegaKit's admitted foliage, and the cottages reuse
the village kit's textures.

| Kit | Script | Pieces |
| --- | --- | --- |
| Coast rocks | `coast_rocks.py` | Cliff modules (straight, corner, and inlet), sea stacks, the arch, the sea cave's mouth, boulders, the tide-pool shelf with pool hollows, and reef rocks, from displaced primitives on one rock atlas |
| Beach | `beach_props.py` | Driftwood, dune fencing, beach grass cards, shells, seaweed wrack, rope, nets, crates, and barrels; floating props carry densities for buoyancy (P1) |
| Harbor | `harbor.py` | Piers and pilings, breakwater blocks, the boathouse, moorings and buoys, bollards, a hand crane, and two moored boats |
| Lighthouse | `lighthouse.py` | The tower, gallery, lamp room with an emissive lens, keeper's cottage, and fog bell |
| Boats | `boats.py` | Oars for Everglade's existing `generated/rowboat`, the moored sloop, and the wreck's hull pieces |
| Underwater | `reef.py` | Kelp strands as vertex-animated cards, anemones, and the reef's small rocks |
| Wildlife | `animals.py` | A gull (`idle`, `flap`, and `glide`), a crab (`idle` and `walk`), a seal (`idle` and `swim`), and a fish for schools |
| Effects | `scripts/blender/fx/` | Surf crest spray and sea mist sheets, beside W6's splash and spray |

## Phases and estimates

Estimates are agent-hours at the water system's pace. #10796 splits into one
issue a phase when W4 and W10 close.

| Phase | Work | Needs | Estimate |
| --- | --- | --- | --- |
| C1 | Zone shell: the zone's identity, the west arch, generated terrain and bathymetry, the ocean body with the tide, the harbor's shelter mask, the estuary, the spawn and return portal, and zone tests (entry, return, the tide's dry and wet sandbar) | W4, W10, W3 | 10 h |
| C2 | The Blender kits and the pinned coast pack, with admission, levels of detail, and captures | C1 | 12 h |
| C3 | The harbor's rowboats on swell, Gull Island, the sandbar and Tern Islet, and the offshore bounds current | C2, W6 | 8 h |
| C4 | Diving and tide pools: the reef, kelp, the wreck, fish, crabs, caustics, and the tide-pool bodies | C2, W7 | 8 h |
| C5 | Zone-to-zone portals in the runtime, and Everglade's estuary gate | C1 | 6 h |
| C6 | The coast's climate, spells on the open sea, the multiplayer checks above, the NIP-MV medium field, and per-tier measurement; device runs go in `NEEDS_OWNER.md` | C3, C4, W8, W9 | 6 h |

The total is about 50 agent-hours. C4 and C5 can run beside C3.

## Open questions

1. **Name.** "The coast" is a working name for the zone and its arch.
2. **Combat.** Should the coast have sea creatures to fight, or stay a
   place to explore?

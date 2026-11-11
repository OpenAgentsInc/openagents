#!/usr/bin/env bash
# Builds the pages for the browser: build/splash.wasm, build/project.wasm,
# build/projects.wasm, build/realm.wasm, build/admin.wasm, build/audiolab.wasm, build/og.wasm, build/verse.wasm, build/garden.wasm, build/characters.wasm, build/fixture.wasm, build/droprealm.wasm, build/builder.wasm, build/myrealm.wasm, build/icecream.wasm, build/deadgears.wasm, build/everglade.wasm, build/villa.wasm, build/private.wasm, build/chat.wasm, build/components.wasm, build/realmlist.wasm, build/gridcrowd.wasm, build/speedrunhud.wasm, build/status.wasm and build/nip01.wasm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
# A failing step names itself (#13117): the line and the command, last in the
# output, and the modules still building stop instead of running on unseen.
trap 'status=$?; trap - ERR; kill $(jobs -p) 2> /dev/null || true; wait 2> /dev/null || true
      echo "build.sh: FAILED (status $status) at line $LINENO: $BASH_COMMAND" >&2; exit "$status"' ERR
mkdir -p "$here/build"
python3 "$here/../../coax/web/nip-pages.py" --check
# The quickstart is authored once; stale generated document/shader bytes refuse a release.
bun "$here/../../coax/web/connect-pylon-page.ts" --check
# The public Coax release history is always built from the canonical Markdown.
(cd "$here/../.." && coax/web/tool.sh coax/web/changelog-page.coax --write)
"$here/../../coax/web/check-visuals.sh" "$here"
# Guard the shared-library/output race before launching parallel world builds.
python3 "$here/../../coax/web/tests/wasm_isolation.py"
# The realm host's table (realms.json): the galaxy's worlds into realms.coax
# and each realm's page (docs/verse/realm-host.md).
(cd "$here/../.." && coax/web/tool.sh bitcoin-lab-map)
bun "$here/../../coax/web/realms.ts"
# wasm.sh keys its cache of each unit's C and object by the unit's #load closure
# (coax/web/tool.sh wasm-size sources): build that tool once here, before the
# modules below start side by side, so they don't each build it.
(cd "$here/../.." && coax/web/tool.sh wasm-size sources apps/openagents-web/fixture.coax > /dev/null)
# Every module this script writes has a size budget in wasm-budgets.txt, and
# every line there names one it writes: a release stops here, in seconds, not
# after the modules (wasm.sh and build-home.sh check each module's bytes).
if [[ "${COAXIAL_MODE:-release}" != dev ]]; then
  (cd "$here/../.." && coax/web/tool.sh wasm-size budgets apps/openagents-web/wasm-budgets.txt apps/openagents-web/build.sh)
fi
# Host-contract v1 uses the same Coax PCM interpreter as native audio.
"$here/../../coax/web/tool.sh" "$here/../../coax/web/build-audio.coax" "$here/../.." "$here/build/host-audio.wasm" "$here/build/host-audio-timeline.wasm" &
host_audio=$!
# Canonical v1 image decode and mip bytes share the native Coax authority.
"$here/../../coax/web/tool.sh" "$here/../../coax/web/build-layers.coax" "$here/../.." "$here/build/host-layers.wasm" &
host_layers=$!
# Canonical v1 still-media bytes (PNG/JPEG) share the native Coax decoder.
"$here/../../coax/web/tool.sh" "$here/../../coax/web/build-media.coax" "$here/../.." "$here/build/host-media.wasm" &
host_media=$!
# Physical bank/packet spans use the shared host-service authorities.
"$here/../../coax/web/tool.sh" "$here/../../coax/web/build-services.coax" "$here/../.." "$here/build/host-services.wasm" &
host_services=$!
"$here/../../coax/web/tool.sh" "$here/../../coax/web/build-sockets.coax" "$here/../.." "$here/build/host-sockets.wasm" &
host_sockets=$!
"$here/../../coax/web/tool.sh" "$here/../../coax/web/build-packs.coax" "$here/../.." "$here/build/host-packs.wasm" &
host_packs=$!
# Range-addressable packs (VPK3, aethyr/forge/page-pack.coax, #13001): each .vpk3 a realm names in
# realms.json is written from its tracked .pack beside it (rewritten only when the pack changed).
# Everglade II's first takes this tree's planting into its catalog (verse/tools/assets/
# everglade2_plant.coax, #13150), so the page reads the woods instead of planting them for 1.8 s.
for vpk3 in "$here"/realms/everglade2/assets/everglade2.vpk3; do
  planted="$here/build/$(basename "${vpk3%.vpk3}")-planted.pack"
  "$here/../../coax/web/tool.sh" "$here/../../verse/tools/assets/everglade2_plant.coax" "${vpk3%.vpk3}.pack" "$planted" > /dev/null
  "$here/../../coax/web/tool.sh" "$here/../../aethyr/tools/page-pack.coax" "$planted" "$vpk3" > /dev/null
  rm -f "$planted"
done
# Opt-in room timing stays separate from the realm's producer.
"$here/../../coax/web/tool.sh" "$here/../../coax/web/build-latency.coax" "$here/../.." "$here/build/latency-client.wasm" &
room_latency=$!
# The HUD bar's light, the same function the iOS stages draw (verse/light.coax).
# (splash.wgsl loads verse/light.coax in its Coax: gpu/splash.coax.)
"$here/../../coax/web/wasm.sh" "$here/splash.coax" "$here/face.c" "$here/build/splash.wasm" &
splash=$!
# The realms: the engine's house, player and thumbs, each its own library as on
# iOS, under realm.coax's HUD; the house's light from the engine itself.
# (realm.wgsl, the house, loads its light in its Coax: gpu/realm.coax.)
# (particles.wgsl loads aethyr/particle-motion.coax and haunt-light.coax in its Coax: gpu/particles.coax.)
# The art styles' shading (verse/style.coax, docs/verse/art-styles.md) into the fixture and every
# painterly realm's WGSL (realms.ts --check refuses a painterly realm without it).
style_fns="verse_style_wrap verse_style_bands verse_style_bands2 verse_style_shade verse_style_cool verse_style_keep_value verse_style_banded verse_style_ambient verse_style_dab verse_style_rim verse_style_haze verse_style_haze_glow verse_style_haze_teal verse_style_haze_hue verse_style_fog verse_style_sky_t verse_style_sky_band verse_style_halo verse_style_hash verse_style_noise verse_style_cloud_field verse_style_cloud_cover verse_style_cloud_lit verse_style_cloud_fade verse_style_tone verse_style_saturation verse_style_lift verse_style_luma verse_style_bloom_share verse_style_bloom_mix verse_style_shadow_reach verse_style_shadow_snap verse_style_shadow_across verse_style_shadow_depth verse_style_shadow_taps verse_style_shadow_radius verse_style_shadow_tap verse_style_contact verse_style_shaft_source verse_style_shaft_fade verse_style_shaft_tap verse_style_shaft_total verse_style_shaft verse_style_gust verse_style_wind verse_style_bend verse_style_grass_tilt verse_style_blade_height verse_style_grass_fade verse_style_grass_blades verse_style_grass_clear verse_style_segment verse_style_part verse_style_tip verse_style_jitter"
# The blob canopies (art-styles.md 7) go where there are kit trees or a traced tree: the fixture and Everglade.
foliage_fns="verse_style_canopy_axis verse_style_canopy_normal verse_style_canopy_sway verse_style_canopy_open verse_style_near_reach verse_style_near_blades verse_style_near_width verse_style_chroma_gain"
# (the fixture's Coax, gpu/fixture.coax, loads verse/look.coax and verse/style.coax.)
# Everglade's whole module is Coax (verse/everglade/gpu.coax, with verse/style.coax): its
# entries and bindings (verse/everglade/gpu.module, gpu.bindings) as coax wgsl writes them; its
# particles' module the same from gpu-fx.coax.
# Every page's and realm's WGSL is written from its Coax: apps/openagents-web/gpu/NAME.coax, with
# NAME.bindings and NAME.module, writes NAME.wgsl (docs/verse/native-gpu.md, G6); none is
# written by hand. gpu/nip.coax is the NIP pages', written to nip01.wgsl below.
# They are committed (the dev server, GPU passes and golden checks read them without a build):
# commit what this writes. `coax/web/tool.sh coax/web/wgsl-module.coax --site --check` fails on drift.
(cd "$here/../.." && coax/web/tool.sh coax/web/wgsl-module.coax --site)
# The closed realm catalog owns these identities and module/entry/binding
# declarations. Unchanged packages reuse verified source and product hashes.
profiles="$(cd "$here/../.." && coax/web/tool.sh aethyr/render/build.coax --profiles)"
while IFS= read -r profile; do
  (cd "$here/../.." && coax/web/tool.sh aethyr/render/build.coax "$profile" "$here/build/render/$profile")
done <<< "$profiles"
# The low-poly facets (art-styles.md, Flat: facets): a face's own tone, nudged round corners, the light outline.
facet_fns="verse_style_facet_tone verse_style_facet_jitter verse_style_facet_edge"
# The flat style (art-styles.md, Flat) and its palettes go to the flat realms: Hecate's Cemetery and Fairy Forest.
flat_fns="verse_style_flat_light verse_style_flat_band verse_style_flat_band2 verse_style_flat_sky verse_style_flat_shadow verse_style_flat_fade verse_style_bayer verse_style_flat_dither verse_style_flat_hem verse_style_flat_srgb verse_style_flat_colour verse_style_flat_time"
# (Hecate's Cemetery, villa.wgsl, loads them in its Coax: gpu/villa.coax.)
# A made realm (myrealm.wgsl) is painterly or flat when its maker picked that (low poly keeps its own look).
# (its Coax, gpu/myrealm.coax, loads them.)
# Fairy Forest draws in the flat style only, faceted: no painterly or grass functions.
# (Fairy Forest, fairy.wgsl, loads them in its Coax: gpu/fairy.coax.)
# (Orbit League, orbitleague.wgsl, loads them in its Coax: gpu/orbitleague.coax.)
# (waterlab.wgsl loads verse/style.coax in its Coax: gpu/waterlab.coax.)
m="$here/../../verse"
a="$here/../../aethyr"   # the engine modules (#12609)
# The voice engine for pages that talk (data-voice: the Haunted House), coax/web/voice.js.
"$here/../../coax/web/voice/build.sh" "$here/build/voice.wasm" &
voice=$!
"$here/../../coax/web/wasm.sh" "$here/realm.coax" "$here/realm-face.c" "$here/build/realm.wasm" \
  "$m/haunt.coax" "$m/haunt-buffer.coax" "$a/player.coax" "$m/grid.coax" "$a/particles.coax" "$a/cinematic.coax" "$m/setpiece.coax" &
realm=$!
# /worlds/garden: the engine's world pieces (camera, terrain, throwing,
# arranging) in an open-air realm, with the player and thumbs as their own libraries.
"$here/../../coax/web/wasm.sh" "$here/garden.coax" "$here/garden-face.c" "$here/build/garden.wasm" \
  "$a/player.coax" "$m/grid.coax" &
garden=$!
# /worlds/everglade-classic (Everglade before the SRD 5e rules; everglade.coax is also the city everglade5e.coax loads): the forest city's blockout (verse/everglade/plan.coax), with the player, thumbs and its river (verse/everglade/river.coax).
"$here/../../coax/web/wasm.sh" "$here/everglade.coax" "$here/everglade-face.c" "$here/build/everglade.wasm" \
  "$here/everglade-step.coax" "$m/grid.coax" "$m/everglade/river.coax" "$m/everglade/guard-guide-policy.coax" &
everglade=$!
# /worlds/5e-mmo: the SRD 5e battle in Everglade (fivemmo.coax on everglade.coax), the battle its own unit.
"$here/../../coax/web/wasm.sh" "$here/fivemmo.coax" "$here/fivemmo-face.c" "$here/build/fivemmo.wasm" \
  "$a/player.coax" "$m/grid.coax" "$m/everglade/river.coax" "$here/fivemmo-battle.coax" &
fivemmo=$!
# /worlds/everglade (since #12710; /worlds/everglade-5e redirects): Everglade on the SRD 5e engine (everglade5e.coax on everglade.coax), the rules their own unit.
"$here/../../coax/web/wasm.sh" "$here/everglade5e.coax" "$here/everglade5e-face.c" "$here/build/everglade5e.wasm" \
  "$here/everglade-step.coax" "$m/grid.coax" "$m/everglade/river.coax" "$m/everglade/guard-guide-policy.coax" "$here/everglade5e-rules.coax" &
everglade5e=$!
# Everglade's offline room (#12784): the room server's simulation, built to wasm for the local host (coax/web/room-worker.js).
"$here/../../coax/web/tool.sh" "$here/../../coax/web/room/worker.coax" "$here/build/everglade-room.wasm" &
everglade_room=$!
# /worlds/fairy-forest: the park (verse/fairy-forest/forest.coax), with the player, thumbs and its stream (verse/fairy-forest/stream.coax).
"$here/../../coax/web/wasm.sh" "$here/fairy.coax" "$here/fairy-face.c" "$here/build/fairy.wasm" \
  "$a/player.coax" "$m/grid.coax" "$m/fairy-forest/stream.coax" "$here/../../coax/lib/qr/qr-memory.coax" &
fairy=$!
# /worlds/hecates-cemetery: Planetary Villa's first expansion (verse/planetary-villa/README.md),
# with the player, thumbs and figures (villafig.coax: the engine's figures and motions) as libraries.
"$here/../../coax/web/wasm.sh" "$here/villa.coax" "$here/villa-face.c" "$here/build/villa.wasm" \
  "$a/player.coax" "$m/grid.coax" "$here/villafig.coax" &
villa=$!
# /worlds/anime-lab: direct an anime (docs/verse/anime-lab.md), with the anime style's numbers as a library.
"$here/../../coax/web/wasm.sh" "$here/animelab.coax" "$here/animelab-face.c" "$here/build/animelab.wasm" \
  "$here/animelab-style.coax" &
animelab=$!
"$here/../../coax/web/wasm.sh" "$here/animationlab.coax" "$here/animationlab-face.c" "$here/build/animationlab.wasm" &
animationlab=$!
"$here/../../coax/web/wasm.sh" "$here/ocarinalab.coax" "$here/ocarinalab-face.c" "$here/build/ocarinalab.wasm" &
ocarinalab=$!
"$here/../../coax/web/wasm.sh" "$here/nights.coax" "$here/nights-face.c" "$here/build/nights.wasm" &
nights=$!
# /worlds/fixture: the realm host's test world, the smallest realm module.
"$here/../../coax/web/wasm.sh" "$here/fixture.coax" "$here/fixture-face.c" "$here/build/fixture.wasm" &
fixture=$!
# /worlds/waterlab: the water system's test scene (docs/verse/water.md), the spectral sea in Coax.
"$here/../../coax/web/wasm.sh" "$here/waterlab.coax" "$here/waterlab-face.c" "$here/build/waterlab.wasm" &
waterlab=$!
"$here/../../coax/web/wasm.sh" "$here/bitcoin-lab.coax" "$here/bitcoin-lab-face.c" "$here/build/bitcoin-lab.wasm" &
bitcoin_lab=$!
"$here/../../coax/web/wasm.sh" "$here/valleylab.coax" "$here/valleylab-face.c" "$here/build/valleylab.wasm" &
valleylab=$!
"$here/../../coax/web/wasm.sh" "$here/portallab.coax" "$here/portallab-face.c" "$here/build/portallab.wasm" &
portallab=$!
"$here/../../coax/web/wasm.sh" "$here/valleycanyon.coax" "$here/valleycanyon-face.c" "$here/build/valleycanyon.wasm" &
valleycanyon=$!
# The character pass's runtime, loaded beside a world that places a character (coax/web/character-host.js).
"$here/../../coax/web/wasm.sh" "$here/character-host.coax" "$here/character-host-face.c" "$here/build/character-host.wasm" &
characterhost=$!
"$here/../../coax/web/wasm.sh" "$here/valleyworld.coax" "$here/valleyworld-face.c" "$here/build/valleyworld.wasm" &
valleyworld=$!
# /worlds/anime-lab: direct an anime (docs/verse/anime-lab.md): the director and stage (verse/anime/), the
# anime style's block from its own library (animelab-style.coax). (animelab.wgsl loads verse/style.coax in gpu/animelab.coax.)
"$here/../../coax/web/wasm.sh" "$here/animelab.coax" "$here/animelab-face.c" "$here/build/animelab.wasm" "$here/animelab-style.coax" &
animelab=$!
# /worlds/drop and every /projects/<slug>/realm: a project's first realm, its drop's models on a floor.
"$here/../../coax/web/wasm.sh" "$here/droprealm.coax" "$here/droprealm-face.c" "$here/build/droprealm.wasm" &
droprealm=$!
# /worlds/built and every /worlds/<slug> made at /worlds/new: a realm someone built (myrealm.coax).
# YOUR COMPUTER's wire (verse/edit/computer-wire.coax) links beside it; its signing tables need the larger budget.
# With its renderer carrier, which loads the wire and entropy too, so one load graph owns
# sha256-kernel.coax's S_hash and S_hex (#12661); wasm.sh folds the two libraries into it (#12675).
COAXIAL_RUN_STEPS=250000000 "$here/../../coax/web/wasm.sh" "$here/myrealm.coax" "$here/myrealm-face.c" "$here/build/myrealm.wasm" \
  "$here/../../verse/edit/computer-wire.coax" "$here/../../coax/lib/crypto/entropy.coax" &
myrealm=$!
# /worlds/new: build your own realm (docs/verse/realm-builder.md).
"$here/../../coax/web/wasm.sh" "$here/builder.coax" "$here/builder-face.c" "$here/build/builder.wasm" &
builder=$!
# /worlds/ice-cream-man: Ice Cream Man's house (verse/icecream/scene.coax, its own library) under its HUD.
"$here/../../coax/web/wasm.sh" "$here/icecream.coax" "$here/icecream-face.c" "$here/build/icecream.wasm" "$m/icecream/scene.coax" &
icecream=$!
# /worlds/dead-gears: the ship survival game (verse/deadgears/rules.coax, loaded by the page).
"$here/../../coax/web/wasm.sh" "$here/deadgears.coax" "$here/deadgears-face.c" "$here/build/deadgears.wasm" &
deadgears=$!
# /worlds/hold-the-stair: the popping tower defense on the SRD 5e rules (verse/rules/srd5-combat.coax).
"$here/../../coax/web/wasm.sh" "$here/stair.coax" "$here/stair-face.c" "$here/build/stair.wasm" &
stair=$!
# /worlds/painted-village: the hand-painted look (aethyr/render/stylized.coax) on generated assets (aethyr/arch/village.coax, flora.coax).
"$here/../../coax/web/wasm.sh" "$here/painted.coax" "$here/painted-face.c" "$here/build/painted.wasm" &
painted=$!
# /worlds/watchtower: a cyber defense range: find the intruder, isolate and reimage hosts (docs/research/cybersecurity).
"$here/../../coax/web/wasm.sh" "$here/watchtower.coax" "$here/watchtower-face.c" "$here/build/watchtower.wasm" &
watchtower=$!
# /worlds/5e-lab (unlisted): druids play the SRD 5e rules on a bare grid (aethyr/rules/srd, docs/dnd).
# Player mode walks you on Everglade's controller (everglade-step.coax) and a phone's thumbs (aethyr/sticks.coax).
"$here/../../coax/web/wasm.sh" "$here/fivelab.coax" "$here/fivelab-face.c" "$here/build/fivelab.wasm" \
  "$here/everglade-step.coax" "$a/sticks.coax" &
fivelab=$!
# /worlds/archery-range (unlisted): the Archery Range, an Everglade zone (everglade/zone.coax) with the range on it
# (everglade/docs/archery-range-spec.md). It draws with Character Create's passes (charactercreator.wgsl).
"$here/../../coax/web/wasm.sh" "$here/archery-range.coax" "$here/archery-range-face.c" "$here/build/archery-range.wasm" \
  "$here/everglade-step.coax" "$a/sticks.coax" &
archery_range=$!
# /worlds/valley-lab/cull (unlisted): the GPU cull's lab (#13008), its residency and its fixture.
"$here/../../coax/web/wasm.sh" "$here/cullstream.coax" "$here/cullstream-face.c" "$here/build/cullstream.wasm" &
cullstream=$!
(cd "$here/../.." && coax/web/tool.sh aethyr/tools/cull-fixture.coax "$here/build" > /dev/null) &
cullfixture=$!
# /worlds/everglade2 (unlisted): Everglade v2, the glade, one path through the woods and a clearing (docs/verse/everglade2.md).
"$here/../../coax/web/wasm.sh" "$here/everglade2.coax" "$here/everglade2-face.c" "$here/build/everglade2.wasm" \
  "$here/everglade-step.coax" "$a/sticks.coax" &
everglade2=$!
# /worlds/metahuman-lab (unlisted): one figure on the CC0 people rig's skeleton and clips (docs/verse/metahuman-support.md).
"$here/../../coax/web/wasm.sh" "$here/metahumanlab.coax" "$here/metahumanlab-face.c" "$here/build/metahumanlab.wasm" &
metahumanlab=$!
# /worlds/splat-lab (unlisted): 3D Gaussian splats, GPU-sorted (docs/verse/2026-11-03-splat-lab.md).
"$here/../../coax/web/wasm.sh" "$here/splatlab.coax" "$here/splatlab-face.c" "$here/build/splatlab.wasm" &
splatlab=$!
# /worlds/one-shot: the campaign creator's Table (oneshot.coax on oneshot-table.coax and the SRD engine).
"$here/../../coax/web/wasm.sh" "$here/oneshot.coax" "$here/oneshot-face.c" "$here/build/oneshot.wasm" &
oneshot=$!
# /demos/grid-crowd: the Grid's art (verse/tools/pack_grid.py) with a staged crowd of players (verse/cast.coax)
# and agents running into the Everglade gate, for clips (?stage=crowd, docs/verse/clips.md).
"$here/../../coax/web/wasm.sh" "$here/gridcrowd.coax" "$here/gridcrowd-face.c" "$here/build/gridcrowd.wasm" \
  "$m/cast.coax" &
gridcrowd=$!
# /demos/speedrun-hud: the Speedrun video's HUD diagrams over the Grid floor, one staged scene each
# (?stage=ident ... ?stage=end, docs/verse/clips.md), drawn at twice its pixels for clips.
"$here/../../coax/web/wasm.sh" "$here/speedrunhud.coax" "$here/speedrunhud-face.c" "$here/build/speedrunhud.wasm" &
speedrunhud=$!
# /worlds/belt-2200: the solar-system strategy game over the engine's orbits (aethyr/orbit.coax, aethyr/spacecam.coax; verse/stars.coax in gpu/belt.coax).
"$here/../../coax/web/wasm.sh" "$here/belt.coax" "$here/belt-face.c" "$here/build/belt.wasm" &
belt=$!
# /worlds/orbit-league: car ball in orbit, on the space primitives (docs/verse/space.md).
"$here/../../coax/web/wasm.sh" "$here/orbitleague.coax" "$here/orbitleague-face.c" "$here/build/orbitleague.wasm" &
orbitleague=$!
# /worlds/characters: the engine's figures (aethyr/figure.coax).
"$here/../../coax/web/wasm.sh" "$here/characters.coax" "$here/characters-face.c" "$here/build/characters.wasm" &
characters=$!
# The public homepage and /new-land alias, and /worlds/the-grid: the same module walked (data-page 1, newland-walk.coax). Warm start is real Coax-verified
# public data; open pages independently watch and refresh Nostr in Coax.
# The capture is `openagents project landing` (apps/openagents-cli, Coax, built here in seconds).
"$here/../openagents-cli/build.sh" > /dev/null
oa="$here/../openagents-cli/build/openagents"
# The project verbs find the checkout from the working folder, so run them from
# this one: release.sh builds a separate tree and may start anywhere.
oa_here() { (cd "$here" && "$oa" "$@"); }
oa_here project landing || echo "home: keeping its last complete verified warm start" >&2
"$here/../../coax/web/project/build-home.sh" "$here/build/newland.wasm" &
newland=$!
# /docs/connect-a-pylon: the Pylon quickstart on the homepage's Grid (connect-pylon.coax loads newland.coax).
"$here/../../coax/web/wasm.sh" "$here/connect-pylon.coax" "$here/connect-pylon-face.c" "$here/build/connect-pylon.wasm" &
connectpylon=$!
# /status: how a change reaches production, on the homepage's Grid (status.coax loads newland.coax;
# docs/verse/status-page.md). A release writes the page's snapshot from the commit it ships.
if [[ "${COAXIAL_MODE:-release}" != dev ]]; then
  (cd "$here/../.." && coax/web/tool.sh coax/web/status-data.coax --write)
fi
"$here/../../coax/web/wasm.sh" "$here/status.coax" "$here/status-face.c" "$here/build/status.wasm" &
status_page=$!
# /coax/changelog: the compiler's complete release history, drawn in Coax.
"$here/../../coax/web/wasm.sh" "$here/changelog.coax" "$here/changelog-face.c" "$here/build/changelog.wasm" &
changelog=$!
# /verse: the playtest's landing (verse.coax's page 1, the galaxy, is the native host's).
"$here/../../coax/web/wasm.sh" "$here/verse.coax" "$here/verse-face.c" "$here/build/verse.wasm" &
verse_app=$!
# The project pages: every projects/<slug>/project.json with a page, its tasks
# read from Nostr into projects/<slug>/work.coax (a release always reads
# afresh, a dev build when the snapshot is ten minutes old; an unreachable
# relay keeps the last snapshot), then `openagents project bake` writes its
# commits and the projects/baked.coax table project.coax draws from. Adding a
# project is data.
fresh=(); [[ "${COAXIAL_MODE:-dev}" == release ]] || fresh=(--if-older 10)
snaps=()
for slug in $(oa_here project bake --list); do
  oa_here project snapshot --project "$slug" ${fresh[@]+"${fresh[@]}"} & snaps+=($!)
done
# The /projects list: every project on the relay (kind 30621) into projects/list.coax.
oa_here project index ${fresh[@]+"${fresh[@]}"}
"$here/../../coax/web/wasm.sh" "$here/projects.coax" "$here/projects-face.c" "$here/build/projects.wasm" &
list=$!
for pid in "${snaps[@]}"; do wait "$pid"; done
oa_here project bake
"$here/../../coax/web/wasm.sh" "$here/project.coax" "$here/project-face.c" "$here/build/project.wasm" "$here/project-verify.coax"   # project-verify.coax: the capacity kernel, the NIP-19 names and the BIP-340 frame check
wait "$list"
# /nip/01: NIP-01 in Coax, its verifier its own library; the splash's passes.
(cd "$here/../.." && coax/web/tool.sh coax/web/wgsl-module.coax apps/openagents-web/gpu/nip.coax apps/openagents-web/nip01.wgsl apps/openagents-web/gpu/nip.module)   # the splash's with sans glyphs
"$here/../../coax/web/wasm.sh" "$here/nip01.coax" "$here/nip01-face.c" "$here/build/nip01.wasm" "$here/nip01-verify.coax" &
nip01=$!
cmp -s "$here/nip01.wgsl" "$here/nip-guide.wgsl" || cp "$here/nip01.wgsl" "$here/nip-guide.wgsl"   # generated, ignored; only on a change
"$here/../../coax/web/wasm.sh" "$here/nip-guide.coax" "$here/nip-guide-face.c" "$here/build/nip-guide.wasm" "$here/nip-guide-observe.coax" &
nip_guide=$!
# /pylon: read-only live Pylon network, shared checked Coax verifier.
cmp -s "$here/nip01.wgsl" "$here/pylon.wgsl" || cp "$here/nip01.wgsl" "$here/pylon.wgsl"
"$here/../../coax/web/wasm.sh" "$here/pylon.coax" "$here/pylon-face.c" "$here/build/pylon.wasm" "$here/../../coax/web/project/landing-nostr.coax" &
pylon=$!
# Keep the public join command identical to the tested source, never a fork.
cmp -s "$here/../../scripts/join-pylon.sh" "$here/pylon/join.sh" || cp "$here/../../scripts/join-pylon.sh" "$here/pylon/join.sh"
# Engine-owned declared-state adoption and host-conformance fixture (#12991).
COAXIAL_RUN_STEPS=250000000 "$here/../../coax/web/wasm.sh" "$here/../../aethyr/state/fixture.coax" "$here/state-lab-face.c" "$here/build/state-lab.wasm" &
state_lab=$!
# Engine-owned compiled actor adoption (#12992).
COAXIAL_RUN_STEPS=250000000 "$here/../../coax/web/wasm.sh" "$here/../../aethyr/actors/lab.coax" "$here/actor-lab-face.c" "$here/build/actor-lab.wasm" &
actor_lab=$!
# /worlds: every listed realm as a list and a grid of towers (realms/list.coax, written by realms.ts).
"$here/../../coax/web/wasm.sh" "$here/realmlist.coax" "$here/realmlist-face.c" "$here/build/realmlist.wasm" &
realmlist=$!
# /memorial/<slug>: a pet's memorial from Hecate's Cemetery, its page (oa-api serves the HTML, villa.c memorial_page).
"$here/../../coax/web/wasm.sh" "$here/memorial.coax" "$here/memorial-face.c" "$here/build/memorial.wasm" &
memorial=$!
# /admin/analytics: the projects list's HUD over the admins' numbers.
"$here/../../coax/web/wasm.sh" "$here/admin.coax" "$here/admin-face.c" "$here/build/admin.wasm" &
admin=$!
# /realms/audio-lab: Everglade's sounds, each candidate and the raid mix (#12832), the projects list's HUD.
"$here/../../coax/web/wasm.sh" "$here/audiolab.coax" "$here/audiolab-face.c" "$here/build/audiolab.wasm" &
audiolab=$!
# /og: the link-preview workbench (docs/web/og-tags.md), og.coax over og/pages.txt.
"$here/../../coax/web/wasm.sh" "$here/og.coax" "$here/og-face.c" "$here/build/og.wasm" &
og=$!
# /chat: a direct chat with the OpenAgents agent (docs/chat.md), admins only,
# drawn with the Zeron components (components/zeron/): Geist, its OFL and
# Zeron's icons (as one JSON, name to SVG) beside it for coax/web/gpui.js.
"$here/../../coax/web/wasm.sh" "$here/chat.coax" "$here/chat-face.c" "$here/build/chat.wasm" &
chat=$!
mkdir -p "$here/build/zeron"
cp "$here/../../components/zeron/fonts/"*.ttf "$here/../../components/zeron/fonts/Geist-OFL.txt" "$here/build/zeron/"
python3 -c 'import json,os,sys; d=sys.argv[1]; json.dump({f[:-4]: open(os.path.join(d,f)).read() for f in sorted(os.listdir(d)) if f.endswith(".svg")}, open(sys.argv[2],"w"))' \
  "$here/../../components/zeron/icons" "$here/build/zeron/icons.json"
# /components: the component library's page (components/spec.md, #13153): every component in
# every skin, drawn by the library (components/gallery/page.coax) with the atlas assets above.
"$here/../../coax/web/wasm.sh" "$here/components.coax" "$here/components-face.c" "$here/build/components.wasm" &
components_page=$!
# A private project's page (/projects/<slug>, served by oa-api to members).
COAXIAL_RUN_STEPS=250000000 "$here/../../coax/web/wasm.sh" "$here/private.coax" "$here/private-face.c" "$here/build/private.wasm" \
  "$here/../../coax/lib/qr/qr-memory.coax" "$here/../../coax/lib/nostr/weblogin.coax" "$here/../../coax/lib/crypto/entropy.coax"
wait "$admin"
wait "$audiolab"
wait "$realmlist"
wait "$memorial"
wait "$og"
wait "$chat"
wait "$components_page"
wait "$nip01"
wait "$nip_guide"
wait "$pylon"
wait "$splash"
wait "$realm"
wait "$voice"
wait "$verse_app"
wait "$garden"
wait "$characters"
wait "$fixture"
wait "$waterlab"
wait "$bitcoin_lab"
wait "$valleylab"
wait "$portallab"
wait "$valleycanyon"
wait "$characterhost"
wait "$animationlab"
wait "$ocarinalab"
wait "$nights"
wait "$valleyworld"
wait "$animelab"
wait "$droprealm"
wait "$builder"
wait "$myrealm"
wait "$icecream"
wait "$deadgears"
wait "$stair"
wait "$painted"
wait "$watchtower"
wait "$fivelab"
wait "$archery_range"
wait "$everglade2"
# Everglade II's admin mode is a dev build's only (everglade2-admin.coax, wasm.sh's --build-profile):
# a release everglade2.wasm holding its panel's words or its room lines is refused.
if [[ "${COAXIAL_MODE:-release}" != dev ]] && LC_ALL=C grep -a -q -e 'ADMIN MODE' -e 'CLICK TP' -e 'X guard' "$here/build/everglade2.wasm"; then
  echo "build.sh: build/everglade2.wasm is a release build with admin mode in it" >&2; exit 1
fi
wait "$cullstream"
wait "$cullfixture"
wait "$metahumanlab"
wait "$splatlab"
wait "$oneshot"
wait "$gridcrowd"
wait "$speedrunhud"
wait "$belt"
wait "$orbitleague"
wait "$everglade"
wait "$fivemmo"
wait "$everglade5e"
wait "$everglade_room"
wait "$host_audio"
wait "$host_layers"
wait "$host_media"
wait "$host_services"
wait "$host_sockets"
wait "$host_packs"
wait "$room_latency"
wait "$fairy"
wait "$villa"
wait "$animelab"
[[ -z "$newland" ]] || wait "$newland"
wait "$connectpylon"
wait "$changelog"
wait "$status_page"

wait "$state_lab"
wait "$actor_lab"

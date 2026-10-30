#!/usr/bin/env bash
# Builds the pages for the browser: build/splash.wasm, build/project.wasm,
# build/projects.wasm, build/realm.wasm, build/admin.wasm, build/verse.wasm, build/garden.wasm, build/characters.wasm, build/fixture.wasm, build/droprealm.wasm, build/builder.wasm, build/myrealm.wasm, build/icecream.wasm, build/deadgears.wasm, build/everglade.wasm, build/thegrid.wasm, build/villa.wasm, build/private.wasm, build/chat.wasm and build/nip01.wasm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
python3 "$here/../../coax/web/nip-pages.py" --check
# Guard the shared-library/output race before launching parallel world builds.
python3 "$here/../../coax/web/tests/wasm_isolation.py"
# The realm host's table (realms.json): the galaxy's worlds into realms.coax
# and each realm's page (docs/verse/realm-host.md).
bun "$here/../../coax/web/realms.ts"
# The HUD bar's light, the same function the iOS stages draw (verse/light.coax).
(cd "$here/../.." && python3 coax/web/wgsl.py verse/light.coax verse_bar --into apps/openagents-web/splash.wgsl)
"$here/../../coax/web/wasm.sh" "$here/splash.coax" "$here/face.c" "$here/build/splash.wasm" &
splash=$!
# The realms: the engine's house, player and thumbs, each its own library as on
# iOS, under realm.coax's HUD; the house's light from the engine itself.
(cd "$here/../.." && python3 coax/web/wgsl.py verse/light.coax verse_bar verse_tone --into apps/openagents-web/realm.wgsl \
  && python3 coax/web/wgsl.py verse/haunt-light.coax verse_haunt_ambient verse_haunt_attenuate verse_haunt_shade verse_haunt_moon_dir verse_haunt_moonlight verse_haunt_shadow_reach verse_haunt_shadow_edge verse_haunt_night verse_haunt_bloom_share verse_haunt_glow --into apps/openagents-web/realm.wgsl \
  && python3 coax/web/wgsl.py verse/look.coax verse_outline_depth verse_outline_crease verse_outline_strength verse_outline_ink --into apps/openagents-web/realm.wgsl \
  && python3 coax/web/wgsl.py verse/particle-motion.coax verse_particle_offset verse_particle_glow verse_particle_size --into apps/openagents-web/particles.wgsl \
  && python3 coax/web/wgsl.py verse/haunt-light.coax verse_haunt_bloom_share --into apps/openagents-web/particles.wgsl \
  && python3 coax/web/wgsl.py verse/look.coax verse_outline_depth verse_outline_crease --into apps/openagents-web/villa.wgsl)
# The art styles' shading (verse/style.coax, docs/verse/art-styles.md) into the fixture and every
# painterly realm's WGSL (realms.ts --check refuses a painterly realm without it).
style_fns="verse_style_wrap verse_style_bands verse_style_bands2 verse_style_shade verse_style_cool verse_style_keep_value verse_style_banded verse_style_ambient verse_style_dab verse_style_rim verse_style_haze verse_style_haze_glow verse_style_haze_teal verse_style_haze_hue verse_style_fog verse_style_sky_t verse_style_sky_band verse_style_halo verse_style_hash verse_style_noise verse_style_cloud_field verse_style_cloud_cover verse_style_cloud_lit verse_style_cloud_fade verse_style_tone verse_style_saturation verse_style_lift verse_style_luma verse_style_bloom_share verse_style_bloom_mix verse_style_shadow_reach verse_style_shadow_snap verse_style_shadow_across verse_style_shadow_depth verse_style_shadow_taps verse_style_shadow_radius verse_style_shadow_tap verse_style_contact verse_style_shaft_source verse_style_shaft_fade verse_style_shaft_tap verse_style_shaft_total verse_style_shaft verse_style_gust verse_style_wind verse_style_bend verse_style_grass_tilt verse_style_blade_height verse_style_grass_fade verse_style_grass_blades verse_style_grass_clear verse_style_segment verse_style_part verse_style_tip verse_style_jitter"
(cd "$here/../.." && python3 coax/web/wgsl.py verse/look.coax verse_outline_styled verse_outline_crease --into apps/openagents-web/fixture.wgsl)
# The blob canopies (art-styles.md 7) go where there are kit trees or a traced tree: the fixture and Everglade.
foliage_fns="verse_style_canopy_axis verse_style_canopy_normal verse_style_canopy_sway verse_style_canopy_open verse_style_near_reach verse_style_near_blades verse_style_near_width verse_style_chroma_gain"
for w in fixture everglade; do
  (cd "$here/../.." && python3 coax/web/wgsl.py verse/style.coax $style_fns $foliage_fns --into "apps/openagents-web/$w.wgsl")
done
# The flat style (art-styles.md, Flat) and its palettes go to the flat realms: Hecate's Cemetery and Fairy Forest.
flat_fns="verse_style_flat_light verse_style_flat_band verse_style_flat_band2 verse_style_flat_sky verse_style_flat_shadow verse_style_flat_fade verse_style_bayer verse_style_flat_dither verse_style_flat_hem verse_style_flat_srgb verse_style_flat_colour verse_style_flat_time"
(cd "$here/../.." && python3 coax/web/wgsl.py verse/style.coax $style_fns $flat_fns --into apps/openagents-web/villa.wgsl)
# A made realm (myrealm.wgsl) is painterly or flat when its maker picked that (low poly keeps its own look).
(cd "$here/../.." && python3 coax/web/wgsl.py verse/style.coax $style_fns $flat_fns --into apps/openagents-web/myrealm.wgsl)
# Fairy Forest draws in the flat style only: no painterly or grass functions.
(cd "$here/../.." && python3 coax/web/wgsl.py verse/style.coax $flat_fns --into apps/openagents-web/fairy.wgsl)
(cd "$here/../.." && python3 coax/web/wgsl.py verse/style.coax $style_fns --into apps/openagents-web/waterlab.wgsl)
m="$here/../../verse"
# The voice engine for pages that talk (data-voice: the Haunted House), coax/web/voice.js.
"$here/../../coax/web/voice/build.sh" "$here/build/voice.wasm" &
voice=$!
"$here/../../coax/web/wasm.sh" "$here/realm.coax" "$here/realm-face.c" "$here/build/realm.wasm" \
  "$m/haunt.coax" "$m/haunt-buffer.coax" "$m/player.coax" "$m/grid.coax" "$m/particles.coax" "$m/cinematic.coax" "$m/setpiece.coax" &
realm=$!
# /realms/garden: the engine's world pieces (camera, terrain, throwing,
# arranging) in an open-air realm, with the player and thumbs as their own libraries.
"$here/../../coax/web/wasm.sh" "$here/garden.coax" "$here/garden-face.c" "$here/build/garden.wasm" \
  "$m/player.coax" "$m/grid.coax" &
garden=$!
# /realms/everglade: the forest city's blockout (verse/everglade/plan.coax), with the player, thumbs and its river (verse/everglade/river.coax).
"$here/../../coax/web/wasm.sh" "$here/everglade.coax" "$here/everglade-face.c" "$here/build/everglade.wasm" \
  "$m/player.coax" "$m/grid.coax" "$m/everglade/river.coax" &
everglade=$!
# /realms/fairy-forest: the park (verse/fairy-forest/forest.coax), with the player, thumbs and its stream (verse/fairy-forest/stream.coax).
"$here/../../coax/web/wasm.sh" "$here/fairy.coax" "$here/fairy-face.c" "$here/build/fairy.wasm" \
  "$m/player.coax" "$m/grid.coax" "$m/fairy-forest/stream.coax" &
fairy=$!
# /realms/the-grid: the bare Verse world from the demo's scene (verse/tools/pack_grid.py), with the player and thumbs.
"$here/../../coax/web/wasm.sh" "$here/thegrid.coax" "$here/thegrid-face.c" "$here/build/thegrid.wasm" \
  "$m/player.coax" "$m/grid.coax" &
thegrid=$!
# /realms/hecates-cemetery: Planetary Villa's first expansion (verse/planetary-villa/README.md),
# with the player, thumbs and figures (villafig.coax: the engine's figures and motions) as libraries.
"$here/../../coax/web/wasm.sh" "$here/villa.coax" "$here/villa-face.c" "$here/build/villa.wasm" \
  "$m/player.coax" "$m/grid.coax" "$here/villafig.coax" &
villa=$!
# /realms/fixture: the realm host's test world, the smallest realm module.
"$here/../../coax/web/wasm.sh" "$here/fixture.coax" "$here/fixture-face.c" "$here/build/fixture.wasm" &
fixture=$!
# /realms/waterlab: the water system's test scene (docs/verse/water.md), the spectral sea in Coax.
"$here/../../coax/web/wasm.sh" "$here/waterlab.coax" "$here/waterlab-face.c" "$here/build/waterlab.wasm" &
waterlab=$!
# /realms/drop and every /projects/<slug>/realm: a project's first realm, its drop's models on a floor.
"$here/../../coax/web/wasm.sh" "$here/droprealm.coax" "$here/droprealm-face.c" "$here/build/droprealm.wasm" &
droprealm=$!
# /realms/built and every /realms/<slug> made at /realms/new: a realm someone built (myrealm.coax).
"$here/../../coax/web/wasm.sh" "$here/myrealm.coax" "$here/myrealm-face.c" "$here/build/myrealm.wasm" &
myrealm=$!
# /realms/new: build your own realm (docs/verse/realm-builder.md).
"$here/../../coax/web/wasm.sh" "$here/builder.coax" "$here/builder-face.c" "$here/build/builder.wasm" &
builder=$!
# /realms/ice-cream-man: Ice Cream Man's house (verse/icecream/scene.coax, its own library) under its HUD.
"$here/../../coax/web/wasm.sh" "$here/icecream.coax" "$here/icecream-face.c" "$here/build/icecream.wasm" "$m/icecream/scene.coax" &
icecream=$!
# /realms/dead-gears: the ship survival game (verse/deadgears/rules.coax, loaded by the page).
"$here/../../coax/web/wasm.sh" "$here/deadgears.coax" "$here/deadgears-face.c" "$here/build/deadgears.wasm" &
deadgears=$!
# /realms/characters: the engine's figures (verse/figure.coax).
"$here/../../coax/web/wasm.sh" "$here/characters.coax" "$here/characters-face.c" "$here/build/characters.wasm" &
characters=$!
# The public homepage and /new-land alias. Warm start is real Coax-verified
# public data; open pages independently watch and refresh Nostr in Coax.
bun "$here/../../coax/web/project/landing.ts" || echo "home: keeping its last complete verified warm start" >&2
"$here/../../coax/web/project/build-home.sh" "$here/build/newland.wasm" &
newland=$!
# /docs/connect-a-pylon: the Pylon quickstart on the homepage's Grid (connect-pylon.coax loads newland.coax).
"$here/../../coax/web/wasm.sh" "$here/connect-pylon.coax" "$here/connect-pylon-face.c" "$here/build/connect-pylon.wasm" &
connectpylon=$!
# /verse and /verse/galaxy: the playtest's landing and galaxy.
"$here/../../coax/web/wasm.sh" "$here/verse.coax" "$here/verse-face.c" "$here/build/verse.wasm" &
verse_app=$!
# The project pages: every projects/<slug>/project.json with a page, its tasks
# read from Nostr into projects/<slug>/work.coax (a release always reads
# afresh, a dev build when the snapshot is ten minutes old; an unreachable
# relay keeps the last snapshot), then baked.ts writes its commits and the
# projects/baked.coax table project.coax draws from. Adding a project is data.
tools="$here/../../coax/web/project"
[[ -d "$tools/node_modules" ]] || (cd "$tools" && bun install --silent)
fresh=(); [[ "${COAXIAL_MODE:-dev}" == release ]] || fresh=(--if-older 10)
snaps=()
for slug in $(bun "$tools/baked.ts" --list); do
  bun "$tools/snapshot.ts" --project "$slug" ${fresh[@]+"${fresh[@]}"} & snaps+=($!)
done
# The /projects list: every project on the relay (kind 30621) into projects/list.coax.
bun "$tools/list.ts" ${fresh[@]+"${fresh[@]}"}
"$here/../../coax/web/wasm.sh" "$here/projects.coax" "$here/projects-face.c" "$here/build/projects.wasm" &
list=$!
for pid in "${snaps[@]}"; do wait "$pid"; done
bun "$tools/baked.ts"
"$here/../../coax/web/wasm.sh" "$here/project.coax" "$here/project-face.c" "$here/build/project.wasm" "$here/project-names.coax"
wait "$list"
# /nip/01: NIP-01 in Coax, its verifier its own library; the splash's passes.
python3 "$here/../../coax/web/nip-shader.py" "$here/splash.wgsl" "$here/nip01.wgsl"
"$here/../../coax/web/wasm.sh" "$here/nip01.coax" "$here/nip01-face.c" "$here/build/nip01.wasm" "$here/nip01-verify.coax" &
nip01=$!
cp "$here/nip01.wgsl" "$here/nip-guide.wgsl"
"$here/../../coax/web/wasm.sh" "$here/nip-guide.coax" "$here/nip-guide-face.c" "$here/build/nip-guide.wasm" "$here/nip-guide-observe.coax" &
nip_guide=$!
# /pylon: read-only live Pylon network, shared checked Coax verifier.
cp "$here/nip01.wgsl" "$here/pylon.wgsl"
"$here/../../coax/web/wasm.sh" "$here/pylon.coax" "$here/pylon-face.c" "$here/build/pylon.wasm" "$here/../../coax/web/project/landing-nostr.coax" &
pylon=$!
# Keep the public join command identical to the tested source, never a fork.
cp "$here/../../scripts/join-pylon.sh" "$here/pylon/join.sh"
# /admin/analytics: the projects list's HUD over the admins' numbers.
"$here/../../coax/web/wasm.sh" "$here/admin.coax" "$here/admin-face.c" "$here/build/admin.wasm" &
admin=$!
# /chat: a direct chat with the OpenAgents agent (docs/chat.md), admins only,
# drawn with the Zeron components (apps/shared/zeron/): Geist, its OFL and
# Zeron's icons (as one JSON, name to SVG) beside it for coax/web/gpui.js.
"$here/../../coax/web/wasm.sh" "$here/chat.coax" "$here/chat-face.c" "$here/build/chat.wasm" &
chat=$!
mkdir -p "$here/build/zeron"
cp "$here/../shared/zeron/fonts/"*.ttf "$here/../shared/zeron/fonts/Geist-OFL.txt" "$here/build/zeron/"
python3 -c 'import json,os,sys; d=sys.argv[1]; json.dump({f[:-4]: open(os.path.join(d,f)).read() for f in sorted(os.listdir(d)) if f.endswith(".svg")}, open(sys.argv[2],"w"))' \
  "$here/../shared/zeron/icons" "$here/build/zeron/icons.json"
# A private project's page (/projects/<slug>, served by oa-api to members).
COAXIAL_RUN_STEPS=250000000 "$here/../../coax/web/wasm.sh" "$here/private.coax" "$here/private-face.c" "$here/build/private.wasm" \
  "$here/../../coax/lib/qr/qr-memory.coax" "$here/../../coax/lib/nostr/weblogin.coax" "$here/../../coax/lib/crypto/entropy.coax"
wait "$admin"
wait "$chat"
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
wait "$droprealm"
wait "$builder"
wait "$myrealm"
wait "$icecream"
wait "$deadgears"
wait "$everglade"
wait "$fairy"
wait "$thegrid"
wait "$villa"
[[ -z "$newland" ]] || wait "$newland"
wait "$connectpylon"

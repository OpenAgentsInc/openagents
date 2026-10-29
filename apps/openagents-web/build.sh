#!/usr/bin/env bash
# Builds the pages for the browser: build/splash.wasm, build/project.wasm,
# build/projects.wasm, build/realm.wasm, build/admin.wasm, build/verse.wasm, build/garden.wasm, build/characters.wasm, build/fixture.wasm, build/icecream.wasm, build/deadgears.wasm, build/everglade.wasm, build/private.wasm and build/chat.wasm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
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
  && python3 coax/web/wgsl.py verse/haunt-light.coax verse_haunt_bloom_share --into apps/openagents-web/particles.wgsl)
m="$here/../../verse"
# The voice engine for pages that talk (data-voice: the Haunted House), coax/web/voice.js.
"$here/../../coax/web/voice/build.sh" "$here/build/voice.wasm" &
voice=$!
"$here/../../coax/web/wasm.sh" "$here/realm.coax" "$here/realm-face.c" "$here/build/realm.wasm" \
  "$m/haunt.coax" "$m/haunt-buffer.coax" "$m/player.coax" "$m/grid.coax" "$m/particles.coax" "$m/cinematic.coax" &
realm=$!
# /realm/garden: the engine's world pieces (camera, terrain, throwing,
# arranging) in an open-air realm, with the player and thumbs as their own libraries.
"$here/../../coax/web/wasm.sh" "$here/garden.coax" "$here/garden-face.c" "$here/build/garden.wasm" \
  "$m/player.coax" "$m/grid.coax" &
garden=$!
# /realm/everglade: the forest city's blockout (verse/everglade/plan.coax), with the player and thumbs.
"$here/../../coax/web/wasm.sh" "$here/everglade.coax" "$here/everglade-face.c" "$here/build/everglade.wasm" \
  "$m/player.coax" "$m/grid.coax" &
everglade=$!
# /realm/fixture: the realm host's test world, the smallest realm module.
"$here/../../coax/web/wasm.sh" "$here/fixture.coax" "$here/fixture-face.c" "$here/build/fixture.wasm" &
fixture=$!
# /realm/ice-cream-man: Ice Cream Man's house (verse/icecream/scene.coax, its own library) under its HUD.
"$here/../../coax/web/wasm.sh" "$here/icecream.coax" "$here/icecream-face.c" "$here/build/icecream.wasm" "$m/icecream/scene.coax" &
icecream=$!
# /realm/dead-gears: the ship survival game (verse/deadgears/rules.coax, loaded by the page).
"$here/../../coax/web/wasm.sh" "$here/deadgears.coax" "$here/deadgears-face.c" "$here/build/deadgears.wasm" &
deadgears=$!
# /realm/characters: the engine's figures (verse/figure.coax).
"$here/../../coax/web/wasm.sh" "$here/characters.coax" "$here/characters-face.c" "$here/build/characters.wasm" &
characters=$!
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
"$here/../../coax/web/wasm.sh" "$here/project.coax" "$here/project-face.c" "$here/build/project.wasm"
wait "$list"
# /admin/analytics: the projects list's HUD over the admins' numbers.
"$here/../../coax/web/wasm.sh" "$here/admin.coax" "$here/admin-face.c" "$here/build/admin.wasm" &
admin=$!
# /chat: a direct chat with the OpenAgents agent (docs/chat.md), admins only.
"$here/../../coax/web/wasm.sh" "$here/chat.coax" "$here/chat-face.c" "$here/build/chat.wasm" &
chat=$!
# A private project's page (/projects/<slug>, served by oa-api to members).
"$here/../../coax/web/wasm.sh" "$here/private.coax" "$here/private-face.c" "$here/build/private.wasm" \
  "$here/../../coax/lib/qr/qr-memory.coax"
wait "$admin"
wait "$chat"
wait "$splash"
wait "$realm"
wait "$voice"
wait "$verse_app"
wait "$garden"
wait "$characters"
wait "$fixture"
wait "$icecream"
wait "$deadgears"
wait "$everglade"

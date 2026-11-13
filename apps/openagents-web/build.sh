#!/usr/bin/env bash
# Builds the pages for the browser: build/splash.wasm, build/project.wasm,
# build/projects.wasm, build/realm.wasm, build/admin.wasm, build/audiolab.wasm, build/og.wasm, build/verse.wasm, build/garden.wasm, build/characters.wasm, build/fixture.wasm, build/droprealm.wasm, build/builder.wasm, build/worldbuilder.wasm, build/myrealm.wasm, build/icecream.wasm, build/deadgears.wasm, build/everglade.wasm, build/villa.wasm, build/private.wasm, build/chat.wasm, build/components.wasm, build/agents.wasm, build/worlds-web.wasm, build/project-files-web.wasm, build/gridcrowd.wasm, build/speedrunhud.wasm, build/status.wasm, build/login.wasm, build/security.wasm, build/life.wasm, build/devices.wasm, build/lightning.wasm, build/wallet.wasm and build/nip01.wasm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
# A failing step names itself (#13117): the line and the command, last in the
# output, and the modules still building stop instead of running on unseen.
# Every process under this one stops, not only its own jobs: a job's compilers
# and chains are its children, and stopping only the jobs left them running
# (#13154). A stopped build (TERM, HUP) stops them the same way.
descendants() { local c; for c in $(pgrep -P "$1" 2> /dev/null); do descendants "$c"; echo "$c"; done; }
stop_all() { local p; p="$(descendants "${1:-$$}")"; [[ -z "$p" ]] || kill $p 2> /dev/null || true; }
trap 'status=$?; trap - ERR; stop_all; wait 2> /dev/null || true
      echo "build.sh: FAILED (status $status) at line $LINENO: $BASH_COMMAND" >&2; exit "$status"' ERR
trap 'trap - ERR TERM HUP; stop_all; exit 143' TERM HUP
mkdir -p "$here/build"
repo="$(cd "$here/../.." && pwd)"
# The default build makes the pages of what is live. v1 Everglade (everglade, fivemmo,
# everglade5e), Experimental and the 5E Lab (fivelab) and the lab pages are the old set (#13269):
# a step with `old` in front runs only under COAXIAL_PAGES=all. A release sets it
# (coax/web/deploy.sh, coax/remote/pages.sh), so what ships is every page, as before. One of
# them alone, on a build host: coax/remote/check.sh module NAME. Every module, old or not, keeps
# its line in wasm-budgets.txt.
old() { [[ "${COAXIAL_PAGES:-live}" != all ]] || "$@"; }
# Every step's wall time (#13154): `timed NAME COMMAND...` runs the command and
# appends "NAME kind start end" (microseconds; kind is "serial" for a step the
# script waits on, "beside" for one started with &) to build/.steps. The table
# of them is the last thing a build prints.
steps="$here/build/.steps"; : > "$steps"
now() { [[ -n "${EPOCHREALTIME:-}" ]] && echo "${EPOCHREALTIME//[.,]/}" || echo $(( $(date +%s) * 1000000 )); }
build_began="$(now)"
timed() {
  local name="$1" began rc=0 kind=serial; shift; began="$(now)"
  [[ "$BASHPID" == "$$" ]] || kind=beside
  "$@" || rc=$?
  printf '%s %s %s %s\n' "$name" "$kind" "$began" "$(now)" >> "$steps"
  [[ "$rc" == 0 ]] || echo "build.sh: step $name failed (status $rc)" >&2
  return "$rc"
}
in_repo() { (cd "$repo" && "$@"); }
# Steps that depend on each other run as a chain: a subshell started with &,
# its first line `chain`. A chain that stops (a failed step, or this script's
# ERR trap) stops the jobs it started.
chain() { trap 'stop_all $BASHPID' EXIT; trap 'exit 143' TERM; }
# wasm.sh keys its cache of each unit's C and object by the unit's #load closure
# (coax/web/tool.sh wasm-size sources): build that tool once here, before the
# CLI's build and the modules below start side by side, so they don't each build it.
timed wasm-size-tool in_repo coax/web/tool.sh wasm-size sources apps/openagents-web/fixture.coax > /dev/null
# The openagents CLI (apps/openagents-cli, Coax) and what it reads from Nostr: the homepage's
# capture and the project pages' tables. The capture verifies every event (about 30 s) and the
# homepage's modules compile after it, so this chain is the longest thing a build waits for; it
# starts first, beside everything else (#13154). apps/openagents-cli/build.sh keeps the built CLI
# under its inputs, so a tree whose CLI sources were built before takes it in a second.
# Its modules load tables the steps below write (realms.coax, realms/list.coax), so each waits
# for those steps (after_generated) before it compiles.
oa="$here/../openagents-cli/build/openagents"
# The project verbs find the checkout from the working folder, so run them from
# this one: release.sh builds a separate tree and may start anywhere.
oa_here() { (cd "$here" && "$oa" "$@"); }
fresh=(); [[ "${COAXIAL_MODE:-dev}" == release ]] || fresh=(--if-older 10)
generated="$here/build/.generated"; rm -f "$generated"
after_generated() { while [[ ! -e "$generated" ]]; do kill -0 $$ 2> /dev/null || exit 1; sleep 0.2; done; }
(
  chain
  timed cli "$here/../openagents-cli/build.sh" --cli > /dev/null
  (
    chain
    # /status's snapshot is the commit a release ships (docs/metaverse/status-page.md).
    status_data=""
    if [[ "${COAXIAL_MODE:-release}" != dev ]]; then
      timed status-data in_repo coax/web/tool.sh coax/web/status-data.coax --write & status_data=$!
    fi
    timed project-landing oa_here project landing || echo "home: keeping its last complete verified warm start" >&2
    after_generated
    timed newland "$here/../../coax/web/project/build-home.sh" "$here/build/newland.wasm" &
    newland=$!
    # /docs/connect-a-pylon: the Pylon quickstart on the homepage's Grid (connect-pylon.coax loads newland.coax).
    timed connect-pylon "$here/../../coax/web/wasm.sh" "$here/connect-pylon.coax" "$here/connect-pylon-face.c" "$here/build/connect-pylon.wasm" &
    connectpylon=$!
    # /status: how a change reaches production, on the homepage's Grid (status.coax loads newland.coax).
    [[ -z "$status_data" ]] || wait "$status_data"
    timed status "$here/../../coax/web/wasm.sh" "$here/status.coax" "$here/status-face.c" "$here/build/status.wasm" &
    status_page=$!
    # /login and /login/email: email-code and key login, on the homepage's Grid (login.coax loads newland.coax).
    timed login "$here/../../coax/web/wasm.sh" "$here/login.coax" "$here/login-face.c" "$here/build/login.wasm" "$here/../../coax/lib/qr/qr-memory.coax" &
    login_page=$!
    # /settings/security: ways in, recovery and the re-auth prompt (security.coax loads login.coax).
    timed security "$here/../../coax/web/wasm.sh" "$here/security.coax" "$here/security-face.c" "$here/build/security.wasm" &
    security_page=$!
    # /settings/data and /settings/delete: freeze, export, merge and deletion (life.coax loads login.coax).
    timed life "$here/../../coax/web/wasm.sh" "$here/life.coax" "$here/life-face.c" "$here/build/life.wasm" &
    life_page=$!
    # /settings/devices: the device keys that hold app logins, with remove (devices.coax loads login.coax).
    timed devices "$here/../../coax/web/wasm.sh" "$here/devices.coax" "$here/devices-face.c" "$here/build/devices.wasm" &
    devices_page=$!
    # /settings/lightning: the name@openagents.com claim card (lightning.coax loads login.coax), with the address's QR.
    timed lightning "$here/../../coax/web/wasm.sh" "$here/lightning.coax" "$here/lightning-face.c" "$here/build/lightning.wasm" "$here/../../coax/lib/qr/qr-memory.coax" &
    lightning_page=$!
    # /settings/wallet: the wallet's daily limit and its encrypted backup (wallet.coax loads login.coax).
    timed wallet "$here/../../coax/web/wasm.sh" "$here/wallet.coax" "$here/wallet-face.c" "$here/build/wallet.wasm" &
    wallet_page=$!
    wait "$newland"; wait "$connectpylon"; wait "$status_page"; wait "$login_page"; wait "$security_page"; wait "$life_page"; wait "$devices_page"; wait "$lightning_page"; wait "$wallet_page"
  ) &
  home=$!
  # The project pages: every projects/<slug>/project.json with a page, its tasks
  # read from Nostr into projects/<slug>/work.coax (a release always reads
  # afresh, a dev build when the snapshot is ten minutes old; an unreachable
  # relay keeps the last snapshot), then `openagents project bake` writes its
  # commits and the projects/baked.coax table project.coax draws from. Adding a
  # project is data.
  (
    chain
    snaps=()
    t="$(now)"
    for slug in $(oa_here project bake --list); do
      oa_here project snapshot --project "$slug" ${fresh[@]+"${fresh[@]}"} & snaps+=($!)
    done
    for pid in "${snaps[@]}"; do wait "$pid"; done; printf 'project-snapshots beside %s %s\n' "$t" "$(now)" >> "$steps"
    timed project-bake oa_here project bake
    after_generated
    timed project "$here/../../coax/web/wasm.sh" "$here/project.coax" "$here/project-face.c" "$here/build/project.wasm" "$here/project-verify.coax"   # project-verify.coax: the capacity kernel, the NIP-19 names and the BIP-340 frame check
  ) &
  project_pages=$!
  # The /projects list: every project on the relay (kind 30621) into projects/list.coax,
  # then the pages that draw with the list's HUD (they load projects.coax).
  (
    chain
    timed project-index oa_here project index ${fresh[@]+"${fresh[@]}"}
    after_generated
    made=()
    timed projects "$here/../../coax/web/wasm.sh" "$here/projects.coax" "$here/projects-face.c" "$here/build/projects.wasm" & made+=($!)
    # /admin/analytics: the projects list's HUD over the admins' numbers.
    timed admin "$here/../../coax/web/wasm.sh" "$here/admin.coax" "$here/admin-face.c" "$here/build/admin.wasm" & made+=($!)
    # /realms/audio-lab: Everglade's sounds, each candidate and the raid mix (#12832), the projects list's HUD.
    old timed audiolab "$here/../../coax/web/wasm.sh" "$here/audiolab.coax" "$here/audiolab-face.c" "$here/build/audiolab.wasm" & made+=($!)
    # /og: the link-preview workbench (docs/web/og-tags.md), og.coax over og/pages.txt.
    timed og "$here/../../coax/web/wasm.sh" "$here/og.coax" "$here/og-face.c" "$here/build/og.wasm" & made+=($!)
    # A private project's page (/projects/<slug>, served by oa-api to members).
    timed private "$here/../../coax/web/wasm.sh" "$here/private.coax" "$here/private-face.c" "$here/build/private.wasm" \
      "$here/../../coax/lib/qr/qr-memory.coax" & made+=($!)
    for pid in "${made[@]}"; do wait "$pid"; done
  ) &
  project_list=$!
  wait "$home"; wait "$project_pages"; wait "$project_list"
) &
tables=$!
timed nip-pages python3 "$here/../../coax/web/nip-pages.py" --check
# The quickstart is authored once; stale generated document/shader bytes refuse a release.
timed connect-pylon-page bun "$here/../../coax/web/connect-pylon-page.ts" --check
# The public Coax release history is always built from the canonical Markdown.
timed changelog-page in_repo coax/web/tool.sh coax/web/changelog-page.coax --write
timed check-visuals "$here/../../coax/web/check-visuals.sh" "$here" &
visuals=$!
# Guard the shared-library/output race of parallel world builds (in its own temporary trees).
timed wasm-isolation python3 "$here/../../coax/web/tests/wasm_isolation.py" &
isolation=$!
# The realm host's table (realms.json): the galaxy's worlds into realms.coax
# and each realm's page (docs/metaverse/realm-host.md).
timed bitcoin-lab-map in_repo coax/web/tool.sh bitcoin-lab-map
timed realms bun "$here/../../coax/web/realms.ts"
# Every module this script writes has a size budget in wasm-budgets.txt, and
# every line there names one it writes: a release stops here, in seconds, not
# after the modules (wasm.sh and build-home.sh check each module's bytes).
if [[ "${COAXIAL_MODE:-release}" != dev ]]; then
  timed budgets in_repo coax/web/tool.sh wasm-size budgets apps/openagents-web/wasm-budgets.txt apps/openagents-web/build.sh
fi
: > "$generated"
# Host-contract v1 uses the same Coax PCM interpreter as native audio.
timed host-audio "$here/../../coax/web/tool.sh" "$here/../../coax/web/build-audio.coax" "$here/../.." "$here/build/host-audio.wasm" "$here/build/host-audio-timeline.wasm" &
host_audio=$!
# Canonical v1 image decode and mip bytes share the native Coax authority.
timed host-layers "$here/../../coax/web/tool.sh" "$here/../../coax/web/build-layers.coax" "$here/../.." "$here/build/host-layers.wasm" &
host_layers=$!
# Canonical v1 still-media bytes (PNG/JPEG) share the native Coax decoder.
timed host-media "$here/../../coax/web/tool.sh" "$here/../../coax/web/build-media.coax" "$here/../.." "$here/build/host-media.wasm" &
host_media=$!
# Physical bank/packet spans use the shared host-service authorities.
timed host-services "$here/../../coax/web/tool.sh" "$here/../../coax/web/build-services.coax" "$here/../.." "$here/build/host-services.wasm" &
host_services=$!
timed host-sockets "$here/../../coax/web/tool.sh" "$here/../../coax/web/build-sockets.coax" "$here/../.." "$here/build/host-sockets.wasm" &
host_sockets=$!
timed host-packs "$here/../../coax/web/tool.sh" "$here/../../coax/web/build-packs.coax" "$here/../.." "$here/build/host-packs.wasm" &
host_packs=$!
# Range-addressable packs (VPK3, metaverse-engine/editor/page-pack.coax, #13001): each .vpk3 a realm names in
# realms.json is written from its tracked .pack beside it (rewritten only when the pack changed),
# and so is each block build (metaverse-engine/tools/block-pack.coax) a realm's blocks: asset names.
# Everglade II's first takes the people's rig as its rig entry (everglade/model/tools/rig-pack.coax,
# #13306: version 2 of the tracked source everglade/model/rigs.vrg) and this tree's world by cell (everglade/model/tools/cell-pack.coax, #13280): the planting
# and the ground's record leave the catalog for a cells entry the page fetches cell by cell.
vpk3s() {
  # Before Everglade II's pack is written, its zone is checked (everglade/model/tools/zone-cells.coax, #13280,
  # #13281): the committed manifest of cells is what this land's colliders build, every spawn and named
  # point can be walked to, and a creature walks the named points' routes on the navigation cells.
  "$here/../../coax/web/tool.sh" "$here/../../everglade/model/tools/zone-cells.coax" --check "$here/../../everglade/world/zone-cells.coax" "$here/../../everglade/content/everglade.content" > /dev/null
  for vpk3 in "$here"/realms/everglade2/assets/everglade2.vpk3; do
    "$here/../../coax/web/tool.sh" "$here/../../everglade/model/tools/mount-pack-check.coax" "${vpk3%.vpk3}.pack" > /dev/null
    # the people's rig as its rig entry: the page bakes from it. It goes in first, because ...
    rigged="$here/build/$(basename "${vpk3%.vpk3}")-rigged.pack"
    "$here/../../coax/web/tool.sh" "$here/../../everglade/model/tools/rig-pack.coax" --into "$here/../../everglade/model/rigs.vrg" "${vpk3%.vpk3}.pack" "$rigged" people > /dev/null
    # ... the world's cells go in last: each cell's line in the catalog names its bytes' place in the pack.
    # page-pack defers the cells entry, so the pack publishes without it (docs/metaverse/one-world.md).
    celled="${rigged%-rigged.pack}-celled.pack"
    "$here/../../coax/web/tool.sh" "$here/../../everglade/model/tools/cell-pack.coax" "$rigged" "$celled" > /dev/null
    # the pack the page pages is one the admission takes as it is: sealing it changes no byte (#13304)
    "$here/../../coax/web/tool.sh" "$here/../../metaverse-engine/tools/pack-seal.coax" "$celled" "$celled.sealed" > /dev/null
    cmp -s "$celled" "$celled.sealed" || { echo "build.sh: Everglade II's pack by cell is not a sealed pack" >&2; exit 1; }
    "$here/../../coax/web/tool.sh" "$here/../../metaverse-engine/tools/page-pack.coax" "$celled" "$vpk3" > /dev/null
    rm -f "$rigged" "$celled" "$celled.sealed"
  done
  # WorldBuilder's people (its second pack, the character pack) take the rigs its zone bakes as their
  # rig entry: the people's and the two-handed clips' (#13306). The pack with them is written beside
  # the tracked one and is what realms.json names; it is rewritten only when its inputs changed.
  local people="$here/realms/character-creator/assets/fantasy" rigs="$here/../../everglade/model/rigs.vrg"
  if [[ ! -f "$people.rigs.pack.gz" || "$people.pack" -nt "$people.rigs.pack.gz" || "$rigs" -nt "$people.rigs.pack.gz" || "$here/../../everglade/model/tools/rig-pack.coax" -nt "$people.rigs.pack.gz" ]]; then
    "$here/../../coax/web/tool.sh" "$here/../../everglade/model/tools/rig-pack.coax" --into "$rigs" "$people.pack" "$here/build/fantasy-rigs.pack" people two-hand > /dev/null
    gzip -9 -n -c "$here/build/fantasy-rigs.pack" > "$people.rigs.pack.gz.new"
    mv "$people.rigs.pack.gz.new" "$people.rigs.pack.gz"; rm -f "$here/build/fantasy-rigs.pack"
  fi
  # Block builds (GitHub #13305): Everglade's layers as GPU blocks with their mip chains, NAME.TAG.vpk3
  # beside the pack, one a format its blocks: asset lists; host.js loads the one this device's GPU takes.
  local builds=()
  for tag in bc7 astc4; do
    "$here/../../coax/web/tool.sh" "$here/../../metaverse-engine/tools/block-pack.coax" "$here/realms/everglade/assets/everglade-zone.pack" \
      "$here/realms/everglade/assets/everglade-zone.$tag.vpk3" --format "$tag" > /dev/null &
    builds+=($!)
  done
  for b in "${builds[@]}"; do wait "$b" || return 1; done
}
timed vpk3 vpk3s &
vpk3=$!
# Opt-in room timing stays separate from the realm's producer.
timed host-latency "$here/../../coax/web/tool.sh" "$here/../../coax/web/build-latency.coax" "$here/../.." "$here/build/latency-client.wasm" &
room_latency=$!
# The HUD bar's light, the same function the iOS stages draw (metaverse/light.coax).
# (splash.wgsl loads metaverse/light.coax in its Coax: gpu/splash.coax.)
timed splash "$here/../../coax/web/wasm.sh" "$here/splash.coax" "$here/face.c" "$here/build/splash.wasm" &
splash=$!
# The realms: the engine's house, player and thumbs, each its own library as on
# iOS, under realm.coax's HUD; the house's light from the engine itself.
# (realm.wgsl, the house, loads its light in its Coax: gpu/realm.coax.)
# (particles.wgsl loads metaverse-engine/render/particle-motion.coax and haunt-light.coax in its Coax: gpu/particles.coax.)
# The art styles' shading (metaverse/style.coax, docs/metaverse/art-styles.md) into the fixture and every
# painterly realm's WGSL (realms.ts --check refuses a painterly realm without it).
style_fns="metaverse_style_wrap metaverse_style_bands metaverse_style_bands2 metaverse_style_shade metaverse_style_cool metaverse_style_keep_value metaverse_style_banded metaverse_style_ambient metaverse_style_dab metaverse_style_rim metaverse_style_haze metaverse_style_haze_glow metaverse_style_haze_teal metaverse_style_haze_hue metaverse_style_fog metaverse_style_sky_t metaverse_style_sky_band metaverse_style_halo metaverse_style_hash metaverse_style_noise metaverse_style_cloud_field metaverse_style_cloud_cover metaverse_style_cloud_lit metaverse_style_cloud_fade metaverse_style_tone metaverse_style_saturation metaverse_style_lift metaverse_style_luma metaverse_style_bloom_share metaverse_style_bloom_mix metaverse_style_shadow_reach metaverse_style_shadow_snap metaverse_style_shadow_across metaverse_style_shadow_depth metaverse_style_shadow_taps metaverse_style_shadow_radius metaverse_style_shadow_tap metaverse_style_contact metaverse_style_shaft_source metaverse_style_shaft_fade metaverse_style_shaft_tap metaverse_style_shaft_total metaverse_style_shaft metaverse_style_gust metaverse_style_wind metaverse_style_bend metaverse_style_grass_tilt metaverse_style_blade_height metaverse_style_grass_fade metaverse_style_grass_blades metaverse_style_grass_clear metaverse_style_segment metaverse_style_part metaverse_style_tip metaverse_style_jitter"
# The blob canopies (art-styles.md 7) go where there are kit trees or a traced tree: the fixture and Everglade.
foliage_fns="metaverse_style_canopy_axis metaverse_style_canopy_normal metaverse_style_canopy_sway metaverse_style_canopy_open metaverse_style_near_reach metaverse_style_near_blades metaverse_style_near_width metaverse_style_chroma_gain"
# (the fixture's Coax, gpu/fixture.coax, loads metaverse/look.coax and metaverse/style.coax.)
# Everglade's whole module is Coax (apps/openagents-web/gpu/everglade-v1/gpu.coax, with metaverse/style.coax): its
# entries and bindings (apps/openagents-web/gpu/everglade-v1/gpu.module, gpu.bindings) as coax wgsl writes them; its
# particles' module the same from gpu-fx.coax.
# Every page's and realm's WGSL is written from its Coax: apps/openagents-web/gpu/NAME.coax, with
# NAME.bindings and NAME.module, writes NAME.wgsl (docs/metaverse/native-gpu.md, G6), and so do the
# game's in everglade/scene/gpu/ and everglade/client/gpu/; none is
# written by hand. gpu/nip.coax is the NIP pages', written to nip01.wgsl below.
# They are committed (the dev server, GPU passes and golden checks read them without a build):
# commit what this writes. `coax/web/tool.sh coax/web/wgsl-module.coax --site --check` fails on drift.
# These two stay in turn, before the modules start: each `coax wgsl` under them has 120 s, and
# beside sixty module builds two of them ran out of it (#13154).
timed wgsl-site in_repo coax/web/tool.sh coax/web/wgsl-module.coax --site
# /nip/01's module is the splash's with sans glyphs (gpu/nip.coax), written after the site's; the NIP
# guide's and /pylon's are copies of it (generated, ignored; copied only on a change).
timed wgsl-nip in_repo coax/web/tool.sh coax/web/wgsl-module.coax apps/openagents-web/gpu/nip.coax apps/openagents-web/nip01.wgsl apps/openagents-web/gpu/nip.module
cmp -s "$here/nip01.wgsl" "$here/nip-guide.wgsl" || cp "$here/nip01.wgsl" "$here/nip-guide.wgsl"
cmp -s "$here/nip01.wgsl" "$here/pylon.wgsl" || cp "$here/nip01.wgsl" "$here/pylon.wgsl"
# The closed realm catalog owns these identities and module/entry/binding
# declarations. Unchanged packages reuse verified source and product hashes.
# Each package is its own folder, so they build side by side; with COAXIAL_PREBUILT=1
# (release.sh) a package a build host made from the same sources is taken as it is.
profiles="$(cd "$here/../.." && coax/web/tool.sh metaverse-engine/render/build.coax --profiles)"
render_packages() {
  local profile made=()
  while IFS= read -r profile; do
    in_repo coax/web/tool.sh metaverse-engine/render/build.coax "$profile" "$here/build/render/$profile" & made+=($!)
  done <<< "$profiles"
  for profile in "${made[@]}"; do wait "$profile"; done
}
(chain; timed render-packages render_packages) &
render=$!
# The low-poly facets (art-styles.md, Flat: facets): a face's own tone, nudged round corners, the light outline.
facet_fns="metaverse_style_facet_tone metaverse_style_facet_jitter metaverse_style_facet_edge"
# The flat style (art-styles.md, Flat) and its palettes go to the flat realms: Hecate's Cemetery and Fairy Forest.
flat_fns="metaverse_style_flat_light metaverse_style_flat_band metaverse_style_flat_band2 metaverse_style_flat_sky metaverse_style_flat_shadow metaverse_style_flat_fade metaverse_style_bayer metaverse_style_flat_dither metaverse_style_flat_hem metaverse_style_flat_srgb metaverse_style_flat_colour metaverse_style_flat_time"
# (Hecate's Cemetery, villa.wgsl, loads them in its Coax: gpu/villa.coax.)
# A made realm (myrealm.wgsl) is painterly or flat when its maker picked that (low poly keeps its own look).
# (its Coax, gpu/myrealm.coax, loads them.)
# Fairy Forest draws in the flat style only, faceted: no painterly or grass functions.
# (Fairy Forest, fairy.wgsl, loads them in its Coax: gpu/fairy.coax.)
# (Orbit League, orbitleague.wgsl, loads them in its Coax: gpu/orbitleague.coax.)
# (waterlab.wgsl loads metaverse/style.coax in its Coax: gpu/waterlab.coax.)
m="$here/../../metaverse"
a="$here/../../metaverse-engine"   # the engine modules (#12609)
# The voice engine for pages that talk (data-voice: the Haunted House), coax/web/voice.js.
timed voice "$here/../../coax/web/voice/build.sh" "$here/build/voice.wasm" &
voice=$!
timed realm "$here/../../coax/web/wasm.sh" "$here/realm.coax" "$here/realm-face.c" "$here/build/realm.wasm" \
  "$m/haunt.coax" "$m/haunt-buffer.coax" "$a/move/player.coax" "$m/grid.coax" "$a/render/particles.coax" "$a/cam/cinematic.coax" "$m/setpiece.coax" &
realm=$!
# /worlds/garden: the engine's world pieces (camera, terrain, throwing,
# arranging) in an open-air realm, with the player and thumbs as their own libraries.
timed garden "$here/../../coax/web/wasm.sh" "$here/garden.coax" "$here/garden-face.c" "$here/build/garden.wasm" \
  "$a/move/player.coax" "$m/grid.coax" &
garden=$!
# /worlds/everglade-classic (Everglade before the SRD 5e rules; everglade.coax is also the city everglade5e.coax loads): the forest city's blockout (everglade/legacy/v1/plan.coax), with the player, thumbs and its river (everglade/legacy/v1/river.coax).
old timed everglade "$here/../../coax/web/wasm.sh" "$here/everglade.coax" "$here/everglade-face.c" "$here/build/everglade.wasm" \
  "$here/../../everglade/shared/everglade-step.coax" "$m/grid.coax" "$here/../../everglade/legacy/v1/river.coax" "$here/../../everglade/npc/guard-guide-policy.coax" &
everglade=$!
# /worlds/5e-mmo: the SRD 5e battle in Everglade (fivemmo.coax on everglade.coax), the battle its own unit.
old timed fivemmo "$here/../../coax/web/wasm.sh" "$here/fivemmo.coax" "$here/fivemmo-face.c" "$here/build/fivemmo.wasm" \
  "$a/move/player.coax" "$m/grid.coax" "$here/../../everglade/legacy/v1/river.coax" "$here/fivemmo-battle.coax" &
fivemmo=$!
# /worlds/everglade (since #12710; /worlds/everglade-5e redirects): Everglade on the SRD 5e engine (everglade5e.coax on everglade.coax), the rules their own unit.
old timed everglade5e "$here/../../coax/web/wasm.sh" "$here/everglade5e.coax" "$here/everglade5e-face.c" "$here/build/everglade5e.wasm" \
  "$here/../../everglade/shared/everglade-step.coax" "$m/grid.coax" "$here/../../everglade/legacy/v1/river.coax" "$here/../../everglade/npc/guard-guide-policy.coax" "$here/everglade5e-rules.coax" &
everglade5e=$!
# Everglade's offline room (#12784): the room server's simulation, built to wasm for the local host (coax/web/room-worker.js).
timed everglade-room "$here/../../coax/web/tool.sh" "$here/../../coax/web/room/worker.coax" "$here/build/everglade-room.wasm" &
everglade_room=$!
# /worlds/fairy-forest: the park (metaverse/fairy-forest/forest.coax), with the player, thumbs and its stream (metaverse/fairy-forest/stream.coax).
timed fairy "$here/../../coax/web/wasm.sh" "$here/fairy.coax" "$here/fairy-face.c" "$here/build/fairy.wasm" \
  "$a/move/player.coax" "$m/grid.coax" "$m/fairy-forest/stream.coax" "$here/../../coax/lib/qr/qr-memory.coax" &
fairy=$!
# /worlds/hecates-cemetery: Planetary Villa's first expansion (metaverse/planetary-villa/README.md),
# with the player, thumbs and figures (villafig.coax: the engine's figures and motions) as libraries.
timed villa "$here/../../coax/web/wasm.sh" "$here/villa.coax" "$here/villa-face.c" "$here/build/villa.wasm" \
  "$a/move/player.coax" "$m/grid.coax" "$here/villafig.coax" &
villa=$!
old timed animationlab "$here/../../coax/web/wasm.sh" "$here/animationlab.coax" "$here/animationlab-face.c" "$here/build/animationlab.wasm" &
animationlab=$!
old timed ocarinalab "$here/../../coax/web/wasm.sh" "$here/ocarinalab.coax" "$here/ocarinalab-face.c" "$here/build/ocarinalab.wasm" &
ocarinalab=$!
timed nights "$here/../../coax/web/wasm.sh" "$here/nights.coax" "$here/nights-face.c" "$here/build/nights.wasm" &
nights=$!
# /worlds/fixture: the realm host's test world, the smallest realm module.
timed fixture "$here/../../coax/web/wasm.sh" "$here/fixture.coax" "$here/fixture-face.c" "$here/build/fixture.wasm" &
fixture=$!
# /worlds/waterlab: the water system's test scene (docs/metaverse/water.md), the spectral sea in Coax.
old timed waterlab "$here/../../coax/web/wasm.sh" "$here/waterlab.coax" "$here/waterlab-face.c" "$here/build/waterlab.wasm" &
waterlab=$!
old timed bitcoin-lab "$here/../../coax/web/wasm.sh" "$here/bitcoin-lab.coax" "$here/bitcoin-lab-face.c" "$here/build/bitcoin-lab.wasm" &
bitcoin_lab=$!
old timed valleylab "$here/../../coax/web/wasm.sh" "$here/valleylab.coax" "$here/valleylab-face.c" "$here/build/valleylab.wasm" &
valleylab=$!
old timed portallab "$here/../../coax/web/wasm.sh" "$here/portallab.coax" "$here/portallab-face.c" "$here/build/portallab.wasm" &
portallab=$!
timed valleycanyon "$here/../../coax/web/wasm.sh" "$here/valleycanyon.coax" "$here/valleycanyon-face.c" "$here/build/valleycanyon.wasm" &
valleycanyon=$!
# The character pass's runtime, loaded beside a world that places a character (coax/web/character-host.js).
timed character-host "$here/../../coax/web/wasm.sh" "$here/character-host.coax" "$here/character-host-face.c" "$here/build/character-host.wasm" &
characterhost=$!
timed valleyworld "$here/../../coax/web/wasm.sh" "$here/valleyworld.coax" "$here/valleyworld-face.c" "$here/build/valleyworld.wasm" &
valleyworld=$!
# /worlds/anime-lab: direct an anime (docs/metaverse/anime-lab.md): the director and stage (metaverse-engine/anime/), the
# anime style's block from its own library (animelab-style.coax). (animelab.wgsl loads metaverse/style.coax in gpu/animelab.coax.)
old timed animelab "$here/../../coax/web/wasm.sh" "$here/animelab.coax" "$here/animelab-face.c" "$here/build/animelab.wasm" "$here/animelab-style.coax" &
animelab=$!
# /worlds/splice-bots (unlisted): Mars at dusk on Anime Lab's shading (metaverse/splice-bots/splice-bots.coax, metaverse/splice-bots/splice-face.c, metaverse/splice-bots/gpu/splice-bots.coax).
timed splice-bots "$here/../../coax/web/wasm.sh" "$here/../../metaverse/splice-bots/splice-bots.coax" "$here/../../metaverse/splice-bots/splice-face.c" "$here/build/splice-bots.wasm" "$here/animelab-style.coax" &
splice=$!
# /worlds/crew (unlisted): character creation, six bodies and one fitted wardrobe on Anime Lab's shading (crew.coax, crew-face.c, gpu/crew.coax; #13462).
timed crew "$here/../../coax/web/wasm.sh" "$here/crew.coax" "$here/crew-face.c" "$here/build/crew.wasm" "$here/animelab-style.coax" &
crew=$!
# ... and its pane, the creation steps beside the scene: the frames set in the mono skin, drawn by the browser (crew-pane.coax, coax/web/dom-host.js).
timed crew-pane "$here/../../coax/web/wasm.sh" "$here/crew-pane.coax" "$here/crew-pane-face.c" "$here/build/crew-pane.wasm" &
crew_pane=$!
# The page's script module: a world's realm scripts, interpreted beside the realm's engine (script-host.coax, coax/web/host.js "A realm's scripts", #13467).
timed script-host "$here/../../coax/web/wasm.sh" "$here/script-host.coax" "$here/script-host-face.c" "$here/build/script-host.wasm" &
script_host=$!
# /worlds/drop and every /projects/<slug>/realm: a project's first realm, its drop's models on a floor.
timed droprealm "$here/../../coax/web/wasm.sh" "$here/droprealm.coax" "$here/droprealm-face.c" "$here/build/droprealm.wasm" &
droprealm=$!
# /worlds/built and every /worlds/<slug> made at /worlds/new: a realm someone built (myrealm.coax).
# YOUR COMPUTER's wire (metaverse/edit/computer-wire.coax) links beside it; its signing tables need the larger budget.
# With its renderer carrier, which loads the wire and entropy too, so one load graph owns
# sha256-kernel.coax's S_hash and S_hex (#12661); wasm.sh folds the two libraries into it (#12675).
COAXIAL_RUN_STEPS=250000000 timed myrealm "$here/../../coax/web/wasm.sh" "$here/myrealm.coax" "$here/myrealm-face.c" "$here/build/myrealm.wasm" \
  "$here/../../metaverse/edit/computer-wire.coax" "$here/../../coax/lib/crypto/entropy.coax" &
myrealm=$!
# /worlds/new: build your own realm (docs/metaverse/realm-builder.md).
timed builder "$here/../../coax/web/wasm.sh" "$here/builder.coax" "$here/builder-face.c" "$here/build/builder.wasm" &
builder=$!
# /create-world: chat a world onto an empty Grid (worldbuilder.coax, the API's coax/web/api/worldbuilder.c).
# It is an Everglade zone too (worldbuilder-walk.coax, everglade/zone.coax): your character walks the world you build.
timed worldbuilder "$here/../../coax/web/wasm.sh" "$here/worldbuilder-walk.coax" "$here/worldbuilder-face.c" "$here/build/worldbuilder.wasm" \
  "$here/../../everglade/shared/everglade-step.coax" "$a/core/sticks.coax" &
worldbuilder=$!
# /worlds/ice-cream-man: Ice Cream Man's house (metaverse/icecream/scene.coax, its own library) under its HUD.
timed icecream "$here/../../coax/web/wasm.sh" "$here/icecream.coax" "$here/icecream-face.c" "$here/build/icecream.wasm" "$m/icecream/scene.coax" &
icecream=$!
# /worlds/dead-gears: the ship survival game (metaverse/deadgears/rules.coax, loaded by the page).
timed deadgears "$here/../../coax/web/wasm.sh" "$here/deadgears.coax" "$here/deadgears-face.c" "$here/build/deadgears.wasm" &
deadgears=$!
# /worlds/hold-the-stair: the popping tower defense on the SRD 5e rules (rules/srd/rolls.coax).
timed stair "$here/../../coax/web/wasm.sh" "$here/stair.coax" "$here/stair-face.c" "$here/build/stair.wasm" &
stair=$!
# /worlds/painted-village: the hand-painted look (metaverse-engine/render/stylized.coax) on generated assets (metaverse-engine/arch/village.coax, flora.coax).
timed painted "$here/../../coax/web/wasm.sh" "$here/painted.coax" "$here/painted-face.c" "$here/build/painted.wasm" &
painted=$!
# /worlds/watchtower: a cyber defense range: find the intruder, isolate and reimage hosts (docs/research/cybersecurity).
timed watchtower "$here/../../coax/web/wasm.sh" "$here/watchtower.coax" "$here/watchtower-face.c" "$here/build/watchtower.wasm" &
watchtower=$!
# /worlds/5e-lab (unlisted): druids play the SRD 5e rules on a bare grid (rules/srd, docs/dnd).
# Player mode walks you on Everglade's controller (everglade-step.coax) and a phone's thumbs (metaverse-engine/core/sticks.coax).
old timed fivelab "$here/../../coax/web/wasm.sh" "$here/fivelab.coax" "$here/fivelab-face.c" "$here/build/fivelab.wasm" \
  "$here/../../everglade/shared/everglade-step.coax" "$a/core/sticks.coax" &
fivelab=$!
# /worlds/archery-range (unlisted): the Archery Range, an Everglade zone (everglade/zone.coax) with the range on it
# (everglade/docs/archery-range-spec.md). It draws with Character Create's passes (charactercreator.wgsl).
timed archery-range "$here/../../coax/web/wasm.sh" "$here/archery-range.coax" "$here/archery-range-face.c" "$here/build/archery-range.wasm" \
  "$here/../../everglade/shared/everglade-step.coax" "$a/core/sticks.coax" &
archery_range=$!
# /worlds/bitcoin-lab (unlisted): Bitcoin Lab, the Bitcoin book's first chapter as a walk on the Grid: an Everglade
# zone (everglade/zone.coax) with your character, its stations drawn as quads by bitcoin-lab-fx.wgsl (metaverse-engine/fx/gpu-quads.coax).
old timed bitcoin-lab-walk "$here/../../coax/web/wasm.sh" "$here/bitcoin-lab-walk.coax" "$here/bitcoin-lab-walk-face.c" "$here/build/bitcoin-lab-walk.wasm" \
  "$here/../../everglade/shared/everglade-step.coax" "$a/core/sticks.coax" &
bitcoin_lab_walk=$!
# /worlds/nostr-lab (unlisted): Nostr Lab, a key pair, an event, its ID and its signature as a walk on the Grid, on
# the same lab walk (metaverse/lab-walk.coax). It signs its event at run time: secp256k1's tables need the larger budget.
COAXIAL_RUN_STEPS=250000000 old timed nostr-lab-walk "$here/../../coax/web/wasm.sh" "$here/nostr-lab-walk.coax" "$here/nostr-lab-walk-face.c" "$here/build/nostr-lab-walk.wasm" \
  "$here/../../everglade/shared/everglade-step.coax" "$a/core/sticks.coax" &
nostr_lab_walk=$!
# /worlds/coax-lab (unlisted): Coax Lab, the Coax language as a walk on the Grid (Episode 296), on the same lab walk.
# Its Mandelbrot table is drawn at compile time: the larger budget.
COAXIAL_RUN_STEPS=60000000 old timed coax-lab-walk "$here/../../coax/web/wasm.sh" "$here/coax-lab-walk.coax" "$here/coax-lab-walk-face.c" "$here/build/coax-lab-walk.wasm" \
  "$here/../../everglade/shared/everglade-step.coax" "$a/core/sticks.coax" &
coax_lab_walk=$!
# /worlds/valley-lab/cull (unlisted): the GPU cull's lab (#13008), its residency and its fixture.
# An engine fixture, in the old set (#13269): coax/web/cull-check.coax needs COAXIAL_PAGES=all or the
# module built by name (coax/remote/check.sh module cullstream) and the fixture tool run by hand.
old timed cullstream "$here/../../coax/web/wasm.sh" "$here/cullstream.coax" "$here/cullstream-face.c" "$here/build/cullstream.wasm" &
cullstream=$!
old timed cull-fixture in_repo coax/web/tool.sh metaverse-engine/tools/cull-fixture.coax "$here/build" > /dev/null &
cullfixture=$!
# /worlds/everglade2 (unlisted): Everglade v2, the glade, one path through the woods and a clearing (docs/metaverse/everglade2.md).
timed everglade2 "$here/../../coax/web/wasm.sh" "$here/../../everglade/client/client.coax" "$here/everglade2-face.c" "$here/build/everglade2.wasm" \
  "$here/../../everglade/client/everglade-step.coax" "$a/core/sticks.coax" &
everglade2=$!
# /worlds/metahuman-lab (unlisted): one figure on the CC0 people rig's skeleton and clips (docs/metaverse/metahuman-support.md).
old timed metahumanlab "$here/../../coax/web/wasm.sh" "$here/metahumanlab.coax" "$here/metahumanlab-face.c" "$here/build/metahumanlab.wasm" &
metahumanlab=$!
# /worlds/splat-lab (unlisted): 3D Gaussian splats, GPU-sorted (docs/metaverse/2026-11-03-splat-lab.md).
old timed splatlab "$here/../../coax/web/wasm.sh" "$here/splatlab.coax" "$here/splatlab-face.c" "$here/build/splatlab.wasm" &
splatlab=$!
# /worlds/one-shot: the campaign creator's Table (oneshot.coax on oneshot-table.coax and the SRD engine).
timed oneshot "$here/../../coax/web/wasm.sh" "$here/oneshot.coax" "$here/oneshot-face.c" "$here/build/oneshot.wasm" &
oneshot=$!
# /demos/grid-crowd: the Grid's art (metaverse/tools/pack_grid.py) with a staged crowd of players (metaverse/cast.coax)
# and agents running into the Everglade gate, for clips (?stage=crowd, docs/metaverse/clips.md).
timed gridcrowd "$here/../../coax/web/wasm.sh" "$here/gridcrowd.coax" "$here/gridcrowd-face.c" "$here/build/gridcrowd.wasm" \
  "$m/cast.coax" &
gridcrowd=$!
# /demos/speedrun-hud: the Speedrun video's HUD diagrams over the Grid floor, one staged scene each
# (?stage=ident ... ?stage=end, docs/metaverse/clips.md), drawn at twice its pixels for clips.
timed speedrunhud "$here/../../coax/web/wasm.sh" "$here/speedrunhud.coax" "$here/speedrunhud-face.c" "$here/build/speedrunhud.wasm" &
speedrunhud=$!
# /worlds/belt-2200: the solar-system strategy game over the engine's orbits (metaverse-engine/physics/orbit.coax, metaverse-engine/cam/spacecam.coax; metaverse/stars.coax in gpu/belt.coax).
timed belt "$here/../../coax/web/wasm.sh" "$here/belt.coax" "$here/belt-face.c" "$here/build/belt.wasm" &
belt=$!
# /worlds/orbit-league: car ball in orbit, on the space primitives (docs/metaverse/space.md).
timed orbitleague "$here/../../coax/web/wasm.sh" "$here/orbitleague.coax" "$here/orbitleague-face.c" "$here/build/orbitleague.wasm" &
orbitleague=$!
# /worlds/characters: the engine's figures (metaverse-engine/anim/figure.coax).
timed characters "$here/../../coax/web/wasm.sh" "$here/characters.coax" "$here/characters-face.c" "$here/build/characters.wasm" &
characters=$!
# The public homepage and /new-land alias, and /worlds/the-grid: the same module walked (data-page 1, newland-walk.coax). Warm start is real Coax-verified
# public data; open pages independently watch and refresh Nostr in Coax.
# The capture is `openagents project landing` (apps/openagents-cli, Coax, built here in seconds).
# /coax/changelog: the compiler's complete release history, drawn in Coax.
timed changelog "$here/../../coax/web/wasm.sh" "$here/changelog.coax" "$here/changelog-face.c" "$here/build/changelog.wasm" &
changelog=$!
# /verse: the playtest's landing (verse.coax's page 1, the galaxy, is the native host's).
timed verse "$here/../../coax/web/wasm.sh" "$here/verse.coax" "$here/verse-face.c" "$here/build/verse.wasm" &
metaverse_app=$!
# /nip/01: NIP-01 in Coax, its verifier its own library; the splash's passes.
timed nip01 "$here/../../coax/web/wasm.sh" "$here/nip01.coax" "$here/nip01-face.c" "$here/build/nip01.wasm" "$here/nip01-verify.coax" &
nip01=$!
timed nip-guide "$here/../../coax/web/wasm.sh" "$here/nip-guide.coax" "$here/nip-guide-face.c" "$here/build/nip-guide.wasm" "$here/nip-guide-observe.coax" &
nip_guide=$!
# /pylon: read-only live Pylon network, shared checked Coax verifier.
timed pylon "$here/../../coax/web/wasm.sh" "$here/pylon.coax" "$here/pylon-face.c" "$here/build/pylon.wasm" "$here/../../coax/web/project/landing-nostr.coax" &
pylon=$!
# Keep the public join command identical to the tested source, never a fork.
cmp -s "$here/../../scripts/join-pylon.sh" "$here/pylon/join.sh" || cp "$here/../../scripts/join-pylon.sh" "$here/pylon/join.sh"
# Engine-owned declared-state adoption and host-conformance fixture (#12991). In the old set (#13269):
# its page and metaverse-engine/state/tests/oracle.py read a module built by name (check.sh module state-lab).
COAXIAL_RUN_STEPS=250000000 old timed state-lab "$here/../../coax/web/wasm.sh" "$here/../../metaverse-engine/state/fixture.coax" "$here/state-lab-face.c" "$here/build/state-lab.wasm" &
state_lab=$!
# /worlds: every world you can play, as kit entries beside the site's side (worlds-web.coax, drawn by the
# browser through coax/web/dom-host.js; its rows in realms/list.coax, written by realms.ts).
timed worlds-web "$here/../../coax/web/wasm.sh" "$here/worlds-web.coax" "$here/worlds-web-face.c" "$here/build/worlds-web.wasm" &
worlds_web=$!
# /projects/SLUG for a member: the project's attachments as kit entries (project-files-web.coax, opened by
# private.c's page through coax/web/dom-host.js with &path=; GitHub #13468).
timed project-files-web "$here/../../coax/web/wasm.sh" "$here/project-files-web.coax" "$here/project-files-web-face.c" "$here/build/project-files-web.wasm" &
project_files_web=$!
# /memorial/<slug>: a pet's memorial from Hecate's Cemetery, its page (oa-api serves the HTML, villa.c memorial_page).
timed memorial "$here/../../coax/web/wasm.sh" "$here/memorial.coax" "$here/memorial-face.c" "$here/build/memorial.wasm" &
memorial=$!
# /chat: a direct chat with the OpenAgents agent (docs/chat.md), admins only,
# drawn with the Zeron components (components/zeron/): Geist, its OFL and
# Zeron's icons (as one JSON, name to SVG) beside it for coax/web/gpui.js.
timed chat "$here/../../coax/web/wasm.sh" "$here/chat.coax" "$here/chat-face.c" "$here/build/chat.wasm" &
chat=$!
mkdir -p "$here/build/zeron"
cp "$here/../../components/zeron/fonts/"*.ttf "$here/../../components/zeron/fonts/Geist-OFL.txt" "$here/build/zeron/"
python3 -c 'import json,os,sys; d=sys.argv[1]; json.dump({f[:-4]: open(os.path.join(d,f)).read() for f in sorted(os.listdir(d)) if f.endswith(".svg")}, open(sys.argv[2],"w"))' \
  "$here/../../components/zeron/icons" "$here/build/zeron/icons.json"
# /components-gpu: the earlier component library's page (components/spec.md, #13153): every component in
# every skin, drawn by the library (components/gallery/page.coax) with the atlas assets above.
timed components "$here/../../coax/web/wasm.sh" "$here/components.coax" "$here/components-face.c" "$here/build/components.wasm" &
components_page=$!
# /components: the frames set in the mono skin, drawn by the browser through the browser backend
# (components/gallery/web.coax, coax/web/dom-host.js; docs/audits/2026-11-11-chat-zero-base-audit.md,
# sections 10 and 11). The page above is now /components-gpu.
timed kit "$here/../../coax/web/wasm.sh" "$here/kit.coax" "$here/kit-face.c" "$here/build/kit.wasm" &
kit_page=$!
# The editor shell `?edit` opens over an engine page (metaverse-engine/editor/shell.coax, #13416): the
# same components and skin, drawn by the browser over the canvas (coax/web/host.js opens it).
timed editor-shell "$here/../../coax/web/wasm.sh" "$here/editor-shell.coax" "$here/editor-shell-face.c" "$here/build/editor-shell.wasm" &
editor_shell=$!
# The icons (components/core/icons.coax): Apps SDK UI's, each its own file, fetched when a page shows it.
mkdir -p "$here/build/icons"
cp "$here/../../components/icons/apps-sdk/"*.svg "$here/../../components/icons/NOTICE" "$here/build/icons/"
# /chat, rebuilt from nothing: the composer alone, drawn by the browser (chat-web.coax, the same
# host and components). The page before it, chat.coax, is now /chat-gpu.
timed chat-web "$here/../../coax/web/wasm.sh" "$here/chat-web.coax" "$here/chat-web-face.c" "$here/build/chat-web.wasm" &
chat_web=$!
# /nostr-chat (unlisted): Nostr from scratch as a scripted chat on the same host and components, every
# value worked out in the page by coax/lib/nostr/ (nostr-chat.coax). It signs at run time: secp256k1's
# tables need the larger budget, as Nostr Lab's do.
COAXIAL_RUN_STEPS=250000000 timed nostr-chat "$here/../../coax/web/wasm.sh" "$here/nostr-chat.coax" "$here/nostr-chat-face.c" "$here/build/nostr-chat.wasm" &
nostr_chat=$!
# /settings/agents: an owner's agents and grants (#13211), the same components, atlas and shader.
timed agents "$here/../../coax/web/wasm.sh" "$here/agents.coax" "$here/agents-face.c" "$here/build/agents.wasm" &
agents_page=$!
wait "$visuals"
wait "$isolation"
wait "$vpk3"
wait "$render"
wait "$tables"
wait "$worlds_web"
wait "$project_files_web"
wait "$memorial"
wait "$chat"
wait "$components_page"
wait "$kit_page"
wait "$editor_shell"
wait "$chat_web"
wait "$nostr_chat"
wait "$agents_page"
wait "$nip01"
wait "$nip_guide"
wait "$pylon"
wait "$splash"
wait "$realm"
wait "$voice"
wait "$metaverse_app"
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
wait "$droprealm"
wait "$builder"
wait "$worldbuilder"
wait "$myrealm"
wait "$icecream"
wait "$deadgears"
wait "$stair"
wait "$painted"
wait "$watchtower"
wait "$fivelab"
wait "$archery_range"
wait "$bitcoin_lab_walk"
wait "$nostr_lab_walk"
wait "$coax_lab_walk"
wait "$everglade2"
# Everglade II's admin mode is a dev build's only (everglade/client/admin.coax, wasm.sh's --build-profile):
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
wait "$splice"
wait "$crew"
wait "$crew_pane"
wait "$script_host"
wait "$changelog"

wait "$state_lab"
# Where the time went: every step of a second or more, in the order they
# started. A "beside" step ran with others (or in a chain), so the seconds
# don't add up to the total; the script itself waited on the serial ones.
awk -v began="$build_began" -v ended="$(now)" '
  { s = ($4 - $3) / 1e6; if ($2 == "serial") serial += s; if (s >= 1) printf "%s %s %.1f %.1f\n", $1, $2, ($3 - began) / 1e6, s }
  END { printf "TOTAL - 0 %.1f %.1f\n", (ended - began) / 1e6, serial }' "$steps" | sort -k3,3n | awk '
  $1 == "TOTAL" { total = $4; serial = $5; next }
  { rows[++n] = sprintf("  %-22s %-7s started %6.1f s  took %6.1f s", $1, $2, $3, $4) }
  END { printf "build.sh: %.1f s, %.1f s of it in serial steps. Steps of a second or more:\n", total, serial; for (i = 1; i <= n; i++) print rows[i] }' >&2

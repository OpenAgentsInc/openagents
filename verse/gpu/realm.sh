#!/usr/bin/env bash
# Realms drawn natively on HOST's GPU by verse/gpu/vk-realm.c (docs/verse/native-gpu.md,
# gpu-realm-host), and travel between them: each realm's Coax as C (verse/native/build.sh), its
# live spec with its modules (and Everglade's layers decoded in Coax), built there with gcc, each
# in a directory of its own. REALM is one or more, the first where play starts: everglade (the
# default; everglade-pass live --emit), verse (the galaxy, page 1), newland (the Grid, page 1), stair (Hold the Stair), belt
# (Belt 2200), waterlab or orbitleague (no pack; flat-pass --emit), deadgears (Dead Gears),
# icecream (Ice Cream Man), villa (Hecate's Cemetery) or fairy (the Fairy Forest) (a scene with index
# assets; scene-pass --emit), haunted-house or sandbox (the realm module, their separate assets made
# one pack by verse/tools/pack-files.coax), fixture, drop or built (the fixture's pack), or painterly
# (the fixture opened with ?style=painterly, verse/native/painterly.facts: its casters and grass) (scene-pass --emit).
#   verse/gpu/realm.sh HOST check                    offscreen, each realm: 90 golden frames, the last
#                                                    against vk-passes drawing the spec's own frame, and
#                                                    the frame times at 320x180, 2560x1440 and 3840x2160;
#                                                    with verse among them, travel: the galaxy headless,
#                                                    a world picked with the arrows (REALM_PICK, the
#                                                    presses of Right) and opened with Enter, then the
#                                                    realm it names started as the screen would
#   verse/gpu/realm.sh HOST screen [vk-realm options: --seconds N --demo --size WxH --shot-at S]
#   verse/gpu/realm.sh HOST window [vk-realm options: --fullscreen --seconds N --size WxH --demo]
#                                                    the same travel as screen, in a window on HOST's
#                                                    Wayland desktop (vkw.h): no root, no VT; the
#                                                    window's close ends it (VERSE_WAYLAND_DISPLAY
#                                                    names the display, default wayland-1)
#   verse/gpu/realm.sh HOST install DIR              builds the realms and keeps them in DIR on HOST, with
#                                                    verse-session (os/verse-session.coax), the boot
#                                                    session that runs them (docs/os/2026-11-02-verse-os.md
#                                                    V0); DIR is replaced whole, the old one kept until the
#                                                    new one is in place
#   verse/gpu/realm.sh HOST ship DIR                 the generated tree only (C, specs, packs, the protocol
#                                                    sources), built by nothing: os/nix/pkgs/verse-realms.nix
#                                                    builds it into the image (HOST is unused; say local)
#   HOST local (or localhost) builds and runs here, with no ssh.
#   verse/gpu/realm.sh HOST net                      the network (verse/native/net.coax, the approved
#                                                    TLS): one request by the helper, then the first
#                                                    realm headless with --net, its requests and answers logged
# Travel (screen): a realm that opens another (a navigation: vk-realm --go writes the slug and leaves
# with status 3) is followed when that realm is one of REALM, as the browser would load its page;
# one that is not native, or a page that is the web's (a tip, a site), leaves the player where they
# were (the realm started again, its place kept). The time is the whole run's.
# screen runs online: the realm's requests go to openagents.com, and its player key, place and bar
# are kept in ~/.local/state/verse on HOST (vk-realm --net), so the next run starts where it left.
# REALM_ESCAPES="S:HOLD,..." (screen) starts a virtual keyboard (vk-escape) for each, pressing Escape at
# S seconds for HOLD: vk-realm leaves only on one held a second, a short press is the realm's.
# A run nobody plays (net, and screen with --demo) marks its requests X-Verse-Automation, so the
# site's usage leaves it out; a person's play has no mark.
# screen takes HOST's screen, so run it when nobody is using it. As display.sh does, on HOST as
# root (sudo -n): switch to a free VT, run, switch back; a watchdog switches back and kills it
# if it outlives --seconds by 20 s. The presented image at --shot-at comes back as realm-shot.ppm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
host="${1:?usage: realm.sh HOST check|screen|window|net [options]}"
mode="${2:?usage: realm.sh HOST check|screen|window|net [options]}"
# the commands below run on HOST: over ssh, or here for local
there() { if [[ "$host" == local || "$host" == localhost ]]; then bash -c "$1"; else ssh -o BatchMode=yes "$host" "$1"; fi; }
shift 2
dest=""
if [[ "$mode" == install || "$mode" == ship ]]; then dest="${1:?usage: realm.sh HOST $mode DIR}"; shift; fi
case "$mode" in check|screen|net|window|install|ship) ;; *) echo "realm.sh: unknown mode $mode" >&2; exit 2 ;; esac
renderer="${VERSE_GPU_RENDERER:-shared}"
case "$renderer" in shared|legacy) ;; *) echo "realm.sh: VERSE_GPU_RENDERER must be shared or legacy" >&2; exit 2 ;; esac
check_sizes="${VERSE_GPU_CHECK_SIZES-320x180 2560x1440 3840x2160}"
check_frames="${VERSE_GPU_CHECK_FRAMES:-600}"
[[ "$check_frames" =~ ^[1-9][0-9]*$ && "$check_frames" -le 90000 ]] || { echo "realm.sh: invalid VERSE_GPU_CHECK_FRAMES" >&2; exit 2; }
for size in $check_sizes; do [[ "$size" =~ ^[1-9][0-9]*x[1-9][0-9]*$ ]] || { echo "realm.sh: invalid VERSE_GPU_CHECK_SIZES" >&2; exit 2; }; done
work="$(mktemp -d "${TMPDIR:-/tmp}/realm.XXXXXX")"
trap 'rm -rf "$work"' EXIT
seconds=60
for ((i = 1; i <= $#; i++)); do [[ "${!i}" == --seconds ]] && { j=$((i + 1)); seconds="${!j}"; }; done

# The realms travel reaches: a slug (what a navigation names: rh_realm_of) and its module.
travel="galaxy:verse the-grid:newland hold-the-stair:stair belt-2200:belt waterlab:waterlab orbit-league:orbitleague everglade:everglade dead-gears:deadgears ice-cream-man:icecream hecates-cemetery:villa fairy-forest:fairy haunted-house:haunted-house sandbox:sandbox fixture:fixture drop:drop built:built garden:garden watchtower:watchtower"
# the module a realm runs, where its name is its slug
module_of_realm() {
  case "$1" in
    haunted-house|sandbox) echo realm ;;
    drop) echo droprealm ;;
    painterly) echo fixture ;;
    built) echo myrealm ;;
    *) echo "$1" ;;
  esac
}
read -r -a realms <<< "${REALM:-everglade}"
if [[ "$renderer" == shared ]]; then
  (cd "$root" && coax/web/tool.sh aethyr/render/build.coax --app-table) > "$work/profiles"
fi
profile_of() {
  # Identity comes from the compiled Coax catalog. A caller's explicit slug
  # wins; an old module alias must resolve to exactly one catalog row.
  local index slug app found="" count=0 module
  module="$(module_of_realm "$1")"
  while read -r index slug app; do [[ "$slug" != "$1" ]] || { echo "$slug"; return; }; done < "$work/profiles"
  while read -r index slug app; do
    if [[ "$app" == "$module" ]]; then found="$slug"; count=$((count + 1)); fi
  done < "$work/profiles"
  [[ "$count" == 1 ]] || { echo "realm.sh: ambiguous or absent Coax profile for $1" >&2; return 2; }
  echo "$found"
}
given_of() {
  case "$1" in
    everglade) echo "everglade.pack --facts everglade.facts --extra 9000" ;;
    verse|newland) echo "- --page 1" ;;
    haunted-house|sandbox) echo "$1.pack" ;;
    fixture|drop|built) echo "fixture.pack" ;;
    painterly) echo "fixture.pack --facts painterly.facts" ;;
    *) echo "-" ;;
  esac
}
for realm in "${realms[@]}"; do
  emit=()
  case "$realm" in
    everglade) emit=(verse/gpu/everglade-pass.coax "$host" live --emit "$work/$realm/spec") ;;
    verse|newland|stair|belt|waterlab|orbitleague) emit=(verse/gpu/flat-pass.coax "$host" "$realm" --emit "$work/$realm/spec") ;;
    deadgears|icecream|villa|fairy|haunted-house|sandbox|fixture|drop|built) emit=(verse/gpu/scene-pass.coax "$host" "$realm" --emit "$work/$realm/spec") ;;
    painterly) emit=(verse/gpu/scene-pass.coax "$host" fixture --facts "$root/verse/native/painterly.facts" --emit "$work/$realm/spec") ;;
    watchtower|garden) [[ "$renderer" == shared ]] || { echo "realm.sh: $realm has no legacy spec route" >&2; exit 2; } ;;
    *) echo "realm.sh: no realm $realm" >&2; exit 2 ;;
  esac
  build=("$(module_of_realm "$realm")" "$work/$realm/c")
  if [[ "$renderer" == shared ]]; then profile="$(profile_of "$realm")"; build+=(--render-profile "$profile"); fi
  "$root/verse/native/build.sh" "${build[@]}" > /dev/null
  if [[ ${#emit[@]} -gt 0 ]]; then (cd "$root" && coax/web/tool.sh "${emit[@]}") >&2; fi
  if [[ "$renderer" == shared ]]; then
    texture_source="$work/$realm/spec"; [[ -d "$texture_source" ]] || texture_source="-"
    (cd "$root" && coax/web/tool.sh aethyr/render/tools/native-textures.coax "$profile" "$texture_source" "$work/$realm/c/render-assets") > "$work/$realm/c/render-assets/textures.config.pending"
    mv "$work/$realm/c/render-assets/textures.config.pending" "$work/$realm/c/render-assets/textures.config"
  fi
  ship="$work/ship/$realm"
  mkdir -p "$ship/spec"
  if [[ ${#emit[@]} -gt 0 ]]; then (cd "$work/$realm/spec" && cp spec.txt *.spv *.spv.bindings *.bin "$ship/spec/"); fi
  cp "$work/$realm/c"/*.c "$work/$realm/c"/*.h "$work/$realm/c/sources" "$ship/"
  if [[ "$renderer" == shared ]]; then
    cp -R "$work/$realm/c/render" "$work/$realm/c/render-assets" "$ship/"
    cp "$work/$realm/c/render-native/"*.c "$work/$realm/c/render-native/"*.h "$ship/"
    printf '.\n' > "$ship/drawdir"
  else printf 'spec\n' > "$ship/drawdir"; fi
  cp "$here"/vk-realm.c "$here"/vk-spec.c "$here"/vk-spec.h "$here"/vks.c "$here"/vks.h "$here"/vkc.c "$here"/vkc.h \
     "$here"/vkp.c "$here"/vkp.h "$here"/vkd.c "$here"/vkd.h "$here"/vkw.h "$here"/vkdesk.h "$ship/"
  # the desk panel's policy (os/verse-panel.coax), linked into vk-realm
  (cd "$root" && "${COAX:-$root/coax/bin/coax}" c os/verse-panel.coax --library --header "$ship/verse-panel.h") > "$ship/verse-panel.c"
  # your hand as the pointer (os/verse-hands.coax), linked into vk-realm too
  (cd "$root" && "${COAX:-$root/coax/bin/coax}" c os/verse-hands.coax --library --header "$ship/verse-hands.h") > "$ship/verse-hands.c"
  cp "$here"/vkhands.h "$ship/"
  # the kms headers apart: a module has a realm.h of its own (the realm module's), and keys.h finds its sibling first
  mkdir -p "$ship/kms" && cp "$root"/verse/kms/evdev.h "$root"/verse/kms/mice.h "$root"/verse/kms/keys.h "$root"/verse/kms/realm.h "$ship/kms/"
  [[ "$realm" == everglade ]] && cp "$root/apps/openagents-web/realms/everglade/assets/everglade.pack" "$root/verse/native/everglade.facts" "$ship/"
  case "$realm" in
    haunted-house|sandbox) (cd "$root" && coax/web/tool.sh verse/tools/pack-files.coax "$realm" "$ship/$realm.pack") > /dev/null ;;
    fixture|drop|built) cp "$root/apps/openagents-web/realms/fixture/assets/fixture.pack" "$ship/" ;;
    painterly) cp "$root/apps/openagents-web/realms/fixture/assets/fixture.pack" "$root/verse/native/painterly.facts" "$ship/" ;;
  esac
  echo "$(given_of "$realm")" > "$ship/given"
done
cp "$root/verse/native/musl-math.sh" "$work/ship/"
cp "$here"/vk-passes.c "$here"/vk-spec.c "$here"/vk-spec.h "$here"/vks.c "$here"/vks.h "$here"/vkc.c "$here"/vkc.h "$here"/vk-escape.c "$work/ship/"
echo "${realms[*]}" > "$work/ship/realms"
echo "$travel" > "$work/ship/travel"
printf '%s\n' "$check_sizes" > "$work/ship/check-sizes"
printf '%s\n' "$check_frames" > "$work/ship/check-frames"
# the network helper: its Coax as C here, built there against OpenSSL (coax/platform/tls-env.sh's)
mkdir -p "$work/ship/net-src"
coax="${COAX:-$root/coax/bin/coax}"
(cd "$root" && "$coax" c coax/lib/crypto/entropy.coax --library --header "$work/ship/net-src/entropy-policy.h") > "$work/ship/net-src/entropy.c"
(cd "$root" && "$coax" c verse/native/net.coax --library) > "$work/ship/net-src/net.c"
(cd "$root" && "$coax" c os/verse-session.coax --library) > "$work/ship/net-src/verse-session.c"
(cd "$root" && "$coax" c os/verse-transcript.coax --library) > "$work/ship/net-src/verse-transcript.c"
(cd "$root" && "$coax" c os/game.coax --library) > "$work/ship/net-src/verse-game.c"
(cd "$root" && "$coax" c os/game-navigate.coax --library) > "$work/ship/net-src/verse-navigate.c"
(cd "$root" && "$coax" c os/game-sense.coax --library) > "$work/ship/net-src/verse-sense.c"
mkdir -p "$work/ship/wow"
cp -a "$root/os/wow/VerseNavigation" "$work/ship/wow/"
(cd "$root" && "$coax" c os/battlenet.coax --library) > "$work/ship/net-src/verse-battlenet.c"
(cd "$root" && "$coax" c os/gpu-limits.coax --library) > "$work/ship/net-src/gpu-limits.c"
# the desk: the Coax compositor (os/compositor.coax), headless, which vk-realm --desk runs
mkdir -p "$work/ship/desk"
(cd "$root" && "$coax" c os/compositor.coax --library) > "$work/ship/desk/compositor.c"
cp "$root/os/compositor-us.xkb" "$work/ship/desk/"
cp "$root/coax/platform/wayland-gpu.h" "$work/ship/net-src/"
for f in wayland-native screen-native; do cp "$root/coax/platform/$f.c" "$root/coax/platform/$f.h" "$work/ship/net-src/"; done
for f in tool-native process-native store-file-native read-file-native socket-native tls-native entropy-native entropy-foreign; do
  cp "$root/coax/platform/$f.c" "$root/coax/platform/$f.h" "$work/ship/net-src/"
done
args="$*"
automation=""
[[ "$mode" == net ]] && automation=native-net-check
[[ " $args " == *" --demo "* ]] && automation=native-demo
first="${realms[0]}"

if [[ "$mode" == ship ]]; then
  rm -rf "$dest" && mkdir -p "$dest" && cp -a "$work/ship/." "$dest/"
  echo "realm.sh: the tree for $(cat "$work/ship/realms") is in $dest"
  exit 0
fi
COPYFILE_DISABLE=1 tar --no-xattrs -C "$work/ship" -cf - . | there "set -e
d=\$(mktemp -d); trap 'rm -rf \$d' EXIT; tar -x -C \"\$d\" 2>/dev/null; cd \"\$d\"
nix-shell -p gcc pkg-config libdrm libglvnd libgbm vulkan-headers vulkan-loader vulkan-validation-layers openssl wayland wayland-scanner wayland-protocols --run '
  set -e
  # the window (vkw.h): xdg-shell and server-side decorations, generated for the wayland here
  wp=\$(pkg-config --variable=pkgdatadir wayland-protocols); mkdir -p wl
  for x in stable/xdg-shell/xdg-shell unstable/xdg-decoration/xdg-decoration-unstable-v1; do
    n=\$(basename \$x)
    wayland-scanner client-header \$wp/\$x.xml wl/\$n-client-protocol.h
    wayland-scanner private-code \$wp/\$x.xml wl/\$n-protocol.c
  done
  (cd net-src && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. net.c tool-native.c process-native.c store-file-native.c socket-native.c \
     tls-native.c entropy-native.c entropy-foreign.c entropy.c \$(pkg-config --cflags --libs openssl) -lpthread -lm -o ../net
   gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. verse-session.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c \
     entropy-native.c entropy-foreign.c entropy.c -lpthread -lm -o ../verse-session
   gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. verse-transcript.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c \
     entropy-native.c entropy-foreign.c entropy.c -lpthread -lm -o ../verse-transcript
   gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. verse-game.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c \
     entropy-native.c entropy-foreign.c entropy.c -lpthread -lm -o ../verse-game
   gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. verse-navigate.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c tls-native.c entropy-native.c entropy-foreign.c entropy.c \$(pkg-config --cflags --libs openssl) -lpthread -lm -o ../verse-navigate
   gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. verse-sense.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c entropy-native.c entropy-foreign.c entropy.c -lpthread -lm -o ../verse-sense
   gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. verse-battlenet.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c \
     entropy-native.c entropy-foreign.c entropy.c -lpthread -lm -o ../verse-battlenet
   gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. gpu-limits.c tool-native.c process-native.c store-file-native.c read-file-native.c socket-native.c \
     entropy-native.c entropy-foreign.c entropy.c -lpthread -lm -o ../gpu-limits
   gcc -std=c11 -O2 -D_DEFAULT_SOURCE -DCOAX_WAYLAND_GPU -w -I. \$(pkg-config --cflags libdrm egl glesv2 gbm) ../desk/compositor.c tool-native.c process-native.c store-file-native.c read-file-native.c \
     socket-native.c entropy-native.c entropy-foreign.c entropy.c wayland-native.c screen-native.c \$(pkg-config --libs libdrm egl glesv2 gbm) -lpthread -lm -o ../desk/compositor)
  # each realm as realm-run builds it: no PIE (its pointers fit 32 bits), no contraction
  flags=\"-std=gnu11 -O2 -no-pie -ffp-contract=off -fno-fast-math -w -I.\"
  # the math of the wasm (musl, verse/native/musl-math.sh) ahead of glibc, as realm-run links it
  math=\$(./musl-math.sh \$PWD)
  realm_pids=()
  for r in \$(cat realms); do
    (cd \$r
     read -r -a sources < sources
     renderer_units=(); link_flags=()
     if [ -f RenderIdentity.h ]; then
       renderer_units=(render-host.c render-realm.c render-media.c render-textures.c RenderTexture.c)
       flags=\"\$flags -ffunction-sections -fdata-sections\"; link_flags=(-Wl,--gc-sections)
     fi
     pids=()
     for s in \"\${sources[@]}\" \"\${renderer_units[@]}\" realm-host.c verse-panel.c verse-hands.c; do gcc \$flags -c \$s -o \${s%.c}.o & pids+=(\$!); done
     for pid in \"\${pids[@]}\"; do wait \$pid; done
     gcc \$flags -c ../wl/xdg-shell-protocol.c -o wl-xdg-shell.o && gcc \$flags -c ../wl/xdg-decoration-unstable-v1-protocol.c -o wl-xdg-decoration.o
     gcc \$flags \"\${link_flags[@]}\" -Ikms -I../wl \$(pkg-config --cflags libdrm wayland-client) vk-realm.c vk-spec.c vks.c vkc.c vkp.c vkd.c *.o \$math \$(pkg-config --libs libdrm wayland-client) -lvulkan -lm -o vk-realm
     cp ../net .) & realm_pids+=(\$!)
  done
  for pid in \"\${realm_pids[@]}\"; do wait \$pid; done
  gcc -std=c11 -O2 -I. vk-passes.c vk-spec.c vks.c vkc.c -lvulkan -lm -o vk-passes
  gcc -std=c11 -O2 -Wall vk-escape.c -o vk-escape
  echo \"VK_ADD_LAYER_PATH=\$(dirname \$(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1))\" > env
  echo \"LD_LIBRARY_PATH=\$(pkg-config --variable=libdir vulkan)\" >> env
' >&2
. ./env
export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json LD_LIBRARY_PATH
here=\$PWD
# The module a slug opens, when it is one of these realms (travel)
module_of() { for p in \$(cat \$here/travel); do [ \"\${p%%:*}\" = \"\$1\" ] && [ -d \"\$here/\${p#*:}\" ] && { echo \${p#*:}; return; }; done; }
if [ $mode = check ]; then
  differ=''
  for r in \$(cat realms); do
    cd \$here/\$r; given=\$(cat given); drawdir=\$(cat drawdir)
    # the frame the spec was written from (wasm-run's 90th, golden, clock 0), drawn by vk-passes and by vk-realm
    export VK_ADD_LAYER_PATH VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
    if [ -f spec/spec.txt ]; then (cd spec && \$here/vk-passes spec.txt \$here/\$r/passes.f32); fi
    VERSE_NATIVE_CLOCK_MS=0 ./vk-realm \$drawdir \$given --golden --size 320x180 --headless 90 --out \$here/\$r/realm.f32
    if [ -f passes.f32 ]; then
      cmp passes.f32 realm.f32 && echo \"realm.sh: \$r: the realm's 90th frame is vk-passes' to the bit\" || { echo \"realm.sh: \$r: the frames differ, \$(cmp -l passes.f32 realm.f32 | wc -l) bytes\"; differ=\"\$differ \$r\"; }
    else echo \"realm.sh: \$r:90-frame native execution checked; image comparison has its separate recorded fixture\"; fi
    # the frame times, without the validation layers
    unset VK_INSTANCE_LAYERS
    for size in \$(cat \$here/check-sizes); do ./vk-realm \$drawdir \$given --size \$size --headless \$(cat \$here/check-frames); done
  done
  if [ -d \$here/verse ]; then
    # travel: the galaxy, a world picked (Right REALM_PICK times) and opened (Enter)
    keys=''; f=20
    for i in \$(seq ${REALM_PICK:-2}); do keys=\"\$keys\$f:-5:1,\$((f + 1)):-5:0,\"; f=\$((f + 10)); done
    keys=\"\$keys\$f:-2:1,\$((f + 1)):-2:0\"
    cd \$here/verse; rm -f \$here/go
    status=0; VERSE_NATIVE_CLOCK_MS=0 ./vk-realm \$(cat drawdir) \$(cat given) --size 320x180 --headless 600 --keys \$keys --go \$here/go || status=\$?
    slug=\$(cat \$here/go 2>/dev/null || true)
    [ \$status = 3 ] && [ -n \"\$slug\" ] || { echo \"realm.sh: travel: the galaxy went nowhere (status \$status)\"; exit 1; }
    to=\$(module_of \$slug)
    if [ -n \"\$to\" ]; then
      cd \$here/\$to && ./vk-realm \$(cat drawdir) \$(cat given) --size 320x180 --headless 120 && echo \"realm.sh: travel: the galaxy opened \$slug, and \$to drew there\"
    else
      echo \"realm.sh: travel: the galaxy opened \$slug, which is not native here; the player stays in the galaxy\"
    fi
  fi
  [ -z \"\$differ\" ] || { echo \"realm.sh: the frames differ for\$differ\"; exit 1; }
  exit 0
fi
if [ $mode = net ]; then
  cd \$here/$first
  VERSE_AUTOMATION=$automation ./net \$here/answer openagents.com 443 4 '/api/playtest/planets?after=1000000000' - - 65536 10000
  echo \"realm.sh: the helper's answer: \$(head -c 120 \$here/answer | tr '\\n' ' ')\"
  VERSE_AUTOMATION=$automation VERSE_NATIVE_NET_LOG=1 ./vk-realm \$(cat drawdir) \$(cat given) --name $first --size 320x180 --headless 3000 --net \$here/kept
  echo \"realm.sh: kept: \$(ls \$here/kept 2>/dev/null | tr '\\n' ' ')\"
  exit 0
fi
if [ $mode = install ]; then
  # the built realms, kept: DIR.new filled, then swapped in for DIR
  dest=$dest; mkdir -p \$(dirname \$dest); rm -rf \$dest.new \$dest.old
  mkdir \$dest.new && cp -a \$here/. \$dest.new/ && rm -rf \$dest.new/net-src \$dest.new/ship 2>/dev/null
  # the desk's launchers (os/compositor.coax runs LAUNCH/NAME): a terminal to start with, and on Super+Return
  mkdir -p \$dest.new/desk/launch
  for n in start coder pane; do printf \"#!/bin/sh\\nexec foot\\n\" > \$dest.new/desk/launch/\$n; chmod +x \$dest.new/desk/launch/\$n; done
  ln -s ../../verse-battlenet \$dest.new/desk/launch/battlenet
  # Keep the installed optional CLI when this checkout has not built a replacement.
  if [ -x \$dest/openagents ] && [ ! -x \$dest.new/openagents ]; then cp \$dest/openagents \$dest.new/openagents; fi
  { [ ! -e \$dest ] || mv \$dest \$dest.old; } && mv \$dest.new \$dest && rm -rf \$dest.old
  # the store paths the programs link (Vulkan, libdrm, wayland, OpenSSL: nix-shell's), kept from
  # the garbage collector while this directory holds them
  mkdir -p \$dest/.gcroots
  for p in \$(LD_LIBRARY_PATH=\$LD_LIBRARY_PATH ldd \$dest/*/vk-realm \$dest/net \$dest/verse-session \$dest/desk/compositor 2>/dev/null | grep -o \"/nix/store/[^/ ]*\" | sort -u); do
    nix-store --add-root \$dest/.gcroots/\$(basename \$p) -r \$p > /dev/null
  done
  echo \"realm.sh: installed \$(cat \$dest/realms) in \$dest\"
  exit 0
fi
if [ $mode = window ]; then
  # travel in a window on the desktop, as screen does on the bare screen: no root, no VT, no watchdog
  # (the window's close, --seconds or a signal end it)
  export XDG_RUNTIME_DIR=\${XDG_RUNTIME_DIR:-/run/user/\$(id -u)} WAYLAND_DISPLAY=${VERSE_WAYLAND_DISPLAY:-wayland-1}
  mkdir -p \$HOME/.local/state/verse
  cur=$first; start=\$(date +%s); status=0
  while :; do
    left=\$(( $seconds - (\$(date +%s) - start) ))
    [ \$left -gt 0 ] || break
    rm -f \$here/go; status=0
    (cd \$here/\$cur && ${automation:+VERSE_AUTOMATION=$automation} ./vk-realm \$(cat drawdir) \$(cat given) --name \$cur --window --net \$HOME/.local/state/verse $args --seconds \$left --go \$here/go) || status=\$?
    [ \$status = 3 ] || break
    slug=\$(cat \$here/go 2>/dev/null || true); next=\$(module_of \$slug)
    if [ -n \"\$next\" ]; then echo \"realm.sh: \$cur opened \$slug (\$next)\"; cur=\$next; else echo \"realm.sh: \$cur opened \$slug, which is not native here; staying\"; fi
  done
  [ \$status = 3 ] && status=0
  exit \${status:-0}
fi
sudo -n rm -f /tmp/verse-realm-shot.ppm
from=\$(sudo -n fgconsole); to=\$(sudo -n fgconsole --next-available)
back() { sudo -n chvt \$from; }
trap 'back; rm -rf \$d' EXIT
# the watchdog: the screen comes back even if everything here dies
sudo -n setsid sh -c \"echo \\\$\\\$ > \$d/dog.pid; sleep $((seconds + 20)); pkill -KILL -x vk-realm; chvt \$from\" > /dev/null 2>&1 < /dev/null &
sudo -n chvt \$to
for e in \$(echo \"${REALM_ESCAPES:-}\" | tr , ' '); do sudo -n ./vk-escape \${e%%:*} \${e#*:} & done
sleep 1
# travel: each realm until one opens another (status 3); the whole run within --seconds
cur=$first; start=\$(date +%s); status=0
while :; do
  left=\$(( $seconds - (\$(date +%s) - start) ))
  [ \$left -gt 0 ] || break
  rm -f \$here/go; status=0
  (cd \$here/\$cur && sudo -n env VK_DRIVER_FILES=\$VK_DRIVER_FILES LD_LIBRARY_PATH=\$LD_LIBRARY_PATH ${automation:+VERSE_AUTOMATION=$automation} \
    ./vk-realm \$(cat drawdir) \$(cat given) --name \$cur --shot \$here/shot.ppm --net \$HOME/.local/state/verse $args --seconds \$left --go \$here/go) || status=\$?
  [ \$status = 3 ] || break
  slug=\$(cat \$here/go 2>/dev/null || true); next=\$(module_of \$slug)
  if [ -n \"\$next\" ]; then echo \"realm.sh: \$cur opened \$slug (\$next)\"; cur=\$next; else echo \"realm.sh: \$cur opened \$slug, which is not native here; staying\"; fi
done
[ \$status = 3 ] && status=0   # the time ran out on the way
sudo -n chown -R \$(id -u):\$(id -g) \$HOME/.local/state/verse 2>/dev/null || true
back
dog=\$(cat dog.pid 2>/dev/null || true)
[ -n \"\$dog\" ] && { sudo -n pkill -P \$dog 2>/dev/null; sudo -n kill \$dog 2>/dev/null; } || true
echo \"realm.sh: back on VT \$from (\$(sudo -n fgconsole))\"
[ -f shot.ppm ] && sudo -n cp shot.ppm /tmp/verse-realm-shot.ppm && sudo -n chmod 644 /tmp/verse-realm-shot.ppm
exit \${status:-0}
"
# the openagents CLI beside verse-session, which runs `openagents nostr new` for the Grid's identity
# (os/verse-session.coax --cli); build it first with apps/openagents-cli/build.sh
if [[ "$mode" == install && ( "$host" == local || "$host" == localhost ) ]]; then
  cli="$root/apps/openagents-cli/build/openagents"
  if [[ -x "$cli" ]]; then cp "$cli" "$dest/openagents.new" && mv "$dest/openagents.new" "$dest/openagents"; echo "realm.sh: the CLI is in $dest/openagents"
  elif [[ -x "$dest/openagents" ]]; then echo "realm.sh: kept the installed CLI"
  else echo "realm.sh: no $cli (apps/openagents-cli/build.sh); the Grid cannot make an identity"; fi
fi
if [[ "$mode" == screen ]]; then
  scp -q "$host:/tmp/verse-realm-shot.ppm" realm-shot.ppm 2>/dev/null && echo "realm.sh: the presented image is in realm-shot.ppm" || true
fi

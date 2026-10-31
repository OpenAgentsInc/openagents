#!/usr/bin/env bash
# Realms drawn natively on HOST's GPU by verse/gpu/vk-realm.c (docs/verse/native-gpu.md,
# gpu-realm-host), and travel between them: each realm's Coax as C (verse/native/build.sh), its
# live spec with its modules (and Everglade's layers decoded in Coax), built there with gcc, each
# in a directory of its own. REALM is one or more, the first where play starts: everglade (the
# default; everglade-pass live --emit), verse (the galaxy, page 1), newland (the Grid, page 1), stair (Hold the Stair), belt
# (Belt 2200), waterlab or orbitleague (no pack; flat-pass --emit).
#   verse/gpu/realm.sh HOST check                    offscreen, each realm: 90 golden frames, the last
#                                                    against vk-passes drawing the spec's own frame, and
#                                                    the frame times at 320x180, 2560x1440 and 3840x2160;
#                                                    with verse among them, travel: the galaxy headless,
#                                                    a world picked with the arrows (REALM_PICK, the
#                                                    presses of Right) and opened with Enter, then the
#                                                    realm it names started as the screen would
#   verse/gpu/realm.sh HOST screen [vk-realm options: --seconds N --demo --size WxH --shot-at S]
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
host="${1:?usage: realm.sh HOST check|screen|net [options]}"
mode="${2:?usage: realm.sh HOST check|screen|net [options]}"
shift 2
work="$(mktemp -d "${TMPDIR:-/tmp}/realm.XXXXXX")"
trap 'rm -rf "$work"' EXIT
seconds=60
for ((i = 1; i <= $#; i++)); do [[ "${!i}" == --seconds ]] && { j=$((i + 1)); seconds="${!j}"; }; done

# The realms travel reaches: a slug (what a navigation names: rh_realm_of) and its module.
travel="galaxy:verse the-grid:newland hold-the-stair:stair belt-2200:belt waterlab:waterlab orbit-league:orbitleague everglade:everglade"
read -r -a realms <<< "${REALM:-everglade}"
given_of() {
  case "$1" in
    everglade) echo "everglade.pack --facts everglade.facts --extra 9000" ;;
    verse|newland) echo "- --page 1" ;;
    *) echo "-" ;;
  esac
}
for realm in "${realms[@]}"; do
  case "$realm" in
    everglade) emit=(verse/gpu/everglade-pass.coax "$host" live --emit "$work/$realm/spec") ;;
    verse|newland|stair|belt|waterlab|orbitleague) emit=(verse/gpu/flat-pass.coax "$host" "$realm" --emit "$work/$realm/spec") ;;
    *) echo "realm.sh: no realm $realm" >&2; exit 2 ;;
  esac
  "$root/verse/native/build.sh" "$realm" "$work/$realm/c" > /dev/null
  (cd "$root" && coax/web/tool.sh "${emit[@]}") >&2
  ship="$work/ship/$realm"
  mkdir -p "$ship/spec"
  (cd "$work/$realm/spec" && cp spec.txt *.spv *.spv.bindings *.bin "$ship/spec/")
  cp "$work/$realm/c"/*.c "$work/$realm/c"/*.h "$work/$realm/c/sources" "$ship/"
  cp "$here"/vk-realm.c "$here"/vk-spec.c "$here"/vk-spec.h "$here"/vks.c "$here"/vks.h "$here"/vkc.c "$here"/vkc.h \
     "$here"/vkp.c "$here"/vkp.h "$here"/vkd.c "$here"/vkd.h "$root"/verse/kms/evdev.h "$root"/verse/kms/mice.h "$root"/verse/kms/keys.h "$root"/verse/kms/realm.h "$ship/"
  [[ "$realm" == everglade ]] && cp "$root/apps/openagents-web/realms/everglade/assets/everglade.pack" "$root/verse/native/everglade.facts" "$ship/"
  echo "$(given_of "$realm")" > "$ship/given"
done
cp "$here"/vk-passes.c "$here"/vk-spec.c "$here"/vk-spec.h "$here"/vks.c "$here"/vks.h "$here"/vkc.c "$here"/vkc.h "$here"/vk-escape.c "$work/ship/"
echo "${realms[*]}" > "$work/ship/realms"
echo "$travel" > "$work/ship/travel"
# the network helper: its Coax as C here, built there against OpenSSL (coax/platform/tls-env.sh's)
mkdir -p "$work/ship/net-src"
coax="${COAX:-$root/coax/bin/coax}"
(cd "$root" && "$coax" c coax/lib/crypto/entropy.coax --library --header "$work/ship/net-src/entropy-policy.h") > "$work/ship/net-src/entropy.c"
(cd "$root" && "$coax" c verse/native/net.coax --library) > "$work/ship/net-src/net.c"
for f in tool-native process-native store-file-native socket-native tls-native entropy-native entropy-foreign; do
  cp "$root/coax/platform/$f.c" "$root/coax/platform/$f.h" "$work/ship/net-src/"
done
args="$*"
automation=""
[[ "$mode" == net ]] && automation=native-net-check
[[ " $args " == *" --demo "* ]] && automation=native-demo
first="${realms[0]}"

COPYFILE_DISABLE=1 tar --no-xattrs -C "$work/ship" -cf - . | ssh -o BatchMode=yes "$host" "set -e
d=\$(mktemp -d); trap 'rm -rf \$d' EXIT; tar -x -C \"\$d\" 2>/dev/null; cd \"\$d\"
nix-shell -p gcc pkg-config libdrm vulkan-headers vulkan-loader vulkan-validation-layers openssl --run '
  set -e
  (cd net-src && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. net.c tool-native.c process-native.c store-file-native.c socket-native.c \
     tls-native.c entropy-native.c entropy-foreign.c entropy.c \$(pkg-config --cflags --libs openssl) -lpthread -lm -o ../net)
  # each realm as realm-run builds it: no PIE (its pointers fit 32 bits), no contraction
  flags=\"-std=gnu11 -O2 -no-pie -ffp-contract=off -fno-fast-math -w -I.\"
  for r in \$(cat realms); do
    (cd \$r
     read -r -a sources < sources
     for s in \"\${sources[@]}\" realm-host.c; do gcc \$flags -c \$s -o \${s%.c}.o & done
     wait
     gcc \$flags \$(pkg-config --cflags libdrm) vk-realm.c vk-spec.c vks.c vkc.c vkp.c vkd.c *.o \$(pkg-config --libs libdrm) -lvulkan -lm -o vk-realm
     cp ../net .) &
  done
  wait
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
  for r in \$(cat realms); do
    cd \$here/\$r; given=\$(cat given)
    # the frame the spec was written from (wasm-run's 90th, golden, clock 0), drawn by vk-passes and by vk-realm
    export VK_ADD_LAYER_PATH VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
    (cd spec && \$here/vk-passes spec.txt \$here/\$r/passes.f32)
    VERSE_NATIVE_CLOCK_MS=0 ./vk-realm spec \$given --golden --size 320x180 --headless 90 --out \$here/\$r/realm.f32
    cmp passes.f32 realm.f32 && echo \"realm.sh: \$r: the realm's 90th frame is vk-passes' to the bit\" || echo \"realm.sh: \$r: the frames differ\"
    # the frame times, without the validation layers
    unset VK_INSTANCE_LAYERS
    for size in 320x180 2560x1440 3840x2160; do ./vk-realm spec \$given --size \$size --headless 600; done
  done
  if [ -d \$here/verse ]; then
    # travel: the galaxy, a world picked (Right REALM_PICK times) and opened (Enter)
    keys=''; f=20
    for i in \$(seq ${REALM_PICK:-2}); do keys=\"\$keys\$f:-5:1,\$((f + 1)):-5:0,\"; f=\$((f + 10)); done
    keys=\"\$keys\$f:-2:1,\$((f + 1)):-2:0\"
    cd \$here/verse; rm -f \$here/go
    status=0; VERSE_NATIVE_CLOCK_MS=0 ./vk-realm spec \$(cat given) --size 320x180 --headless 600 --keys \$keys --go \$here/go || status=\$?
    slug=\$(cat \$here/go 2>/dev/null || true)
    [ \$status = 3 ] && [ -n \"\$slug\" ] || { echo \"realm.sh: travel: the galaxy went nowhere (status \$status)\"; exit 1; }
    to=\$(module_of \$slug)
    if [ -n \"\$to\" ]; then
      cd \$here/\$to && ./vk-realm spec \$(cat given) --size 320x180 --headless 120 && echo \"realm.sh: travel: the galaxy opened \$slug, and \$to drew there\"
    else
      echo \"realm.sh: travel: the galaxy opened \$slug, which is not native here; the player stays in the galaxy\"
    fi
  fi
  exit 0
fi
if [ $mode = net ]; then
  cd \$here/$first
  VERSE_AUTOMATION=$automation ./net \$here/answer openagents.com 443 4 '/api/playtest/planets?after=1000000000' - - 65536 10000
  echo \"realm.sh: the helper's answer: \$(head -c 120 \$here/answer | tr '\\n' ' ')\"
  VERSE_AUTOMATION=$automation VERSE_NATIVE_NET_LOG=1 ./vk-realm spec \$(cat given) --name $first --size 320x180 --headless 3000 --net \$here/kept
  echo \"realm.sh: kept: \$(ls \$here/kept 2>/dev/null | tr '\\n' ' ')\"
  exit 0
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
    ./vk-realm spec \$(cat given) --name \$cur --shot \$here/shot.ppm --net \$HOME/.local/state/verse $args --seconds \$left --go \$here/go) || status=\$?
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
if [[ "$mode" == screen ]]; then
  scp -q "$host:/tmp/verse-realm-shot.ppm" realm-shot.ppm 2>/dev/null && echo "realm.sh: the presented image is in realm-shot.ppm" || true
fi

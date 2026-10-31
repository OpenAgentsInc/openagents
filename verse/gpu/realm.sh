#!/usr/bin/env bash
# Everglade drawn natively on HOST's GPU by verse/gpu/vk-realm.c (docs/verse/native-gpu.md,
# gpu-realm-host): the realm's Coax as C (verse/native/build.sh), the live spec with its
# modules and the layers decoded in Coax (everglade-pass live --emit), built there with gcc.
#   verse/gpu/realm.sh HOST check                    offscreen: 90 golden frames, the last against
#                                                    vk-passes drawing the spec's own frame, and the
#                                                    frame times at 320x180 and at 2560x1440
#   verse/gpu/realm.sh HOST screen [vk-realm options: --seconds N --demo --size WxH --shot-at S]
#   verse/gpu/realm.sh HOST net                      the network (verse/native/net.coax, the approved
#                                                    TLS): one request by the helper, then the realm
#                                                    headless with --net, its requests and answers logged
# screen runs online: the realm's requests go to openagents.com, and its player key, place and bar
# are kept in ~/.local/state/verse on HOST (vk-realm --net), so the next run starts where it left.
# A run nobody plays (net, and screen with --demo) marks its requests X-Verse-Automation, so the
# site's usage leaves it out; a person's play has no mark.
# screen takes HOST's screen, so run it when nobody is using it. As display.sh does, on HOST as
# root (sudo -n): switch to a free VT, run, switch back; a watchdog switches back and kills it
# if it outlives --seconds by 20 s. The presented image at --shot-at comes back as realm-shot.ppm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
host="${1:?usage: realm.sh HOST check|screen [options]}"
mode="${2:?usage: realm.sh HOST check|screen [options]}"
shift 2
work="$(mktemp -d "${TMPDIR:-/tmp}/realm.XXXXXX")"
trap 'rm -rf "$work"' EXIT
seconds=60
for ((i = 1; i <= $#; i++)); do [[ "${!i}" == --seconds ]] && { j=$((i + 1)); seconds="${!j}"; }; done

"$root/verse/native/build.sh" everglade "$work/realm" > /dev/null
(cd "$root" && coax/web/tool.sh verse/gpu/everglade-pass.coax "$host" live --emit "$work/spec") >&2
mkdir -p "$work/ship/spec"
(cd "$work/spec" && cp spec.txt *.spv *.spv.bindings *.bin "$work/ship/spec/")
cp "$work/realm"/*.c "$work/realm"/*.h "$work/realm/sources" "$work/ship/"
cp "$here"/vk-realm.c "$here"/vk-passes.c "$here"/vk-spec.c "$here"/vk-spec.h "$here"/vks.c "$here"/vks.h "$here"/vkc.c "$here"/vkc.h \
   "$here"/vkp.c "$here"/vkp.h "$here"/vkd.c "$here"/vkd.h "$root"/verse/kms/evdev.h "$root"/verse/kms/mice.h "$root"/verse/kms/keys.h "$root"/verse/kms/realm.h "$work/ship/"
cp "$root/apps/openagents-web/realms/everglade/assets/everglade.pack" "$root/verse/native/everglade.facts" "$work/ship/"
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

COPYFILE_DISABLE=1 tar --no-xattrs -C "$work/ship" -cf - . | ssh -o BatchMode=yes "$host" "set -e
d=\$(mktemp -d); trap 'rm -rf \$d' EXIT; tar -x -C \"\$d\" 2>/dev/null; cd \"\$d\"
nix-shell -p gcc pkg-config libdrm vulkan-headers vulkan-loader vulkan-validation-layers openssl --run '
  set -e
  (cd net-src && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I. net.c tool-native.c process-native.c store-file-native.c socket-native.c \
     tls-native.c entropy-native.c entropy-foreign.c entropy.c \$(pkg-config --cflags --libs openssl) -lpthread -lm -o ../net)
  read -r -a sources < sources
  # the realm as realm-run builds it: no PIE (its pointers fit 32 bits), no contraction
  flags=\"-std=gnu11 -O2 -no-pie -ffp-contract=off -fno-fast-math -w -I.\"
  for s in \"\${sources[@]}\" realm-host.c; do gcc \$flags -c \$s -o \${s%.c}.o & done
  wait
  gcc \$flags \$(pkg-config --cflags libdrm) vk-realm.c vk-spec.c vks.c vkc.c vkp.c vkd.c *.o \$(pkg-config --libs libdrm) -lvulkan -lm -o vk-realm
  gcc -std=c11 -O2 -I. vk-passes.c vk-spec.c vks.c vkc.c -lvulkan -lm -o vk-passes
  echo \"VK_ADD_LAYER_PATH=\$(dirname \$(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1))\" > env
  echo \"LD_LIBRARY_PATH=\$(pkg-config --variable=libdir vulkan)\" >> env
' >&2
. ./env
export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json LD_LIBRARY_PATH
here=\$PWD
if [ $mode = check ]; then
  # the frame the spec was written from (wasm-run's 90th, golden, clock 0), drawn by vk-passes and by vk-realm
  export VK_ADD_LAYER_PATH VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
  (cd spec && ../vk-passes spec.txt \$here/passes.f32)
  VERSE_NATIVE_CLOCK_MS=0 ./vk-realm spec everglade.pack --facts everglade.facts --extra 9000 --golden --size 320x180 --headless 90 --out \$here/realm.f32
  cmp passes.f32 realm.f32 && echo 'realm.sh: the realm'\"'\"'s 90th frame is vk-passes'\"'\"' to the bit' || echo 'realm.sh: the frames differ'
  # the frame times, without the validation layers
  unset VK_INSTANCE_LAYERS
  ./vk-realm spec everglade.pack --facts everglade.facts --extra 9000 --size 320x180 --headless 600
  ./vk-realm spec everglade.pack --facts everglade.facts --extra 9000 --size 2560x1440 --headless 600
  ./vk-realm spec everglade.pack --facts everglade.facts --extra 9000 --size 3840x2160 --headless 600
  exit 0
fi
if [ $mode = net ]; then
  VERSE_AUTOMATION=$automation ./net \$here/answer openagents.com 443 4 '/api/playtest/planets?after=1000000000' - - 65536 10000
  echo \"realm.sh: the helper's answer: \$(head -c 120 answer | tr '\\n' ' ')\"
  VERSE_AUTOMATION=$automation VERSE_NATIVE_NET_LOG=1 ./vk-realm spec everglade.pack --facts everglade.facts --extra 9000 --size 320x180 --headless 3000 --net \$here/kept
  echo \"realm.sh: kept: \$(ls kept 2>/dev/null | tr '\\n' ' ')\"
  exit 0
fi
sudo -n rm -f /tmp/verse-realm-shot.ppm
from=\$(sudo -n fgconsole); to=\$(sudo -n fgconsole --next-available)
back() { sudo -n chvt \$from; }
trap 'back; rm -rf \$d' EXIT
# the watchdog: the screen comes back even if everything here dies
sudo -n setsid sh -c \"echo \\\$\\\$ > \$d/dog.pid; sleep $((seconds + 20)); pkill -KILL -x vk-realm; chvt \$from\" > /dev/null 2>&1 < /dev/null &
sudo -n chvt \$to
sleep 1
sudo -n env VK_DRIVER_FILES=\$VK_DRIVER_FILES LD_LIBRARY_PATH=\$LD_LIBRARY_PATH ${automation:+VERSE_AUTOMATION=$automation} \
  ./vk-realm spec everglade.pack --facts everglade.facts --extra 9000 --shot \$d/shot.ppm --net \$HOME/.local/state/verse $args || status=\$?
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

#!/usr/bin/env bash
# Star Catcher straight on HOST's screen with Vulkan, no desktop
# (verse/gpu/vk-display.c), the G2 boot-session check
# (docs/verse/native-gpu.md). It takes the screen, so run it when nobody is
# using it.
#   verse/gpu/display.sh HOST [vk-display options: --seconds N --demo --shot-at S]
#   VERSE_DISPLAY_ESCAPE=S   also press Escape (a virtual keyboard) after S seconds
# On HOST, as root (sudo -n): switch to a free VT so the desktop pauses and
# lets go of the display, run, and switch back. A watchdog switches back and
# kills the host if it outlives --seconds by 20 s. The presented image at
# --shot-at comes back as display-shot.ppm in the current directory.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
host="${1:?usage: display.sh HOST [--seconds N] [--demo] [--shot-at S]}"
shift
coax="${COAX:-$root/coax/bin/coax}"
work="$(mktemp -d "${TMPDIR:-/tmp}/display.XXXXXX")"
trap 'rm -rf "$work"' EXIT
seconds=60
for ((i = 1; i <= $#; i++)); do [[ "${!i}" == --seconds ]] && { j=$((i + 1)); seconds="${!j}"; }; done

VERSE_KMS_GEN_ONLY=1 VERSE_KMS_REGEN=1 "$root/verse/kms/build.sh" "$work" > /dev/null
light="$root/verse/kms/starcatch-light.coax"
"$coax" spirv "$light" --fill starcatch_sky_r,starcatch_sky_g,starcatch_sky_b -o "$work/sky.spv"
"$coax" spirv "$light" --stroke starcatch_fx -o "$work/fx.spv"
cp "$here"/vk-display.c "$here"/vk-escape.c "$here"/vkc.c "$here"/vkc.h "$here"/vkd.c "$here"/vkd.h "$here"/vkp.c "$here"/vkp.h "$here"/vkr.c "$here"/vkr.h "$work/"
args="$*"
escape="${VERSE_DISPLAY_ESCAPE:-}"

COPYFILE_DISABLE=1 tar --no-xattrs -C "$work" -cf - . | ssh -o BatchMode=yes "$host" "set -e
d=\$(mktemp -d); trap 'rm -rf \$d' EXIT; tar -x -C \"\$d\" 2>/dev/null; cd \"\$d\"
nix-shell -p gcc pkg-config libdrm vulkan-headers vulkan-loader vulkan-validation-layers --run '
  set -e
  realm=\"starcatch-native.c starcatch.c starcatch-light.c\"
  gcc -std=c11 -O2 -Wall -Wno-unknown-pragmas -I. \$(pkg-config --cflags libdrm) vk-display.c vkc.c vkd.c vkp.c vkr.c \$realm \$(pkg-config --libs libdrm) -lvulkan -lm -o vk-display
  gcc -std=c11 -O2 -Wall vk-escape.c -o vk-escape
  ${VERSE_DISPLAY_BUILD_ONLY:+echo built; exit 0}
  echo \"VK_ADD_LAYER_PATH=\$(dirname \$(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path \"*validation-layers*\" | head -1))\" > env
  echo \"LD_LIBRARY_PATH=\$(pkg-config --variable=libdir vulkan)\" >> env
'
[ -x vk-display ] || exit 0
[ -n \"${VERSE_DISPLAY_BUILD_ONLY:-}\" ] && exit 0
. ./env
sudo -n rm -f /tmp/verse-display-shot.ppm
from=\$(sudo -n fgconsole); to=\$(sudo -n fgconsole --next-available)
back() { sudo -n chvt \$from; }
trap 'back; rm -rf \$d' EXIT
# The watchdog: the screen comes back even if everything here dies.
sudo -n setsid sh -c \"echo \\\$\\\$ > \$d/dog.pid; sleep $((seconds + 20)); pkill -KILL -x vk-display; chvt \$from\" > /dev/null 2>&1 < /dev/null &
sudo -n chvt \$to
sleep 1
[ -n \"$escape\" ] && { sudo -n ./vk-escape $escape & sleep 0.5; }
sudo -n env VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json VK_ADD_LAYER_PATH=\$VK_ADD_LAYER_PATH \
  VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation LD_LIBRARY_PATH=\$LD_LIBRARY_PATH \
  ./vk-display sky.spv 0,1,2,4,5 fx.spv 3 --shot \$d/shot.ppm $args || status=\$?
back
dog=\$(cat dog.pid 2>/dev/null || true)
[ -n \"\$dog\" ] && { sudo -n pkill -P \$dog 2>/dev/null; sudo -n kill \$dog 2>/dev/null; } || true
echo \"display.sh: back on VT \$from (\$(sudo -n fgconsole))\"
[ -f shot.ppm ] && sudo -n cp shot.ppm /tmp/verse-display-shot.ppm && sudo -n chmod 644 /tmp/verse-display-shot.ppm
exit \${status:-0}
"
if [[ -z "${VERSE_DISPLAY_BUILD_ONLY:-}" ]]; then
  scp -q "$host:/tmp/verse-display-shot.ppm" display-shot.ppm 2>/dev/null && echo "display.sh: the presented image is in display-shot.ppm" || true
fi

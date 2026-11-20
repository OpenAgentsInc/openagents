#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around build.sh, nix-shell, wine, Xvfb, openbox and xdotool; holds no logic)
# Opens the Windows app's real window under wine on a virtual screen (GitHub #13782), types a
# message, sends it (with no model key the reply says so, through mcworker.exe), opens Foreman
# and settings, and saves a picture after each step in windows/build/wine-shots/:
#   OA_REMOTE=1 coax/remote/remote.sh run -- apps/openagents-desktop/windows/wine-window.sh
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
if [[ -z "${OA_WINE_SMOKE:-}" ]]; then
  export OA_WINE_SMOKE=1
  exec nix-shell -p wineWow64Packages.stable mesa vulkan-loader zig vulkan-headers zip xvfb-run imagemagick xdotool openbox --run "exec $(printf '%q' "$here/wine-window.sh")"
fi
"$here/build.sh" --arch x64 >/dev/null
export VK_DRIVER_FILES="$(ls -1 /nix/store/*mesa*/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | grep x86_64 | head -n 1)"
export WINEPREFIX="$(mktemp -d)" WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml="
out="$here/build/wine-shots"; rm -rf "$out"; mkdir -p "$out"
xvfb-run -a -s "-screen 0 1600x1000x24" bash -c '
  openbox & wine wineboot -i >/dev/null 2>&1
  wine "'"$here"'/build/x64/OpenAgents.exe" & app=$!
  sleep 20; import -window root "'"$out"'/1-open.png"
  w=$(xdotool search --name OpenAgents | head -1); xdotool windowfocus --sync $w; xdotool mousemove 930 520 click 1; sleep 1
  xdotool type --delay 60 "hello from wine"; sleep 3; import -window root "'"$out"'/2-typed.png"
  xdotool key Return; sleep 8; import -window root "'"$out"'/3-sent.png"
  xdotool mousemove 265 221 click 1; sleep 2; import -window root "'"$out"'/4-foreman.png"
  xdotool mousemove 447 900 click 1; sleep 2; import -window root "'"$out"'/5-settings.png"
  wineserver -k; wait $app 2>/dev/null
' 
find "$WINEPREFIX/drive_c/users" -name "openagents.log*" -exec cp {} "$out/" \;
cat "$out"/openagents.log 2>/dev/null | head -30
ls -la "$out"

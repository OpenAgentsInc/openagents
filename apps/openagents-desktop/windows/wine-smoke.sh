#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around build.sh, nix-shell and wine; holds no logic)
# Runs the Windows app's headless smoke (host-main.c run_smoke: the page drawn offscreen on Vulkan,
# a chat kept and read back, an untrusted update refused, and mcworker.exe started until it says
# it is ready) under wine with Mesa's software Vulkan, on a Linux build host (GitHub #13782):
#   OA_REMOTE=1 coax/remote/remote.sh run -- apps/openagents-desktop/windows/wine-smoke.sh
# It is the nearest check to a Windows computer this side of one; it does not stand in for one.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
if [[ -z "${OA_WINE_SMOKE:-}" ]]; then
  export OA_WINE_SMOKE=1
  exec nix-shell -p wineWow64Packages.stable mesa vulkan-loader zig vulkan-headers zip --run "exec $(printf '%q' "$here/wine-smoke.sh")"
fi
"$here/build.sh" --arch x64
icd=""
for f in $(printf '%s\n' ${XDG_DATA_DIRS:-} ${NIX_PROFILES:-} | tr ':' '\n'); do
  for j in "$f"/share/vulkan/icd.d/lvp_icd*.json "$f"/vulkan/icd.d/lvp_icd*.json; do [[ -f "$j" ]] && { icd="$j"; break 2; }; done
done
[[ -n "$icd" ]] || icd="$(ls -1 /nix/store/*mesa*/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -n 1 || true)"
[[ -n "$icd" ]] && export VK_DRIVER_FILES="$icd"
export WINEPREFIX="$(mktemp -d)" WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml="
trap 'wineserver -k 2>/dev/null || true; rm -rf "$WINEPREFIX"' EXIT
wine=wine; command -v wine64 >/dev/null && ! command -v wine >/dev/null && wine=wine64
"$wine" wineboot -i >/dev/null 2>&1 || true
"$wine" "$here/build/x64/OpenAgents.exe" --smoke
# Verify Windows .cmd launcher resolution & spawning for agy acp (#13839):
agy_cmd="$WINEPREFIX/drive_c/fake-agy.cmd"
cat > "$agy_cmd" <<'CMDEOF'
@echo off
if /I not "%~1"=="acp" exit /b 91
set /p line1=
echo {"jsonrpc":"2.0","id":1,"result":{"agentInfo":{"name":"agy","version":"1.5.13"}}}
set /p line2=
echo {"jsonrpc":"2.0","id":2,"result":{"sessionId":"wine-smoke-1","models":{"currentModelId":"gemini-4-argon-medium","availableModels":[{"modelId":"gemini-4-argon-medium","name":"Gemini 4 Argon (Medium)"}]},"modes":{"currentModeId":"default","availableModes":[{"id":"default"}]}}}
CMDEOF
probe_out="$(CODER_AGY_BIN='C:\fake-agy.cmd' "$wine" "$here/build/x64/agyworker.exe" --probe)"
printf 'agyworker .cmd wine smoke: %s\n' "$probe_out"
printf '%s\n' "$probe_out" | grep -q "gemini-4-argon-medium"

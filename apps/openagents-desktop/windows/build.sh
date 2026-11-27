#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, zig cc, iconutil and zip; holds no logic)
# Builds OpenAgents Desktop for Windows (GitHub #13782) from a Mac or Linux computer with zig:
# page.coax + desktop core (coax/lib/agent/desktop/, host-core.coax) + the shared Vulkan host
# (apps/shared/host/windows/), and mcworker.exe, for x86_64 and aarch64 Windows.
#   apps/openagents-desktop/windows/build.sh [--arch x64|arm64|all] [--check] [--zip DIR]
# OpenAgents Desktop builds a single unified profile (#14116); --minimal and --full are accepted as no-op aliases.
# --check builds both and keeps nothing (coax/remote/check.sh windows-build; any compile or link
# error fails it). --zip writes OpenAgents-windows-ARCH.zip into DIR: OpenAgents.exe,
# workers, resources/ and NOTICE. The program is not signed (see README.md).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
app_dir="$(cd "$here/.." && pwd)"
repo="$(cd "$here/../../.." && pwd)"
args=("$@"); arches=(x64 arm64); check=0; zip_dir=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --minimal|--full) ;;  # unified build profile (#14116); kept as compatibility aliases
    --arch) shift; [[ "${1:-}" == all ]] && arches=(x64 arm64) || arches=("${1:-}") ;;
    --check) check=1 ;;
    --zip) shift; zip_dir="${1:-}" ;;
    *) echo "build.sh: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done
# zig and the Vulkan headers: from Homebrew on a Mac, from nix on a build host
if { ! command -v zig >/dev/null || ! ls /opt/homebrew/include/vulkan/vulkan.h /usr/local/include/vulkan/vulkan.h /usr/include/vulkan/vulkan.h >/dev/null 2>&1; } \
   && [[ -z "${OA_DESKTOP_WINDOWS_BUILD:-}" ]] && command -v nix-shell >/dev/null 2>&1; then
  export OA_DESKTOP_WINDOWS_BUILD=1
  cmd="$(printf '%q ' "$here/build.sh" ${args[@]+"${args[@]}"})"
  exec nix-shell -p zig vulkan-headers zip --run "exec $cmd"
fi
command -v zig >/dev/null || { echo "build.sh: zig is needed (brew install zig)" >&2; exit 1; }
vk_include="${VULKAN_INCLUDE:-}"
if [[ -z "$vk_include" ]]; then
  for d in /opt/homebrew/include /usr/local/include /usr/include $(printf '%s\n' ${NIX_CFLAGS_COMPILE:-} | grep '^/'); do
    [[ -f "$d/vulkan/vulkan.h" ]] && { vk_include="$d"; break; }
  done
fi
[[ -n "$vk_include" ]] || { echo "build.sh: the Vulkan headers are needed (brew install vulkan-headers)" >&2; exit 1; }

coax="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax" ]]; then "$repo/coax/build.sh"; fi
version="$(sed -n 's/^#define OA_APP_VERSION "\(.*\)"$/\1/p' "$app_dir/host-main.c")"
build_no="$(sed -n 's/^#define OA_APP_BUILD \([0-9]*\)$/\1/p' "$app_dir/host-main.c")"

if [[ "$check" == 1 ]]; then work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT; else work="$here/build"; fi
mkdir -p "$work/c" "$work/res"

# 1. The page, the desktop core, the entropy policy, wallet units, workers and daemon, in C (the same C for both arches).
app_src="$app_dir/app.coax"
"$coax" c "$app_src" --library --header "$work/c/gallery.h" > "$work/c/gallery.c"
"$coax" c "$app_dir/host-core.coax" --library --header "$work/c/core.h" > "$work/c/core.c"
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/c/entropy-policy.h" > "$work/c/entropy.c"
"$coax" c "$repo/pay/wallet.coax" --library --header "$work/c/pay-wallet.h" > "$work/c/pay-wallet.c"
"$coax" c "$repo/pay/wallet-backup.coax" --library --header "$work/c/pay-wallet-backup.h" > "$work/c/pay-wallet-backup.c"
"$coax" c "$repo/pay/wallet-rpc.coax" --library --header "$work/c/pay-wallet-rpc.h" > "$work/c/pay-wallet-rpc.c"
wallet_srcs=("$work/c/pay-wallet.c" "$work/c/pay-wallet-backup.c" "$work/c/pay-wallet-rpc.c")
"$coax" c "$repo/openagents/workers/legacy/inky.coax" --library > "$work/c/inky.c"
"$coax" c "$repo/openagents/workers/legacy/mcworker.coax" --library > "$work/c/mcworker.c"
"$coax" c "$repo/openagents/workers/claudeworker.coax" --library > "$work/c/claudeworker.c"
"$coax" c "$repo/openagents/workers/agyworker.coax" --library > "$work/c/agyworker.c"
"$coax" c "$repo/openagents/workers/gitguard.coax" --library > "$work/c/gitguard.c"
"$coax" c "$repo/openagents/daemon/host.coax" --run-steps 250000000 --library > "$work/c/coder-host.c"
"$coax" spirv "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs \
  -o "$work/res/components.spv" 2> "$work/res/components.spv.bindings"
"$coax" spirv "$repo/components/backend/composition-shader.coax" --vertex uc_vs --fragment uc_fs \
  -o "$work/res/composition.spv" 2> "$work/res/composition.spv.bindings"
"$coax" spirv "$repo/metaverse-engine/gpu/model-view-shader.coax" --vertex mv_vs --fragment mv_fs \
  -o "$work/res/metaverse-model.spv" 2> "$work/res/metaverse-model.spv.bindings"
"$coax" spirv "$repo/metaverse-engine/gpu/ple-shader.coax" --vertex ple_vs --fragment ple_fs \
  -o "$work/res/metaverse-ple.spv" 2> "$work/res/metaverse-ple.spv.bindings"
"$coax" spirv "$repo/gamebuilder/core/gpu/gb-instances.coax" --vertex gb_vs --fragment gb_fs \
  -o "$work/res/gb-instances.spv" 2> "$work/res/gb-instances.spv.bindings"

# 2. The icon (from the Mac app's), the manifest and the version, as Windows resources.
iconset="$work/res/OpenAgents.iconset"
if [[ ! -f "$work/res/OpenAgents.ico" ]]; then
  rm -rf "$iconset"
  if command -v iconutil >/dev/null; then
    iconutil -c iconset "$app_dir/resources/OpenAgents.icns" -o "$iconset"
  fi
  sizes=(); for s in 16 32 48 128 256; do [[ -f "$iconset/icon_${s}x${s}.png" ]] && sizes+=("$s"); done
  if [[ ${#sizes[@]} -eq 0 && -n "$zip_dir" ]]; then echo "build.sh: a zip needs the icon, which needs iconutil (macOS)" >&2; exit 1; fi
  # an .ico of PNG images: a 6-byte header, a 16-byte entry each, then the PNGs
  [[ ${#sizes[@]} -gt 0 ]] && {
  le16() { printf "\\$(printf %03o $(($1 & 255)))\\$(printf %03o $((($1 >> 8) & 255)))"; }
  le32() { le16 $(($1 & 65535)); le16 $((($1 >> 16) & 65535)); }
  {
    le16 0; le16 1; le16 ${#sizes[@]}
    offset=$((6 + 16 * ${#sizes[@]}))
    for s in "${sizes[@]}"; do
      bytes=$(wc -c < "$iconset/icon_${s}x${s}.png" | tr -d ' ')
      side=$((s >= 256 ? 0 : s))
      printf "\\$(printf %03o $side)\\$(printf %03o $side)\\000\\000"; le16 1; le16 32; le32 "$bytes"; le32 "$offset"
      offset=$((offset + bytes))
    done
    for s in "${sizes[@]}"; do cat "$iconset/icon_${s}x${s}.png"; done
  } > "$work/res/OpenAgents.ico"
  }
fi
icon_line=""; [[ -f "$work/res/OpenAgents.ico" ]] && icon_line='1 ICON "OpenAgents.ico"'   # a build without iconutil (Linux) links no icon
IFS=. read -r v1 v2 v3 <<< "$version"
cat > "$work/res/app.rc" <<EOF
#include <windows.h>
$icon_line
1 24 "app.manifest"
1 VERSIONINFO
FILEVERSION $v1,$v2,$v3,$build_no
PRODUCTVERSION $v1,$v2,$v3,$build_no
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "OpenAgents, Inc."
      VALUE "FileDescription", "OpenAgents"
      VALUE "FileVersion", "$version"
      VALUE "InternalName", "OpenAgents"
      VALUE "OriginalFilename", "OpenAgents.exe"
      VALUE "ProductName", "OpenAgents"
      VALUE "ProductVersion", "$version"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
EOF
cp "$here/app.manifest" "$work/res/app.manifest"

# 3. Each arch: the app and the worker.
p="$repo/coax/platform"
h="$repo/apps/shared/host/windows"
inkling_dir="${OA_INKLING_BUILD_DIR:-$repo/psionic/build}"
common=(-std=gnu11 -O2 -ffp-contract=off -fno-fast-math -fno-builtin-pow -w -D_WIN32_WINNT=0x0A00 -DWINVER=0x0A00 -DUNICODE -D_UNICODE)
for arch in "${arches[@]}"; do
  case "$arch" in
    x64) target=x86_64-windows-gnu ;;
    arm64) target=aarch64-windows-gnu ;;
    *) echo "build.sh: --arch is x64, arm64 or all" >&2; exit 2 ;;
  esac
  out="$work/$arch"; rm -rf "$out"; mkdir -p "$out/resources"
  core_renames=(-Dcb_span=oa_hc_cb_span -Dcb_load_be32=oa_hc_cb_load_be32 -Dcb_load_le32=oa_hc_cb_load_le32 -Dcb_load_be64=oa_hc_cb_load_be64 -Dcb_load_le64=oa_hc_cb_load_le64 -Dcb_store_be32=oa_hc_cb_store_be32 -Dcb_store_le32=oa_hc_cb_store_le32 -Dcb_store_be64=oa_hc_cb_store_be64 -Dcb_store_le64=oa_hc_cb_store_le64 -Dcb_mask=oa_hc_cb_mask -Dcb_ct_select64=oa_hc_cb_ct_select64 -Dcb_ct_select32=oa_hc_cb_ct_select32 -Dcb_ct_equal=oa_hc_cb_ct_equal -Dsha256_state_size=oa_hc_sha256_state_size -Dsha256_update_limit=oa_hc_sha256_update_limit -Dsha256_init=oa_hc_sha256_init -Dsha256_info=oa_hc_sha256_info -DS_hash=oa_hc_S_hash -DS_hex=oa_hc_S_hex)
  zig cc -target "$target" "${common[@]}" "${core_renames[@]}" -I"$work/c" -I"$p" -c "$work/c/core.c" -o "$work/c/core-$arch.o"
  zig cc -target "$target" "${common[@]}" -municode -Wl,--subsystem,windows \
    -include "$h/vk-load.h" -I"$work/c" -I"$app_dir" -I"$h" -I"$repo/metaverse/gpu" -I"$p" -I"$vk_include" \
    "$here/main.c" "$app_dir/carrier.c" "$h/ui-host.c" "$h/portal.c" "$h/vk-load.c" \
    "$work/c/gallery.c" "$work/c/core-$arch.o" "$work/c/entropy.c" \
    ${wallet_srcs[@]+"${wallet_srcs[@]}"} \
    "$repo/metaverse/gpu/vkc.c" "$repo/metaverse/gpu/vks.c" "$repo/metaverse/gpu/vkp.c" \
    "$p/process-win32.c" "$p/store-file-win32.c" "$p/socket-win32.c" "$p/entropy-native.c" "$p/entropy-foreign.c" \
    "$work/res/app.rc" \
    -Wl,--stack,67108864 -lole32 -luuid -lshell32 -limm32 -lbcrypt -lws2_32 -luser32 -lgdi32 -ladvapi32 \
    -o "$out/OpenAgents.exe"
  zig cc -target "$target" "${common[@]}" -include "$p/win32-stdio.h" -I"$work/c" -I"$p" \
    "$work/c/inky.c" "$p/tool-win32.c" "$p/process-win32.c" "$p/store-file-win32.c" "$p/socket-win32.c" \
    "$p/entropy-native.c" "$p/entropy-foreign.c" "$p/agent-signer-none.c" "$work/c/entropy.c" \
    -lbcrypt -lws2_32 -lshell32 -ladvapi32 -o "$out/inky.exe"
  zig cc -target "$target" "${common[@]}" -include "$p/win32-stdio.h" -I"$work/c" -I"$p" \
    "$work/c/mcworker.c" "$p/tool-win32.c" "$p/process-win32.c" "$p/store-file-win32.c" "$p/socket-win32.c" \
    "$p/entropy-native.c" "$p/entropy-foreign.c" "$p/agent-signer-none.c" "$work/c/entropy.c" \
    -lbcrypt -lws2_32 -lshell32 -ladvapi32 -o "$out/mcworker.exe"
  zig cc -target "$target" "${common[@]}" -include "$p/win32-stdio.h" -I"$work/c" -I"$p" \
    "$work/c/claudeworker.c" "$p/tool-win32.c" "$p/process-win32.c" "$p/store-file-win32.c" "$p/socket-win32.c" \
    "$p/entropy-native.c" "$p/entropy-foreign.c" "$p/agent-signer-none.c" "$work/c/entropy.c" \
    -lbcrypt -lws2_32 -lshell32 -ladvapi32 -o "$out/claudeworker.exe"
  zig cc -target "$target" "${common[@]}" -include "$p/win32-stdio.h" -I"$work/c" -I"$p" \
    "$work/c/agyworker.c" "$p/tool-win32.c" "$p/process-win32.c" "$p/store-file-win32.c" "$p/socket-win32.c" \
    "$p/entropy-native.c" "$p/entropy-foreign.c" "$p/agent-signer-none.c" "$work/c/entropy.c" \
    -lbcrypt -lws2_32 -lshell32 -ladvapi32 -o "$out/agyworker.exe"
  zig cc -target "$target" "${common[@]}" -include "$p/win32-stdio.h" -I"$work/c" -I"$p" \
    "$work/c/gitguard.c" "$p/tool-win32.c" "$p/process-win32.c" "$p/store-file-win32.c" "$p/socket-win32.c" \
    "$p/entropy-native.c" "$p/entropy-foreign.c" "$p/agent-signer-none.c" "$work/c/entropy.c" \
    -lbcrypt -lws2_32 -lshell32 -ladvapi32 -o "$out/gitguard.exe"
  zig cc -target "$target" "${common[@]}" -include "$p/win32-stdio.h" -I"$work/c" -I"$p" \
    "$work/c/coder-host.c" "$p/tool-win32.c" "$p/process-win32.c" "$p/store-file-win32.c" "$p/socket-win32.c" \
    "$p/tls-win32.c" "$p/entropy-native.c" "$p/entropy-foreign.c" "$p/agent-signer-none.c" "$work/c/entropy.c" \
    -lbcrypt -lws2_32 -lshell32 -ladvapi32 -o "$out/coder-host.exe"
  if [[ -f "$inkling_dir/$arch/psionic-inkling-serve.exe" ]]; then
    cp "$inkling_dir/$arch/psionic-inkling-serve.exe" "$out/psionic-inkling-serve.exe"
  elif [[ -f "$inkling_dir/psionic-inkling-serve.exe" ]]; then
    cp "$inkling_dir/psionic-inkling-serve.exe" "$out/psionic-inkling-serve.exe"
  fi
  for kfile in inkling-kernels-cuda.cu inkling-kernels-vulkan.spv; do
    if [[ -f "$inkling_dir/$arch/$kfile" ]]; then
      cp "$inkling_dir/$arch/$kfile" "$out/$kfile"
    elif [[ -f "$inkling_dir/$kfile" ]]; then
      cp "$inkling_dir/$kfile" "$out/$kfile"
    fi
  done
  cp "$work/res/components.spv" "$work/res/composition.spv" "$out/resources/"
  for spv in metaverse-model.spv metaverse-ple.spv gb-instances.spv; do
    [[ -f "$work/res/$spv" ]] && cp "$work/res/$spv" "$out/resources/"
  done
  mkdir -p "$out/resources/fonts" "$out/resources/icons"
  cp "$repo/components/fonts/"Geist* "$out/resources/fonts/"
  cp "$repo/components/icons/"*.svg "$out/resources/icons/"
  cp "$repo/components/NOTICE" "$out/NOTICE"
  rm -f "$out"/*.pdb "$out"/*.lib
  echo "built $out/OpenAgents.exe ($(du -sh "$out" | cut -f 1))"
  zip_path=""
  if [[ -n "$zip_dir" ]]; then
    mkdir -p "$zip_dir"
    zip_path="$(cd "$zip_dir" && pwd)/OpenAgents-windows-$arch.zip"
    rm -f "$zip_path"
    stage="$work/stage-$arch/OpenAgents"; rm -rf "$work/stage-$arch"; mkdir -p "$stage"
    cp -R "$out/." "$stage/"
    (cd "$work/stage-$arch" && zip -qr -X "$zip_path" OpenAgents)
    echo "zipped $zip_path ($(du -h "$zip_path" | cut -f 1))"
  fi
done
[[ "$check" == 0 ]] || echo "windows-build: ${arches[*]} compiled and linked"

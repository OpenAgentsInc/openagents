#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, cc, wayland-scanner and pkg-config; holds no logic)
# Builds OpenAgents Desktop for Linux (GitHub #13781): page.coax + desktop core
# (coax/lib/agent/desktop/) + shared Wayland/Vulkan host (apps/shared/host/linux/).
#   apps/openagents-desktop/linux/build.sh [--full|--minimal] [--stage-dir DIR] [--verify] [--smoke] [--flow-test] [--input-test] [--open] [--no-worker]
# --minimal (or OA_RELEASE_MINIMAL=1): build the v1.0 minimal release slice (omits wallet units,
#   claudeworker, and agyworker; bundles oa-worker, gitguard, coder-host, and optional psionic-inkling-serve; stages a clean
#   release directory and verifies it with v1-bundle-check.coax). Default is --full.
# --stage-dir DIR: stage the clean minimal release layout into DIR (default: build/minimal-stage in --minimal).
# --verify: run apps/openagents-desktop/v1-bundle-check.coax against the staged minimal layout.
# --flow-test: the chat flow test (flow-test.sh) on the mock worker, headless.
# --input-test: the real window typed into and pressed in a headless compositor (input-test.sh).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
app_dir="$(cd "$here/.." && pwd)"
repo="$(cd "$here/../../.." && pwd)"

if [[ -z "${OA_DESKTOP_LINUX_BUILD:-}" ]] && command -v nix-shell >/dev/null 2>&1; then
  export OA_DESKTOP_LINUX_BUILD=1
  cmd="$(printf '%q ' "$here/build.sh" "$@")"
  exec nix-shell -p gcc pkg-config openssl vulkan-headers vulkan-loader wayland wayland-scanner wayland-protocols wlr-protocols libxkbcommon dbus systemd mesa \
    --run "exec $cmd"
fi

minimal=0
if [[ "${OA_RELEASE_MINIMAL:-0}" == "1" ]]; then minimal=1; fi
want_worker=1
stage_dir=""
want_verify=0
want_input_test=0
actions=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --minimal) minimal=1 ;;
    --full) minimal=0 ;;
    --no-worker) want_worker=0 ;;
    --stage-dir)
      shift
      [[ $# -gt 0 && -n "${1:-}" ]] || { echo "build.sh: --stage-dir requires a directory" >&2; exit 2; }
      stage_dir="$1"
      ;;
    --verify) want_verify=1 ;;
    --smoke|--flow-test|--open) actions+=("$1") ;;
    --input-test) want_input_test=1; actions+=("$1") ;;
    *) echo "build.sh: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

coax="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax" ]]; then "$repo/coax/build.sh"; fi

work="$here/build"
mkdir -p "$work/c" "$work/wl" "$work/resources"

# 1. Compile page (app-v1.coax in --minimal when available, else app.coax), desktop core bridge
#    (host-core.coax), entropy policy, and Wayland input/text.
app_src="$app_dir/app.coax"
if [[ "$minimal" == 1 && -f "$app_dir/app-v1.coax" ]]; then
  app_src="$app_dir/app-v1.coax"
fi
"$coax" c "$app_src" --library --header "$work/c/gallery.h" > "$work/c/gallery.c"
"$coax" c "$app_dir/host-core.coax" --library --header "$work/c/core.h" > "$work/c/core.c"
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/c/entropy-policy.h" > "$work/c/entropy.c"
wallet_srcs=()
profile_defs=(-DOA_FULL)
if [[ "$minimal" == 0 ]]; then
  "$coax" c "$repo/pay/wallet.coax" --library --header "$work/c/pay-wallet.h" > "$work/c/pay-wallet.c"
  "$coax" c "$repo/pay/wallet-backup.coax" --library --header "$work/c/pay-wallet-backup.h" > "$work/c/pay-wallet-backup.c"
  "$coax" c "$repo/pay/wallet-rpc.coax" --library --header "$work/c/pay-wallet-rpc.h" > "$work/c/pay-wallet-rpc.c"
  wallet_srcs=("$work/c/pay-wallet.c" "$work/c/pay-wallet-backup.c" "$work/c/pay-wallet-rpc.c")
else
  profile_defs=(-DOA_MINIMAL)
fi
"$coax" c "$repo/metaverse-engine/host/wayland-input.coax" --library --header "$work/c/wayland-input.h" > "$work/c/wayland-input.c"
"$coax" c "$repo/metaverse-engine/host/wayland-text.coax" --library --header "$work/c/wayland-text.h" > "$work/c/wayland-text.c"

# 2. Emit the shared component, composition-v2, and Metaverse/GameBuilder SPIR-V shaders.
"$coax" spirv "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$work/resources/components.spv" 2> "$work/resources/components.spv.bindings"
"$coax" spirv "$repo/components/backend/composition-shader.coax" --vertex uc_vs --fragment uc_fs -o "$work/resources/composition.spv" 2> "$work/resources/composition.spv.bindings"
if [[ "$minimal" == 0 ]]; then
  "$coax" spirv "$repo/metaverse-engine/gpu/model-view-shader.coax" --vertex mv_vs --fragment mv_fs -o "$work/resources/metaverse-model.spv" 2> "$work/resources/metaverse-model.spv.bindings"
  "$coax" spirv "$repo/metaverse-engine/gpu/ple-shader.coax" --vertex ple_vs --fragment ple_fs -o "$work/resources/metaverse-ple.spv" 2> "$work/resources/metaverse-ple.spv.bindings"
  "$coax" spirv "$repo/gamebuilder/core/gpu/gb-instances.coax" --vertex gb_vs --fragment gb_fs -o "$work/resources/gb-instances.spv" 2> "$work/resources/gb-instances.spv.bindings"
fi

# 3. Generate Wayland protocol C bindings.
protocols="$(pkg-config --variable=pkgdatadir wayland-protocols)"
for spec in stable/xdg-shell/xdg-shell unstable/xdg-decoration/xdg-decoration-unstable-v1 unstable/text-input/text-input-unstable-v3; do
  name="$(basename "$spec")"
  wayland-scanner client-header "$protocols/$spec.xml" "$work/wl/$name-client-protocol.h"
  wayland-scanner private-code "$protocols/$spec.xml" "$work/wl/$name-protocol.c"
done

# 4. Compile C units and link OpenAgents.
p="$repo/coax/platform"
cflags=(-std=gnu11 -O2 -ffp-contract=off -fno-fast-math -fno-builtin-pow -w \
  -D_DEFAULT_SOURCE -DENGINE_WAYLAND_INPUT_V1 -DENGINE_WAYLAND_TEXT_V1 \
  "${profile_defs[@]}" \
  -I"$work/c" -I"$work/wl" -I"$app_dir" -I"$repo/apps/shared/host/linux" \
  -I"$repo/metaverse/gpu" -I"$repo/metaverse/kms" -I"$p")

# shellcheck disable=SC2046
cc "${cflags[@]}" \
  "$here/main.c" \
  "$app_dir/carrier.c" \
  "$repo/apps/shared/host/linux/ui-host.c" \
  "$repo/apps/shared/host/linux/portal.c" \
  \
  "$work/c/gallery.c" "$work/c/core.c" "$work/c/entropy.c" \
  ${wallet_srcs[@]+"${wallet_srcs[@]}"} \
  "$work/c/wayland-input.c" "$work/c/wayland-text.c" \
  "$repo/metaverse/gpu/vkc.c" "$repo/metaverse/gpu/vks.c" "$repo/metaverse/gpu/vkp.c" \
  "$work/wl/xdg-shell-protocol.c" "$work/wl/xdg-decoration-unstable-v1-protocol.c" "$work/wl/text-input-unstable-v3-protocol.c" \
  "$p/process-native.c" "$p/store-file-native.c" "$p/read-file-native.c" \
  "$p/socket-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" \
  $(pkg-config --cflags --libs wayland-client wayland-cursor xkbcommon libsystemd) \
  -Wl,--allow-multiple-definition -lvulkan -lpthread -lm -o "$work/OpenAgents.new"

rm -rf "$work/resources/fonts" "$work/resources/icons"
mkdir -p "$work/resources/fonts" "$work/resources/icons"
cp "$repo/components/fonts/"Geist* "$work/resources/fonts/"
cp "$repo/components/icons/"*.svg "$work/resources/icons/"
cp "$repo/components/NOTICE" "$work/resources/NOTICE"
mv "$work/OpenAgents.new" "$work/OpenAgents"
if [[ "$minimal" == 1 ]]; then
  echo "built $work/OpenAgents (minimal)"
else
  echo "built $work/OpenAgents"
fi

# The input test's virtual pointer (vpointer.c), when the wlroots protocols are here.
if [[ "$minimal" == 0 || "$want_input_test" == 1 ]]; then
  wlr="$(pkg-config --variable=pkgdatadir wlr-protocols 2>/dev/null || true)"
  if [[ -n "$wlr" && -f "$wlr/unstable/wlr-virtual-pointer-unstable-v1.xml" ]]; then
    wayland-scanner client-header "$wlr/unstable/wlr-virtual-pointer-unstable-v1.xml" "$work/wl/wlr-virtual-pointer-unstable-v1-client-protocol.h"
    wayland-scanner private-code "$wlr/unstable/wlr-virtual-pointer-unstable-v1.xml" "$work/wl/wlr-virtual-pointer-unstable-v1-protocol.c"
    # shellcheck disable=SC2046
    cc -std=gnu11 -O2 -w -I"$work/wl" "$here/vpointer.c" "$work/wl/wlr-virtual-pointer-unstable-v1-protocol.c" \
      $(pkg-config --cflags --libs wayland-client) -o "$work/vpointer"
  fi
fi

# 5. Build workers, gitguard and resident daemon beside OpenAgents unless --no-worker is passed.
if [[ "$want_worker" == 1 ]]; then
  coax_hash="$(sha256sum "$coax" | cut -d " " -f 1)"
  if [[ "$minimal" == 1 ]]; then
    worker_pairs=("oa-worker:oa-worker" "gitguard:gitguard" "host:coder-host")
    rm -f "$work/inky" "$work/mcworker" "$work/claudeworker" "$work/flawedworker" "$work/agyworker"
  else
    worker_pairs=("oa-worker:oa-worker" "claudeworker:claudeworker" "agyworker:agyworker" "gitguard:gitguard" "host:coder-host")
    rm -f "$work/inky" "$work/mcworker"
  fi
  for pair in "${worker_pairs[@]}"; do
    src_name="${pair%%:*}"; dst_name="${pair##*:}"
    if [[ -f "$repo/coder/tui/$src_name.coax" ]]; then
      COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/$src_name.coax" build-only < /dev/null > "$work/.$dst_name.log" 2>&1 || true
      built="$repo/coax/web/build/tools/$src_name-$coax_hash"
      # beside, then renamed over: a running daemon (coder-host serve outlives the app) keeps its old file
      if [[ -x "$built" ]]; then
        cp "$built" "$work/.$dst_name.new" && mv -f "$work/.$dst_name.new" "$work/$dst_name"
        rm -f "$work/.$dst_name.log"
      elif [[ "$minimal" == 1 ]]; then
        echo "build.sh: failed to build required minimal helper $dst_name ($src_name.coax)" >&2
        tail -n 40 "$work/.$dst_name.log" >&2 || true
        exit 1
      fi
    fi
  done
fi

# 6. Bundle native Psionic Inkling runtime and kernels when already built under psionic/build/.
inkling_dir="${OA_INKLING_BUILD_DIR:-$repo/psionic/build}"
if [[ -x "$inkling_dir/psionic-inkling-serve" ]]; then
  cp "$inkling_dir/psionic-inkling-serve" "$work/.psionic-inkling-serve.new"
  chmod 0755 "$work/.psionic-inkling-serve.new"
  mv -f "$work/.psionic-inkling-serve.new" "$work/psionic-inkling-serve"
fi
for kfile in inkling-kernels-cuda.cu inkling-kernels-vulkan.spv; do
  if [[ -f "$inkling_dir/$kfile" ]]; then
    cp "$inkling_dir/$kfile" "$work/$kfile"
  fi
done

# 7. Stage a clean minimal release directory and verify with v1-bundle-check.coax.
out_stage=""
if [[ "$minimal" == 1 || -n "$stage_dir" ]]; then
  out_stage="${stage_dir:-$work/minimal-stage}"
  rm -rf "$out_stage"
  mkdir -p "$out_stage/resources/fonts" "$out_stage/resources/icons"
  install -m 0755 "$work/OpenAgents" "$out_stage/OpenAgents"
  stage_bins=(oa-worker coder-host gitguard psionic-inkling-serve)
  if [[ "$minimal" == 0 ]]; then stage_bins+=(agyworker); fi
  for bin in "${stage_bins[@]}"; do
    if [[ -x "$work/$bin" ]]; then
      install -m 0755 "$work/$bin" "$out_stage/$bin"
    fi
  done
  if [[ "$minimal" == 1 ]] && command -v strip >/dev/null 2>&1; then
    for bin in OpenAgents oa-worker coder-host gitguard psionic-inkling-serve; do
      if [[ -x "$out_stage/$bin" ]]; then
        strip --strip-unneeded "$out_stage/$bin" 2>/dev/null || strip "$out_stage/$bin" 2>/dev/null || true
      fi
    done
  fi
  for kfile in inkling-kernels-cuda.cu inkling-kernels-vulkan.spv; do
    if [[ -f "$work/$kfile" ]]; then
      install -m 0644 "$work/$kfile" "$out_stage/$kfile"
    fi
  done
  cp "$work/resources/components.spv" "$work/resources/composition.spv" "$out_stage/resources/"
  if [[ "$minimal" == 0 ]]; then
    for spv in metaverse-model.spv metaverse-ple.spv gb-instances.spv; do
      [[ -f "$work/resources/$spv" ]] && cp "$work/resources/$spv" "$out_stage/resources/"
    done
  fi
  cp "$work/resources/fonts/"Geist* "$out_stage/resources/fonts/"
  cp "$work/resources/icons/"*.svg "$out_stage/resources/icons/"
  cp "$work/resources/NOTICE" "$out_stage/resources/NOTICE"
  cp "$here/openagents-desktop.desktop" "$out_stage/openagents-desktop.desktop"
  cp "$repo/components/NOTICE" "$out_stage/NOTICE"
  echo "staged minimal bundle at $out_stage ($(du -sh "$out_stage" | cut -f 1))"

  if [[ ("$minimal" == 1 && "$want_worker" == 1) || "$want_verify" == 1 ]]; then
    if [[ "$minimal" == 1 && "$want_worker" == 1 && ! -x "$out_stage/gitguard" ]]; then
      echo "FAIL [missing] required v1.0 artifact absent: gitguard" >&2
      exit 1
    fi
    v1_stage="${OA_V1_STAGE:-}"
    if [[ -z "$v1_stage" ]]; then
      if [[ -x "$out_stage/psionic-inkling-serve" ]]; then v1_stage=2; else v1_stage=1; fi
    fi
    vbc_args=(--platform linux --stage "$v1_stage")
    if [[ "$(uname -s)" != Linux ]]; then
      vbc_args+=(--skip-ld)
    fi
    vbc_rc=0
    vbc_out="$(COAX="$coax" "$repo/coax/web/tool.sh" "$app_dir/v1-bundle-check.coax" "${vbc_args[@]}" "$out_stage" 2>&1)" || vbc_rc=$?
    printf '%s\n' "$vbc_out"
    if [[ "$vbc_rc" != 0 ]]; then
      if [[ "$vbc_rc" == 1 && "$v1_stage" == 1 && ! -x "$out_stage/psionic-inkling-serve" ]]; then
        other_fails="$(printf '%s\n' "$vbc_out" | grep '^FAIL \[' | grep -vxF 'FAIL [missing] required v1.0 artifact absent: psionic-inkling-serve' || true)"
        missing_inkling="$(printf '%s\n' "$vbc_out" | grep -cxF 'FAIL [missing] required v1.0 artifact absent: psionic-inkling-serve' || true)"
        if [[ -z "$other_fails" && "$missing_inkling" == 1 ]]; then
          echo "PASS v1-bundle-check (linux stage 1: size/allowlist/workers/ldd verified; psionic-inkling-serve pending Phase B1/B2)"
        else
          exit "$vbc_rc"
        fi
      else
        exit "$vbc_rc"
      fi
    fi
  fi
fi

# Headless runs draw on Mesa's software Vulkan (lavapipe) when no driver is set.
software_vulkan() {
  if [[ -z "${VK_DRIVER_FILES:-}" && -z "${VK_ICD_FILENAMES:-}" ]]; then
    for d in $(echo "${NIX_PROFILES:-} ${XDG_DATA_DIRS:-} /usr/share /run/opengl-driver/share" | tr ':' ' '); do
      for icd in "$d"/vulkan/icd.d/lvp_icd*.json "$d"/share/vulkan/icd.d/lvp_icd*.json; do
        if [[ -f "$icd" ]]; then export VK_DRIVER_FILES="$icd"; return; fi
      done
    done
    if [[ -d /nix/store ]]; then
      icd="$(ls -1 /nix/store/*mesa*/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -n 1 || true)"
      if [[ -n "$icd" ]]; then export VK_DRIVER_FILES="$icd"; fi
    fi
  fi
}

run_root="$work"
if [[ "$minimal" == 1 && -n "$out_stage" && -x "$out_stage/OpenAgents" ]]; then
  run_root="$out_stage"
fi

for arg in ${actions[@]+"${actions[@]}"}; do
  if [[ "$arg" == --smoke ]]; then
    software_vulkan
    "$run_root/OpenAgents" --smoke --resources "$run_root/resources"
  elif [[ "$arg" == --flow-test ]]; then
    software_vulkan
    "$here/flow-test.sh" --no-build
  elif [[ "$arg" == --input-test ]]; then
    "$here/input-test.sh" --no-build
  elif [[ "$arg" == --open ]]; then
    exec "$run_root/OpenAgents" --resources "$run_root/resources"
  fi
done

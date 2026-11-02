#!/usr/bin/env bash
# Coax → C11 → wasm32 for the browser: one Coax file plus its C face, and
# any Coax libraries the face calls, each its own C library and header
# (Haunt.h for haunt.coax), as the iOS app links the engine's.
#   coax/web/wasm.sh APP.coax FACE.c OUT.wasm [LIBRARY.coax...]
# Needs upstream LLVM (clang + wasm-ld) and wasi-libc: toolchain.sh finds
# them (Homebrew on a Mac, nixpkgs on Linux). Override with COAXIAL_CLANG,
# COAXIAL_SYSROOT, COAXIAL_RESOURCES.
set -euo pipefail
coax_file="$1"; face="$2"; out="$3"; shift 3
libraries=("$@")
repo="$(cd "$(dirname "$0")/../.." && pwd)"
# Optional absolute path to an explicitly qualified compiler.
coax_driver="${COAX:-$repo/coax/bin/coax}"
# Coax sources by their path in the repository (apps/openagents-web/x.coax),
# never the checkout's absolute path: the C embeds source paths for its
# runtime messages, so a module built in any checkout or temporary release
# tree has the same bytes and the same content-named URL (2026-10-30: every
# release renamed every engine and players downloaded them again).
relative() {
  local p; p="$(cd "$(dirname "$1")" && pwd -P)/$(basename "$1")"
  local r; r="$(cd "$repo" && pwd -P)"
  [[ "$p" == "$r"/* ]] && echo "${p#"$r"/}" || echo "$p"
}
coax_file="$(relative "$coax_file")"
for i in ${libraries[@]+"${!libraries[@]}"}; do libraries[i]="$(relative "${libraries[i]}")"; done
face="$(cd "$(dirname "$face")" && pwd)/$(basename "$face")"
mkdir -p "$(dirname "$out")"; out="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
source "$repo/coax/web/toolchain.sh"
clang="$COAXIAL_CLANG"; sysroot="$COAXIAL_SYSROOT"; resources="$COAXIAL_RESOURCES"
name="$(basename "$coax_file" .coax)"
# Catalog applications include the shared renderer in ordinary builds. An
# explicit0 retains the producer-only artifact used by legacy diagnostics.
# Auto-selection never substitutes a different source with the same basename.
render="${COAXIAL_RENDER:-auto}"
carrier="aethyr/render/carriers/$name.coax"
if [[ "$render" == auto ]]; then
  render=0
  if [[ "$coax_file" == "apps/openagents-web/$name.coax" && -f "$repo/$carrier" ]]; then render=1; fi
fi
case "$render" in 0|1) ;; *) echo "COAXIAL_RENDER must be auto,0 or1" >&2; exit 2 ;; esac
if [[ "$render" == 1 && "$coax_file" != "apps/openagents-web/$name.coax" ]]; then
  echo "renderer source carrier requires the canonical catalog app path" >&2; exit 2
fi
# Concurrent pages often compile the same libraries (and dev rebuilds may
# target the same output). Give each invocation its own headers, C and objects.
work_root="$(dirname "$out")/.c"
mkdir -p "$work_root"
work="$(mktemp -d "$work_root/$name.XXXXXXXX")"
trap 'rm -rf "$work"' EXIT
# COAXIAL_RUN_STEPS: a larger compile-time budget (coax --run-steps) for libraries whose
# tables are computed at compile time (secp256k1 signing, for /login's words account).
run_steps=()
if [[ -n "${COAXIAL_RUN_STEPS:-}" ]]; then run_steps=(--run-steps "$COAXIAL_RUN_STEPS")
elif [[ "$render" == 1 ]]; then
  # The measured largest closed producer+renderer initializer is102,161
  # steps;128,000 retains25% headroom. Authored check budgets are separate.
  run_steps=(--run-steps 128000)
fi
# No fast math and no contraction: the browser computes what the checks computed.
# COAXIAL_MODE=dev (what coax serve uses) compiles at -O1 in parallel and
# links without the optimizer pass, for the fastest edit-to-screen loop;
# release (the default) builds at -O2.
mode="${COAXIAL_MODE:-release}"
opt=-O2; [[ "$mode" == dev ]] && opt=-O1
cflags=(--target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -std=c11 "$opt"
        -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable
        -Wno-unused-but-set-variable -I"$work")
# Every unit's C first (in parallel): the face includes their headers.
units=("$coax_file:$name"); gen=()
if [[ "$render" == 1 ]]; then
  # One physical load graph owns shared exported kernels. The facade is not
  # emitted as a second library with duplicate S_hash/S_hex C definitions.
  carrier="aethyr/render/carriers/$name.coax"
  if [[ ! -f "$repo/$carrier" ]]; then echo "no renderer source carrier for $name" >&2; exit 1; fi
  units=("$carrier:$name")
fi
for library in ${libraries[@]+"${libraries[@]}"}; do
  base="$(basename "$library" .coax)"; base="$(printf '%s' "${base:0:1}" | tr a-z A-Z)${base:1}"
  units+=("$library:$base")
done
for unit in "${units[@]}"; do
  (cd "$repo" && "$coax_driver" c "${unit%%:*}" --library --header "$work/${unit##*:}.h" ${run_steps[@]+"${run_steps[@]}"}) > "$work/${unit##*:}.c" & gen+=($!)
done
for pid in "${gen[@]}"; do wait "$pid"; done
if [[ "$render" == 1 ]]; then cp "$work/$name.h" "$work/Render.h"; fi
if [[ -n "${COAXIAL_ADMITTED_DIR:-}" ]]; then
  mkdir -p "$COAXIAL_ADMITTED_DIR"
  cp "$work/"*.c "$work/"*.h "$COAXIAL_ADMITTED_DIR/"
fi
# Struct defaults come from the compiler: each unit's header declares
# coax_default_T for its types with declared defaults, and the face calls
# them for a new state (coaxial_state_init) and its own globals.
"$clang" "${cflags[@]}" -c "$work/$name.c" -o "$work/$name.o" & coax_pid=$!
objects=("$work/$name.o" "$work/$name.face.o")
pids=()
for unit in "${units[@]:1}"; do
  base="${unit##*:}"
  "$clang" "${cflags[@]}" -c "$work/$base.c" -o "$work/$base.o" & pids+=($!)
  objects+=("$work/$base.o")
done
"$clang" "${cflags[@]}" -I"$(dirname "$face")" -c "$face" -o "$work/$name.face.o"
if [[ "$render" == 1 ]]; then
  "$clang" "${cflags[@]}" -c "$repo/coax/web/render-face.c" -o "$work/Render.face.o"
  objects+=("$work/Render.face.o")
fi
wait "$coax_pid"
for pid in ${pids[@]+"${pids[@]}"}; do wait "$pid"; done
link_opt=(); [[ "$mode" == dev ]] || link_opt=("$opt")
# The bounded package parser's 16,384 JSON nodes alone occupy 1 MiB.
# Its measured wasm32 admission frame is 1,057,936 bytes; renderer frames
# and shader validation also use bounded scratch. Keep their stack inside
# the same 64 MiB imported-memory limit, with room for the measured chain.
stack_bytes=1048576
[[ "$render" == 1 ]] && stack_bytes=4194304
"$clang" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" ${link_opt[@]+"${link_opt[@]}"} \
  -mexec-model=reactor -o "$work/output.wasm" "${objects[@]}" \
  -Wl,--import-memory,--max-memory=67108864,-z,stack-size="$stack_bytes",--export=__heap_base,--gc-sections,--strip-debug
mv "$work/output.wasm" "$out"   # the server never serves a half-written module

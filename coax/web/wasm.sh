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
# Catalog applications include the shared renderer in ordinary builds (host.js
# needs each world's renderer facade: with 0 every world page fails "engine has
# no compatible Coax renderer facade"). An explicit 0 retains the producer-only
# artifact used by legacy diagnostics. Auto-selection never substitutes a
# different source with the same basename.
render="${COAXIAL_RENDER:-auto}"
carrier="aethyr/render/carriers/$name.coax"
# A library the carrier itself #loads is part of the carrier's load graph, so
# with the carrier it is not a second unit: one graph owns every #export_c the
# two share (S_hash and S_hex, #12661), and the library's header is the
# carrier's. Since #12675 a library's locals don't clash with the page's
# procedures, so one graph can hold both (myrealm's carrier loads YOUR
# COMPUTER's wire).
folded=()
fold_libraries() {
  folded=()
  [[ ${#libraries[@]} -gt 0 && -f "$repo/$carrier" ]] || return 0
  local sources kept=() library
  sources="$(cd "$repo" && "$repo/coax/web/tool.sh" wasm-size sources "$carrier" 2> /dev/null)" || return 0
  for library in "${libraries[@]}"; do
    if grep -qxF "$library" <<< "$sources"; then folded+=("$library"); else kept+=("$library"); fi
  done
  libraries=(${kept[@]+"${kept[@]}"})
}
if [[ "$render" == auto ]]; then
  render=0
  if [[ "$coax_file" == "apps/openagents-web/$name.coax" && -f "$repo/$carrier" ]]; then render=1; fi
  # Each unit is its own C library, so a file with an #export_c procedure that
  # the carrier and a library both load is defined twice at the link (#12661:
  # the renderer and YOUR COMPUTER's wire both load sha256-kernel.coax, for
  # S_hash and S_hex). Libraries the carrier loads are folded into it; if one
  # it doesn't load still shares an export, the app builds without its
  # carrier, and says so; COAXIAL_RENDER=1 asks for the carrier and fails,
  # naming the units.
  if [[ "$render" == 1 ]]; then
    all_libraries=(${libraries[@]+"${libraries[@]}"}); fold_libraries
  fi
  if [[ "$render" == 1 && ${#libraries[@]} -gt 0 ]]; then
    if ! twice="$(cd "$repo" && "$repo/coax/web/tool.sh" wasm-size exports "$carrier" "${libraries[@]}" 2>&1 > /dev/null)"; then
      libraries=("${all_libraries[@]}"); folded=()
      render=0
      echo "wasm.sh: $(basename "$out"): built without its renderer carrier:" >&2
      grep '^wasm-size: ' <<< "$twice" | sed 's/^wasm-size: /  /' >&2 || true
    fi
  fi
fi
case "$render" in 0|1) ;; *) echo "COAXIAL_RENDER must be auto,0 or1" >&2; exit 2 ;; esac
[[ "${COAXIAL_RENDER:-auto}" != 1 ]] || fold_libraries
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
# links without the optimizer pass, for the fastest edit-to-screen loop.
# Release (the default) compiles every unit at -Os to LLVM bitcode (-flto) and
# links them with full link-time optimization, so code is inlined and dropped
# across units, then runs wasm-opt -O2 on the linked module. Measured
# 2026-11-03 on coderos-4080 (#12648, clang 21, Everglade with its renderer):
# -Os and wasm-opt -O2 without LTO made 1,919,015 bytes; with full LTO,
# 1,569,186 (18% smaller, 16% smaller gzipped), and its frame code takes no
# longer. ThinLTO made 1,864,911; -O2, -O3 and -Oz were larger; wasm-opt -Os
# or -Oz after LTO gave under 2% more for 25 s or more a module. (2026-11-02,
# on the Mac: -Os with wasm-opt -O2 was 16% smaller than -O2.) Linked without
# -O, so clang doesn't run a wasm-opt of its own choosing; wasm.sh runs
# COAXIAL_WASM_OPT, else the wasm-opt on PATH (toolchain.sh links one on
# Linux; brew install binaryen on a Mac).
mode="${COAXIAL_MODE:-release}"
opt=-Os; lto=(-flto)
if [[ "$mode" == dev ]]; then opt=-O1; lto=(); fi
cflags=(--target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -std=c11 "$opt" ${lto[@]+"${lto[@]}"}
        -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable
        -Wno-unused-but-set-variable)
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
# Each unit is its own C library, so an #export_c procedure two units load is
# a duplicate symbol at the link. Say which, before any C is compiled.
unit_paths=(); for unit in "${units[@]}"; do unit_paths+=("${unit%%:*}"); done
if [[ ${#unit_paths[@]} -gt 1 ]]; then
  (cd "$repo" && "$repo/coax/web/tool.sh" wasm-size exports "${unit_paths[@]}") || { echo "wasm.sh: $(basename "$out"): the same #export_c symbol in two units" >&2; exit 1; }
fi
# The cache: a unit's C, header and object are kept under the SHA-256 of the
# compiler binary, the options, the unit's path and every file in its #load
# closure (coax/web/tool.sh wasm-size sources), so a unit whose closure didn't
# change since an earlier build, in any checkout or release tree, isn't compiled
# again (Everglade's C takes ten minutes or more: its closure has an #authority
# unit, aethyr/ground.coax, so its compile runs every check in it). Objects are
# also keyed by clang's version and flags. A unit with an #import_proto isn't
# cached. COAXIAL_WASM_CACHE names the folder (default ~/.cache/coax-wasm); set
# it empty to build everything afresh. Entries unused for 14 days go.
cache="${COAXIAL_WASM_CACHE-$HOME/.cache/coax-wasm}"
if [[ -n "$cache" ]]; then
  mkdir -p "$cache"
  compiler_sum="$(shasum -a 256 < "$coax_driver" | cut -c1-64)"
  flags_sum="$({ "$clang" --version; printf '%s\n' "${cflags[@]}"; } | shasum -a 256 | cut -c1-16)"
  find "$cache" -maxdepth 1 -type f -mtime +14 -delete 2> /dev/null || true
fi
unit_key() {   # unit header-base: the cache key, or nothing (not cacheable)
  local list
  list="$(cd "$repo" && "$repo/coax/web/tool.sh" wasm-size sources "$1")" && [[ -n "$list" ]] || return 0
  { printf '%s\n' coax-wasm-c-1 "$compiler_sum" "$1" "$2" ${run_steps[@]+"${run_steps[@]}"}
    (cd "$repo" && shasum -a 256 $list); } | shasum -a 256 | cut -c1-64
}
keep() {   # file entry: into the cache whole (a rename), never half-written
  local into; into="$(mktemp "$cache/.keep.XXXXXXXX")"
  cp "$1" "$into" && mv -f "$into" "$2" || rm -f "$into"
}
from_cache() {   # entry file: copied out (and marked used) when it's there
  [[ -f "$1" ]] && cp "$1" "$2" && touch "$1"
}
emit() {   # unit header-base: its C and header into $work, from the cache or the compiler
  local key=""
  [[ -z "$cache" ]] || key="$(unit_key "$1" "$2")"
  printf '%s' "$key" > "$work/$2.key"
  if [[ -n "$key" && -f "$cache/$key.h" ]] && from_cache "$cache/$key.c" "$work/$2.c" && from_cache "$cache/$key.h" "$work/$2.h"; then
    return 0
  fi
  (cd "$repo" && "$coax_driver" c "$1" --library --header "$work/$2.h" ${run_steps[@]+"${run_steps[@]}"}) > "$work/$2.c"
  [[ -z "$key" ]] || { keep "$work/$2.h" "$cache/$key.h"; keep "$work/$2.c" "$cache/$key.c"; }
}
compile() {   # header-base: its object, from the cache or clang
  local key; key="$(< "$work/$1.key")"
  [[ -n "$key" ]] && from_cache "$cache/$key.$flags_sum.o" "$work/$1.o" && return 0
  "$clang" "${cflags[@]}" -I"$work" -c "$work/$1.c" -o "$work/$1.o"
  [[ -z "$key" ]] || keep "$work/$1.o" "$cache/$key.$flags_sum.o"
}
started=$SECONDS
for unit in "${units[@]}"; do
  emit "${unit%%:*}" "${unit##*:}" & gen+=($!)
done
for pid in "${gen[@]}"; do wait "$pid"; done
if [[ "$render" == 1 ]]; then cp "$work/$name.h" "$work/Render.h"; fi
# A folded library's header is the carrier's (a header may be included twice).
for library in ${folded[@]+"${folded[@]}"}; do
  base="$(basename "$library" .coax)"; base="$(printf '%s' "${base:0:1}" | tr a-z A-Z)${base:1}"
  cp "$work/$name.h" "$work/$base.h"
done
if [[ -n "${COAXIAL_ADMITTED_DIR:-}" ]]; then
  mkdir -p "$COAXIAL_ADMITTED_DIR"
  cp "$work/"*.c "$work/"*.h "$COAXIAL_ADMITTED_DIR/"
fi
c_done=$SECONDS
# Struct defaults come from the compiler: each unit's header declares
# coax_default_T for its types with declared defaults, and the face calls
# them for a new state (coaxial_state_init) and its own globals.
objects=("$work/$name.face.o"); pids=()
for unit in "${units[@]}"; do
  compile "${unit##*:}" & pids+=($!)
  objects+=("$work/${unit##*:}.o")
done
"$clang" "${cflags[@]}" -I"$work" -I"$(dirname "$face")" -c "$face" -o "$work/$name.face.o"
if [[ "$render" == 1 ]]; then
  "$clang" "${cflags[@]}" -I"$work" -c "$repo/coax/web/render-face.c" -o "$work/Render.face.o"
  objects+=("$work/Render.face.o")
fi
for pid in "${pids[@]}"; do wait "$pid"; done
objects_done=$SECONDS
# The bounded package parser's 16,384 JSON nodes alone occupy 1 MiB.
# Its measured wasm32 admission frame is 1,057,936 bytes; renderer frames
# and shader validation also use bounded scratch. Keep their stack inside
# the same 64 MiB imported-memory limit, with room for the measured chain.
stack_bytes=1048576
[[ "$render" == 1 ]] && stack_bytes=4194304
# Linked without -O, so clang doesn't run a wasm-opt of its own choosing (a
# release's objects are bitcode: the link optimizes them together).
"$clang" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" \
  -mexec-model=reactor -o "$work/output.wasm" "${objects[@]}" \
  -Wl,--import-memory,--max-memory=67108864,-z,stack-size="$stack_bytes",--export=__heap_base,--gc-sections,--strip-debug
if [[ "$mode" != dev ]]; then
  wasm_opt="${COAXIAL_WASM_OPT:-$(command -v wasm-opt || true)}"
  if [[ -n "$wasm_opt" ]]; then
    "$wasm_opt" "$work/output.wasm" -O2 -o "$work/optimized.wasm" && mv "$work/optimized.wasm" "$work/output.wasm"
  else
    echo "wasm.sh: no wasm-opt (brew install binaryen, or set COAXIAL_WASM_OPT): $(basename "$out") is about 6% larger" >&2
  fi
fi
# A module that took over a minute says where the time went (or every module, with COAXIAL_WASM_TIMES=1).
if (( SECONDS - started > 60 )) || [[ "${COAXIAL_WASM_TIMES:-}" == 1 ]]; then
  echo "wasm.sh: $(basename "$out"): $((SECONDS - started)) s: Coax to C $((c_done - started)) s, clang $((objects_done - c_done)) s, link and wasm-opt $((SECONDS - objects_done)) s; $(wc -c < "$work/output.wasm" | tr -d ' ') bytes" >&2
fi
# The size budget: apps/openagents-web/wasm-budgets.txt (beside the app's Coax)
# gives each module its most bytes (coax/web/tool.sh wasm-size budget). A
# release module over it, or written to the app's build/ with no line, stops the build; over it, wasm.sh
# names the module's sections and biggest functions (wasm-size report). build.sh
# checks first that every module it writes has a line (wasm-size budgets).
budgets="$repo/$(dirname "$coax_file")/wasm-budgets.txt"
module="$(basename "$out" .wasm)"
if [[ "$mode" != dev && -f "$budgets" ]]; then
  status=0
  (cd "$repo" && coax/web/tool.sh wasm-size budget "${budgets#"$repo"/}" "$module" "$work/output.wasm") || status=$?
  if [[ "$status" == 1 ]]; then
    echo "wasm.sh: $module's biggest parts:" >&2
    "$clang" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -mexec-model=reactor -o "$work/named.wasm" "${objects[@]}" \
      -Wl,--import-memory,--max-memory=67108864,-z,stack-size="$stack_bytes",--export=__heap_base,--gc-sections || true
    (cd "$repo" && coax/web/tool.sh wasm-size report "$work/named.wasm" 25) >&2 || true
    echo "wasm.sh: (sizes before wasm-opt, by name.) Make it smaller, or raise its budget in the same commit and say why." >&2
  fi
  # A module written anywhere but the app's build/ (a check's or a bench's copy,
  # under its own name) may have no line: that only warns.
  if [[ "$status" == 4 && "$out" != "$(cd "$(dirname "$budgets")" && pwd)/build/$module.wasm" ]]; then status=0; fi
  [[ "$status" == 0 ]] || exit 1
fi
mv "$work/output.wasm" "$out"   # the server never serves a half-written module

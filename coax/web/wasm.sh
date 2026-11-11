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
if [[ -n "${COAX_FAST_EDIT:-}" ]]; then
  cmp -s "$coax_driver" "$repo/coax/bin/coax" || { echo 'fast-edit: compiler differs from the sealed default' >&2; exit 2; }
  [[ "${COAXIAL_MODE:-release}" == dev || "${COAX_FAST_EDIT_ORACLE:-0}" == 1 ]] || { echo 'COAX_FAST_EDIT requires COAXIAL_MODE=dev' >&2; exit 2; }
fi
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
if [[ -n "${COAX_FAST_EDIT:-}" ]]; then
  [[ "$clang" == "$(sed -n '1p' "$COAX_FAST_EDIT/toolchain")" && "$sysroot" == "$(sed -n '2p' "$COAX_FAST_EDIT/toolchain")" && "$resources" == "$(sed -n '3p' "$COAX_FAST_EDIT/toolchain")" ]] || { echo 'fast-edit: Wasm toolchain differs from the sealed baseline' >&2; exit 2; }
fi
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
# The Coax build profile follows the mode: `#if BUILD_DEBUG { ... }` (coax/spec.md 10.5) is in a dev
# build and never in a release (Everglade II's admin mode, apps/openagents-web/everglade2-admin.coax).
# A unit whose closure has no BUILD_DEBUG emits the same C either way, so only such a unit's cache
# key names the profile.
profile=(--build-profile release); [[ "$mode" == dev ]] && profile=(--build-profile development)
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
# unit, aethyr/ground.coax, so its compile runs every check in it). A unit with
# an #import_proto isn't cached. An object is kept under what its compile reads:
# clang's version and flags and the bytes of every file the preprocessor opens
# for it (clang -M: the C, its headers, the sysroot's), so the same C compiles
# once however it was made (an edit to a #test, a comment or a library its C
# doesn't change). A linked module is kept under its objects' keys, the link
# and wasm-opt flags, which wasm-ld and wasm-opt ran (each tool by its resolved
# path and version) and which sysroot and runtime libraries it links (by the
# paths they resolve to: a Nix store or Homebrew Cellar path names its build)
# (#12925). COAXIAL_WASM_CACHE names the
# folder (default ~/.cache/coax-wasm); set it empty to build everything afresh.
# Entries unused for 14 days go.
cache="${COAXIAL_WASM_CACHE-$HOME/.cache/coax-wasm}"
if [[ -n "$cache" ]]; then
  mkdir -p "$cache"
  compiler_sum="$(shasum -a 256 < "$coax_driver" | cut -c1-64)"
  # A tool is known by its version and the file it resolves to (a Nix store or
  # Homebrew Cellar path names its build), not by hashing its bytes each build.
  tool_id() { local at; at="$(command -v "$1")" && printf '%s\n' "$(realpath "$at")" && "$at" --version; }
  flags_sum="$({ tool_id "$clang"; printf '%s\n' "${cflags[@]}"; } | shasum -a 256 | cut -c1-16)"
  find "$cache" -maxdepth 1 -type f -mtime +14 -delete 2> /dev/null || true
fi
# The shared tier (#13119, docs/verse/remote-builds.md): a Cloud Storage bucket
# behind this disk cache, under the same keys (wasm/KEY.c, .h, .notes, .o,
# .wasm). A lookup reads this disk first, then the bucket (and keeps what it
# fetched here); a compile writes through to both. It is on when the job has a
# token for the bucket (remote.sh sends one to each host job,
# COAX_SHARED_CACHE_TOKEN) and COAX_SHARED_CACHE isn't 0. The Mac's builds
# leave it off: no key can match across machines of two kinds (the key names
# the compiler's bytes and each tool's install path), so the Mac shares through
# built pages instead (coax/remote/pages.sh, COAXIAL_PREBUILT).
shared_tool="${COAX_SHARED_CACHE_TOOL:-$repo/coax/remote/shared-cache.sh}"
shared=0
[[ -z "$cache" || -z "${COAX_SHARED_CACHE_TOKEN:-}" || "${COAX_SHARED_CACHE:-}" == 0 || ! -x "$shared_tool" || -n "${COAX_FAST_EDIT:-}" ]] || shared=1
check_cache="${COAX_CHECK_CACHE:-$HOME/.cache/coax-check}"
# COAXIAL_SPLIT=N (#12938): the first unit's C (the app, or its renderer
# carrier) as up to N translation units and a shared one (coax c --split), so
# an edit compiles again only the parts whose C changed: Everglade 5e's carrier
# is 9 MB of C, 12-23 s of clang on coderos-4080 as one unit. Default 16 for a
# renderer carrier, else 0 (one unit). The compiler makes one part per 512 KiB
# of procedures' C (COAXIAL_SPLIT_UNIT bytes) and emits a unit under four
# parts' worth whole. Each part includes the prelude every part shares (the
# runtime, the types and the prototypes).
split="${COAXIAL_SPLIT:-}"
[[ -n "$split" ]] || { split=0; [[ "$render" == 1 ]] && split=16; }
[[ "$split" =~ ^[0-9]+$ && "$split" -le 256 ]] || { echo "COAXIAL_SPLIT must be a count from 0 to 256" >&2; exit 2; }
split_unit=(); [[ -z "${COAXIAL_SPLIT_UNIT:-}" ]] || split_unit=(--split-unit "$COAXIAL_SPLIT_UNIT")
unit_split() { [[ "$1" == "${units[0]%%:*}" && "$split" != 0 ]] && echo "$split" || echo 0; }   # unit: its --split count
unit_key() {   # unit header-base: the cache key, or nothing (not cacheable)
  local list
  # Development checks and compilation use an immutable private source tree.
  # Its C embeds that tree's physical paths, so never reuse ordinary C here.
  [[ -z "${COAX_FAST_EDIT:-}" ]] || return 0
  list="$(cd "$repo" && "$repo/coax/web/tool.sh" wasm-size sources "$1")" && [[ -n "$list" ]] || return 0
  { printf '%s\n' coax-wasm-c-1 "$compiler_sum" "$1" "$2" ${run_steps[@]+"${run_steps[@]}"}
    [[ "$(unit_split "$1")" == 0 ]] || printf 'split %s %s\n' "$(unit_split "$1")" "${COAXIAL_SPLIT_UNIT:-}"
    ! (cd "$repo" && grep -q 'BUILD_DEBUG' $list) || printf 'profile %s\n' "${profile[1]}"
    (cd "$repo" && shasum -a 256 $list); } | shasum -a 256 | cut -c1-64
}
keep() {   # file entry: into the cache whole (a rename), never half-written
  local into; into="$(mktemp "$cache/.keep.XXXXXXXX")"
  cp "$1" "$into" && mv -f "$into" "$2" || rm -f "$into"
}
have() {   # entry: there on this disk, or fetched from the shared store (then fetched=1 in the caller)
  [[ -f "$1" ]] && return 0
  [[ "$shared" == 1 ]] && "$shared_tool" get "wasm/$(basename "$1")" "$1" && fetched=1
}
from_cache() {   # entry file: copied out (and marked used) when it's there
  have "$1" && cp "$1" "$2" && touch "$1"
}
share() {   # entry: into the shared store after a compile: 0 written, 2 another host had written it (a duplicate compile)
  [[ "$shared" == 1 && -f "$1" ]] || return 0
  "$shared_tool" put "wasm/$(basename "$1")" "$1"
}
compiled() {   # entry...: each into the shared store, logged as one compile; prints miss, or dup when the first was there
  local status=0 e
  for e in "$@"; do share "$e" || { (( $? != 2 )) || status=2; }; done
  [[ "$shared" != 1 ]] || "$shared_tool" log "$(basename "$1")" > /dev/null 2>&1 &
  [[ "$status" == 2 ]] && echo dup || echo miss
}
# One compile per key (#13119). Before a compile, once ENTRY takes the right to
# make ENTRY: on this machine a lock folder in the cache (the wasm.sh runs of
# one build.sh share units: Everglade's controller, the thumbs, the Grid), and
# across hosts a claim object in the bucket (claim/wasm/KEY, written only when
# absent, refreshed every 30 seconds while its holder lives, anyone's after 150
# seconds without one). A run that finds the right taken waits for the holder's
# result instead of compiling the same thing beside it.
#   0: compile it, then call done_once ENTRY
#   1: ENTRY is in this disk's cache now (fetched=1 in the caller when it came from the bucket)
self_pid() { exec sh -c 'echo $PPID'; }
once() {
  local e="$1" lock me who rc name said=0
  lock="$cache/.once.$(basename "$1")"; me="$(self_pid)"
  while ! mkdir "$lock" 2> /dev/null; do
    [[ ! -f "$e" ]] || return 1
    who="$(cat "$lock/pid" 2> /dev/null || true)"
    # A holder that is gone (its build failed or was killed) holds nothing.
    if [[ -n "$who" ]] && ! kill -0 "$who" 2> /dev/null; then rm -rf "$lock"; continue; fi
    [[ -n "$who" || -z "$(find "$lock" -maxdepth 0 -mmin +1 2> /dev/null)" ]] || { rm -rf "$lock"; continue; }
    sleep 0.3
  done
  echo "$me" > "$lock/pid"
  if [[ -f "$e" ]]; then rm -rf "$lock"; return 1; fi
  if [[ "$shared" == 1 ]]; then
    name="wasm/$(basename "$e")"
    while :; do
      rc=0; "$shared_tool" claim "$name" || rc=$?
      (( rc == 4 )) || break   # 0: ours. Else the store didn't answer (or its tool is an older one): compile.
      if "$shared_tool" get "$name" "$e"; then fetched=1; rm -rf "$lock"; return 1; fi
      (( said )) || echo "wasm.sh: another host is compiling $(basename "$e"); waiting for its result" >&2
      said=1; sleep 3
    done
    if (( rc == 0 )); then
      # Written between our lookup and our claim: take it.
      if "$shared_tool" get "$name" "$e"; then "$shared_tool" unclaim "$name"; fetched=1; rm -rf "$lock"; return 1; fi
      ( while sleep 30; do
          kill -0 "$me" 2> /dev/null && [[ "$(cat "$lock/pid" 2> /dev/null)" == "$me" ]] || exit 0
          "$shared_tool" claim "$name" refresh
        done ) < /dev/null > /dev/null 2>&1 &
    fi
  fi
  return 0
}
done_once() {   # entry: the compile is over and its result written
  [[ "$shared" != 1 ]] || "$shared_tool" unclaim "wasm/$(basename "$1")" || true
  rm -rf "$cache/.once.$(basename "$1")"
}
# The build report (#12925): each step's wall time in microseconds, and whether
# the cache supplied it, as "base step cache start end" lines in $work/steps;
# each compiler run's own phase report (coax --timings) in $work/BASE.coax.json.
now() { [[ -n "${EPOCHREALTIME:-}" ]] && echo "${EPOCHREALTIME//[.,]/}" || echo $(( $(date +%s) * 1000000 )); }   # microseconds
step() {
  local end; end="$(now)"
  printf '%s %s %s %s %s\n' "$1" "$2" "$3" "$4" "$end" >> "$work/steps"
  # The job's stats (host-job.sh sums them in remote.sh's report line).
  [[ -z "${COAX_SHARED_CACHE_STATS:-}" ]] || printf '%s %s %s\n' "$2" "$3" "$(( end - $4 ))" >> "$COAX_SHARED_CACHE_STATS"
}
emit() {   # unit header-base: its C and header into $work, from the cache or the compiler
  local key="" began fetched=0; began="$(now)"
  [[ -z "$cache" ]] || key="$(unit_key "$1" "$2")"
  printf '%s' "$key" > "$work/$2.key"
  # A unit with a headroom line needs its compile's notes, so its cached C
  # counts only with them (entries made before the notes were kept compile again).
  local held=0 try
  for try in 1 2; do
    if [[ -n "$key" ]] && have "$cache/$key.h" && { [[ -z "$(headroom_of "$2")" ]] || from_cache "$cache/$key.notes" "$work/$2.notes"; } &&
       from_cache "$cache/$key.c" "$work/$2.c" && from_cache "$cache/$key.h" "$work/$2.h"; then
      [[ ! -s "$work/$2.notes" ]] || grep '^coax: note\[' "$work/$2.notes" | sed 's/^/(cached) /' >&2 || true
      step "$2" coax_to_c "$( (( fetched )) && echo shared || echo hit)" "$began"; return 0
    fi
    # Not there: compile it, unless another run is (then look again for its result).
    [[ -n "$key" && "$try" == 1 ]] || break
    if once "$cache/$key.h"; then held=1; break; fi
  done
  # The check records the last compile of this unit on any host used (the
  # compiler's check cache, keyed as always): so a compile after an edit reuses
  # the checks the edit didn't touch, whichever host ran them before.
  local pack=""
  if [[ -n "$key" && "$shared" == 1 ]]; then
    pack="checks/$compiler_sum/$(printf '%s' "$1" | shasum -a 256 | cut -c1-32).tar.gz"
    mkdir -p "$check_cache"
    if "$shared_tool" get "$pack" "$work/$2.checks-in.tgz" && tar -xzmf "$work/$2.checks-in.tgz" -C "$check_cache" 2> /dev/null; then
      [[ -z "${COAX_SHARED_CACHE_STATS:-}" ]] || echo "checks shared 0" >> "$COAX_SHARED_CACHE_STATS"
    fi
    touch "$work/$2.mark"
  fi
  local parts=(); [[ "$(unit_split "$1")" == 0 ]] || parts=(--split "$(unit_split "$1")" ${split_unit[@]+"${split_unit[@]}"})
  local source="$1"
  [[ -z "${COAX_FAST_EDIT:-}" ]] || source="$COAX_FAST_EDIT/source/$1"
  # Its stderr goes on to ours and into $work/$2.notes, for the headroom check.
  { (cd "$repo" && "$coax_driver" c "$source" --library "${profile[@]}" --header "$work/$2.h" ${parts[@]+"${parts[@]}"} ${run_steps[@]+"${run_steps[@]}"} --timings "$work/$2.coax.json") 2>&1 > "$work/$2.c" | tee "$work/$2.notes" >&2; }
  [[ -z "$key" ]] || { keep "$work/$2.notes" "$cache/$key.notes"; keep "$work/$2.h" "$cache/$key.h"; keep "$work/$2.c" "$cache/$key.c"; }
  local result=off
  # The C before its header: a host that finds the header finds the C.
  [[ -z "$key" ]] || result="$(compiled "$cache/$key.c" "$cache/$key.notes" "$cache/$key.h")"
  (( ! held )) || done_once "$cache/$key.h"
  if [[ -n "$pack" ]]; then   # the check records this compile used (a record read is touched), for the next compile of the unit anywhere
    (cd "$check_cache" && find . -maxdepth 1 -type f ! -name '.*' -newer "$work/$2.mark" > "$work/$2.checks-out.list") || true
    [[ ! -s "$work/$2.checks-out.list" ]] ||
      { (cd "$check_cache" && tar -czf "$work/$2.checks-out.tgz" -T "$work/$2.checks-out.list" 2> /dev/null) &&
        "$shared_tool" replace "$pack" "$work/$2.checks-out.tgz"; } || true
  fi
  step "$2" coax_to_c "$result" "$began"
}
object_key() {   # source [flags...]: the SHA-256 of clang, its flags and every file compiling source reads
  local deps
  deps="$("$clang" "${cflags[@]}" "$@" -M -MT o)" || return 0
  deps="${deps#o:}"; deps="${deps//\\/ }"
  # $work's name is new each build: a file in it is keyed by its name inside.
  { printf '%s\n' coax-wasm-object-1 "$flags_sum"; shasum -a 256 $deps | sed "s|  $work/|  |"; } | shasum -a 256 | cut -c1-64
}
compile() {   # name source [flags...]: $work/name.o from the cache or clang, and its key in $work/name.okey
  local name="$1" key="" began fetched=0 key_held=0; shift; began="$(now)"
  [[ -z "$cache" ]] || key="$(object_key "$@")"
  printf '%s' "$key" > "$work/$name.okey"
  [[ -n "$key" ]] && from_cache "$cache/$key.o" "$work/$name.o" && { step "$name" clang "$( (( fetched )) && echo shared || echo hit)" "$began"; return 0; }
  if [[ -n "$key" ]] && ! once "$cache/$key.o"; then   # another run compiled it meanwhile
    cp "$cache/$key.o" "$work/$name.o" && { step "$name" clang "$( (( fetched )) && echo shared || echo hit)" "$began"; return 0; }
    key_held=0
  else
    key_held=1
  fi
  "$clang" "${cflags[@]}" "$@" -c -o "$work/$name.o"
  local result=off
  [[ -z "$key" ]] || { keep "$work/$name.o" "$cache/$key.o"; result="$(compiled "$cache/$key.o")"; (( ! key_held )) || done_once "$cache/$key.o"; }
  step "$name" clang "$result" "$began"
}
# Compile headroom (#13115): apps/openagents-web/wasm-headroom.txt (beside
# the app's Coax) gives a unit, by its header base (fivelab, Sticks), the most
# percent of any compile-time limit it may use. coax notes each limit used above
# 75% (note[E0400] and the rest: "NAME: USED of LIMIT used (P%)", then its top
# users); a unit past its line stops the build with those notes, so a creep
# toward E0400 shows in the commit that makes it, not in a later one that fails.
headroom_file="$repo/$(dirname "$coax_file")/wasm-headroom.txt"
headroom_of() {   # header-base: its most percent, or nothing
  [[ -f "$headroom_file" ]] || return 0
  awk -v u="$1" '$1 == u && $2 ~ /^[0-9]+$/ { print $2; exit }' "$headroom_file"
}
headroom_check() {   # every unit with a line, against its notes
  local unit base most over status=0
  for unit in "${units[@]}"; do
    base="${unit##*:}"; most="$(headroom_of "$base")"
    [[ -n "$most" && -f "$work/$base.notes" ]] || continue
    over="$(awk -v most="$most" '
      /^coax: note\[/ { keep = 0; if (match($0, /\(([0-9]+)%\)/)) { p = substr($0, RSTART + 1, RLENGTH - 3) + 0; if (p > most) keep = 1 } }
      !/^coax: note\[/ && !/^ / { keep = 0 }
      keep { print }' "$work/$base.notes")"
    [[ -n "$over" ]] || continue
    echo "wasm.sh: $base uses more than $most% of a compile-time limit (${headroom_file#"$repo"/}):" >&2
    printf '%s\n' "$over" | sed 's/^/  /' >&2
    echo "wasm.sh: pass big values by view or slice, hold one at a time, or split the check; never raise the limit to fit." >&2
    status=1
  done
  return "$status"
}
# The size budget: apps/openagents-web/wasm-budgets.txt (beside the app's Coax)
# gives each module its most bytes (coax/web/tool.sh wasm-size budget). A
# release module over it, or written to the app's build/ with no line, stops the build; over it, wasm.sh
# names the module's sections and biggest functions (wasm-size report). build.sh
# checks first that every module it writes has a line (wasm-size budgets).
budgets="$repo/$(dirname "$coax_file")/wasm-budgets.txt"
module="$(basename "$out" .wasm)"
budget_check() {   # [named]: $work/output.wasm against its budget; named: name the biggest parts when over
  [[ "$mode" != dev && -f "$budgets" ]] || return 0
  local status=0
  (cd "$repo" && coax/web/tool.sh wasm-size budget "${budgets#"$repo"/}" "$module" "$work/output.wasm") || status=$?
  if [[ "$status" == 1 && "${1:-}" == named ]]; then
    echo "wasm.sh: $module's biggest parts:" >&2
    "$clang" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -mexec-model=reactor -o "$work/named.wasm" "${object_files[@]}" \
      -Wl,--import-memory,--max-memory="$max_memory",-z,stack-size="$stack_bytes",--export=__heap_base,--gc-sections || true
    (cd "$repo" && coax/web/tool.sh wasm-size report "$work/named.wasm" 25) >&2 || true
    echo "wasm.sh: (sizes before wasm-opt, by name.) Make it smaller, or raise its budget in the same commit and say why." >&2
  fi
  # A module written anywhere but the app's build/ (a check's or a bench's copy,
  # under its own name) may have no line: that only warns.
  if [[ "$status" == 4 && "$out" != "$(cd "$(dirname "$budgets")" && pwd)/build/$module.wasm" ]]; then status=0; fi
  [[ "$status" == 0 ]] || return 1
}
# What this module is built from, whatever compiler and tools build it: the
# options, its units, each unit's #load closure (every file's SHA-256), and the
# C the face compiles with (its directory's C and headers, coax/platform's, the
# renderer face). Two builds with the same sum read the same sources.
inputs_sum() {
  local text
  text="$( printf '%s\n' coax-wasm-inputs-1 "$mode" "$render" "${profile[1]}" "$split" "${COAXIAL_SPLIT_UNIT:-}" ${run_steps[@]+"${run_steps[@]}"}
    printf 'unit %s\n' "${units[@]}"; printf 'folded %s\n' ${folded[@]+"${folded[@]}"}
    for unit in "${units[@]}"; do
      list="$(cd "$repo" && "$repo/coax/web/tool.sh" wasm-size sources "${unit%%:*}")" && [[ -n "$list" ]] || exit 1
      (cd "$repo" && shasum -a 256 $list)
    done
    (cd "$(dirname "$face")" && shasum -a 256 "$(basename "$face")" *.h *.c 2> /dev/null)
    (cd "$repo" && shasum -a 256 coax/platform/*.c coax/platform/*.h coax/web/render-face.c 2> /dev/null)
  )" || return 1   # a closure the size tool can't list: no sum, so no record
  printf '%s\n' "$text" | shasum -a 256 | cut -c1-64
}
inputs_job=""
if [[ "$mode" != dev && -z "${COAX_FAST_EDIT:-}" ]]; then inputs_sum > "$work/inputs" & inputs_job=$!; fi
: > "$work/steps"
started=$SECONDS; started_us="$(now)"
# COAXIAL_PREBUILT=1 (release.sh, after pages.sh fetched the hosts' build of
# the same commit): the module already at OUT is taken when its .c/MODULE.built
# record names these inputs and its bytes. Its compile-headroom lines and its
# size budget are checked against those bytes as for a build here.
built="$work_root/$module.built"
if [[ "${COAXIAL_PREBUILT:-0}" == 1 && "$mode" != dev && -z "${COAX_FAST_EDIT:-}" && -f "$built" && -f "$out" ]]; then
  wait "$inputs_job" 2> /dev/null || true
  if [[ -s "$work/inputs" && "$(sed -n 2p "$built")" == "inputs $(< "$work/inputs")" &&
        "$(sed -n 3p "$built")" == "sha256 $(shasum -a 256 < "$out" | cut -c1-64)" ]]; then
    for unit in "${units[@]}"; do b="${unit##*:}"; sed -n "s/^note $b //p" "$built" > "$work/$b.notes"; done
    headroom_check || exit 1
    cp "$out" "$work/output.wasm"
    budget_check || exit 1
    step module link prebuilt "$started_us"
    [[ "${COAXIAL_WASM_TIMES:-}" == 0 ]] || echo "wasm.sh: $(basename "$out"): prebuilt (the hosts' build of these sources, sha256 $(sed -n 3p "$built" | cut -c8-19)); $(wc -c < "$out" | tr -d ' ') bytes" >&2
    exit 0
  fi
  echo "wasm.sh: $(basename "$out"): the prebuilt module doesn't match these sources; building it" >&2
fi
if [[ -n "${COAX_FAST_EDIT:-}" && "${COAX_FAST_EDIT_ORACLE:-0}" != 1 ]]; then units+=("coax/web/fast-edit-offer.coax:fast-edit-policy"); fi
for unit in "${units[@]}"; do
  emit "${unit%%:*}" "${unit##*:}" & gen+=($!)
done
for pid in "${gen[@]}"; do wait "$pid"; done
[[ "$mode" == dev ]] || headroom_check || exit 1
if [[ -n "${COAX_FAST_EDIT:-}" && "${COAX_FAST_EDIT_ORACLE:-0}" != 1 ]]; then
  [[ "$mode" == dev ]] || { echo 'COAX_FAST_EDIT requires COAXIAL_MODE=dev' >&2; exit 2; }
  # Cache entries remain ordinary checked C; object keys hash the private
  # instrumented copies, including their sealed development identity.
  for unit in "${units[@]}"; do "$repo/coax/web/tool.sh" fast-edit instrument "$COAX_FAST_EDIT" "$work/${unit##*:}.c"; done
fi
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
# Struct defaults come from the compiler: each unit's header declares
# coax_default_T for its types with declared defaults, and the face calls
# them for a new state (coaxial_state_init) and its own globals.
objects=("$name.face"); pids=()
for unit in "${units[@]}"; do
  base="${unit##*:}"
  if [[ "$(unit_split "${unit%%:*}")" == 0 ]]; then
    compile "$base" -I"$work" "$work/$base.c" & pids+=($!)
    objects+=("$base")
    continue
  fi
  # A split unit's stream: the prelude, then each part after "/* coax-unit ID */".
  mkdir -p "$work/parts"
  ids="$(awk -v dir="$work/parts" -v base="$base" '
    /^\/\* coax-unit [A-Za-z0-9]+ \*\/$/ { if (f) close(f); f = dir "/" base ".u" $3 ".c"; print "#include \"" base ".prelude.h\"" > f; print $3; next }
    { if (f) print > f; else print > (dir "/" base ".prelude.h") }' "$work/$base.c")"
  if [[ -z "$ids" ]]; then   # small enough to be emitted whole
    compile "$base" -I"$work" "$work/$base.c" & pids+=($!)
    objects+=("$base")
  fi
  for id in $ids; do
    compile "$base.u$id" -I"$work" "$work/parts/$base.u$id.c" & pids+=($!)
    objects+=("$base.u$id")
  done
done
compile "$name.face" -I"$work" -I"$(dirname "$face")" "$face" & pids+=($!)
if [[ "$render" == 1 ]]; then
  compile Render.face -I"$work" "$repo/coax/web/render-face.c" & pids+=($!)
  objects+=(Render.face)
fi
for pid in "${pids[@]}"; do wait "$pid"; done
# The bounded package parser's 16,384 JSON nodes alone occupy 1 MiB.
# Its measured wasm32 admission frame is 1,057,936 bytes; renderer frames
# and shader validation also use bounded scratch. Keep their stack inside
# the ordinary 64 MiB imported-memory limit. The Coax page declaration
# below also bounds the document inspector's larger retained source image.
stack_bytes=1048576
# Compiled Actor Lab owns bounded catalogue/instance drafts during initialization.
[[ "$(basename "$out" .wasm)" != actor-lab ]] || stack_bytes=4194304
[[ "$render" == 1 ]] && stack_bytes=4194304
memory_pages="$(cd "$repo" && coax/web/tool.sh aethyr/render/page-render.coax --memory "$name")"
max_memory=$((memory_pages * 65536))
[[ "$memory_pages" == 1024 ]] || stack_bytes=4194304
# Scene project admission nests a bounded JSON arena and an ALB1 reader.
[[ "$name" != animationlab ]] || stack_bytes=8388608
link=(--target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -mexec-model=reactor
      -Wl,--import-memory,--max-memory="$max_memory",-z,stack-size="$stack_bytes",--export=__heap_base,--gc-sections,--strip-debug)
[[ -z "${COAX_FAST_EDIT:-}" || "${COAX_FAST_EDIT_ORACLE:-0}" == 1 ]] || link+=(-Wl,--export=fe_publish_offer)
# A release's link still optimizes every unit together (full LTO), then
# generates code in 8 parallel parts (#12938). Everglade 5e with its carrier
# split, on coderos-4080 at load 9: 10.2 s linked in one part, 7.3 s in 8,
# 2,014,681 and 2,015,523 bytes after wasm-opt (the unsplit carrier linked in
# one part made 2,038,414). The count is fixed, never the machine's cores: it
# decides the bytes.
[[ "$mode" == dev ]] || link+=(-Wl,--lto-partitions=8)
object_files=(); for o in "${objects[@]}"; do object_files+=("$work/$o.o"); done
wasm_opt=""; [[ "$mode" == dev ]] || wasm_opt="${COAXIAL_WASM_OPT:-$(command -v wasm-opt || true)}"
module_key=""
if [[ -n "$cache" ]]; then
  module_key="$({ printf '%s\n' coax-wasm-module-1 "$flags_sum" "${link[@]}" "$mode" "$wasm_opt"
    for o in "${objects[@]}"; do k="$(< "$work/$o.okey")"; [[ -n "$k" ]] || exit 1; printf '%s\n' "$k"; done
    tool_id wasm-ld; [[ -z "$wasm_opt" ]] || tool_id "$wasm_opt"
    for d in "$sysroot" "$resources"; do realpath "$d"; find "$(realpath "$d")" -type l -exec realpath {} + 2> /dev/null || true; done | sort
  } | shasum -a 256 | cut -c1-64)" || module_key=""
fi
began="$(now)"; fetched=0
module_held=0
if [[ -n "$module_key" ]] && ! from_cache "$cache/$module_key.wasm" "$work/output.wasm"; then
  if once "$cache/$module_key.wasm"; then module_held=1; fi   # else another run linked it meanwhile
fi
if [[ -n "$module_key" ]] && (( ! module_held )) && from_cache "$cache/$module_key.wasm" "$work/output.wasm"; then
  step module link "$( (( fetched )) && echo shared || echo hit)" "$began"
else
  # Linked without -O, so clang doesn't run a wasm-opt of its own choosing (a
  # release's objects are bitcode: the link optimizes them together).
  "$clang" "${link[@]}" -o "$work/output.wasm" "${object_files[@]}"
  step module link "$([[ -n "$module_key" ]] && echo miss || echo off)" "$began"
  if [[ "$mode" != dev ]]; then
    if [[ -n "$wasm_opt" ]]; then
      began="$(now)"
      "$wasm_opt" "$work/output.wasm" -O2 -o "$work/optimized.wasm" && mv "$work/optimized.wasm" "$work/output.wasm"
      step module wasm_opt off "$began"
    else
      echo "wasm.sh: no wasm-opt (brew install binaryen, or set COAXIAL_WASM_OPT): $(basename "$out") is about 6% larger" >&2
    fi
  fi
  [[ -z "$module_key" ]] || { keep "$work/output.wasm" "$cache/$module_key.wasm"; compiled "$cache/$module_key.wasm" > /dev/null; }
  (( ! module_held )) || done_once "$cache/$module_key.wasm"
fi
# Every module says where its time went in one line, and writes the whole
# report as JSON: COAXIAL_WASM_REPORT, else .c/MODULE.timings.json beside the
# output. Each step's wall time (steps run in parallel, so they overlap), the
# cache's part in it, each compiler run's phases, checks, capacities and its
# costliest declarations (coax --timings), and the machine and its load.
# COAXIAL_WASM_TIMES=0 keeps the line quiet; the report is still written.
report="${COAXIAL_WASM_REPORT:-$work_root/$(basename "$out" .wasm).timings.json}"
seconds() { printf '%d.%06d' $(( $1 / 1000000 )) $(( $1 % 1000000 )); }
{
  ended="$(now)"; bytes="$(wc -c < "$work/output.wasm" | tr -d ' ')"
  load="$( { cat /proc/loadavg 2> /dev/null || sysctl -n vm.loadavg 2> /dev/null; } | tr -d '{}' | awk '{print $1}')"
  cores="$(getconf _NPROCESSORS_ONLN 2> /dev/null || echo 0)"
  printf '{"schema_version":1,"module":"%s","source":"%s","mode":"%s","render":%s,"output_bytes":%s,"seconds":%s,' \
    "$(basename "$out" .wasm)" "$coax_file" "$mode" "$render" "$bytes" "$(seconds $(( ended - started_us )))"
  printf '"machine":{"host":"%s","os":"%s","cores":%s,"load":%s},"compiler_sha256":"%s","cache":"%s","steps":[' \
    "$(hostname -s 2> /dev/null || hostname)" "$(uname -sm)" "${cores:-0}" "${load:-null}" "$(shasum -a 256 < "$coax_driver" | cut -c1-64)" "$([[ -n "$cache" ]] && echo on || echo off)"
  first=1
  while read -r unit what hit from to; do
    [[ $first == 1 ]] || printf ','; first=0
    printf '{"unit":"%s","step":"%s","cache":"%s","start":%s,"seconds":%s' "$unit" "$what" "$hit" "$(seconds $(( from - started_us )))" "$(seconds $(( to - from )))"
    if [[ "$what" == coax_to_c && -s "$work/$unit.coax.json" ]]; then printf ',"coax":'; tr -d '\n' < "$work/$unit.coax.json"; fi
    printf '}'
  done < "$work/steps"
  printf ']}\n'
} > "$report.tmp.$$" && mv -f "$report.tmp.$$" "$report"
if [[ "${COAXIAL_WASM_TIMES:-}" != 0 ]]; then
  summary="$(awk -v total="$(( $(now) - started_us ))" '
    { s = ($5 - $4) / 1e6; if (s > most[$2]) most[$2] = s; if ($3 == "hit" || $3 == "shared") hits[$2]++; n[$2]++ }
    END { printf "%.1f s: Coax to C %.1f s (%d/%d cached), clang %.1f s (%d/%d cached), link %.1f s, wasm-opt %.1f s", total / 1e6,
          most["coax_to_c"], hits["coax_to_c"], n["coax_to_c"], most["clang"], hits["clang"], n["clang"], most["link"], most["wasm_opt"] }' "$work/steps")"
  echo "wasm.sh: $(basename "$out"): $summary; $(wc -c < "$work/output.wasm" | tr -d ' ') bytes; report $report" >&2
fi
budget_check named || exit 1
# The built-module record (#13119): beside the report, .c/MODULE.built names
# what the module was built from (inputs_sum), its bytes' SHA-256 and each
# unit's compile notes. pages.sh carries it with the module; COAXIAL_PREBUILT=1
# (release.sh) takes a module whose record matches instead of building it.
if [[ "$mode" != dev && -z "${COAX_FAST_EDIT:-}" ]]; then
  wait "$inputs_job" 2> /dev/null || true
  if [[ -s "$work/inputs" ]]; then
    { echo coax-wasm-built-1; echo "inputs $(< "$work/inputs")"; echo "sha256 $(shasum -a 256 < "$work/output.wasm" | cut -c1-64)"
      for unit in "${units[@]}"; do b="${unit##*:}"; [[ ! -s "$work/$b.notes" ]] || sed "s/^/note $b /" "$work/$b.notes"; done
    } > "$work_root/$module.built.$$" && mv -f "$work_root/$module.built.$$" "$work_root/$module.built"
  fi
fi
mv "$work/output.wasm" "$out"   # the server never serves a half-written module
# What `coax drive` (coax/web/drive.coax) reads beside the module, under build/.drive/ (never served):
# NAME.layout, the state's layout as `coax layout` prints it (`state PATH` reads a field through it), and
# NAME.sources, a SHA-256 for each source the module was built from (the Coax files `coax deps` names,
# the face and the libraries), so the driver refuses a module older than its sources.
drive_dir="$(dirname "$out")/.drive"; mkdir -p "$drive_dir"
if ! (cd "$repo" && "$coax_driver" layout "$coax_file" --library > "$drive_dir/$module.layout.tmp.$$" 2> /dev/null && mv -f "$drive_dir/$module.layout.tmp.$$" "$drive_dir/$module.layout"); then
  rm -f "$drive_dir/$module.layout.tmp.$$"; echo "wasm.sh: $module: no state layout for coax drive" >&2
fi
if deps="$(cd "$repo" && "$coax_driver" deps "$coax_file" --library 2> /dev/null)"; then
  {
    { grep -o '"sources":\[[^]]*\]' <<< "$deps" | grep -o '"[^"]*"' | tr -d '"' | grep -v '^sources$'; echo "$face"; for l in ${libraries[@]+"${libraries[@]}"}; do echo "$repo/$l"; done; } |
      while IFS= read -r f; do
        [[ "$f" == /* ]] || f="$repo/$f"
        [[ -f "$f" ]] || continue
        r="$(cd "$repo" && pwd -P)"; p="$(cd "$(dirname "$f")" && pwd -P)/$(basename "$f")"
        printf '%s  %s\n' "$(shasum -a 256 "$f" | cut -d ' ' -f 1)" "${p#"$r"/}"
      done | sort -u -k2
  } > "$drive_dir/$module.sources.tmp.$$" && mv -f "$drive_dir/$module.sources.tmp.$$" "$drive_dir/$module.sources"
else echo "wasm.sh: $module: no source list for coax drive" >&2; fi

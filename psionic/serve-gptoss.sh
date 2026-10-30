#!/usr/bin/env bash
# Build our Coax Responses edge and keep original weights resident until exit.
#   psionic/serve-gptoss.sh [--metal] ORIGINAL_CACHE [PORT=8080]
#   psionic/serve-gptoss.sh [--metal] --build-only     (build into the cache and exit)
# --metal builds the Apple GPU engine (gptoss-metal.c, coax metal-compute,
# macOS only) instead of CUDA; on a Mac serve it on its own loopback port,
# 8794 by convention (8080 is the 4080's tunnel, 8793 Elephant on Metal).
# A clean checkout's build is kept in ~/.cache/psionic/responses-server/KEY
# (KEY: the commit's tree, the coax binary and the C compiler), so a restart
# of the same commit only loads the weights; compiling the server is about
# 25 s of a load on coderos-4080. To deploy a new commit with the least
# downtime, run --build-only first while the old server still serves, then
# restart it (the GPU switch: POST /stop, then POST /use/gptoss).
# A checkout with uncommitted changes builds fresh into a temporary folder.
set -euo pipefail
cd "$(dirname "$0")/.."
build_only=
metal=
if [[ ${1:-} == --metal ]]; then metal=1; shift; fi
if [[ ${1:-} == --build-only ]]; then build_only=1; shift; fi
if [[ -z $build_only && ( $# -lt 1 || $# -gt 2 ) ]]; then echo 'usage: psionic/serve-gptoss.sh [--metal] ORIGINAL_CACHE [PORT=8080] | [--metal] --build-only' >&2; exit 2; fi
if [[ -n $metal && $(uname -s) != Darwin ]]; then echo 'serve-gptoss.sh --metal: Metal needs macOS' >&2; exit 2; fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
sum() { if command -v sha256sum >/dev/null; then sha256sum | cut -c1-24; else shasum -a 256 | cut -c1-24; fi; }

key=
if git rev-parse -q --verify HEAD >/dev/null 2>&1 && [[ -z $(git status --porcelain --untracked-files=no 2>/dev/null) ]]; then
    key=$( { git rev-parse 'HEAD^{tree}'; sum < "$coax"; "$cc" --version 2>&1 | sed -n 1p; echo "metal=$metal"; } | sum )
fi
cache=${PSIONIC_RESPONSES_CACHE:-$HOME/.cache/psionic/responses-server}

build() {   # build into $1
    "$coax" c psionic/lib/responses/server-host.coax --library --release --run-steps 2000000 --header "$1/device-host.h" > "$1/device-host.c"
    local libs=(-lm -pthread)
    if [[ -n $metal ]]; then
        "$coax" metal-compute psionic/lib/gptoss/device-kernels.coax --release > "$1/device-kernels.src"
        "$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -c psionic/platform/metal-host.m -o "$1/metal-host.o"
        "$cc" -std=c11 -O3 -g -ffp-contract=off -fno-fast-math -DPSIONIC_METAL -I"$1" -Ipsionic/platform \
            "$1/device-host.c" psionic/platform/gptoss-metal.c psionic/platform/responses-server.c "$1/metal-host.o" \
            -framework Metal -framework Foundation "${libs[@]}" -o "$1/responses-server"
        return
    fi
    "$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$1/device-kernels.src"
    if [[ $(uname -s) == Linux ]]; then libs+=(-ldl); fi
    "$cc" -std=c11 -O3 -g -ffp-contract=off -fno-fast-math -I"$1" -Ipsionic/platform \
        "$1/device-host.c" psionic/platform/cuda-host.c psionic/platform/gptoss-device.c \
        psionic/platform/responses-server.c "${libs[@]}" -o "$1/responses-server"
}

if [[ -n $key ]]; then
    out=$cache/$key
    if [[ ! -x $out/responses-server ]]; then
        mkdir -p "$cache"
        tmp=$(mktemp -d "$cache/.building.XXXXXX")
        trap 'rm -rf "$tmp"' EXIT
        build "$tmp"
        mv "$tmp" "$out" 2>/dev/null || rm -rf "$tmp"   # another build won the race: keep its copy
        trap - EXIT
        # keep the newest four builds
        ls -1dt "$cache"/*/ 2>/dev/null | tail -n +5 | while read -r old; do rm -rf "$old"; done
        echo "built the Responses server into $out"
    else
        echo "the Responses server for this commit is already built: $out"
    fi
else
    [[ -n $build_only ]] && { echo 'serve-gptoss.sh --build-only: the checkout has uncommitted changes, so there is nothing to cache' >&2; exit 2; }
    out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-responses-server.XXXXXX")
    trap 'rm -rf "$out"' EXIT
    build "$out"
fi
[[ -n $build_only ]] && exit 0
"$out/responses-server" "$1" psionic/fixtures/harmony/gptoss.hst "$out/device-kernels.src" "${2:-8080}"

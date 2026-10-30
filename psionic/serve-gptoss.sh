#!/usr/bin/env bash
# Build our Coax Responses edge and keep original weights resident until exit.
#   psionic/serve-gptoss.sh ORIGINAL_CACHE [PORT=8080]
#   psionic/serve-gptoss.sh --build-only     (build into the cache and exit)
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
if [[ ${1:-} == --build-only ]]; then build_only=1; shift; fi
if [[ -z $build_only && ( $# -lt 1 || $# -gt 2 ) ]]; then echo 'usage: psionic/serve-gptoss.sh ORIGINAL_CACHE [PORT=8080] | --build-only' >&2; exit 2; fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
sum() { if command -v sha256sum >/dev/null; then sha256sum | cut -c1-24; else shasum -a 256 | cut -c1-24; fi; }

key=
if git rev-parse -q --verify HEAD >/dev/null 2>&1 && [[ -z $(git status --porcelain --untracked-files=no 2>/dev/null) ]]; then
    key=$( { git rev-parse 'HEAD^{tree}'; sum < "$coax"; "$cc" --version 2>&1 | sed -n 1p; } | sum )
fi
cache=${PSIONIC_RESPONSES_CACHE:-$HOME/.cache/psionic/responses-server}

build() {   # build into $1
    "$coax" c psionic/lib/responses/server-host.coax --library --release --run-steps 2000000 --header "$1/device-host.h" > "$1/device-host.c"
    "$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$1/device-kernels.cu"
    local libs=(-lm -pthread)
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
"$out/responses-server" "$1" psionic/fixtures/harmony/gptoss.hst "$out/device-kernels.cu" "${2:-8080}"

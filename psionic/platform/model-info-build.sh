#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
[[ $# == 1 || ( $# == 2 && $2 == sanitized ) ]] || { echo 'usage: model-info-build.sh OUT [sanitized]' >&2; exit 2; }
out=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mkdir -p "$out"
"$coax" c psionic/lib/ops/model-manifest.coax --library --header "$out/model-info.h" > "$out/model-info.c"
flags=(-std=c11 -O1 -g -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas)
if [[ ${2:-} == sanitized ]]; then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
"$cc" "${flags[@]}" -I"$out" psionic/platform/model-info-face.c "$out/model-info.c" -lm -o "$out/psionic-model-info"

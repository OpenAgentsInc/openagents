#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
[[ $# == 1 || ( $# == 2 && $2 == sanitized ) ]] || { echo 'usage: backend-watch-build.sh OUT [sanitized]' >&2;exit 2; }
out=$1;mkdir -p "$out"
coax=${COAX:-$root/coax/bin/coax};cc=${COAX_CC:-cc}
flags=(-std=c11 -O1 -g -Wno-unused-function -Wno-unused-parameter)
if [[ ${2:-} == sanitized ]];then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
"$coax" c "$root/psionic/lib/ops/backend-watch.coax" --library --header "$out/backend-watch.h" > "$out/backend-watch.c"
"$cc" "${flags[@]}" -I"$out" "$out/backend-watch.c" "$root/psionic/platform/backend-watch-face.c" -lm -o "$out/backend-watch"

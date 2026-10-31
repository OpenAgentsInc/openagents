#!/usr/bin/env bash
# Build verse-kms for one realm: Coax to C, then the host C compiler.
#   verse/kms/build.sh [OUT_DIR]       generate C and build OUT_DIR/verse-kms
#   VERSE_KMS_GEN_ONLY=1 verse/kms/build.sh OUT_DIR
#                                      generate C only (for a Linux box without coax)
# On Linux it links libdrm (pkg-config libdrm; on NixOS run it in
# `nix-shell -p gcc pkg-config libdrm`). On macOS it builds the headless mode.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
out="${1:-$here/build}"
mkdir -p "$out"
if [[ -f "$out/starcatch.c" && -z "${VERSE_KMS_REGEN:-}" && ! -x "$root/coax/bin/coax" ]]; then
  echo "using the C already in $out"
else
  coax="$root/coax/bin/coax"
  stage="$(mktemp -d "${TMPDIR:-/tmp}/verse-kms.XXXXXX")"
  trap 'rm -rf "$stage"' EXIT
  # starcatch.coax loads font.coax from the web app, as it does on the site.
  cp "$root/verse/realm-template/starcatch.coax" "$root/apps/openagents-web/font.coax" "$here/starcatch-kms.coax" "$stage/"
  (cd "$stage" && "$coax" test starcatch-kms.coax > /dev/null && "$coax" c starcatch-kms.coax --library --header "$out/starcatch.h" > "$out/starcatch.c")
  "$coax" c "$here/starcatch-light.coax" --library --header "$out/starcatch-light.h" > "$out/starcatch-light.c"
  cp "$here/kms.c" "$here/realm.h" "$here/starcatch-native.c" "$out/"
fi
[[ -n "${VERSE_KMS_GEN_ONLY:-}" ]] && exit 0
cc="${CC:-cc}"
flags=(-std=c11 -O2 -g -pthread -I "$out")
libs=(-lm)
if [[ "$(uname)" == Linux ]]; then
  flags+=($(pkg-config --cflags libdrm))
  libs+=($(pkg-config --libs libdrm))
fi
"$cc" "${flags[@]}" "$out/kms.c" "$out/starcatch-native.c" "$out/starcatch.c" "$out/starcatch-light.c" "${libs[@]}" -o "$out/verse-kms"
echo "$out/verse-kms"

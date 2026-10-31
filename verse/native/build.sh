#!/usr/bin/env bash
# A realm natively: its Coax to C (as coax/web/wasm.sh does for wasm32),
# its browser face, and this directory's host, in OUT.
#   verse/native/build.sh REALM OUT     generate the C (any machine with coax)
# On Linux it then builds OUT/realm-run with gcc, without PIE, so every
# pointer the face hands back fits its 32 bits (realm-host.c). The realms:
#   everglade   apps/openagents-web/everglade.coax and everglade-face.c, as
#               apps/openagents-web/build.sh links them for the browser
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
realm="${1:?usage: build.sh REALM OUT}"
out="${2:?usage: build.sh REALM OUT}"
web="$root/apps/openagents-web"
case "$realm" in
  everglade) app="$web/everglade.coax"; face="$web/everglade-face.c"
    libraries=("$root/verse/player.coax" "$root/verse/grid.coax" "$root/verse/everglade/river.coax" "$root/verse/everglade/guard-guide-policy.coax") ;;
  *) echo "build.sh: no realm $realm" >&2; exit 2 ;;
esac
mkdir -p "$out"
coax="${COAX:-$root/coax/bin/coax}"
if [[ -x "$coax" ]]; then
  relative() { echo "${1#"$root"/}"; }
  (cd "$root" && "$coax" c "$(relative "$app")" --library --header "$out/$realm.h") > "$out/$realm.c" &
  for library in "${libraries[@]}"; do
    base="$(basename "$library" .coax)"; base="$(printf '%s' "${base:0:1}" | tr a-z A-Z)${base:1}"
    (cd "$root" && "$coax" c "$(relative "$library")" --library --header "$out/$base.h") > "$out/$base.c" &
  done
  wait
  cp "$face" "$web"/*-face.h "$out/"
  cp "$here"/face.h "$here"/realm-host.h "$here"/realm-host.c "$here"/realm-run.c "$out/"
  echo "$(basename "$face") $(cd "$out" && ls ./*.c | grep -v -e face -e realm-host -e realm-run | xargs -n1 basename | tr '\n' ' ')" > "$out/sources"
fi
[[ "$(uname)" == Linux ]] || exit 0
read -r -a sources < "$out/sources"
# The browser's flags: no fast math and no contraction, so it computes what the checks computed.
flags=(-std=gnu11 -O2 -no-pie -ffp-contract=off -fno-fast-math -Wno-attributes -Wno-unused-function -Wno-unused-variable
       -Wno-unused-but-set-variable -w -I "$out")
objects=()
for s in "${sources[@]}" realm-host.c; do
  gcc "${flags[@]}" -c "$out/$s" -o "$out/${s%.c}.o" & objects+=("$out/${s%.c}.o")
done
wait
gcc "${flags[@]}" "$out/realm-run.c" "${objects[@]}" -lm -o "$out/realm-run"
echo "$out/realm-run"

#!/usr/bin/env bash
# Ice Cream Man's models: the Coax checks, then every case built by the
# demo's own C++ (mesh-oracle.cpp over ~/work/aethyr-demo/src, read only) and
# compared vertex by vertex with the Coax port (mesh-check.c), under
# sanitizers. --table rewrites fixtures/mesh-oracle.coax from the C++.
#   verse/icecream/test-mesh.sh [--table]
set -euo pipefail
cd "$(dirname "$0")/../.."
demo="${AETHYR_DEMO:-$HOME/work/aethyr-demo}"
work=$(mktemp -d "${TMPDIR:-/tmp}/verse-icecream-mesh.XXXXXX")
trap 'rm -rf "$work"' EXIT
cc="${CC:-/usr/bin/clang}"
cxx="${CXX:-/usr/bin/clang++}"
san=(-O1 -g -ffp-contract=off -fsanitize=address,undefined -fno-omit-frame-pointer)
coax="${COAX:-coax/bin/coax}"
checks() { "$coax" test verse/icecream/mesh.coax --library --steps 4000000000; }
if [[ ! -d "$demo/src/icecream" ]]; then checks; echo "mesh: no $demo, the C++ comparison is skipped"; exit 0; fi
# rules.cpp's ECS hookup (icecream::install) is never called here, so its symbols may stay unresolved.
"$cxx" -std=c++20 "${san[@]}" -I "$demo/src" "$demo/src/icecream/rules.cpp" "$demo/src/icecream/mesh.cpp" \
  verse/icecream/mesh-oracle.cpp -Wl,-undefined,dynamic_lookup -o "$work/oracle"
"$work/oracle" > "$work/oracle.bin"
"$coax" c verse/icecream/mesh.coax --library --header "$work/IceCream.h" > "$work/IceCream.c"
"$cc" -std=c11 "${san[@]}" -Wno-unused-value -I "$work" "$work/IceCream.c" verse/icecream/mesh-check.c -lm -o "$work/check"
if [[ "${1:-}" == "--table" ]]; then
  "$work/check" "$work/oracle.bin" --table verse/icecream/fixtures/mesh-oracle.coax
else
  "$work/check" "$work/oracle.bin"
fi
checks

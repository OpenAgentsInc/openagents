#!/usr/bin/env bash
# The Metal host's contract on this Mac's GPU, with the minimum compute
# consumer (psionic/fixtures/compute/consumer.coax). SKIP without Metal.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc}
if [[ $(uname) != Darwin ]]; then echo "SKIP: Metal needs macOS"; exit 0; fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-metal-host.XXXXXX");trap 'rm -rf "$out"' EXIT
"$coax" metal-compute psionic/fixtures/compute/consumer.coax --release > "$out/consumer.metal"
"$coax" c psionic/fixtures/compute/consumer.coax --library --release --header "$out/consumer.h" > "$out/consumer.c"
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -I"$out" -Ipsionic/platform)
"$cc" "${flags[@]}" -c "$out/consumer.c" -o "$out/consumer.o"
"$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -Wall -Wextra -Werror -c psionic/platform/metal-host.m -o "$out/metal-host.o"
"$cc" "${flags[@]}" psionic/tests/metal_host.c "$out/consumer.o" "$out/metal-host.o" -framework Metal -framework Foundation -lm -o "$out/metal-host"
"$out/metal-host" "$out/consumer.metal"

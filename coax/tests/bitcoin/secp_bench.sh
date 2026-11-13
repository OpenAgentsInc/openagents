#!/usr/bin/env bash
# Firm reason: 2 (independent oracle: libsecp256k1 decides and is timed
# beside Coax in one process). Times Coax's secp256k1 verifiers and signers
# against libsecp256k1 v0.8.0 (issue #13450).
#
#   coax/tests/bitcoin/secp_bench.sh [--release | --checked] [SIGN_COUNT]
#   coax/tests/bitcoin/secp_bench.sh [--release | --checked] --wasm
#   coax/tests/bitcoin/secp_bench.sh [--release | --checked] 0 agree COUNT
#
# --release (default) is `coax lib --release`: wrapping integers, cc -O2.
# --checked is `coax lib --opt 2`: checked integers, cc -O2. The oracle is
# built once with:
#   CC=/usr/bin/clang python3 -I tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed
# Each row is the best of 7 passes over 256 seeded keys and signatures, in
# microseconds per call. Signing in Coax is slow, so SIGN_COUNT (default 256)
# can lower the number of signatures timed. --wasm builds the same C for
# wasm32 (coax/web/toolchain.sh's clang) and times the two verifiers in Node
# on the same keys and signatures; libsecp256k1 is not built for Wasm here.
# `0 agree COUNT` times nothing: it compares both libraries' decisions on
# COUNT more seeded cases, most of them changed signatures, keys and hashes.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
mode=(--release); label=release
case "${1:-}" in
  --release) shift ;;
  --checked) mode=(--opt 2 --no-debug-info); label=checked; shift ;;
esac
oracle="$root/tools/coax-nostr/fuzz/.cache"
[[ -f "$oracle/build/libsecp_oracle.dylib" || -f "$oracle/build/libsecp_oracle.so" ]] || { echo "secp_bench.sh: build the oracle first (see this file's header)" >&2; exit 2; }
lib="$(ls "$oracle"/build/libsecp_oracle.* | head -1)"
out="${SECP_BENCH_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/secp-bench.XXXXXXXX")}"
cc="${COAX_CC:-/usr/bin/cc}"
"$root/coax/bin/coax" header "$here/secp_bench.coax" -o "$out/secp_bench.h"
COAX_CC="$cc" "$root/coax/bin/coax" lib "$here/secp_bench.coax" "${mode[@]}" -o "$out/secp_bench.o"
"$cc" -std=c11 -O2 -I"$out" -I"$oracle/source/include" "$here/secp_bench.c" "$out/secp_bench.o" "$lib" -lm -o "$out/secp_bench"
echo "build: $label; compiler: $("$cc" --version | head -1); machine: $(uname -sm)"
if [[ "${1:-}" == --wasm ]]; then
  # The same C for wasm32, timed in Node on the native program's signatures.
  source "$root/coax/web/toolchain.sh"
  "$out/secp_bench" 0 dump > "$out/fixture.txt"
  "$root/coax/bin/coax" c "$here/secp_bench.coax" --library "${mode[@]}" > "$out/secp_bench_wasm_lib.c"
  "$COAXIAL_CLANG" --target=wasm32-wasip1 --sysroot="$COAXIAL_SYSROOT" -resource-dir="$COAXIAL_RESOURCES" \
    -std=c11 -O2 -I"$out" -mexec-model=reactor "$out/secp_bench_wasm_lib.c" "$here/secp_bench_wasm.c" \
    -Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections \
    -o "$out/secp_bench.wasm"
  echo "microseconds     Coax (Wasm)"
  node "$here/secp_bench_wasm.cjs" "$out/secp_bench.wasm" "$out/fixture.txt"
  exit 0
fi
"$out/secp_bench" "$@"

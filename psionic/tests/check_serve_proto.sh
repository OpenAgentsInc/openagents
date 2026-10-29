#!/usr/bin/env bash
# The /v1/systemone protobuf wire: Coax checks with independent vectors, then
# native answers decoded by the official protobuf runtime. Needs PROTO_PYTHON
# (proto/tools/requirements.txt) for the oracle; no model, service or GPU.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
python=${PROTO_PYTHON:?set PROTO_PYTHON to a Python with protobuf 5.29.3 (see proto/README.md)}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-serve-proto.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$python" -I psionic/tests/serve_proto_oracle.py --coax > "$out/vectors.coax"
# The checked-in vectors must be the oracle's current ones.
awk '/^\/\/ begin generated vectors$/{f=1;next}/^\/\/ end generated vectors$/{f=0}f' psionic/lib/serve-proto.coax | cmp - "$out/vectors.coax"
"$python" -I psionic/tests/serve_proto_oracle.py --header > "$out/serve-proto-oracle.h"
for mode in debug release; do
    flags=(); opt=-O0
    if [[ $mode == release ]]; then flags=(--release); opt=-O2; fi
    "$coax" test psionic/lib/serve-proto.coax "${flags[@]}"
    "$coax" c psionic/lib/serve-proto.coax --library --header "$out/serve-proto.h" "${flags[@]}" > "$out/serve-proto.c"
    for native in plain sanitized; do
        san=()
        if [[ $native == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-unused-variable \
            "${san[@]}" -I"$out" -Ipsionic/platform "$out/serve-proto.c" psionic/tests/serve_proto.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native" > "$out/answers.hex"
        "$python" -I psionic/tests/serve_proto_oracle.py --check-answers "$out/answers.hex"
    done
done
echo "Protobuf wire passes: independent request vectors, native answers decoded by the official runtime, debug/release, ASan/UBSan"

#!/usr/bin/env bash
set -euo pipefail
# New public-API consumer only. No canonical learning or old model gates.
reader_root=$(cd "$(dirname "$0")/../.." && pwd)
reader_out=${1:?usage: COAX=/qualified/coax bash check_residual_fresh_reader.sh NEW_OUT_DIR}
reader_coax=${COAX:?set COAX to a qualified compiler}
reader_cc=${CC:-clang}
if [[ -e "$reader_out" ]]; then
    echo "output path must be unused: $reader_out" >&2
    exit 2
fi
mkdir -p "$reader_out"
reader_out=$(cd "$reader_out" && pwd)
cd "$reader_root"
reader_source=psionic/examples/residual-fresh-reader.coax
"$reader_coax" test "$reader_source" --only 'fresh reader metadata' > "$reader_out/metadata.log" 2>&1
"$reader_coax" test "$reader_source" --only 'fresh reader nonzero' > "$reader_out/inference-state.log" 2>&1
"$reader_coax" c "$reader_source" > "$reader_out/consumer.c"
"$reader_cc" -std=c11 -O0 -fno-fast-math -ffp-contract=off "$reader_out/consumer.c" -lm -o "$reader_out/consumer-O0"
"$reader_cc" -std=c11 -O2 -fno-fast-math -ffp-contract=off "$reader_out/consumer.c" -lm -o "$reader_out/consumer-O2"
"$reader_cc" -std=c11 -O1 -g -fno-omit-frame-pointer -fsanitize=address,undefined -fno-fast-math -ffp-contract=off "$reader_out/consumer.c" -lm -o "$reader_out/consumer-asan-ubsan"
"$reader_out/consumer-O0" > "$reader_out/O0.stdout"
"$reader_out/consumer-O2" > "$reader_out/O2.stdout"
# LeakSanitizer is unavailable on the reader's recorded Apple host.
ASAN_OPTIONS=detect_leaks=0 UBSAN_OPTIONS=halt_on_error=1 "$reader_out/consumer-asan-ubsan" > "$reader_out/asan-ubsan.stdout"
cmp "$reader_out/O0.stdout" "$reader_out/O2.stdout"
cmp "$reader_out/O0.stdout" "$reader_out/asan-ubsan.stdout"
cat "$reader_out/O0.stdout"

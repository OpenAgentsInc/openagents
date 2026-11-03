#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: emits char-gpt's C and CUDA with coax, then compiles the effects driver with the host C compiler)
# Usage: psionic/tests/char_gpt_build.sh OUT_DIR
# Produces OUT_DIR/char-gpt (the driver) and OUT_DIR/char-gpt-kernels.cu.
set -euo pipefail
cd "$(dirname "$0")/../.."
out=$1
mkdir -p "$out"
coax=${COAX:-coax/bin/coax}
"$coax" c psionic/tests/char-gpt-native.coax --library --release --header "$out/char-gpt.h" > "$out/char-gpt.c"
"$coax" cuda psionic/lib/transformer/char-gpt-kernels.coax --release > "$out/char-gpt-kernels.cu"
${COAX_CC:-cc} -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas \
    -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation \
    -I"$out" -Ipsionic/platform "$out/char-gpt.c" psionic/platform/cuda-host.c psionic/tests/char_gpt_device.c \
    -ldl -lm -o "$out/char-gpt"

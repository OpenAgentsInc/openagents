#!/usr/bin/env bash
# Firm reason: 5 (thin Docker/musl cross-compile build wrapper around coax c and gcc; holds no agent logic)
# Build static Linux musl binary of coder/tui/oa-worker.coax for native Terminal-Bench execution.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
OUT_DIR="${FLAWED_LINUX_OUT_DIR:-$HOME/.openagents/tbench/flawed-linux-build}"
mkdir -p "$OUT_DIR"

export DOCKER_HOST="${DOCKER_HOST:-unix://$HOME/.colima/oa-agent0/docker.sock}"

echo "[build_flawed_linux] Emitting Coax C..."
"$REPO/coax/bin/coax" c "$REPO/coax/lib/crypto/entropy.coax" --library --header "$OUT_DIR/entropy-policy.h" > "$OUT_DIR/entropy.c"
"$REPO/coax/bin/coax" c "$REPO/coder/tui/oa-worker.coax" --library --release > "$OUT_DIR/flawedworker.c"

cat > "$OUT_DIR/flawedworker_wrap.c" <<'EOF'
#include <stdio.h>
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#undef stdin
#undef stdout
#undef stderr
#include "flawedworker.c"
EOF

if ! docker image inspect oa-flawed-musl-builder >/dev/null 2>&1; then
  echo "[build_flawed_linux] Building oa-flawed-musl-builder image..."
  docker build -t oa-flawed-musl-builder - <<'DOCKER'
FROM alpine:3.21
RUN apk add --no-cache gcc musl-dev
DOCKER
fi

echo "[build_flawed_linux] Compiling static Linux arm64 binary..."
docker run --rm \
  -v "$OUT_DIR:/work" \
  -v "$REPO/coax/platform:/plat:ro" \
  oa-flawed-musl-builder sh -c '
    gcc -static -std=c11 -O2 -ffp-contract=off -fno-fast-math -fno-builtin-pow \
      -I/work -I/plat \
      -o /work/flawedworker-linux-arm64 \
      /work/flawedworker_wrap.c \
      /plat/tool-native.c /plat/process-native.c /plat/store-file-native.c \
      /plat/read-file-native.c /plat/socket-native.c /plat/entropy-native.c \
      /plat/entropy-foreign.c /plat/agent-signer-none.c \
      /work/entropy.c -lm -lpthread
  '

ls -lh "$OUT_DIR/flawedworker-linux-arm64"

#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
work="$(mktemp -d)";trap 'rm -rf "$work"' EXIT
"${COAX_CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  psionic/tests/pylon/demo_label_host.c -lpthread -o "$work/test"
"$work/test"
echo 'demo label file metadata, bounds, symlink/FIFO refusal and buffer clearing passed'

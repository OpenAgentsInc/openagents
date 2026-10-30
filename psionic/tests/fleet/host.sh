#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
work="$(mktemp -d)";trap 'rm -rf "$work"' EXIT
"${COAX_CC:-cc}" -std=c11 -O1 -g -Wall -Wextra -Werror -fsanitize=address,undefined \
  -fno-omit-frame-pointer psionic/tests/fleet/host.c -lpthread -o "$work/test"
"$work/test"

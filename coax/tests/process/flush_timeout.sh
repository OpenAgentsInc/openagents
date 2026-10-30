#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
out="$(mktemp -d "${TMPDIR:-/tmp}/coax-process-flush.XXXXXXXX")"
trap 'rm -rf "$out"' EXIT
proc_cc="${COAX_CC:-${CC:-}}"
if [[ -z "$proc_cc" ]]; then
  if [[ "$(uname -s)" == Linux ]] && command -v gcc >/dev/null 2>&1; then proc_cc=gcc; else proc_cc=cc; fi
fi
"$proc_cc" -std=c11 -Wall -Wextra -Werror -pthread -O1 -g \
  -fsanitize=address,undefined -fno-omit-frame-pointer "$here/flush_timeout.c" -o "$out/check"
"$out/check"


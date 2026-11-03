#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles confine-check.coax and its C with ASan and UBSan, then runs it)
# The file tools' confinement on a real temporary directory (confine.coax,
# confine-host.coax, confine-native.c), with openat2 and with the by-hand
# walk. Also: coax test coax/web/agent/confine.coax --library.
set -euo pipefail
cd "$(dirname "$0")/../../.."
work=$(mktemp -d "${TMPDIR:-/tmp}/confine-check.XXXXXX")
trap 'rm -rf "$work"' EXIT
coax/bin/coax c coax/web/agent/confine-check.coax > "$work/check.c"
"${CC:-cc}" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=undefined -fno-omit-frame-pointer \
  -D_DEFAULT_SOURCE "$work/check.c" coax/web/agent/confine-native.c coax/web/agent/confine-check-native.c -lm -o "$work/check"
TMPDIR="$work" "$work/check"

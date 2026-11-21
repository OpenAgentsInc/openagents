#!/usr/bin/env bash
# Firm reason: 5 (serves the generated player on the local loopback).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
port="${1:-8828}"
printf 'Play SPLICEBOTS at http://127.0.0.1:%s/\n' "$port"
exec python3 -m http.server --bind 127.0.0.1 --directory "$here/player" "$port"

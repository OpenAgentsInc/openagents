#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around coax c).
# Checks that nip98.coax emits C11 cleanly; api-sources.sh and deploy.sh
# generate nip98.generated.{c,h} at build time (#13849).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
cd "$repo"
coax=${COAX:-$repo/coax/bin/coax}
[[ $# == 1 && ( $1 == --check || $1 == --generate || $1 == --check-inputs ) ]] || { echo 'usage: nip98.sh --check|--generate|--check-inputs' >&2; exit 2; }
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
"$coax" c coax/web/api/nip98.coax --run-steps 250000000 --library --header "$work/nip98.generated.h" > "$work/nip98.generated.c"
echo 'nip98: Coax/C build-time emission ok'

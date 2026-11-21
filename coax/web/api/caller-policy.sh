#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around coax c).
# Checks that caller-policy.coax emits C11 cleanly; api-sources.sh and deploy.sh
# generate caller-policy.generated.{c,h} at build time (#13849).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
cd "$repo"
coax=${COAX:-$repo/coax/bin/coax}
[[ $# == 1 && ( $1 == --check || $1 == --generate || $1 == --check-inputs ) ]] || { echo 'usage: caller-policy.sh --check|--generate|--check-inputs' >&2; exit 2; }
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
"$coax" c coax/web/api/caller-policy.coax --library --header "$work/caller-policy.generated.h" > "$work/caller-policy.generated.c"
echo 'caller-policy: Coax/C build-time emission ok'

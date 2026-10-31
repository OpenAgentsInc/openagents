#!/usr/bin/env bash
# Firm reason: 4 (bootstrap seed: regenerates a Coax C seed with a trusted compiler)
# Optional second argument retains this invocation's check/mutation/source receipts.
set -euo pipefail
cd "$(dirname "$0")"
[[ $# -le 2 ]] || { echo 'usage: refresh-check-domains-seed.sh [TRUSTED_COMPILER [RECEIPT_DIR]]' >&2; exit 2; }
compiler=${1:-bin/coax}
receipts=${2:-}
work=$(mktemp -d "${TMPDIR:-/tmp}/coax-check-domains.XXXXXX")
cleanup() {
  if [[ -n "$receipts" ]]; then
    mkdir -p "$receipts"
    for name in domain-test.json domain-test.log domain-mutation.json domain-mutation.log check-domains.c check-domains.h; do
      if [[ -f "$work/$name" ]]; then cp "$work/$name" "$receipts/$name"; fi
    done
  fi
  find "$work" -depth -delete
}
trap cleanup EXIT
"$compiler" test lib/compiler/check-domains.coax --library --evidence "$work/domain-test.json" > "$work/domain-test.log" 2>&1
"$compiler" mutate lib/compiler/check-domains.coax --library --root-only --evidence "$work/domain-mutation.json" > "$work/domain-mutation.log" 2>&1
"$compiler" c lib/compiler/check-domains.coax --library --header "$work/check-domains.h" > "$work/check-domains.c"
mv "$work/check-domains.c" bootstrap/check-domains.c
mv "$work/check-domains.h" bootstrap/check-domains.h
if [[ -n "$receipts" ]]; then
  cp bootstrap/check-domains.c "$receipts/check-domains.c"
  cp bootstrap/check-domains.h "$receipts/check-domains.h"
fi

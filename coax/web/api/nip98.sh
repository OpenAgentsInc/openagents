#!/usr/bin/env bash
# The checked C of nip98.coax (the API's NIP-98 check) for builds without the
# Coax compiler. The image build regenerates it from source and compares.
# Firm reason: 5 (a thin wrapper around coax c, cmp and shasum).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
cd "$repo"
coax=${COAX:-$repo/coax/bin/coax}
[[ $# == 1 && ( $1 == --check || $1 == --generate || $1 == --check-inputs ) ]] || { echo 'usage: nip98.sh --check|--generate|--check-inputs' >&2; exit 2; }
if [[ $1 == --check-inputs ]]; then shasum -a 256 -c coax/web/api/nip98.sha256; exit; fi
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
"$coax" c coax/web/api/nip98.coax --run-steps 250000000 --library --header "$work/nip98.generated.h" > "$work/nip98.generated.c"
for suffix in c h; do
  if [[ $1 == --generate ]]; then cp "$work/nip98.generated.$suffix" "$here/nip98.generated.$suffix"
  else cmp "$work/nip98.generated.$suffix" "$here/nip98.generated.$suffix" || { echo 'nip98: regenerate the C cache from its Coax source' >&2; exit 1; }; fi
done
if [[ $1 == --generate ]]; then
  { printf '# Qualified compiler SHA-256: '; shasum -a 256 "$coax" | cut -d' ' -f1
    shasum -a 256 coax/web/api/nip98.coax coax/web/api/nip98_vectors.coax coax/web/api/nip98_testkit.coax coax/web/api/nip98.generated.c coax/web/api/nip98.generated.h
  } > "$here/nip98.sha256"
fi
echo 'nip98: Coax/C cache matches'

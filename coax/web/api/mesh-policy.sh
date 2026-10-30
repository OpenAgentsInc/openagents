#!/usr/bin/env bash
# Explicit checked C cache for the existing C-only API image. Product rules
# remain Coax. No compiler or Python is needed inside the production container.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
cd "$repo"
coax=${COAX:-$repo/coax/bin/coax}
[[ $# == 1 && ( $1 == --check || $1 == --generate || $1 == --check-inputs ) ]] || { echo 'usage: mesh-policy.sh --check|--generate|--check-inputs' >&2; exit 2; }
if [[ $1 == --check-inputs ]]; then shasum -a 256 -c coax/web/api/mesh-policy.sha256; exit; fi
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
"$coax" c coax/web/api/mesh-policy.coax --library --header "$work/mesh-policy.generated.h" > "$work/mesh-policy.generated.c"
for suffix in c h; do
  if [[ $1 == --generate ]]; then cp "$work/mesh-policy.generated.$suffix" "$here/mesh-policy.generated.$suffix"
  else cmp "$work/mesh-policy.generated.$suffix" "$here/mesh-policy.generated.$suffix" || { echo 'mesh: regenerate the C cache from its Coax source' >&2; exit 1; }; fi
done
if [[ $1 == --generate ]]; then
  { printf '# Qualified compiler SHA-256: '; shasum -a 256 "$coax" | cut -d' ' -f1
    shasum -a 256 coax/web/api/mesh-policy.coax coax/lib/nostr/json.coax coax/web/api/mesh-policy.generated.c coax/web/api/mesh-policy.generated.h
  } > "$here/mesh-policy.sha256"
fi
echo 'mesh: Coax/C cache matches'

#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around coax c, cmp and shasum). The checked C of
# caller-policy.coax for builds without the Coax compiler. Product rules
# remain Coax. No compiler or Python is needed inside the production container.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
cd "$repo"
coax=${COAX:-$repo/coax/bin/coax}
[[ $# == 1 && ( $1 == --check || $1 == --generate || $1 == --check-inputs ) ]] || { echo 'usage: caller-policy.sh --check|--generate|--check-inputs' >&2; exit 2; }
if [[ $1 == --check-inputs ]]; then shasum -a 256 -c coax/web/api/caller-policy.sha256; exit; fi
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
"$coax" c coax/web/api/caller-policy.coax --library --header "$work/caller-policy.generated.h" > "$work/caller-policy.generated.c"
for suffix in c h; do
  if [[ $1 == --generate ]]; then cp "$work/caller-policy.generated.$suffix" "$here/caller-policy.generated.$suffix"
  else cmp "$work/caller-policy.generated.$suffix" "$here/caller-policy.generated.$suffix" || { echo 'caller-policy: regenerate the C cache from its Coax source' >&2; exit 1; }; fi
done
if [[ $1 == --generate ]]; then
  { printf '# Qualified compiler SHA-256: '; shasum -a 256 "$coax" | cut -d' ' -f1
    shasum -a 256 coax/web/api/caller-policy.coax coax/web/api/caller-policy.generated.c coax/web/api/caller-policy.generated.h
  } > "$here/caller-policy.sha256"
fi
echo 'caller-policy: Coax/C cache matches'

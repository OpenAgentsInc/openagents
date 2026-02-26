#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OWNERSHIP_DOC="$ROOT_DIR/docs/OWNERSHIP.md"

if [[ ! -f "$OWNERSHIP_DOC" ]]; then
    printf 'Missing ownership authority doc: %s\n' "$OWNERSHIP_DOC" >&2
    exit 1
fi

crate_tomls=()
while IFS= read -r toml; do
    crate_tomls+=("$toml")
done < <(find "$ROOT_DIR/crates" -type f -name Cargo.toml | sort)

violations=0

# Reusable crates must not path-depend on apps.
for toml in "${crate_tomls[@]}"; do
    if rg -n 'path\\s*=\\s*".*apps/' "$toml" >/dev/null 2>&1; then
        printf 'Boundary violation: crate path dependency on apps in %s\n' "$toml" >&2
        rg -n 'path\\s*=\\s*".*apps/' "$toml" >&2 || true
        violations=1
    fi
done

# WGPUI should remain independent of product crates.
wgpui_toml="$ROOT_DIR/crates/wgpui/Cargo.toml"
if rg -n 'nostr\\s*=|openagents-spark\\s*=|spark\\s*=' "$wgpui_toml" >/dev/null 2>&1; then
    printf 'Boundary violation: wgpui depends on product/domain crate(s) in %s\n' "$wgpui_toml" >&2
    rg -n 'nostr\\s*=|openagents-spark\\s*=|spark\\s*=' "$wgpui_toml" >&2 || true
    violations=1
fi

if [[ "$violations" -ne 0 ]]; then
    exit 1
fi

printf 'Ownership boundary check passed.\n'

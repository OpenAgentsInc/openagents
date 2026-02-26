#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROOT_TOML="$ROOT_DIR/Cargo.toml"

if [[ ! -f "$ROOT_TOML" ]]; then
    printf 'Missing workspace Cargo.toml at %s\n' "$ROOT_TOML" >&2
    exit 1
fi

WORKSPACE_DEPS=()
while IFS= read -r dep; do
    WORKSPACE_DEPS+=("$dep")
done < <(
    awk '
        /^\[workspace\.dependencies\]$/ { in_deps=1; next }
        /^\[/ && in_deps { exit }
        in_deps && $0 ~ /^[[:space:]]*[A-Za-z0-9_-]+[[:space:]]*=/ {
            line=$0
            sub(/^[[:space:]]*/, "", line)
            sub(/[[:space:]]*=.*/, "", line)
            print line
        }
    ' "$ROOT_TOML"
)

if [[ ${#WORKSPACE_DEPS[@]} -eq 0 ]]; then
    printf 'No [workspace.dependencies] entries found in %s\n' "$ROOT_TOML" >&2
    exit 1
fi

MEMBER_TOMLS=()
while IFS= read -r member_toml; do
    MEMBER_TOMLS+=("$member_toml")
done < <(find "$ROOT_DIR" -type f -name Cargo.toml ! -path "$ROOT_TOML" | sort)

if [[ ${#MEMBER_TOMLS[@]} -eq 0 ]]; then
    printf 'No member Cargo.toml files found under %s\n' "$ROOT_DIR" >&2
    exit 1
fi

missing=()
for dep in "${WORKSPACE_DEPS[@]}"; do
    pattern="^[[:space:]]*${dep}([.]workspace[[:space:]]*=|[[:space:]]*=[[:space:]]*\\{[^\\n}]*workspace[[:space:]]*=[[:space:]]*true)"
    if ! rg -q --pcre2 "$pattern" "${MEMBER_TOMLS[@]}"; then
        missing+=("$dep")
    fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
    printf 'Workspace dependency drift detected. Unused [workspace.dependencies] entries:\n' >&2
    for dep in "${missing[@]}"; do
        printf '  - %s\n' "$dep" >&2
    done
    exit 1
fi

printf 'Workspace dependency drift check passed (%s shared deps in active use).\n' "${#WORKSPACE_DEPS[@]}"

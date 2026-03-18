#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
MANIFEST=""
LISTING_FILE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --manifest)
            MANIFEST="${2:?missing value for --manifest}"
            shift 2
            ;;
        --file)
            LISTING_FILE="${2:?missing value for --file}"
            shift 2
            ;;
        *)
            printf 'Unknown argument: %s\n' "$1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$MANIFEST" || -z "$LISTING_FILE" ]]; then
    printf 'Usage: %s --manifest <path> --file <listing-template.json>\n' "$0" >&2
    exit 1
fi

AUTOPILOTCTL=(
    cargo run -p autopilot-desktop --bin autopilotctl --
    --manifest "$MANIFEST"
    --json
)

"${AUTOPILOTCTL[@]}" data-market seller-status
"${AUTOPILOTCTL[@]}" data-market draft-asset --file "$LISTING_FILE"
"${AUTOPILOTCTL[@]}" data-market preview-asset
"${AUTOPILOTCTL[@]}" data-market publish-asset --confirm
"${AUTOPILOTCTL[@]}" data-market snapshot

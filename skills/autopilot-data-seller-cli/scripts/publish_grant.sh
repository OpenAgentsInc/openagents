#!/usr/bin/env bash
set -euo pipefail

MANIFEST=""
GRANT_FILE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --manifest)
            MANIFEST="${2:?missing value for --manifest}"
            shift 2
            ;;
        --file)
            GRANT_FILE="${2:?missing value for --file}"
            shift 2
            ;;
        *)
            printf 'Unknown argument: %s\n' "$1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$MANIFEST" || -z "$GRANT_FILE" ]]; then
    printf 'Usage: %s --manifest <path> --file <grant-template.json>\n' "$0" >&2
    exit 1
fi

AUTOPILOTCTL=(
    cargo run -p autopilot-desktop --bin autopilotctl --
    --manifest "$MANIFEST"
    --json
)

"${AUTOPILOTCTL[@]}" data-market seller-status
"${AUTOPILOTCTL[@]}" data-market draft-grant --file "$GRANT_FILE"
"${AUTOPILOTCTL[@]}" data-market preview-grant
"${AUTOPILOTCTL[@]}" data-market publish-grant --confirm
"${AUTOPILOTCTL[@]}" data-market snapshot

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

PROFILE="phase_a_baseline_v1"
CHECK_ONLY=0
LIST_ONLY=0

usage() {
    cat <<USAGE
Usage:
  scripts/cad/parity-blocker-workflow.sh
  scripts/cad/parity-blocker-workflow.sh --check
  scripts/cad/parity-blocker-workflow.sh --profile <id>
  scripts/cad/parity-blocker-workflow.sh --list

Options:
  --check         Verify fixture lock and enforce blocker profile
  --profile <id>  Blocker profile to enforce (default: phase_a_baseline_v1)
  --list          Print available blocker profile IDs and exit
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)
            CHECK_ONLY=1
            shift
            ;;
        --profile)
            if [[ $# -lt 2 ]]; then
                printf 'missing value for --profile\n\n' >&2
                usage >&2
                exit 2
            fi
            PROFILE="$2"
            shift 2
            ;;
        --list)
            LIST_ONLY=1
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if (( LIST_ONLY == 1 )); then
    printf 'phase_a_baseline_v1\n'
    printf 'parity_complete_v1\n'
    exit 0
fi

cd "$ROOT_DIR"

ARGS=(--enforce-profile "$PROFILE")
if (( CHECK_ONLY == 1 )); then
    ARGS=(--check "${ARGS[@]}")
fi

cargo run -p openagents-cad --bin parity-risk-register -- "${ARGS[@]}"

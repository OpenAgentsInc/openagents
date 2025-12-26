#!/usr/bin/env bash
set -euo pipefail

bin_dir=""
target=""

usage() {
    echo "Usage: $0 --bin-dir <dir> [--target <openagents_path>]" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bin-dir)
            bin_dir="$2"
            shift 2
            ;;
        --target)
            target="$2"
            shift 2
            ;;
        --help|-h)
            usage
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage
            ;;
    esac
done

if [[ -z "$bin_dir" ]]; then
    echo "Error: --bin-dir is required" >&2
    usage
fi

if [[ -z "$target" ]]; then
    if command -v openagents >/dev/null 2>&1; then
        target="$(command -v openagents)"
    else
        echo "Error: openagents not found in PATH. Provide --target." >&2
        exit 1
    fi
fi

mkdir -p "$bin_dir"

for name in wallet marketplace autopilot autopilotd gitafter; do
    ln -sf "$target" "$bin_dir/$name"
done

echo "Legacy symlinks created in $bin_dir"

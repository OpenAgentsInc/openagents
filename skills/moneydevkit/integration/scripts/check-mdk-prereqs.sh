#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    printf 'Usage: %s <agent-wallet|checkout>\n' "$0" >&2
    exit 1
fi

mode="$1"

require_command() {
    local command_name="$1"
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'Missing required command: %s\n' "$command_name" >&2
        exit 1
    fi
}

check_node_20_plus() {
    require_command node
    local node_version major
    node_version="$(node --version)"
    major="${node_version#v}"
    major="${major%%.*}"
    if [[ "$major" -lt 20 ]]; then
        printf 'Node.js 20+ required, found %s\n' "$node_version" >&2
        exit 1
    fi
}

case "$mode" in
agent-wallet)
    check_node_20_plus
    require_command npx
    printf 'agent-wallet prereqs OK (node %s, npx present)\n' "$(node --version)"
    ;;
checkout)
    check_node_20_plus
    require_command npx
    if [[ -z "${MDK_ACCESS_TOKEN:-}" ]]; then
        printf 'MDK_ACCESS_TOKEN is required for checkout mode\n' >&2
        exit 1
    fi
    if [[ -z "${MDK_MNEMONIC:-}" ]]; then
        printf 'MDK_MNEMONIC is required for checkout mode\n' >&2
        exit 1
    fi
    printf 'checkout prereqs OK (node %s, required env vars present)\n' "$(node --version)"
    ;;
*)
    printf "Unsupported mode '%s'. Use agent-wallet or checkout.\n" "$mode" >&2
    exit 1
    ;;
esac

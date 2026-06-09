#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    printf 'Usage: %s <mcp|sdk|agent>\n' "$0" >&2
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

check_node() {
    require_command node
    require_command npx

    local node_version major
    node_version="$(node --version)"
    major="${node_version#v}"
    major="${major%%.*}"

    if [[ "$major" -lt 18 ]]; then
        printf 'Node.js 18+ required, found %s\n' "$node_version" >&2
        exit 1
    fi
}

require_neutron_creds() {
    if [[ -z "${NEUTRON_API_KEY:-}" ]]; then
        printf 'NEUTRON_API_KEY is required\n' >&2
        exit 1
    fi
    if [[ -z "${NEUTRON_API_SECRET:-}" ]]; then
        printf 'NEUTRON_API_SECRET is required\n' >&2
        exit 1
    fi
}

case "$mode" in
mcp)
    check_node
    require_neutron_creds
    printf 'MCP prereqs OK (node %s, neutron creds present)\n' "$(node --version)"
    ;;
sdk)
    check_node
    require_neutron_creds
    printf 'SDK prereqs OK (node %s, neutron creds present)\n' "$(node --version)"
    ;;
agent)
    check_node
    require_neutron_creds
    if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
        printf 'ANTHROPIC_API_KEY is required for agent mode\n' >&2
        exit 1
    fi
    if [[ -z "${WEBHOOK_SECRET:-}" ]]; then
        printf 'WEBHOOK_SECRET is required for agent mode\n' >&2
        exit 1
    fi
    printf 'Agent prereqs OK (node %s, neutron + anthropic + webhook env present)\n' "$(node --version)"
    ;;
*)
    printf "Unsupported mode '%s'. Use mcp, sdk, or agent.\n" "$mode" >&2
    exit 1
    ;;
esac

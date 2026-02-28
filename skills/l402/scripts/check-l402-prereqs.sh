#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    printf 'Usage: %s <buyer|seller|observer|full>\n' "$0" >&2
    exit 1
fi

mode="$1"
lat_dir="${LIGHTNING_AGENT_TOOLS_DIR:-$HOME/code/lightning-agent-tools}"

require_command() {
    local cmd="$1"
    if ! command -v "$cmd" >/dev/null 2>&1; then
        printf 'Missing required command: %s\n' "$cmd" >&2
        exit 1
    fi
}

require_file() {
    local file_path="$1"
    if [[ ! -f "$file_path" ]]; then
        printf 'Missing required file: %s\n' "$file_path" >&2
        exit 1
    fi
}

base_checks() {
    require_command bash
    require_command curl
    require_command jq
    require_command docker
    if [[ ! -d "$lat_dir" ]]; then
        printf 'lightning-agent-tools directory not found: %s\n' "$lat_dir" >&2
        exit 1
    fi
}

buyer_checks() {
    base_checks
    require_file "$lat_dir/skills/lnd/scripts/install.sh"
    require_file "$lat_dir/skills/lnget/scripts/install.sh"
    printf 'buyer prereqs OK (base + lnd + lnget scripts present)\n'
}

seller_checks() {
    base_checks
    require_command python3
    require_file "$lat_dir/skills/aperture/scripts/install.sh"
    require_file "$lat_dir/skills/aperture/scripts/setup.sh"
    require_file "$lat_dir/skills/macaroon-bakery/scripts/bake.sh"
    printf 'seller prereqs OK (base + aperture + bakery scripts present)\n'
}

observer_checks() {
    base_checks
    if ! command -v npx >/dev/null 2>&1 && ! command -v go >/dev/null 2>&1; then
        printf 'observer mode requires either npx (zero-install MCP) or go (source build)\n' >&2
        exit 1
    fi
    require_file "$lat_dir/skills/lightning-mcp-server/scripts/install.sh"
    require_file "$lat_dir/skills/lightning-mcp-server/scripts/configure.sh"
    printf 'observer prereqs OK (base + MCP scripts + npx/go)\n'
}

case "$mode" in
buyer)
    buyer_checks
    ;;
seller)
    seller_checks
    ;;
observer)
    observer_checks
    ;;
full)
    buyer_checks
    seller_checks
    observer_checks
    printf 'full prereqs OK\n'
    ;;
*)
    printf "Unsupported mode '%s'. Use buyer, seller, observer, or full.\n" "$mode" >&2
    exit 1
    ;;
esac

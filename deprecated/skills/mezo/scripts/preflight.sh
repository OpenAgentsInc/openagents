#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF' >&2
Usage: preflight.sh <testnet|mainnet> [env_file]

Defaults:
  env_file = ~/.config/openagents/mezo-agent.env
EOF
}

if [[ $# -lt 1 || $# -gt 2 ]]; then
    usage
    exit 1
fi

network="$1"
env_file="${2:-$HOME/.config/openagents/mezo-agent.env}"

if [[ "$network" != "testnet" && "$network" != "mainnet" ]]; then
    printf "Unsupported network '%s' (expected: testnet|mainnet)\n" "$network" >&2
    exit 1
fi

for cmd in bash curl cast sed; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        printf "Missing required command: %s\n" "$cmd" >&2
        exit 1
    fi
done

if [[ ! -f "$env_file" ]]; then
    printf "Env file not found: %s\n" "$env_file" >&2
    exit 1
fi

set -a
# shellcheck disable=SC1090
source "$env_file"
set +a

if [[ -z "${MEZO_PRIVATE_KEY:-}" ]]; then
    printf "MEZO_PRIVATE_KEY is required\n" >&2
    exit 1
fi

if [[ ! "$MEZO_PRIVATE_KEY" =~ ^0x[0-9a-fA-F]{64}$ ]]; then
    printf "Invalid MEZO_PRIVATE_KEY format (expected 0x + 64 hex chars)\n" >&2
    exit 1
fi

if ! signer_address="$(cast wallet address --private-key "$MEZO_PRIVATE_KEY" 2>/dev/null)"; then
    printf "Failed to derive signer address from MEZO_PRIVATE_KEY\n" >&2
    exit 1
fi

if [[ "$network" == "testnet" ]]; then
    expected_chain_id="${MEZO_TESTNET_CHAIN_ID:-31611}"
    rpc_candidates=(
        "${MEZO_TESTNET_RPC_URL:-https://rpc.test.mezo.org}"
    )
else
    expected_chain_id="${MEZO_MAINNET_CHAIN_ID:-31612}"
    rpc_candidates=(
        "${MEZO_MAINNET_RPC_URL:-https://rpc-http.mezo.boar.network}"
        "${MEZO_MAINNET_RPC_FALLBACK_1:-https://mainnet.mezo.public.validationcloud.io}"
        "${MEZO_MAINNET_RPC_FALLBACK_2:-https://mezo.drpc.org}"
    )
fi

probe_chain_id() {
    local rpc_url="$1"
    local payload response chain_id_hex

    payload='{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'
    if ! response="$(curl -sS --max-time 12 -X POST -H 'content-type: application/json' --data "$payload" "$rpc_url" 2>/dev/null)"; then
        return 1
    fi

    chain_id_hex="$(printf '%s' "$response" | sed -nE 's/.*"result"[[:space:]]*:[[:space:]]*"(0x[0-9a-fA-F]+)".*/\1/p')"
    if [[ -z "$chain_id_hex" ]]; then
        return 1
    fi

    printf '%s' "$((chain_id_hex))"
    return 0
}

selected_rpc=""
resolved_chain_id=""

printf 'Running preflight: network=%s env=%s\n' "$network" "$env_file"
printf 'Derived signer address: %s\n' "$signer_address"

for rpc in "${rpc_candidates[@]}"; do
    printf 'Checking RPC: %s\n' "$rpc"
    if ! chain_id="$(probe_chain_id "$rpc")"; then
        printf '  -> unavailable or invalid response\n'
        continue
    fi

    if [[ "$chain_id" != "$expected_chain_id" ]]; then
        printf '  -> chain id mismatch (expected=%s actual=%s)\n' "$expected_chain_id" "$chain_id"
        continue
    fi

    selected_rpc="$rpc"
    resolved_chain_id="$chain_id"
    printf '  -> ok (chain id %s)\n' "$chain_id"
    break
done

if [[ -z "$selected_rpc" ]]; then
    printf "No healthy RPC matched chain id %s for %s\n" "$expected_chain_id" "$network" >&2
    exit 1
fi

if ! balance_wei="$(cast balance "$signer_address" --rpc-url "$selected_rpc" 2>/dev/null)"; then
    printf "Failed to fetch signer balance on %s\n" "$selected_rpc" >&2
    exit 1
fi

printf 'Preflight OK\n'
printf '  network=%s\n' "$network"
printf '  rpc=%s\n' "$selected_rpc"
printf '  chain_id=%s\n' "$resolved_chain_id"
printf '  signer=%s\n' "$signer_address"
printf '  balance_wei=%s\n' "$balance_wei"

if [[ "$balance_wei" == "0" ]]; then
    printf 'WARNING: signer has zero balance on %s (%s)\n' "$network" "$selected_rpc" >&2
fi

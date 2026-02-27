#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    printf 'Usage: %s <rpc_url> [expected_chain_id_decimal]\n' "$0" >&2
    exit 1
fi

rpc_url="$1"
expected_chain_id="${2:-}"

payload='{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'
response="$(curl -sS -X POST -H 'content-type: application/json' --data "$payload" "$rpc_url")"

chain_id_hex="$(printf '%s' "$response" | sed -nE 's/.*"result"[[:space:]]*:[[:space:]]*"(0x[0-9a-fA-F]+)".*/\1/p')"
if [[ -z "$chain_id_hex" ]]; then
    printf 'Failed to parse chain id from RPC response: %s\n' "$response" >&2
    exit 1
fi

chain_id_dec="$((chain_id_hex))"
printf 'RPC %s -> chain_id_hex=%s chain_id_dec=%s\n' "$rpc_url" "$chain_id_hex" "$chain_id_dec"

if [[ -n "$expected_chain_id" && "$chain_id_dec" != "$expected_chain_id" ]]; then
    printf 'Chain id mismatch: expected=%s actual=%s\n' "$expected_chain_id" "$chain_id_dec" >&2
    exit 1
fi

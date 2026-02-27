#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    printf 'Usage: %s <app|spell|wallet|server>\n' "$0" >&2
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

require_bitcoin_rpc() {
    if ! bitcoin-cli getblockchaininfo >/dev/null 2>&1; then
        printf 'bitcoin-cli cannot reach bitcoind. Start bitcoind and load a wallet.\n' >&2
        exit 1
    fi
}

require_wasm_target() {
    if ! rustup target list --installed | grep -Fxq "wasm32-wasip1"; then
        printf 'Missing Rust target wasm32-wasip1. Run: rustup target add wasm32-wasip1\n' >&2
        exit 1
    fi
}

case "$mode" in
app)
    require_command charms
    require_command cargo
    require_command rustup
    require_wasm_target
    printf 'app prereqs OK (charms, cargo, rustup, wasm32-wasip1)\n'
    ;;
spell)
    require_command charms
    require_command bitcoin-cli
    require_command jq
    require_command envsubst
    require_bitcoin_rpc
    printf 'spell prereqs OK (charms + bitcoin-cli + jq + envsubst)\n'
    ;;
wallet)
    require_command charms
    require_command bitcoin-cli
    require_bitcoin_rpc
    printf 'wallet prereqs OK (charms + bitcoin-cli RPC access)\n'
    ;;
server)
    require_command charms
    require_command curl
    printf 'server prereqs OK (charms + curl)\n'
    ;;
*)
    printf "Unsupported mode '%s'. Use app, spell, wallet, or server.\n" "$mode" >&2
    exit 1
    ;;
esac

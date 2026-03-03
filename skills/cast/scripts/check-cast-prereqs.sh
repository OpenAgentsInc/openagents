#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
    printf 'Usage: %s <maker|taker|cancel|server>\n' "$0" >&2
    exit 1
fi

mode="$1"

require_command() {
    local name="$1"
    if ! command -v "$name" >/dev/null 2>&1; then
        printf 'Missing required command: %s\n' "$name" >&2
        exit 1
    fi
}

require_bitcoin_rpc() {
    if ! bitcoin-cli getblockchaininfo >/dev/null 2>&1; then
        printf 'bitcoin-cli cannot reach bitcoind. Start bitcoind and load a wallet.\n' >&2
        exit 1
    fi
}

case "$mode" in
maker)
    require_command charms
    require_command bitcoin-cli
    require_command jq
    require_command curl
    require_command envsubst
    require_command scrolls-nonce
    require_command sign-txs
    require_bitcoin_rpc
    printf 'maker prereqs OK (charms, bitcoin-cli RPC, jq, curl, envsubst, scrolls-nonce, sign-txs)\n'
    ;;
taker)
    require_command charms
    require_command bitcoin-cli
    require_command jq
    require_command curl
    require_command envsubst
    require_command scrolls-nonce
    require_command sign-txs
    require_bitcoin_rpc
    printf 'taker prereqs OK (charms, bitcoin-cli RPC, jq, curl, envsubst, scrolls-nonce, sign-txs)\n'
    ;;
cancel)
    require_command charms
    require_command cancel-msg
    require_command jq
    require_command envsubst
    printf 'cancel prereqs OK (charms, cancel-msg, jq, envsubst)\n'
    ;;
server)
    require_command charms
    require_command curl
    require_command jq
    printf 'server prereqs OK (charms, curl, jq)\n'
    ;;
*)
    printf "Unsupported mode '%s'. Use maker, taker, cancel, or server.\n" "$mode" >&2
    exit 1
    ;;
esac

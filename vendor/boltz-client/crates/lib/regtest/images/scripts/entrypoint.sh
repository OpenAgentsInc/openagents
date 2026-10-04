#!/usr/bin/env bash
set -xe

DIRECTORIES=(
    "/root/.lightning-1"
    "/root/.lightning-2"
    "/root/.lnd-1"
    "/root/.lnd-2"
    "/root/.lnd-3"
    "/root/.bitcoin"
    "/root/.elements"
    "/root/.boltz-client"
    "/root/.boltz-backend"
)

cleanup() {
    echo 'SIGINT or SIGTERM received, exiting'
    exit 0
}

set_permissions() {
    for dir in "${DIRECTORIES[@]}"; do
        chmod -R 777 "$dir"
    done
}

main() {
    set_permissions
    trap cleanup SIGINT SIGTERM
    sleep infinity &
    wait
}

main

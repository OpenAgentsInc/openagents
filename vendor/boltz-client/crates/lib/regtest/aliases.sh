#!/usr/bin/env bash

run_in_container() {
    docker exec -it boltz-scripts bash -c "source /etc/profile.d/utils.sh && $(printf '%q ' "$@")"
}

alias bitcoin-cli-sim-client='run_in_container bitcoin-cli-sim-client'
alias bitcoin-cli-sim-server='run_in_container bitcoin-cli-sim-server'
alias elements-cli-sim-client='run_in_container elements-cli-sim-client'
alias elements-cli-sim-server='run_in_container elements-cli-sim-server'
alias boltz-client-cli-sim='run_in_container boltzcli-sim'
alias boltzcli-sim='run_in_container boltzcli-sim' #backwards compat
alias mine-block='bitcoin-cli-sim-client -generate 1 && elements-cli-sim-client -generate 1'

lightning-cli-sim() {
    run_in_container lightning-cli-sim "$@"
}

lncli-sim() {
    run_in_container lncli-sim "$@"
}

boltz-backend-cli-sim() {
    docker exec -it boltz-backend bash -c "/boltz-backend/target/release/boltzr-cli --grpc-certificates /boltz-data/certificates $(printf '%q ' "$@")"
}

lightning-cli-sim-client() {
    run_in_container lightning-cli-sim 1 "$@"
}

lightning-cli-sim-server() {
    run_in_container lightning-cli-sim 2 "$@"
}

lncli-sim-client() {
    run_in_container lncli-sim 1 "$@"
}

lncli-sim-server() {
    run_in_container lncli-sim 2 "$@"
}

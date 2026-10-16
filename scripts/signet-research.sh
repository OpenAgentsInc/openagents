#!/usr/bin/env bash
# A separate public-signet node for payment and LayerTree experiments.
set -euo pipefail
root=${SIGNET_RESEARCH_ROOT:?set a persistent scratch data directory}
name=oa-signet-research
image=bitcoin/bitcoin:30.2@sha256:2b1d8a8d23c67b426afa8cd5b3ffd66b8c4ebbbceef3e214df8627aaead1f517
owns() {
  [[ $(docker inspect --format '{{index .Config.Labels "openagents.issue"}}' "$name") == 11770 &&
     $(docker inspect --format '{{.Config.Image}}' "$name") == "$image" ]] || { echo 'The container is not this signet experiment.' >&2; exit 2; }
}
status() {
  owns
  docker exec "$name" bitcoin-cli -datadir=/data -signet getblockchaininfo
}
case ${1:-} in
  start)
    mkdir -p "$root"
    root=$(cd "$root" && pwd)
    if docker inspect "$name" >/dev/null 2>&1; then
      owns
      mount=$(docker inspect --format '{{range .Mounts}}{{if eq .Destination "/data"}}{{.Source}}{{end}}{{end}}' "$name")
      [[ $mount == "$root" ]] || { echo 'Use this experiment’s existing data directory.' >&2; exit 2; }
      docker start "$name" >/dev/null
    else
      docker run -d --name "$name" --label openagents.issue=11770 \
        --entrypoint bitcoind --user "$(id -u):$(id -g)" --network host -v "$root:/data" "$image" \
        -datadir=/data -signet -server -listen=0 -rpcbind=127.0.0.1 \
        -rpcallowip=127.0.0.1 -rpcport=38332 -disablewallet -txindex=1 \
        -blockfilterindex=1 -dbcache=768 -printtoconsole=0 >/dev/null
    fi
    for _ in $(seq 1 30); do
      if docker exec "$name" bitcoin-cli -datadir=/data -signet getblockchaininfo >/dev/null 2>&1; then status; exit 0; fi
      sleep 1
    done
    echo 'The signet node did not start; inspect its private scratch log.' >&2; exit 1
    ;;
  status) status ;;
  stop)
    owns
    docker stop --time 60 "$name" >/dev/null
    echo 'Signet node stopped; its chain data is kept.'
    ;;
  *) echo 'Use start, status, or stop.' >&2; exit 2 ;;
esac

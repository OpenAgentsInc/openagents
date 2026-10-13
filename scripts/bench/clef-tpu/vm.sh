#!/usr/bin/env bash
# Helpers from the laptop. usage: vm.sh push|setup|run|pull NAME ZONE [cmd...]
set -u
OP=$1 NAME=$2 ZONE=$3; shift 3
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
HERE=$(cd "$(dirname "$0")" && pwd)
G="gcloud compute tpus tpu-vm"
P="--zone $ZONE --project openagentsgemini --quiet"
DATE=${DATE:-$(date -u +%F)}
DEST=gs://openagentsgemini-decision-throughput/tpu/$DATE/$NAME
case $OP in
  push) $G scp $P --worker=all "$HERE"/*.py "$HERE"/*.sh "$HERE"/*.yaml "$NAME":~/ ;;
  setup) $G ssh $NAME $P --worker=all --command "bash ~/setup_vm.sh" ;;
  run) $G ssh $NAME $P --worker=0 --command "source ~/venv/bin/activate; export PATH=\$HOME/.local/bin:\$PATH; cd ~; $*" ;;
  sync) $G ssh $NAME $P --worker=0 --command "gcloud storage rsync -r ~/out $DEST/ -q 2>&1 | tail -1; cp ~/setup.json ~/out/ 2>/dev/null; true" ;;
esac

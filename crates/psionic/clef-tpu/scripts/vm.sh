#!/usr/bin/env bash
# Spot TPU VM helper for the Clef TPU lane (#11459).
#   vm.sh create [type]   create a TPU VM (default v6e-1; spot unless SPOT=0), trying zones in order
#   vm.sh setup           install uv + Python 3.12 venv with jax[tpu]; fetch Cloudflare/clef-flash
#   vm.sh ssh 'cmd'       run a command in the venv
#   vm.sh scp src dst     copy to the VM
#   vm.sh delete          delete the VM
# Uses the automation SA gcloud config; NAME/ZONE persist in $STATE.
set -euo pipefail
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
PROJECT=${PROJECT:-openagentsgemini}
NAME=${NAME:-clef-tpu}
STATE=${STATE:-${TMPDIR:-/tmp}/clef-tpu-vm.zone}
SPOTFLAG=$([ "${SPOT:-1}" = 1 ] && echo --spot || true)
SA=oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com
zone() { cat "$STATE"; }
case "${1:-}" in
  create)
    type=${2:-v6e-1}
    case "$type" in
      v6e-*) ver=v2-alpha-tpuv6e; zones=${ZONES:-"us-east1-d us-east5-b us-east5-a us-central1-b us-central1-a us-south1-a us-east4-a europe-west4-a asia-northeast1-b"} ;;
      v5litepod-*) ver=v2-alpha-tpuv5-lite; zones=${ZONES:-"us-east5-a us-east5-b us-central1-a us-south1-a us-west4-a us-east1-c europe-west4-b"} ;;
      v5p-*) ver=v2-alpha-tpuv5; zones=${ZONES:-"us-east5-a us-east5-b us-central1-a us-south1-a us-east1-d"} ;;
      *) echo "unknown type $type" >&2; exit 2 ;;
    esac
    for z in $zones; do
      echo "== $z" >&2
      if gcloud compute tpus tpu-vm create "$NAME" --zone "$z" --project "$PROJECT" \
          --accelerator-type "$type" --version "$ver" $SPOTFLAG --service-account "$SA" 2>&1 \
          | grep -oE '"message": "[^"]{0,120}|ERROR: .{0,160}' >&2; then :; fi
      if gcloud compute tpus tpu-vm describe "$NAME" --zone "$z" --project "$PROJECT" --format='value(state)' 2>/dev/null | grep -q READY; then
        echo "$z" > "$STATE"; echo "READY $NAME $z $(date -u +%FT%TZ)"; exit 0
      fi
      gcloud compute tpus tpu-vm delete "$NAME" --zone "$z" --project "$PROJECT" --quiet >/dev/null 2>&1 || true
    done
    exit 1 ;;
  setup)
    "$0" ssh-raw 'curl -LsSf https://astral.sh/uv/install.sh | sh >/dev/null 2>&1; export PATH=$HOME/.local/bin:$PATH;
      [ -d ~/venv ] || uv venv -q -p 3.12 ~/venv; . ~/venv/bin/activate;
      uv pip install -q "jax[tpu]" safetensors numpy huggingface_hub hf_transfer tokenizers fastapi uvicorn;
      HF_HUB_ENABLE_HF_TRANSFER=1 python -c "from huggingface_hub import snapshot_download as s; s(\"Cloudflare/clef-flash\", local_dir=\"$HOME/clef-flash\")" 2>&1 | tail -1;
      python -c "import jax; print(jax.__version__, jax.devices())"' ;;
  ssh-raw) gcloud compute tpus tpu-vm ssh "$NAME" --zone "$(zone)" --project "$PROJECT" --command "$2" 2>&1 | grep -v -e hugepage -e 'SSH:' -e 'ssh batch' -e 'SSH key' ;;
  ssh) "$0" ssh-raw ". ~/venv/bin/activate; $2" ;;
  scp) gcloud compute tpus tpu-vm scp --recurse "$2" "$NAME:$3" --zone "$(zone)" --project "$PROJECT" 2>&1 | grep -v -e 'SCP:' -e 'ssh batch' -e 'SSH key' || true ;;
  state) gcloud compute tpus tpu-vm describe "$NAME" --zone "$(zone)" --project "$PROJECT" --format='value(state,healthDescription)' ;;
  delete) gcloud compute tpus tpu-vm delete "$NAME" --zone "$(zone)" --project "$PROJECT" --quiet ;;
  *) sed -n 2,9p "$0"; exit 2 ;;
esac

#!/usr/bin/env bash
# usage on VM: MODEL=.. TP=.. QUANT=.. bash prefill.sh  (wraps bench_prefill.py, logs to ~/out)
# The JAX compile cache is shared through GCS per accelerator type, so a fresh (re-acquired)
# spot VM skips most of SGL-JAX's precompile.
source ~/venv/bin/activate; cd ~; mkdir -p ~/out /tmp/jit_cache
export JAX_COMPILATION_CACHE_DIR=/tmp/jit_cache HF_HUB_ENABLE_HF_TRANSFER=1
ACC=$(curl -s -H 'Metadata-Flavor: Google' http://metadata.google.internal/computeMetadata/v1/instance/attributes/accelerator-type | cut -d- -f1)
CACHE=gs://openagentsgemini-decision-throughput/tpu/jit-cache/$ACC-jax0.11.1
gcloud storage rsync -r $CACHE /tmp/jit_cache -q >/dev/null 2>&1
TAG=${TAG:-$(echo "${MODEL:-Qwen/Qwen3.5-9B}" | tr '/' '_')-tp${TP:-1}-${QUANT:-bf16}-${LOAD:-dummy}}
t0=$(date +%s)
timeout ${STEP_TIMEOUT:-5400} python -u bench_prefill.py > ~/out/log-prefill-$TAG${CLEF_CHIPS:+-c${CLEF_CHIPS//,/}}.txt 2>&1
rc=$?
echo "{\"tag\":\"$TAG\",\"rc\":$rc,\"wall_s\":$(( $(date +%s) - t0 ))}" >> ~/out/prefill-runs.jsonl
gcloud storage rsync -r /tmp/jit_cache $CACHE -q >/dev/null 2>&1 &

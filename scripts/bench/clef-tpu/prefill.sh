#!/usr/bin/env bash
# usage on VM: MODEL=.. TP=.. QUANT=.. bash prefill.sh  (wraps bench_prefill.py, logs to ~/out)
source ~/venv/bin/activate; cd ~; mkdir -p ~/out
export JAX_COMPILATION_CACHE_DIR=/tmp/jit_cache HF_HUB_ENABLE_HF_TRANSFER=1
TAG=${TAG:-$(echo "${MODEL:-Qwen/Qwen3.5-9B}" | tr '/' '_')-tp${TP:-1}-${QUANT:-bf16}-${LOAD:-dummy}}
t0=$(date +%s)
timeout ${STEP_TIMEOUT:-5400} python -u bench_prefill.py > ~/out/log-prefill-$TAG${CLEF_CHIPS:+-c${CLEF_CHIPS//,/}}.txt 2>&1
echo "{\"tag\":\"$TAG\",\"rc\":$?,\"wall_s\":$(( $(date +%s) - t0 ))}" >> ~/out/prefill-runs.jsonl

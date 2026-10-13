#!/usr/bin/env bash
# Runs ON the VM: matmul, GDN and attention benches, one after another; logs to ~/out.
source ~/venv/bin/activate; cd ~; mkdir -p ~/out
export PYTHONPATH=~/sglang-jax/python:$PYTHONPATH
for b in ${BENCHES:-gdn attn matmul}; do
  t0=$(date +%s)
  python -u bench_$b.py > ~/out/log-$b${CLEF_CHIPS:+-c$CLEF_CHIPS}.txt 2>&1
  echo "{\"bench\":\"$b\",\"rc\":$?,\"wall_s\":$(( $(date +%s) - t0 ))}" >> ~/out/kernels-runs.jsonl
done

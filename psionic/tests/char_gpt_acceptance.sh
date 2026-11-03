#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: runs the char-gpt driver for the three v2 acceptance runs; every decision is in the driver's Coax)
# Usage: psionic/tests/char_gpt_acceptance.sh BUILD_DIR CORPUS OUT_DIR
#   OUT/fp32        750 steps uninterrupted (FP32 profile)
#   OUT/fp32-resume 375 steps, then a new process resumes to 750
#   OUT/bf16        750 steps (BF16 profile)
# Each command stays far under 15 minutes on an RTX 4080.
set -euo pipefail
build=$1 corpus=$2 out=$3
mkdir -p "$out/fp32" "$out/fp32-resume" "$out/bf16"
nvidia-smi --query-gpu=name,memory.used,utilization.gpu --format=csv,noheader > "$out/gpu-before.txt"
"$build/char-gpt" run gpu full "$corpus" "$out/fp32" --kernels "$build/char-gpt-kernels.cu" > "$out/fp32/summary.json" 2> "$out/fp32/stderr.txt"
"$build/char-gpt" run gpu full "$corpus" "$out/fp32-resume" --steps 375 --kernels "$build/char-gpt-kernels.cu" > "$out/fp32-resume/summary-1.json" 2> "$out/fp32-resume/stderr-1.txt"
"$build/char-gpt" run gpu full "$corpus" "$out/fp32-resume" --kernels "$build/char-gpt-kernels.cu" > "$out/fp32-resume/summary-2.json" 2> "$out/fp32-resume/stderr-2.txt"
"$build/char-gpt" run gpu full-bf16 "$corpus" "$out/bf16" --kernels "$build/char-gpt-kernels.cu" > "$out/bf16/summary.json" 2> "$out/bf16/stderr.txt"
cmp "$out/fp32/state.ckpt" "$out/fp32-resume/state.ckpt" && echo "resume: final checkpoints identical" > "$out/resume.txt"
nvidia-smi --query-gpu=name,memory.used,utilization.gpu --format=csv,noheader > "$out/gpu-after.txt"

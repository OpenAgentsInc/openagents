#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: runs the char-gpt driver for the acceptance runs; every decision is in the driver's Coax)
# Usage: psionic/tests/char_gpt_acceptance.sh BUILD_DIR CORPUS OUT_DIR [mma]
#   OUT/fp32        750 steps uninterrupted (FP32 profile)
#   OUT/fp32-resume 375 steps, then a new process resumes to 750
#   OUT/bf16        750 steps (operands rounded to BF16, FP32 products)
# With mma (#12701), the frozen bf16 profile on the tensor cores instead:
#   OUT/mma         750 steps uninterrupted
#   OUT/mma-resume  375 steps, then a new process resumes to 750
#   OUT/fp32        750 steps of the FP32 profile on the same build (before)
# Each command stays far under 15 minutes on an RTX 4080. nvidia-smi samples
# the card's memory every 100 ms into OUT/RUN/smi.csv.
set -euo pipefail
build=$1 corpus=$2 out=$3 mode=${4:-}
run() {  # run NAME SUMMARY PROFILE [ARGS...]
  local name=$1 summary=$2 profile=$3 smi
  shift 3
  mkdir -p "$out/$name"
  nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits -lms 100 >> "$out/$name/smi.csv" &
  smi=$!
  "$build/char-gpt" run gpu "$profile" "$corpus" "$out/$name" --kernels "$build/char-gpt-kernels.cu" "$@" > "$out/$name/$summary.json" 2> "$out/$name/$summary.stderr.txt"
  kill "$smi"
  wait "$smi" 2> /dev/null || true
}
mkdir -p "$out"
nvidia-smi --query-gpu=name,memory.used,utilization.gpu --format=csv,noheader > "$out/gpu-before.txt"
if [[ "$mode" == mma ]]; then
  run mma summary full-mma
  run mma-resume summary-1 full-mma --steps 375
  run mma-resume summary-2 full-mma
  run fp32 summary full
  cmp "$out/mma/state.ckpt" "$out/mma-resume/state.ckpt" && echo "resume: final checkpoints identical" > "$out/resume.txt"
else
  run fp32 summary full
  run fp32-resume summary-1 full --steps 375
  run fp32-resume summary-2 full
  run bf16 summary full-bf16
  cmp "$out/fp32/state.ckpt" "$out/fp32-resume/state.ckpt" && echo "resume: final checkpoints identical" > "$out/resume.txt"
fi
nvidia-smi --query-gpu=name,memory.used,utilization.gpu --format=csv,noheader > "$out/gpu-after.txt"

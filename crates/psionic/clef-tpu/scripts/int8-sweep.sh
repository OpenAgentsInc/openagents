#!/usr/bin/env bash
# int8 accuracy sweep on a TPU VM (#11502): each line of $CONFIGS is
# "<label> <env assignments...>"; for each, the 40 e2e requests run through
# the runner with the calibration map and are compared with the HF f32
# reference. Output: one JSON line per config in $OUT.
#   CONFIGS=configs.txt CALIB=~/calib-flash.npz OUT=~/int8-sweep.jsonl scripts/int8-sweep.sh
set -uo pipefail
cd "$(dirname "$0")/.."
FX=${FX:-$HOME/oa/fixtures/clef}
MODEL=${MODEL:-$HOME/clef-flash}
OUT=${OUT:-$HOME/int8-sweep.jsonl}
# compiled programs persist across configs (same shapes)
export JAX_COMPILATION_CACHE_DIR=${JAX_COMPILATION_CACHE_DIR:-$HOME/.cache/clef-tpu-jax}
export JAX_PERSISTENT_CACHE_MIN_COMPILE_TIME_SECS=1
while read -r label envs; do
  [ -z "$label" ] && continue
  case "$label" in \#*) continue ;; esac
  rows=$HOME/e2e-$label.jsonl
  env $envs python bench/e2e_tpu.py "$MODEL" "$FX" "$rows" --precision ${PRECISION:-int8} --calib "${CALIB:-}" >/dev/null 2>$HOME/e2e-$label.log
  cmp=$(python "$FX/tools/e2e.py" compare "$FX/e2e/reference-hf-f32.jsonl" "$rows" 2>/dev/null | python -c 'import json,sys; d=json.load(sys.stdin); print(json.dumps({k: d[k] for k in ("questions","top_answer_agreement","median_abs_dp","max_abs_dp")}))')
  echo "{\"label\": \"$label\", \"env\": \"$envs\", \"cmp\": $cmp}" | tee -a "$OUT"
done < "${CONFIGS:-/dev/stdin}"

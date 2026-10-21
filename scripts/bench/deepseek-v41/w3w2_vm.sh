#!/bin/sh
# W3 (#12390) and W2 (#12389) on a one-quad stage machine (g4-standard-192,
# four RTX PRO 6000) that `pylon cloud up one-quad` set up: its weights in
# /mnt/stage/weights and the stage worker in /opt/openagents. Run it only
# after the group head has stopped, so the four GPUs are free.
#
#   w3w2_vm.sh answers RUNTIME    p1 (thinking) and p2 (chat) to the end token
#   w3w2_vm.sh memory RUNTIME     load the stages at several contexts
#   w3w2_vm.sh long RUNTIME FILE  decode after a 16k-token prompt
#   w3w2_vm.sh cpu                p1 and p2 on the CPU model (diagnostic)
#
# Each run writes OUT/NAME.json (the chain report) and OUT/NAME.err, and
# appends "NAME EXIT" to OUT/done. RUNTIME labels the build.
set -eu
W=/mnt/stage/weights
OUT=/mnt/stage/w3w2
BIN=/opt/openagents/stage-worker
MODELS=gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1
export LD_LIBRARY_PATH=/opt/openagents/lib CUDA_DEVICE_ORDER=PCI_BUS_ID
mkdir -p "$OUT"
# The oracle's p1 and p2 conversations (scripts/bench/deepseek-v41/oracle_dump.py).
P1='[{"role":"user","content":"Why is the sky blue? Answer in two sentences."}]'
P2='[{"role":"user","content":"Write a Rust function `fn fib(n: u32) -> Option<u128>` that returns the n-th Fibonacci number, using checked arithmetic so it returns None on overflow."}]'
HI='[{"role":"user","content":"Hi"}]'

chain() {
  name=$1
  shift
  set +e
  timeout 3600 "$BIN" chain --weights "$W" --runtime "$RUNTIME" "$@" \
    >"$OUT/$name.json" 2>"$OUT/$name.err"
  echo "$name $?" >>"$OUT/done"
  set -e
}

case ${1:-} in
answers)
  RUNTIME=$2
  chain p1-thinking --prompt "$P1" --mode thinking --max-tokens 2048
  chain p2-chat --prompt "$P2" --mode chat --max-tokens 2048
  chain p1-thinking-repeat --prompt "$P1" --mode thinking --max-tokens 2048
  chain p2-chat-repeat --prompt "$P2" --mode chat --max-tokens 2048
  ;;
memory)
  RUNTIME=$2
  # Context, then step tokens. A serving stage's step equals its context.
  for pair in 32768:8192 131072:8192 1048576:8192 32768:32768 131072:131072; do
    ctx=${pair%%:*}
    step=${pair##*:}
    chain "mem-$ctx-$step" --prompt "$HI" --mode chat --max-tokens 4 \
      --max-context "$ctx" --max-step-tokens "$step" --engram-lookups 500
  done
  ;;
long)
  RUNTIME=$2
  chain long-16k --prompt-file "$3" --mode chat --max-tokens 32 \
    --max-context 16384 --max-step-tokens 2048 --prefill-chunk 2048
  ;;
cpu)
  # The CPU model opens every shard; copy the MTP and vision shards the
  # stages don't read.
  python3 - "$W/model.safetensors.index.json" "$W" >"$OUT/missing" <<'PY'
import json, os, sys
index, root = sys.argv[1], sys.argv[2]
shards = set(json.load(open(index))["weight_map"].values())
print("\n".join(sorted(s for s in shards if not os.path.exists(os.path.join(root, s)))))
PY
  # The startup script wrote the weights as root.
  sed "s|^|$MODELS/|" "$OUT/missing" | sudo gcloud storage cp -q -I "$W/"
  for case in "p1:$P1" "p2:$P2"; do
    name=${case%%:*}
    prompt=${case#*:}
    if [ "$name" = p1 ]; then mode=thinking; else mode=chat; fi
    (
      set +e
      start=$(date +%s)
      RAYON_NUM_THREADS=96 timeout 7200 "$BIN" reference --weights "$W" --backend cpu \
        --prompt "$prompt" --mode "$mode" \
        --steps 2048 >"$OUT/cpu-$name.json" 2>"$OUT/cpu-$name.err"
      echo "cpu-$name $? $(($(date +%s) - start))" >>"$OUT/done"
    ) &
  done
  wait
  ;;
*)
  echo "usage: $0 answers|memory RUNTIME | long RUNTIME FILE | cpu" >&2
  exit 2
  ;;
esac

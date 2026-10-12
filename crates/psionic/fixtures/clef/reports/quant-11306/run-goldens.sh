#!/bin/bash
# The #11306 frozen product comparison, run on the VM vm-startup.sh prepared.
#
#   run-goldens.sh build              build chat-goldens and the routes-v5 eval at the plan's revision
#   run-goldens.sh run SYSTEM MAP     SYSTEM = jev | q4 | q8, MAP = on | off (Clef map; ignored for jev):
#                                     chat-goldens router (123 cases), plus routes-v5 held-out for jev
#   run-goldens.sh fit SYSTEM         q4 | q8: fit a 27B map on routes-v5's calibration partition
#                                     and score held-out raw and calibrated (ROUTER_EVAL_PUBLISH=1)
#   run-goldens.sh all                build, then every row of plan.json in order
#
# Keys come from the caller's environment (TYPESAFE_API_KEY for Jev,
# AI_GATEWAY_API_KEY for the product notes' embeddings) or, when unset, from
# Secret Manager through the VM's service account (decision-throughput-typesafe-key,
# coder-ai-gateway-key); nothing is written to disk.
#
# Resumable: a row that finished leaves results/<row>/done and is skipped on a
# rerun. After every row the results directory is copied to
# $RESULTS_URI (default gs://openagentsgemini-decision-throughput/clef-11306/),
# so a preempted or deleted VM keeps every finished row. Every run uses the same request shape, answer bank, product-note
# availability, router revision and cache setting: a freshly started Clef
# server (empty exact-logit cache) per row, product notes on, staged routing
# as shipped. Results: /var/log/clef-11306/results/<system>-<map>/.
set -euo pipefail
ROOT=/opt/clef-11306
LOG=/var/log/clef-11306
OUT=$LOG/results
REV=$(cat "$LOG/rev")
export PATH=/root/.cargo/bin:/usr/local/cuda/bin:$PATH
export CARGO_TARGET_DIR=$ROOT/target-coder
RESULTS_URI=${RESULTS_URI:-gs://openagentsgemini-decision-throughput/clef-11306}

secret() { # Secret Manager name -> value on stdout, through the VM's account
  local tok
  tok=$(curl -sf -H 'Metadata-Flavor: Google' \
    http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["access_token"])') || return 0
  curl -sf -H "Authorization: Bearer $tok" \
    "https://secretmanager.googleapis.com/v1/projects/openagentsgemini/secrets/$1/versions/latest:access" \
    | python3 -c 'import base64,json,sys; sys.stdout.write(base64.b64decode(json.load(sys.stdin)["payload"]["data"]).decode())' \
    2>/dev/null || true
}
keys() {
  [ -n "${TYPESAFE_API_KEY:-}" ] || TYPESAFE_API_KEY=$(secret decision-throughput-typesafe-key)
  [ -n "${AI_GATEWAY_API_KEY:-}" ] || AI_GATEWAY_API_KEY=$(secret coder-ai-gateway-key)
  export TYPESAFE_API_KEY AI_GATEWAY_API_KEY
}
push_results() { # copy logs and results (not the models or targets) off the VM
  gcloud storage rsync -r -q "$LOG" "$RESULTS_URI/$(hostname)" >/dev/null 2>&1 || true
}
row() { # row NAME CMD...: run once, mark done, copy off the VM
  local name=$1; shift
  if [ -f "$OUT/$name/done" ]; then echo "skip $name (done)"; return; fi
  echo "$(date -u +%FT%TZ) start $name" >> "$LOG/progress.log"
  "$@"
  mkdir -p "$OUT/$name"; date -u +%FT%TZ > "$OUT/$name/done"
  echo "$(date -u +%FT%TZ) done $name" >> "$LOG/progress.log"
  push_results
}

build() {
  if [ ! -d "$ROOT/full/.git" ]; then
    git clone -q --filter=blob:none https://github.com/OpenAgentsInc/openagents.git "$ROOT/full"
  fi
  git -C "$ROOT/full" checkout -q "$REV"
  ( cd "$ROOT/full" && cargo build -q -p coder --bin chat-goldens \
      && cargo test -q -p coder --test router_eval --no-run ) > "$LOG/build-coder.log" 2>&1
  echo built > "$LOG/coder-state"
}

run() {
  local system=$1 map=${2:-on} dir
  dir=$OUT/$system-$map; mkdir -p "$dir"
  : "${AI_GATEWAY_API_KEY:?product notes need AI_GATEWAY_API_KEY}"
  export OPENAGENTS_PRODUCT_KB_EMBEDDINGS="$AI_GATEWAY_API_KEY"
  local -a env=()
  case "$system" in
    jev) : "${TYPESAFE_API_KEY:?Jev needs TYPESAFE_API_KEY}" ;;
    q4|q8)
      "$ROOT/serve.sh" "$system"
      local port; port=$([ "$system" = q4 ] && echo 18201 || echo 18202)
      env=(CODER_DECISION_PROFILE=direct_local CODER_DECISION_URL="http://127.0.0.1:$port"
           CODER_DECISION_MODEL=clef CODER_WORKER_ROUTER_CALIBRATION="$map")
      ;;
    *) echo "system is jev, q4 or q8" >&2; exit 2 ;;
  esac
  cd "$ROOT/full"
  # chat-goldens router: all 123 cases (held-out evaluation).
  env "${env[@]}" "$CARGO_TARGET_DIR/debug/chat-goldens" router --out "$dir/chat-goldens" \
    > "$dir/chat-goldens.log" 2>&1 || true
  # routes-v5 held-out for Jev (its map is fitted elsewhere); a Clef artifact's
  # routes-v5 numbers come from fit, which reads both partitions once.
  if [ "$system" = jev ]; then
    ROUTER_EVAL_SPLIT=held_out ROUTER_EVAL_MAPS=on ROUTER_EVAL_OUT="$dir/routes-v5-held_out" \
      cargo test -q -p coder --test router_eval -- --ignored --nocapture --test-threads 1 live_router \
      > "$dir/routes-v5-held_out.log" 2>&1 || true
  fi
  [ "$system" = jev ] || cp "$LOG/server-$system.log" "$LOG/vram-$system-after-load.txt" "$dir/" 2>/dev/null || true
  nvidia-smi --query-gpu=memory.used --format=csv,noheader > "$dir/vram-after-run.txt"
}

fit() {
  local system=$1 dir port
  dir=$OUT/$system-fit; mkdir -p "$dir"
  "$ROOT/serve.sh" "$system"
  port=$([ "$system" = q4 ] && echo 18201 || echo 18202)
  cd "$ROOT/full"
  CODER_DECISION_PROFILE=direct_local CODER_DECISION_URL="http://127.0.0.1:$port" CODER_DECISION_MODEL=clef \
    ROUTER_EVAL_PUBLISH=1 ROUTER_EVAL_OUT="$dir" \
    cargo test -q -p coder --test router_eval -- --ignored --nocapture --test-threads 1 live_router \
    > "$dir/fit.log" 2>&1 || true
}

case "${1:-}" in
  build) build ;;
  run) shift; keys; run "$@"; push_results ;;
  fit) shift; keys; fit "$@"; push_results ;;
  all) # Keys arrive in a root-only file that is read and removed at once.
       if [ -f /run/clef-11306.env ]; then set -a; . /run/clef-11306.env; set +a; rm -f /run/clef-11306.env; fi
       keys
       ( while sleep 600; do push_results; done ) &
       [ -f "$LOG/coder-state" ] || build
       until [ "$(cat "$LOG/state")" = ready ]; do sleep 30; done
       row jev-on run jev on; row q4-on run q4 on; row q4-off run q4 off; row q4-fit fit q4
       row q8-on run q8 on; row q8-off run q8 off; row q8-fit fit q8
       echo done > "$LOG/run-state"; push_results ;;
  *) sed -n 2,6p "$0"; exit 2 ;;
esac

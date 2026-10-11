#!/bin/sh
# Daily decision-throughput run on CoderOS-4080 (docs/inference/decision-throughput.md).
# Starts a bench Psionic Clef server on 127.0.0.1:18097 beside the production
# decision pylon (port 18096, never touched), runs the harness's daily plan
# against it, stops it, and appends the points to
# gs://openagentsgemini-decision-throughput/runs/YYYY-MM-DD.jsonl as the
# bucket-scoped writer account (key ~/decision-bench/writer-key.json).
# If the bench server does not fit in VRAM beside the text pylon, the text
# pylon (pylon-psionic) is stopped for the run and restored as it ran.
#   PLAN=smoke DURATION=20 sh daily.sh    # a quick end-to-end check
#
# Installed on coderos-4080 as ~/decision-bench/daily.sh (harness copy in
# ~/decision-bench/harness, corpus in ~/decision-bench/corpus, Psionic built
# into ~/decision-bench/target) and run by user units in ~/.config/systemd/user:
#   decision-throughput.timer    OnCalendar=*-*-* 09:30:00 UTC, Persistent=true, RandomizedDelaySec=300
#   decision-throughput.service  Type=oneshot, ExecStart=/bin/sh %h/decision-bench/daily.sh, TimeoutStartSec=3h
# The Cloud Run job runs at 11:00 UTC so its gateway-pylon points don't
# share the GPU with this run.
set -u
B=$HOME/decision-bench
PLAN=${PLAN:-daily}
DURATION=${DURATION:-}
LOG=$B/logs/daily-$(date -u +%Y%m%d).log
mkdir -p $B/logs $B/results
exec >>"$LOG" 2>&1
echo "== $(date -u +%FT%TZ) plan=$PLAN"
if pgrep -f -- "--port 18097" >/dev/null; then echo "a bench server already holds 18097; skipping"; exit 0; fi
. $HOME/clef-m1/env.sh
start() {
  systemctl --user reset-failed decision-bench-clef 2>/dev/null || true
  systemd-run --user --unit=decision-bench-clef --setenv=LD_LIBRARY_PATH="$LD_LIBRARY_PATH" \
    -p StandardOutput=append:$B/logs/clef-18097.log -p StandardError=append:$B/logs/clef-18097.log \
    $B/target/release/psionic-openai-server -m $HOME/clef-m1/models/Clef-Flash-Q4_K_M.gguf \
    --host 127.0.0.1 --port 18097 --decision-device cuda --decision-chunk 2048
  for i in $(seq 60); do
    curl -sf 127.0.0.1:18097/health >/dev/null && return 0
    systemctl --user is-active -q decision-bench-clef || return 1
    sleep 2
  done
  return 1
}
stopped_text=0
if ! start; then
  echo "bench server did not start beside the text pylon; stopping pylon-psionic for the run"
  systemctl --user stop decision-bench-clef 2>/dev/null || true
  systemctl --user stop pylon-psionic && stopped_text=1
  sleep 3
  if ! start; then
    echo "bench server failed to start"
    [ $stopped_text = 1 ] && sh $HOME/work/pylon-decide/psionic-restart.sh original
    exit 1
  fi
fi
export CLOUDSDK_CONFIG=$B/gcloud
gcloud auth activate-service-account --key-file=$B/writer-key.json >/dev/null 2>&1
GOOGLE_OAUTH_ACCESS_TOKEN=$(gcloud auth print-access-token)
export GOOGLE_OAUTH_ACCESS_TOKEN
cd $B/harness
python3 harness.py --target coderos-clef --plan "$PLAN" ${DURATION:+--duration $DURATION} \
  --corpus $B/corpus --out $B/results/coderos-daily.jsonl \
  --gcs gs://openagentsgemini-decision-throughput/runs \
  --gpu-cmd "nvidia-smi --query-gpu=utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits" \
  --note "coderos daily timer ($PLAN)"
rc=$?
systemctl --user stop decision-bench-clef
[ $stopped_text = 1 ] && sh $HOME/work/pylon-decide/psionic-restart.sh original
echo "== done rc=$rc"
exit $rc

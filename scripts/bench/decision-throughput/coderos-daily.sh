#!/bin/sh
# Daily decision-throughput run on CoderOS-4080 (docs/inference/decision-throughput.md).
# Measures the production decision server itself (pylon-clef, Psionic Clef on
# 127.0.0.1:18096), read-only and briefly: 1 and 2 clients, then 13 questions
# and the router at one client, 45 s a point, about 4 minutes in all. It starts
# no second server and never stops, restarts or pauses a Pylon service: a
# second Clef server on the same GPU ran production out of memory (#11303).
# The harness's guard (target coderos-clef-prod in targets.json) stops the run
# at the first sign of pressure: free GPU memory under 768 MiB, a failed
# /health, or any unanswered request (503 busy included). The points are
# appended to gs://openagentsgemini-decision-throughput/runs/YYYY-MM-DD.jsonl
# as the bucket-scoped writer account (key ~/decision-bench/writer-key.json).
#   DRY_RUN=1 sh daily.sh                 # the plan and guard checks, no requests
#   PLAN=smoke DURATION=20 sh daily.sh    # a quick end-to-end check
#
# Installed on coderos-4080 as ~/decision-bench/daily.sh (harness copy in
# ~/decision-bench/harness, corpus in ~/decision-bench/corpus) and run by user
# units in ~/.config/systemd/user:
#   decision-throughput.timer    OnCalendar=*-*-* 09:30:00 UTC, Persistent=true, RandomizedDelaySec=300
#   decision-throughput.service  Type=oneshot, ExecStart=/bin/sh %h/decision-bench/daily.sh, TimeoutStartSec=20min
# The Cloud Run job runs at 11:00 UTC so its gateway-pylon points don't
# share the GPU with this run.
set -u
B=$HOME/decision-bench
PLAN=${PLAN:-daily}
DURATION=${DURATION:-}
DRY_RUN=${DRY_RUN:-}
PROD=http://127.0.0.1:18096
GPU="nvidia-smi --query-gpu=utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits"
LOG=$B/logs/daily-$(date -u +%Y%m%d).log
mkdir -p $B/logs $B/results
exec >>"$LOG" 2>&1
echo "== $(date -u +%FT%TZ) plan=$PLAN target=coderos-clef-prod${DRY_RUN:+ dry-run}"
# An old bench server must never sit beside production.
if pgrep -f -- "--port 18097" >/dev/null; then echo "a bench server holds 18097; not running beside it"; exit 1; fi
if ! curl -sf -m 5 $PROD/health >/dev/null; then echo "production pylon-clef is not healthy; skipping"; exit 0; fi
echo "gpu before: $($GPU)"
cd $B/harness
if [ -n "$DRY_RUN" ]; then
  python3 harness.py --target coderos-clef-prod --plan "$PLAN" ${DURATION:+--duration $DURATION} \
    --corpus $B/corpus --out /dev/null --gpu-cmd "$GPU" --dry-run
  rc=$?
  echo "== done rc=$rc"
  exit $rc
fi
export CLOUDSDK_CONFIG=$B/gcloud
gcloud auth activate-service-account --key-file=$B/writer-key.json >/dev/null 2>&1
GOOGLE_OAUTH_ACCESS_TOKEN=$(gcloud auth print-access-token)
export GOOGLE_OAUTH_ACCESS_TOKEN
python3 harness.py --target coderos-clef-prod --plan "$PLAN" ${DURATION:+--duration $DURATION} \
  --corpus $B/corpus --out $B/results/coderos-daily.jsonl \
  --gcs gs://openagentsgemini-decision-throughput/runs \
  --gpu-cmd "$GPU" \
  --note "coderos daily timer ($PLAN), production pylon-clef read-only"
rc=$?
echo "gpu after: $($GPU)"
curl -sf -m 5 $PROD/health >/dev/null && echo "production health after: ok" || echo "production health after: FAILED"
echo "== done rc=$rc"
exit $rc

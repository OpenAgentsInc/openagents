#!/bin/bash
# Startup script of the decision-throughput H100 VM (h100-run.sh creates it;
# the weekly Cloud Scheduler job creates the same VM). Runs as root on a
# plain Spot a3-highgpu-1g with the Deep Learning VM image (NVIDIA driver
# 580), as the automation service account. It:
#   1. fetches the bundle (psionic-openai-server sm_90 + CUDA 13 libs +
#      oa-att, cloudbuild-bundle.yaml), the harness and the corpus from GCS;
#   2. fetches Clef-Flash and Clef 27B Q4_K_M (GCS cache, else Hugging Face
#      at a pinned revision, then cached), checks their SHA-256, into RAM;
#   3. starts Psionic Flash on :18096 and 27B on :18098;
#   4. optionally (metadata dt-sealed=1) wakes the sealed GPU oa-att-h100-1,
#      measures it over NIP-ATT in parallel, and stops it;
#   5. runs the harness against both, appending to <bucket>/runs/DAY.jsonl;
#   6. uploads its log and a done marker, and deletes this VM.
# Metadata: dt-run (run id), dt-bundle (bundle tag; default bin/latest),
# dt-sealed (1 to include the sealed GPU), dt-keep (1: don't self-delete),
# dt-flash-args / dt-27b-args (extra harness args).
set -u
B=gs://openagentsgemini-decision-throughput
md() { curl -sf -H Metadata-Flavor:Google "http://metadata.google.internal/computeMetadata/v1/instance/$1"; }
NAME=$(md name)
ZONE=$(md zone | awk -F/ '{print $NF}')
RUN=$(md attributes/dt-run || date -u +%Y%m%dT%H%M%SZ)
LOG=/var/log/decision-throughput.log
exec > >(tee -a "$LOG") 2>&1
T0=$(date +%s)
say() { echo "[$(( $(date +%s) - T0 ))s] $*"; }
finish() {
    say "finishing: $1"
    pkill -f psionic-openai-server || true
    gcloud storage cp "$LOG" "$B/h100/$RUN/startup.log" || true
    echo "$1" | gcloud storage cp - "$B/h100/$RUN/done" || true
    if [ "$(md attributes/dt-keep || true)" != 1 ]; then
        gcloud compute instances delete "$NAME" --zone "$ZONE" --quiet
    fi
}
trap 'finish "exit $?"' EXIT
say "run $RUN on $NAME ($ZONE)"
for i in $(seq 60); do nvidia-smi >/dev/null 2>&1 && break; sleep 5; done
nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv,noheader

W=/opt/dt; M=/dev/shm/models; mkdir -p $W/harness $W/corpus $M
TAG=$(md attributes/dt-bundle || gcloud storage cat $B/bin/latest)
say "bundle $TAG"
gcloud storage cp "$B/bin/$TAG/bundle.tar.gz" $W/ && tar -xzf $W/bundle.tar.gz -C $W || exit 10
cat $W/image.txt
gcloud storage cp "$B/harness/*" $W/harness/ || exit 11
gcloud storage cp "$B/corpus/v1/*" $W/corpus/ || exit 12

fetch() { # name url sha256
    if gcloud storage cp "$B/models/$1" $M/ 2>/dev/null; then say "$1 from the GCS cache"
    else
        say "$1 from Hugging Face"
        curl -sfL --retry 3 -o "$M/$1" "$2" || return 1
        CACHE=1
    fi
    got=$(sha256sum "$M/$1" | cut -d' ' -f1)
    say "$1 sha256 $got"
    if [ -n "$3" ] && [ "$got" != "$3" ]; then say "$1: digest mismatch"; return 1; fi
    if [ "${CACHE:-0}" = 1 ]; then gcloud storage cp "$M/$1" "$B/models/$1" & CACHE_PIDS="${CACHE_PIDS:-} $!"; fi
    CACHE=0
}
fetch Clef-Flash-Q4_K_M.gguf \
    https://huggingface.co/ggml-org/Clef-Flash-GGUF/resolve/4a192915ef971886004b5b13294f2b4c7a7fc39d/Clef-Flash-Q4_K_M.gguf \
    fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c || exit 13
fetch Clef-Q4_K_M.gguf \
    https://huggingface.co/ggml-org/Clef-GGUF/resolve/63840a1a68cb7084c88610cffc328509356b04cb/Clef-Q4_K_M.gguf \
    bd9b8ae24c5752a2bb5e2733d5168340d5f04457e02b8bf0d1bc9285ec3d4f61 || exit 14

export LD_LIBRARY_PATH=$W/lib
serve() { # model port log
    nohup $W/psionic-openai-server -m "$M/$1" --host 127.0.0.1 --port "$2" --decision-device cuda \
        --decision-chunk 2048 --decision-max-tokens 65536 > "/var/log/$3.log" 2>&1 &
}
serve Clef-Flash-Q4_K_M.gguf 18096 psionic-flash
serve Clef-Q4_K_M.gguf 18098 psionic-27b
for p in 18096 18098; do
    for i in $(seq 180); do curl -sf 127.0.0.1:$p/health >/dev/null && break; sleep 2; done
    curl -sf 127.0.0.1:$p/health >/dev/null || { say "server :$p did not come up"; tail -20 /var/log/psionic-*.log; exit 15; }
done
say "servers up"; nvidia-smi --query-gpu=memory.used --format=csv,noheader
model_of() { curl -sf 127.0.0.1:$1/v1/models | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"][0]["id"])'; }
FLASH_MODEL=$(model_of 18096); BIG_MODEL=$(model_of 18098)
say "models: $FLASH_MODEL, $BIG_MODEL"

GPU='nvidia-smi --query-gpu=utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits'
H="python3 $W/harness/harness.py --corpus $W/corpus --out /var/log/runs.jsonl --gcs $B/runs --note weekly-h100:$RUN"

if [ "$(md attributes/dt-sealed || true)" = 1 ]; then
    (
        say "sealed: waking oa-att-h100-1"
        S0=$(date +%s)
        gcloud compute instances start oa-att-h100-1 --zone us-east5-a --async
        ok=0
        for i in $(seq 60); do  # an answered round means its fresh endpoint is up
            if $W/oa-att round --publisher 77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e \
                --workload clef-decisions-gpu --state "The weather in Lisbon is sunny today." \
                --question "Is this about the weather?" 2>/dev/null | grep -q '"answer"'; then ok=1; break; fi
            sleep 10
        done
        say "sealed: ready=$ok after $(( $(date +%s) - S0 ))s"
        if [ $ok = 1 ]; then
            OA_ATT=$W/oa-att $H --target sealed-gpu --plan full --duration 90 $(md attributes/dt-sealed-args || true)
        fi
        gcloud compute instances stop oa-att-h100-1 --zone us-east5-a --discard-local-ssd=true
        say "sealed: stopped after $(( $(date +%s) - S0 ))s"
    ) > /var/log/sealed.log 2>&1 &
    SEALED=$!
fi

say "flash"
$H --target h100-clef-flash --model "$FLASH_MODEL" --plan full --duration 120 --gpu-cmd "$GPU" \
    $(md attributes/dt-flash-args || true)
say "27b"
$H --target h100-clef-27b --model "$BIG_MODEL" --plan full --duration 100 --gpu-cmd "$GPU" \
    --only c1,c4,c16,t500q1-cold,t4000q1-cold,t16000q1-cold,t4000q13-cold,router-cold \
    $(md attributes/dt-27b-args || true)
if [ -n "${SEALED:-}" ]; then wait $SEALED; cat /var/log/sealed.log; fi
gcloud storage cp /var/log/runs.jsonl /var/log/psionic-*.log /var/log/sealed.log "$B/h100/$RUN/" 2>/dev/null
# Only the model cache uploads: a bare `wait` would also wait for the
# `tee` behind this script's own output and never return.
for p in ${CACHE_PIDS:-}; do wait "$p"; done
say "done"

#!/usr/bin/env bash
# Finish a sharded frozen plan on one machine (#11309): merge every shard
# archive from GCS (plus any extra archives or result dirs given), judge the
# merged trials with the frozen Claude judge and with judge2.py (Gemini on
# Vertex), then write results/<tag>.md and the CSV with write_report.sh.
# Judging skips trials that already have a verdict, so a rerun only fills
# gaps (for example verdicts that hit the login's usage limit).
#   finish.sh PLAN BASE [--from ARCHIVE_OR_DIR ...]
#   e.g. finish.sh s2b A --from gs://openagents-bench-artifacts/briefed-ab-11309/judged-2026-10-12/s2b-judged-11309.tgz
#        finish.sh ablate-levers B0
set -euo pipefail
cd "$(dirname "$0")"
plan=$1 base=$2; shift 2
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
tag=$(python3 -c "import json; print(json.load(open('plans/$plan.json'))['tag'])")
python3 merge_shards.py "$tag" "$@"
python3 judge2.py judge --tag "$tag" --workers "${J2_WORKERS:-24}"
python3 ab.py judge --tag "$tag" --workers "${J1_WORKERS:-10}"
./write_report.sh "$plan" "$base"

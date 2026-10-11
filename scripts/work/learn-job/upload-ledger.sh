#!/usr/bin/env bash
# Upload this machine's briefed-run records to the learning loop (#11312):
# one ledger per machine, gs://openagentsgemini-filefind-learn/ledger/NAME.jsonl,
# merged with what it uploaded before (runs whose directories were since
# removed stay). Production runs: `work_issue.py`'s ~/.openagents/work/runs
# and a work host's host-runs. Bench trials: pass their results directory.
#
#   scripts/work/learn-job/upload-ledger.sh [--ab DIR] [--name NAME]
#
# Work hosts run it daily (oa-work-learn-upload.timer, scripts/work/host-setup.sh).
set -euo pipefail
here=$(cd "$(dirname "$0")/../../.." && pwd)
bucket=${OA_LEARN_BUCKET:-openagentsgemini-filefind-learn}
name=host-$(hostname)
ab=()
while (($#)); do
  case $1 in
    --ab) ab=(--ab "$2"); shift 2 ;;
    --name) name=$2; shift 2 ;;
    *) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
  esac
done
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
prev=()
if gcloud storage cp -q "gs://$bucket/ledger/$name.jsonl" "$tmp/prev.jsonl" 2>/dev/null; then
  prev=(--ledger-in "$tmp/prev.jsonl")
fi
OA_LEARN_HOST=$(hostname) python3 "$here/scripts/work/learn.py" collect \
  --runs "$HOME/.openagents/work/runs" "$HOME/.openagents/work/host-runs" ${ab[@]+"${ab[@]}"} \
  ${prev[@]+"${prev[@]}"} --out "$tmp/ledger.jsonl"
gcloud storage cp -q "$tmp/ledger.jsonl" "gs://$bucket/ledger/$name.jsonl"
echo "uploaded $(wc -l < "$tmp/ledger.jsonl") runs -> gs://$bucket/ledger/$name.jsonl"

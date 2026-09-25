#!/bin/sh
# Reproduces the tiered-acceptance measurement (issue #9629) with no model
# calls: every retained frozen suite runs in Docker on the four Microluna
# tasks' retained workspaces, then `accept validity` joins the runs with
# the recorded classes.
#
# Usage: reproduce.sh CODER_ONE_BIN SCRATCH [JOBS]
#
# Classification itself called Jev once per red-at-start test; its answers
# are in records/authority.json, and `accept classify` keeps a suite that
# file already holds, so rerunning it asks nothing again.
set -eu
BIN=$1
R=$2/replay
JOBS=${3:-$HOME/.openagents/terminal-bench/jobs}
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../../.." && pwd)
AF=$REPO/bench/terminal-bench/experiments/2026-09-24-acceptance-first/records
CE=$REPO/bench/terminal-bench/experiments/2026-09-24-candidate-evidence/records
REC=$HERE/records

image() {
  case "$1" in
    embedding-drift-monitor) echo accept-env/embedding-drift-monitor:latest ;;
    sound-change-cascade) echo sound-change-cascade__avnavjz__env-main:latest ;;
    interleaved-vigenere) echo tbench-warm/interleaved-vigenere:environment-37a3e35da105ad0d814a ;;
    fin-saccr-rwa) echo accept-env/fin-saccr-rwa:latest ;;
  esac
}

stage() {
  mkdir -p "$R/$1/$2"
  cp "$3" "$R/$1/$2/suite.accept.json"
  rm -rf "$R/$1/$2/suite"
  cp -r "$4" "$R/$1/$2/suite"
}

for pass in pass2-calibrated pass3-facts; do
  for task in embedding-drift-monitor sound-change-cascade interleaved-vigenere fin-saccr-rwa; do
    stage "$pass" "$task" "$AF/$pass/$task/suite.accept.json" "$AF/$pass/$task/suite"
  done
done
for dir in "$REC"/replay/live-*/*/; do
  name=$(basename "$(dirname "$dir")")
  stage "$name" "$(basename "$dir")" "$dir/suite.accept.json" "$dir/suite"
done

cd "$REPO"
for dir in "$R"/*/; do
  for taskdir in "$dir"*/; do
    task=$(basename "$taskdir")
    [ -f "$taskdir/validity.json" ] && continue
    "$BIN" accept offline "$task" --reuse --jev off --out "$dir" --jobs "$JOBS" \
      --image "$(image "$task")" \
      --kinds snapshot,final,candidate,reconstruction \
      --grades "$CE/candidate-grades" \
      --reconstruction "$CE/reconstructed-v12-embedding-r1-before-review" \
      --workers 4 --containers accept-9629 > "$taskdir/offline.out"
  done
done

# The Coder One snapshot records of the other tasks, as published.
for pass in pass2-calibrated pass3-facts; do
  for dir in "$AF/$pass"/*/; do
    task=$(basename "$dir")
    case "$task" in
      embedding-drift-monitor|sound-change-cascade|interleaved-vigenere|fin-saccr-rwa) continue ;;
    esac
    mkdir -p "$2/published/$pass/$task"
    cp "$dir/validity.json" "$2/published/$pass/$task/validity.json"
  done
done

"$BIN" accept validity "$R"/*/ "$2"/published/*/ --authority "$REC/authority.json" \
  --in-sample embedding-drift-monitor,sound-change-cascade,interleaved-vigenere

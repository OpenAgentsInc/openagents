#!/usr/bin/env bash
# One weekly learning-loop cycle (#11312) on oa-learn-1, run at boot by
# oa-learn.service (scripts/work/learn-job/setup.sh install):
#
# 1. a fresh worktree of origin/main and the VM's session (GitHub token from
#    Secret Manager; gcloud and Vertex act as the VM's account);
# 2. the finder's caches (first boot: from gs://$BUCKET/cache) and the run
#    ledgers the work hosts and benches uploaded to gs://$BUCKET/ledger;
# 3. `learn.py cycle --promote`: training rows from the accepted runs,
#    `scripts/filefind/retrain.sh` under the embedding spend cap, the gate's
#    comparison on the held-out window, and `ranker_gate.py promote` only
#    when the receipt passed;
# 4. on a promotion: the new model.json, its receipt and an entry in
#    docs/inference/filefind-promotions.md, committed and pushed to main;
# 5. the report to gs://$BUCKET/reports/DAY/ and as a comment on the epic
#    (OA_LEARN_REPORT_ISSUE, default #11313); then the VM powers off.
#
# Instance metadata oa-learn-hold=1 skips the cycle and keeps the VM up.
#   run.sh --poweroff   power off unless held (the unit's ExecStopPost)
set -uo pipefail
md=http://metadata.google.internal/computeMetadata/v1/instance
hold() { curl -fsS -H 'Metadata-Flavor: Google' "$md/attributes/oa-learn-hold" 2>/dev/null | grep -qx 1; }
if [[ ${1:-} == --poweroff ]]; then
  hold || sudo -n systemctl poweroff
  exit 0
fi
if hold; then echo "held (metadata oa-learn-hold=1): no cycle"; exit 0; fi

BUCKET=${OA_LEARN_BUCKET:-openagentsgemini-filefind-learn}
B=gs://$BUCKET
CAP=${OA_LEARN_SPEND_CAP_USD:-5}
ISSUE=${OA_LEARN_REPORT_ISSUE:-11313}
repo=$HOME/openagents
home=$HOME/learn
work=$home/work
wt=$home/wt
day=$(date -u +%Y-%m-%d)
cache=$HOME/.cache/openagents/filefind
mkdir -p "$work/train" "$work/ledgers"
export PATH=$HOME/.cargo/bin:$PATH
# Vertex embeddings as the VM's own account (the metadata server's token)
export GCE_METADATA_HOST=${GCE_METADATA_HOST:-metadata.google.internal}
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }

git -C "$repo" fetch -q origin main || { log "fetch failed"; exit 1; }
git -C "$repo" worktree remove --force "$wt" > /dev/null 2>&1
rm -rf "$wt"
git -C "$repo" worktree prune
git -C "$repo" worktree add -q --detach "$wt" origin/main || exit 1
eval "$("$wt/scripts/cloud/dev-env-session.sh")"
python3 -c 'import numpy' 2> /dev/null || sudo -n apt-get install -y -q python3-numpy > /dev/null

if ! ls -d "$cache"/openagents-* > /dev/null 2>&1; then
  log "first run: the finder's index from $B/cache/index.tar"
  mkdir -p "$cache"
  gcloud storage cat "$B/cache/index.tar" | tar -x -C "$cache" || exit 1
fi
if [[ ! -f $work/train/features.pkl ]]; then
  log "first run: the training work dir from $B/cache/work.tar"
  gcloud storage cat "$B/cache/work.tar" | tar -x -C "$work/train" || exit 1
fi
gcloud storage rsync -q "$B/ledger" "$work/ledgers" || log "no ledgers in $B/ledger"
ledgers=("$work"/ledgers/*.jsonl)
[[ -e ${ledgers[0]} ]] || ledgers=()

log "cycle: spend cap \$$CAP"
python3 "$wt/scripts/work/learn.py" cycle --repo "$wt" --work "$work" --promote --spend-cap "$CAP" \
  --runs "$HOME/.openagents/work/runs" ${ledgers[@]+--ledger-in "${ledgers[@]}"} > "$work/cycle.out" 2>&1
log "cycle exit $?"
gcloud storage cp -q "$work/report.json" "$work/report.md" "$work/retrain.log" "$B/reports/$day/" 2> /dev/null
rec=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("receipt_path") or "")' "$work/report.json" 2> /dev/null)
[[ -n $rec && -f $rec ]] && gcloud storage cp -q "$rec" "$B/reports/$day/" 2> /dev/null

promoted=$(python3 -c 'import json,sys; print(int(bool(json.load(open(sys.argv[1])).get("promoted"))))' "$work/report.json" 2> /dev/null || echo 0)
if [[ $promoted == 1 ]]; then
  doc=docs/inference/filefind-promotions.md
  for attempt in 1 2 3 4 5; do
    python3 "$wt/scripts/work/learn.py" record --report "$work/report.json" --doc "$wt/$doc" || break
    git -C "$wt" add scripts/filefind/model.json scripts/filefind/model.receipt.json "$doc"
    git -C "$wt" commit -q -m "filefind: promote a ranker retrained from briefed runs through the gate (#11312)

Scheduled learning-loop run on $(hostname), $day. Receipt: scripts/filefind/model.receipt.json.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" || break
    if git -C "$wt" push -q origin HEAD:main; then log "pushed the promotion"; break; fi
    # main moved: put the promotion on top of it again
    git -C "$wt" fetch -q origin main
    keep=$(mktemp -d)
    cp "$wt/scripts/filefind/model.json" "$wt/scripts/filefind/model.receipt.json" "$keep/"
    git -C "$wt" reset -q --hard origin/main
    cp "$keep/model.json" "$keep/model.receipt.json" "$wt/scripts/filefind/"
    rm -rf "$keep"
  done
fi
if [[ -n ${GH_TOKEN:-} && -s $work/report.md ]]; then
  gh issue comment "$ISSUE" --repo OpenAgentsInc/openagents --body-file "$work/report.md" > /dev/null \
    && log "report on #$ISSUE"
fi
exit 0

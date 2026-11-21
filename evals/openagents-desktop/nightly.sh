#!/usr/bin/env bash
# Firm reason: 5. The scheduled Coder Desktop evals (#13705): git worktrees, coax/build.sh,
# evals/openagents-desktop/run.sh and compare.py, and gh for the one issue a bad run opens.
#
#   evals/openagents-desktop/nightly.sh night [--commit REV] [--out DIR]
#       Tier 0, Tier 1 and compaction (free), Tier 2 (15 live tasks, about $0.50) and, on
#       macOS, Tier 3 (the real Coder.app on the mock worker, free). Tier 2 is compared with
#       the previous night's run (the committed tier2/baseline before there is one).
#   evals/openagents-desktop/nightly.sh week [--commit REV] [--out DIR]
#       Tier 4 (4 golden trajectories and the 25-task benchmark slice, about $1.20), compared with
#       the committed tier4/baseline: the weekly and pre-release scorecard.
#   evals/openagents-desktop/nightly.sh trend
#       The trend table of every kept run (pass rate, cost a task, p95 wall, regressions).
#   evals/openagents-desktop/nightly.sh install
#       Prints the crontab lines that run it each night at 03:00 and each Sunday at 05:00 (as
#       coax/remote/mutation-sweep.sh install does on a Mac); it changes nothing itself. It makes
#       the checkout those lines run from (STATE/checkout, moved to origin/main before each run).
#
# It needs a Mac with the model credentials Tier 2 links (see README.md), not a build host:
# Tier 3 runs only on macOS. It never touches the checkout it is
# called from: it checks REV (default origin/main-green, the newest head the green-main pipeline
# passed; origin/main when there is none) out into its own worktree under the state folder,
# builds the compiler there, runs, keeps the run folders and removes the worktree.
#
# Spend: each task is capped by its own chat budget (Tier 2: $1.41 a night at most; Tier 4:
# $2.95 a run at most). OA_DESKTOP_EVALS_MONTHLY_USD (default 40) caps the sum of
# the kept runs' cost over 30 days: past it, the paid tiers are skipped and the run says so.
# Typical: about $0.50 a night and $1.20 a week, near $20 a month.
#
# A run with a regression (a task that passed in the run it is compared with and fails now),
# a pass rate down more than 5 points, a failing Tier 3 flow or a run that could not finish
# opens one issue, "Scheduled Coder Desktop evals failed", or comments on it while it is open
# (OA_DESKTOP_EVALS_ISSUES=0: no issue).
#
# Settings: OA_DESKTOP_EVALS_DIR (state: ~/.openagents/coder-desktop-evals), OA_DESKTOP_EVALS_TIER3
# (1 on macOS: run Tier 3 with the night), OA_DESKTOP_EVALS_MONTHLY_USD, OA_DESKTOP_EVALS_ISSUES,
# OA_DESKTOP_EVALS_REPO (OpenAgentsInc/openagents).
set -uo pipefail
export LC_ALL=C BROWSER=/usr/bin/true
here="$(cd "$(dirname "$0")" && pwd -P)"
repo="$(git -C "$here" rev-parse --show-toplevel)" || exit 2
state="${OA_DESKTOP_EVALS_DIR:-$HOME/.openagents/coder-desktop-evals}"
monthly_usd="${OA_DESKTOP_EVALS_MONTHLY_USD:-40}"
gh_repo="${OA_DESKTOP_EVALS_REPO:-OpenAgentsInc/openagents}"
title="Scheduled Coder Desktop evals failed"

mode="${1:-}"; shift || true
commit="" out=""
while (( $# )); do
  case "$1" in
    --commit) commit="${2:?--commit needs a revision}"; shift 2 ;;
    --out) out="${2:?--out needs a folder}"; shift 2 ;;
    *) echo "nightly.sh: unknown argument $1" >&2; exit 2 ;;
  esac
done
case "$mode" in
  night|week|trend|install) ;;
  *) sed -n '2,/^set -uo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
mkdir -p "$state/runs"

if [[ "$mode" == install ]]; then
  co="$state/checkout"
  if [[ ! -e "$co/.git" ]]; then
    git -C "$repo" fetch -q origin main || exit 2
    git -C "$repo" worktree add -q --detach "$co" origin/main || exit 2
  fi
  run="cd '$co' && git fetch -q origin && git checkout -q --detach origin/main && PATH='$PATH' evals/openagents-desktop/nightly.sh"
  echo "# Coder Desktop evals (#13705): add with 'crontab -e', or paste this whole block into a terminal."
  echo "( crontab -l 2> /dev/null; echo \"0 3 * * * $run night > '$state/night.log' 2>&1\"; echo \"0 5 * * 0 $run week > '$state/week.log' 2>&1\" ) | crontab -"
  exit 0
fi
if [[ "$mode" == trend ]]; then
  exec python3 "$here/compare.py" trend --results "$state/runs" --out "$state/trend.md"
fi

# One scheduled run at a time.
lock="$state/lock"
if ! mkdir "$lock" 2> /dev/null; then
  if [[ -f "$lock/pid" ]] && kill -0 "$(cat "$lock/pid")" 2> /dev/null; then
    echo "nightly.sh: a run is already going (pid $(cat "$lock/pid"))"; exit 0
  fi
  rm -rf "$lock"; mkdir "$lock" || exit 2
fi
echo $$ > "$lock/pid"
wt=""
cleanup() {
  [[ -z "$wt" ]] || git -C "$repo" worktree remove --force "$wt" > /dev/null 2>&1 || rm -rf "$wt"
  git -C "$repo" worktree prune > /dev/null 2>&1
  rm -rf "$lock"
}
trap cleanup EXIT

# The kept runs' cost over the last 30 days, in micros.
spent_micros() {
  python3 - "$state/runs" <<'PY'
import json, sys, time
from pathlib import Path
cut = time.time() - 30 * 86400
total = 0
for p in Path(sys.argv[1]).glob("*/summary.json"):
    if p.stat().st_mtime >= cut:
        try:
            total += int(json.loads(p.read_text()).get("cost_usd_micros", 0))
        except (OSError, ValueError):
            pass
print(total)
PY
}

if [[ -z "$commit" ]]; then
  git -C "$repo" fetch -q origin main main-green 2> /dev/null || git -C "$repo" fetch -q origin main 2> /dev/null || true
  commit="origin/main-green"
  git -C "$repo" rev-parse -q --verify "$commit^{commit}" > /dev/null || commit="origin/main"
fi
sha="$(git -C "$repo" rev-parse --verify "$commit^{commit}")" || { echo "nightly.sh: no commit $commit" >&2; exit 2; }
short="${sha:0:10}"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
out="${out:-$state/out/$mode-$stamp-$short}"
mkdir -p "$out"
report="$out/report.md"
: > "$report"
bad=()

say() { echo "$*"; echo "$*" >> "$report"; }

spent="$(spent_micros)"
if (( spent >= monthly_usd * 1000000 )); then
  say "nightly.sh: skipped the paid tiers: the kept runs cost \$$(( spent / 1000000 )) over 30 days (OA_DESKTOP_EVALS_MONTHLY_USD=$monthly_usd)"
  paid=0
else
  paid=1
fi

wt="$state/wt-$short-$$"
git -C "$repo" worktree add -q --detach "$wt" "$sha" || { echo "nightly.sh: could not check out $short" >&2; exit 2; }
if ! (cd "$wt" && coax/build.sh) > "$out/build.log" 2>&1; then
  bad+=("the compiler did not build at $short (build.log)")
fi

# run TIER LABEL: one tier in the worktree; its run folder goes to the kept runs.
newest() { ls -1dt "$wt/evals/openagents-desktop/results/$1"-* 2> /dev/null | head -n 1; }
run_tier() {
  local tier="$1" log="$out/tier$1.log" s=0
  (cd "$wt" && evals/openagents-desktop/run.sh --tier "$tier") > "$log" 2>&1 || s=$?
  say "tier $tier: status $s ($log)"
  grep -E '^\[(PASS|FAIL)\]|^Summary:' "$log" >> "$report"
  return "$s"
}
compare_with() {   # tier run_folder baseline_summary
  local tier="$1" dir="$2" base="$3" name
  name="$(basename "$dir")"
  cp -R "$dir" "$out/$name"   # the whole run folder (wire logs, journals) beside the report
  mkdir -p "$state/runs/$name"   # the trend and the spend cap read only the summaries
  cp "$dir/summary.json" "$dir/report.md" "$state/runs/$name/"
  if [[ -n "$base" && -f "$base" ]]; then
    if ! python3 "$here/compare.py" diff "$base" "$dir/summary.json" --out "$out/compare-tier$tier.md" --fail-on-regression > /dev/null; then
      bad+=("tier $tier regressed against $(dirname "$base") (compare-tier$tier.md)")
    fi
    cat "$out/compare-tier$tier.md" >> "$report"
  fi
}

if (( ${#bad[@]} == 0 )); then
  if [[ "$mode" == night ]]; then
    run_tier all || bad+=("Tier 0, Tier 1 or compaction failed (tierall.log)")
    if (( paid )); then
      prev="$(ls -1dt "$state/runs"/tier2-* 2> /dev/null | head -n 1)"
      base="${prev:+$prev/summary.json}"; base="${base:-$wt/evals/openagents-desktop/tier2/baseline/summary.json}"
      run_tier 2 || true
      dir="$(newest tier2)"
      if [[ -n "$dir" && -f "$dir/summary.json" ]]; then compare_with 2 "$dir" "$base"
      else bad+=("tier 2 wrote no summary (tier2.log)"); fi
    fi
    if [[ "$(uname -s)" == Darwin && "${OA_DESKTOP_EVALS_TIER3:-1}" != 0 ]]; then
      run_tier 3 || bad+=("tier 3 failed (tier3.log; captures under the worktree's results/, kept in $out/tier3)")
      t3="$(newest tier3-2)"; [[ -z "$t3" ]] || cp -R "$t3" "$out/tier3"
    fi
  else
    if (( paid )); then
      run_tier 4 || true
      dir="$(newest tier4)"
      if [[ -n "$dir" && -f "$dir/summary.json" ]]; then compare_with 4 "$dir" "$wt/evals/openagents-desktop/tier4/baseline/summary.json"
      else bad+=("tier 4 wrote no summary (tier4.log)"); fi
    fi
  fi
fi

python3 "$here/compare.py" trend --results "$state/runs" --out "$state/trend.md" > /dev/null || true
say "trend: $state/trend.md"

if (( ${#bad[@]} == 0 )); then
  say "nightly.sh: $mode at $short passed"
  exit 0
fi
for b in "${bad[@]}"; do say "FAIL: $b"; done
if [[ "${OA_DESKTOP_EVALS_ISSUES:-1}" != 0 ]] && command -v gh > /dev/null; then
  body="The scheduled Coder Desktop evals (\`evals/openagents-desktop/nightly.sh $mode\`, #13705) failed at \`$sha\`. The run folder is \`$out/\` on the machine that ran it.

$(printf -- '- %s\n' "${bad[@]}")

$(head -c 6000 "$report")"
  num="$(gh issue list --repo "$gh_repo" --state open --search "\"$title\" in:title" --json number --jq '.[0].number' 2> /dev/null)"
  if [[ -n "$num" ]]; then
    gh issue comment "$num" --repo "$gh_repo" --body "$body" > /dev/null 2>&1 || true
  else
    gh issue create --repo "$gh_repo" --title "$title" --body "$body" > /dev/null 2>&1 || true
  fi
fi
exit 1

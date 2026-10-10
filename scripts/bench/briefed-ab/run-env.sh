#!/usr/bin/env bash
# Run frozen plans on a cloud dev environment (#11211), then power it off.
#
#   setsid nohup scripts/bench/briefed-ab/run-env.sh PLAN... > ~/ab/run-env.log 2>&1 &
#
# Each PLAN is a plans/*.json name without the extension. Each plan's trials
# run (ab.py batch --plan), then its judging. Everything runs on this
# machine (AB_HOST=local): agents, worktrees, builds and grading. Plans whose
# issues lack validation are validated first, in a second build checkout,
# before any of that plan's trials. When the list is done, or the script
# exits for any reason, the VM powers off (it bills only its disk while
# stopped); AB_NO_POWEROFF=1 keeps it on.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
cd "$here"
export PATH=$HOME/.cargo/bin:$PATH AB_HOST=local
export AB_AGENT_BIN=$here/.work/bin/briefed-agent-env
set -a; . "$HOME/.openagents/dev-env.env"; set +a
mkdir -p "$HOME/ab/bin" .work
cp remote/* "$HOME/ab/bin/"
touch "$HOME/.openagents/keep-awake"
finish() {
  rm -f "$HOME/.openagents/keep-awake"
  [ -n "${AB_NO_POWEROFF:-}" ] || sudo -n poweroff
}
trap finish EXIT
for plan in "$@"; do
  echo "== $plan $(date -u +%FT%TZ)"
  need=$(python3 - "$plan" <<'PY'
import json, os, sys
p = json.load(open(f"plans/{sys.argv[1]}.json"))
print(" ".join(f"{i}:{json.load(open(f'tasks/{i}/task.json'))['fix']}" for i in p["issues"]
               if not os.path.exists(f"tasks/{i}/validation.json")))
PY
)
  if [ -n "$need" ]; then
    echo "validating: $need"
    AB_BUILD=v python3 prepare.py --validate --slot 9 $need
  fi
  tag=$(python3 -c "import json,sys; print(json.load(open('plans/$plan.json'))['tag'])")
  python3 ab.py batch --plan "plans/$plan.json"
  python3 ab.py judge --tag "$tag" --workers 3
  # Publish the plan's rows and report to main (rebase and retry once).
  base=$(python3 -c "import json; a=json.load(open('plans/$plan.json'))['arms']; print(a[0])")
  python3 ab.py report --tag "$tag" --csv "results/$tag-trials.csv" > /dev/null
  {
    echo "# $tag (plans/$plan.json)"; echo
    python3 ab.py report --tag "$tag" --md; echo
    for arm in $(python3 -c "import json; print(' '.join(json.load(open('plans/$plan.json'))['arms'][1:]))"); do
      echo "## $arm vs $base"; echo; python3 ab.py report --tag "$tag" --s2 "$base,$arm"; echo
    done
    echo "## Tools"; echo; python3 ab.py report --tag "$tag" --md --tools
  } > "results/$tag.md"
  git -C "$here" add "results/$tag-trials.csv" "results/$tag.md"
  git -C "$here" commit -q -m "Briefed-agent A/B (#11211): results of plans/$plan.json, run on $(hostname)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" && \
    { git -C "$here" pull -q --rebase origin main && git -C "$here" push -q origin HEAD:main || \
      { sleep 30; git -C "$here" pull -q --rebase origin main && git -C "$here" push -q origin HEAD:main; }; }
  echo "== $plan done $(date -u +%FT%TZ)"
done

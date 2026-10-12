#!/usr/bin/env bash
# Write results/<tag>.md and results/<tag>-trials.csv for a frozen plan (#11309):
# the per-arm table, the frozen --s2 analysis of every arm against BASE, the
# two-judge comparison (judge2.py) and the same --s2 analysis with judge 2 in
# place of judge 1, so a missing or disputed judge-1 verdict is visible.
#   write_report.sh PLAN BASE      e.g. write_report.sh ablate-levers B0
#                                       write_report.sh s2b A
set -euo pipefail
cd "$(dirname "$0")"
plan=$1 base=$2
tag=$(python3 -c "import json; print(json.load(open('plans/$plan.json'))['tag'])")
arms=$(python3 -c "import json; print(' '.join(a for a in json.load(open('plans/$plan.json'))['arms'] if a != '$base'))")
work=${AB_WORK:-.work}
j2="$tag-j2"
rm -rf "$work/results/$j2"; cp -R "$work/results/$tag" "$work/results/$j2"
for d in "$work/results/$j2"/*/; do [ -f "$d/judge2.json" ] && cp "$d/judge2.json" "$d/judge.json" || rm -f "$d/judge.json"; done
missing=$(python3 - "$work/results/$tag" <<'PY'
import json, sys
from pathlib import Path
n = m = 0
for d in Path(sys.argv[1]).iterdir():
    if (d / "result.json").exists():
        n += 1
        j = d / "judge.json"
        m += (not j.exists()) or "error" in json.loads(j.read_text())
print(f"{m} of {n}")
PY
)
out=results/$tag.md
{
  echo "# $tag (plans/$plan.json)"
  echo
  [ -f "results/$tag-decision.md" ] && { cat "results/$tag-decision.md"; echo; }
  echo "Trials without a judge-1 verdict (counted as not accepted below): $missing."
  echo
  python3 ab.py report --tag "$tag" --md
  for a in $arms; do
    echo; echo "## $a vs $base"; echo
    python3 ab.py report --tag "$tag" --s2 "$base,$a"
  done
  for a in $arms; do
    echo; echo "## Two judges: $a vs $base"; echo
    python3 judge2.py report --tag "$tag" --base "$base" --cand "$a"
  done
  for a in $arms; do
    echo; echo "## $a vs $base with judge 2 (Gemini) as the judge"; echo
    python3 ab.py report --tag "$j2" --s2 "$base,$a" | sed -n '1,12p'
  done
} > "$out"
python3 ab.py report --tag "$tag" --csv "results/$tag-trials.csv" > /dev/null
rm -rf "$work/results/$j2"
echo "wrote $out and results/$tag-trials.csv"

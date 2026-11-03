#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: runs the report checks and copies their output to the bucket)
# Reports on a release that is already live (release.sh starts it in the
# background). Nothing here stops or rolls back a release. Each report goes to
# gs://openagentsgemini-openagents-web/releases/STAMP/:
#   AFTER       the golden walk and Everglade's 60 FPS check
#   JOURNEY     the maker journey, desktop and phone
#   REALMS.tsv  every world at dpr 1 and 2 and as a phone (realm-perf.coax, #12649),
#   REALMS.txt  compared with the last release's
#   SUITES      the Coax suites whose sources changed since PREVIOUS
#   REVIEW      the project review report PREVIOUS..COMMIT
#
#   after-release.sh COMMIT STAMP [PREVIOUS]
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
full="$1" stamp="$2" previous="${3:-}"
source "$here/release-lib.sh"
site=https://openagents.com
put() { gcloud storage cp "$1" "gs://$prod_bucket/releases/$stamp/$2" --quiet > /dev/null 2>&1 || true; }
to="$(command -v timeout || command -v gtimeout || true)"
limit() { local s=$1; shift; if [[ -n "$to" ]]; then "$to" --kill-after=10 "$s" "$@"; else "$@"; fi; }

work="$(mktemp -d)"; tree="$work/t"
git -C "$repo" worktree add -q --detach "$tree" "$full" || exit 1
trap 'git -C "$repo" worktree remove --force "$tree" 2>/dev/null || true' EXIT

{ limit 1800 env GOLDENS_FULL=1 bun "$tree/coax/web/goldens.ts" "$site" && echo "after: goldens passed" ||
    echo "after: goldens differ (fix or retake: bun coax/web/goldens.ts https://staging.openagents.com --update)"
  limit 900 bun "$tree/coax/web/perf.ts" "$site" && echo "after: 60 FPS held" || echo "after: 60 FPS missed"
  [[ ! -f "$tree/coax/web/tests/home_live_browser.ts" ]] || { limit 300 bun "$tree/coax/web/tests/home_live_browser.ts" "$site" && echo "after: live homepage passed" || echo "after: live homepage failed"; }
} > "$work/after" 2>&1
put "$work/after" AFTER

compiler "$tree" > /dev/null || { echo "after: no compiler for the remaining reports" > "$work/realms.txt"; put "$work/realms.txt" REALMS.txt; exit 0; }

for size in desktop phone; do
  phone=(); [[ "$size" == phone ]] && phone=(--phone)
  step=$(cd "$tree" && limit 600 coax/web/tool.sh forge-journey "$site" ${phone[@]+"${phone[@]}"} 2>&1 | grep -E '^(ok|FAIL) ' | tail -1)
  echo "journey $size: ${step:-did not run}"
done > "$work/journey"
put "$work/journey" JOURNEY

prev_url="$(gcloud storage ls "gs://$prod_bucket/releases/*/REALMS.tsv" 2>/dev/null | sort | tail -1)"
[[ -z "$prev_url" ]] || gcloud storage cp "$prev_url" "$work/before.tsv" --quiet > /dev/null 2>&1 || true
for part in 1 2 3; do
  ( cd "$tree" && limit 900 coax/web/tool.sh realm-perf "$site" --shard "$part/3" --previous "$work/before.tsv" --out "$work/realms.$part.tsv" ) >> "$work/realms.txt" 2>&1 || true
  cat "$work/realms.$part.tsv" >> "$work/realms.tsv" 2>/dev/null || true
done
echo "after: every world's 60 FPS report, against ${prev_url:-no earlier report}" >> "$work/realms.txt"
[[ ! -s "$work/realms.tsv" ]] || put "$work/realms.tsv" REALMS.tsv
put "$work/realms.txt" REALMS.txt

since=(); [[ -n "$previous" ]] && since=(--since "$previous")
{ ( cd "$tree" && limit 3600 coax/web/tool.sh suites "${since[@]+"${since[@]}"}" ) && echo "after: suites passed" ||
    echo "after: a suite failed (coax/web/tool.sh suites --only PATH)"; } > "$work/suites" 2>&1
put "$work/suites" SUITES

if [[ -n "$previous" ]] && git -C "$repo" merge-base --is-ancestor "$previous" "$full" 2>/dev/null; then
  [[ -x "$tree/apps/openagents-cli/build/openagents" ]] || "$tree/apps/openagents-cli/build.sh" > /dev/null 2>&1 || true
  ( cd "$tree" && limit 180 apps/openagents-cli/build/openagents project review report --from "$previous" --to "$full" --notes ) > "$work/review" 2>&1 ||
    echo "review: the report did not finish" >> "$work/review"
  put "$work/review" REVIEW
fi
echo "after: done ($work)"

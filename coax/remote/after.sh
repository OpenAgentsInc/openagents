#!/usr/bin/env bash
# Firm reason: 5. The green-main pipeline's after-landing check
# (docs/verse/green-main-pipeline.md, #13102): git, gh, jq and
# coax/remote/remote.sh. The checks run on the remote build hosts.
#
#   coax/remote/after.sh [--once]   one run on the newest head of main, unless one
#                                   is running or that head was checked
#   coax/remote/after.sh --watch    the same every minute, for ever, and the
#                                   30-minute revert (integrate.sh --once --revert;
#                                   --no-revert or OA_AFTER_REVERT=0: no reverts)
#   coax/remote/after.sh state      main, main-green, the last run and the red list
#   coax/remote/after.sh red        the red list: failures on main and who owns them
#   coax/remote/after.sh history [NAME]
#                                   compile-limit use and wasm sizes per commit
#
# Two runs, each a set of jobs side by side on host slots (coax/remote/check.sh):
#
# A fast run, on every new head H once main-green exists: only what changed
# since main-green G. The suites whose sources changed (check.sh since G, in
# OA_AFTER_FAST_PARTS parts), the web modules the change touched and the
# report checks (check.sh web G), the room build when the change reaches it
# (check.sh room-since G), and coax/tests/run.sh in 8 parts when compiler
# sources changed. Everything else
# passed at G and reads no changed file, so it passes at H. A head that changes
# only documents is green with no job.
#
# With no main-green, a fast run compares against the head of the last full
# run and runs that run's failures again: when they and everything the change
# touched pass, the head is green. A new failure is bisected from that head.
#
# A full run, when there is no main-green (at most once an hour), and at most
# every OA_AFTER_GAP seconds after that: every suite in OA_AFTER_PARTS parts, every web module
# against its budget in OA_AFTER_MODULE_PARTS parts, the report checks and the
# room build (check.sh tools), and coax/tests/run.sh in 8 parts when compiler
# sources changed since the last full green run (always, when there is none).
#
# Green: main-green moves to H, and the red-list entries H fixes close. Then,
# in the background and at most every OA_AFTER_PAGES_GAP seconds,
# coax/remote/pages.sh builds H's pages into the shared store (#13119).
# Red: each failing check is bisected between main-green and H (several
# commits at once, OA_BISECT_WIDTH, running only that check). For the culprit
# it files an issue naming the commit, the check, its output and the agent in
# the commit's "Agent:" trailer, messages that agent, and adds an entry to the
# red list. A failure already on the red list is not bisected again; each run
# that still sees it stamps the entry (seen), and integrate.sh --revert
# reverts a culprit still failing 30 minutes after its entry opened. A check
# that fails on H and passes when run again is flaky: reported, no culprit.
# A full run that fails where the fast runs passed bisects from the last full
# green run and moves main-green back to it.
# Before the first green run there is no main-green to bisect against: every
# failure goes on the red list owned by nobody, under one issue.
#
# A job a host could not finish (a spot host stopped under it) runs again, up
# to OA_AFTER_RETRIES times, before the run counts as unfinished.
#
# Results: OA_PIPELINE_DIR/releases/after/COMMIT/ (each job's output and
# report, run.tsv with each job's status and seconds, kind and status),
# history.tsv (commit, time, kind, name, value, limit), runs.tsv (time, commit,
# kind, status, seconds, slot-seconds, jobs) and red.json.
#
# Settings: OA_AFTER_PARTS (6), OA_AFTER_MODULE_PARTS (3), OA_AFTER_FAST_PARTS
# (3), OA_AFTER_GAP (10800: seconds between full runs), OA_AFTER_FAST_GAP (0:
# seconds between fast runs), OA_AFTER_COMPILER (auto, always or never),
# OA_BISECT_WIDTH (4), OA_BISECT_CHECKS (4: failing checks bisected one by
# one; the rest wait for the next run), OA_AFTER_TIMEOUT (3000 s a job),
# OA_AFTER_RETRIES (2), OA_AFTER_PAGES (0: no pages), OA_AFTER_PAGES_GAP
# (3600), OA_AFTER_CHECK (a command run as CMD COMMIT OUTDIR CHECK-ARGS... in
# place of the remote check, for scratch runs), and
# coax/remote/pipeline-lib.sh's OA_PIPELINE_*.
set -uo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
source "$here/pipeline-lib.sh"
parts="${OA_AFTER_PARTS:-6}"
module_parts="${OA_AFTER_MODULE_PARTS:-3}"
fast_parts="${OA_AFTER_FAST_PARTS:-3}"
gap="${OA_AFTER_GAP:-10800}"   # a full run costs about $0.30 of host time (2026-11-09)
fast_gap="${OA_AFTER_FAST_GAP:-0}"
retries="${OA_AFTER_RETRIES:-2}"
pages_gap="${OA_AFTER_PAGES_GAP:-3600}"
compiler_mode="${OA_AFTER_COMPILER:-auto}"
width="${OA_BISECT_WIDTH:-4}"
bisect_checks="${OA_BISECT_CHECKS:-4}"
limit="${OA_AFTER_TIMEOUT:-3000}"
results="$pl_dir/releases/after"
history="$results/history.tsv"

case "${1:-}" in
  state)
    git fetch -q "$pl_remote" "+refs/heads/$pl_main:refs/remotes/$pl_remote/$pl_main" 2> /dev/null
    git fetch -q "$pl_remote" "+refs/heads/$pl_green:refs/remotes/$pl_remote/$pl_green" 2> /dev/null || git update-ref -d "refs/remotes/$pl_remote/$pl_green" 2> /dev/null
    h="$(git rev-parse -q --verify "refs/remotes/$pl_remote/$pl_main" 2> /dev/null || true)"
    g="$(git rev-parse -q --verify "refs/remotes/$pl_remote/$pl_green" 2> /dev/null || true)"
    echo "$pl_main        ${h:0:10} $(git log -1 --format='%s' "$h" 2> /dev/null | cut -c1-70)"
    if [[ -z "$g" ]]; then echo "$pl_green  none: no run has passed yet"
    else echo "$pl_green  ${g:0:10} $(git log -1 --format='%cd' --date=format-local:'%Y-%m-%d %H:%M' "$g"), $(git rev-list --count "$g..$h" 2> /dev/null) commit(s) behind $pl_main"; fi
    if [[ -s "$results/runs.tsv" ]]; then
      echo "last runs (time, commit, kind, status, seconds, slot-seconds, jobs):"
      tail -n 5 "$results/runs.tsv" | awk -F'\t' '{ printf "  %s  %s  %-4s  %-10s %5s s  %6s slot-s  %s job(s)\n", $1, $2, $3, $4, $5, $6, $7 }'
    fi
    n="$(jq '[.[] | select(.status == "open")] | length' "$pl_red")"
    echo "red list: $n open"
    jq -r '.[] | select(.status == "open") | "  \(.check): \(if .culprit == "" then "no culprit" else .culprit[0:10] + " by " + .agent end)\(if .issue then " (#" + .issue + ")" else "" end)"' "$pl_red" | head -n 40
    exit 0 ;;
  red)
    jq -r '.[] | "\(.status)\t\(.check)\t\(if .culprit == "" then "-" else .culprit[0:10] end) \(.subject)\tagent \(.agent)\t#\(.issue // "-")\t\(.opened | strflocaltime("%Y-%m-%d %H:%M %Z"))"' "$pl_red"
    exit 0 ;;
  history)
    [[ -f "$history" ]] || exit 0
    if [[ -n "${2:-}" ]]; then awk -F'\t' -v n="$2" '$4 == n' "$history"; else cat "$history"; fi
    exit 0 ;;
esac
watch=0 revert="${OA_AFTER_REVERT:-1}"
while (($#)); do
  case "$1" in
    --once) shift ;;
    --watch) watch=1; shift ;;
    --no-revert) revert=0; shift ;;
    -h|--help) sed -n '2,62p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "after: unknown option $1" >&2; exit 2 ;;
  esac
done
root="$(git rev-parse --show-toplevel 2> /dev/null)" || { echo "after: not in a Git checkout" >&2; exit 2; }
cd "$root" || exit 2
main_ref="refs/remotes/$pl_remote/$pl_main"
green_ref="refs/remotes/$pl_remote/$pl_green"

fetch() {
  git fetch -q "$pl_remote" "+refs/heads/$pl_main:$main_ref" 2> /dev/null || return 1
  git fetch -q "$pl_remote" "+refs/heads/$pl_green:$green_ref" 2> /dev/null || git update-ref -d "$green_ref" 2> /dev/null
  return 0
}
# One check on one commit: 0 passed, 1 failed, 97 the hosts could not run it
# (after OA_AFTER_RETRIES more tries: a spot host can stop under a job).
check() {   # commit outdir check-args...
  local commit="$1" dir="$2" s try=0; shift 2
  local t0; t0=$(date +%s)
  while true; do
    rm -rf "$dir"; mkdir -p "$dir"
    if [[ -n "${OA_AFTER_CHECK:-}" ]]; then
      # shellcheck disable=SC2086
      timeout "$limit" $OA_AFTER_CHECK "$commit" "$dir" "$@" > "$dir/out" 2>&1; s=$?
    else
      OA_REMOTE_TIMEOUT=$(( limit - 60 )) timeout "$limit" "$here/remote.sh" run --commit "$commit" --out "$dir" --no-fallback -- coax/remote/check.sh "$@" > "$dir/out" 2>&1; s=$?
    fi
    case "$s" in 0|1) break ;; 124) echo "after: the check ran past $limit s" >> "$dir/out"; s=97; break ;; 97|255) s=97 ;; *) s=1; break ;; esac
    (( try < retries )) || break
    try=$((try + 1)); cp "$dir/out" "$dir/../$(basename "$dir").try$try.out" 2> /dev/null
  done
  printf '%s\t%s\t%s\t%s\n' "$*" "$s" "$(( $(date +%s) - t0 ))" "${commit:0:10}" >> "$dir/../run.tsv"
  return "$s"
}
# A failing item ("suite PATH", "step NAME", "compiler UNIT") as the check
# that runs it alone.
args_for() {   # item job-args...
  local item="$1"; shift
  case "$item" in
    suite\ *) echo "only ${item#suite }" ;;
    step\ wasm) echo "wasm" ;;
    module\ *) echo "module ${item#module }" ;;
    compiler\ *) echo "unit ${item#compiler }" ;;
    job\ *) echo "${item#job }" ;;
    step\ tools-check|step\ everglade-parity|step\ wgsl|step\ realm-pages|step\ changelog-page) echo "reports" ;;
    step\ room-build) echo "room" ;;
    *) echo "$*" ;;   # the whole job again
  esac
}

# Bisect one check between good (passes) and the commits after it up to bad.
# Prints the culprit, "flaky" when bad passes when run again, "infra" when the
# hosts could not run a check, or "" when it did not settle in 20 rounds.
bisect() {   # dir good bad check-args...
  local dir="$1" good="$2" bad="$3"; shift 3
  local list=() c
  while read -r c; do list+=("$c"); done < <(git rev-list --reverse --first-parent "$good..$bad")
  (( ${#list[@]} > 0 )) || { echo ""; return 0; }
  local lo=-1 hi=$(( ${#list[@]} - 1 )) round=0 confirmed=0
  # lo: the index known to pass (-1: good), hi: the index known to fail.
  while (( hi - lo > 1 || confirmed == 0 )); do
    round=$((round + 1))
    local points=() n=$(( hi - lo - 1 )) i
    if (( confirmed == 0 )); then points+=("$hi"); fi
    local want=$(( width - ${#points[@]} ))
    if (( n > 0 && want > 0 )); then
      if (( n <= want )); then for (( i = lo + 1; i < hi; i++ )); do points+=("$i"); done
      else for (( i = 1; i <= want; i++ )); do points+=("$(( lo + (n + 1) * i / (want + 1) ))"); done
      fi
    fi
    local pids=() p
    for p in "${points[@]}"; do check "${list[$p]}" "$dir/r$round-${list[$p]:0:10}" "$@" & pids+=($!); done
    local st=() k
    for k in "${!pids[@]}"; do wait "${pids[$k]}"; st+=($?); done
    # Lowest failing point, highest passing point under it.
    local newhi=$hi newlo=$lo
    for k in "${!points[@]}"; do
      p="${points[$k]}"
      case "${st[$k]}" in
        97) echo "infra"; return 0 ;;
        1) (( p < newhi )) && newhi=$p ;;
      esac
    done
    if (( confirmed == 0 )); then
      for k in "${!points[@]}"; do [[ "${points[$k]}" == "$hi" ]] && { [[ "${st[$k]}" == 1 ]] || { echo flaky; return 0; }; }; done
      confirmed=1
    fi
    for k in "${!points[@]}"; do
      p="${points[$k]}"
      [[ "${st[$k]}" == 0 ]] && (( p < newhi && p > newlo )) && newlo=$p
    done
    lo=$newlo; hi=$newhi
    (( round < 20 )) || { echo ""; return 0; }
  done
  echo "${list[$hi]}"
}

file_issue() {   # check culprit agent outfile → issue number ("" when none)
  [[ "$pl_issues" == 1 ]] || { echo ""; return; }
  local check="$1" culprit="$2" agent="$3" out="$4" subject body url
  subject="$(git log -1 --format=%s "$culprit")"
  body="The after-landing check \`$check\` fails on \`$pl_main\` since ${culprit:0:10} (\"$subject\"), by agent \`$agent\`.

Bisected between \`$pl_green\` and the head by \`coax/remote/after.sh\` (docs/verse/green-main-pipeline.md).

Output (last 60 lines):

\`\`\`
$(tail -n 60 "$out" 2> /dev/null | cut -c1-300)
\`\`\`

Fixing this comes before new work for \`$agent\`. If it still fails 30 minutes from now, the integrator reverts ${culprit:0:10}. Run the check alone with \`coax/remote/remote.sh run -- coax/remote/check.sh $(args_for "$check")\`."
  url="$(gh issue create --repo "$pl_repo" --title "Red main: $check fails since ${culprit:0:10}" --body "$body" 2> /dev/null)" || { echo ""; return; }
  echo "${url##*/}"
}

pages_of() {   # head: its pages into the shared store, once, in the background (OA_AFTER_PAGES=0: not)
  [[ "${OA_AFTER_PAGES:-1}" != 0 && -z "${OA_AFTER_CHECK:-}" ]] || return 0
  due last-pages "$pages_gap" || return 0
  date +%s > "$results/last-pages"
  ( "$here/pages.sh" build "$1" > "$results/$1/pages.log" 2>&1 && pl_say "after: ${1:0:10}'s pages are in the shared store" ) &
}


due() {   # stamp seconds: whether the stamp is that old, or missing
  [[ ! -f "$results/$1" ]] || (( $(date +%s) - $(cat "$results/$1") >= $2 ))
}
# The open red-list entries a run still sees failing get its time (seen):
# integrate.sh --revert reverts a culprit still failing 30 minutes after its
# entry opened.
still_red() {   # file of failing checks, time
  [[ -s "$1" ]] || return 0
  pl_red_update '($f | split("\n")) as $l | map(if .status == "open" and (.check as $c | $l | index($c)) then .seen = ($t | tonumber) else . end)' --rawfile f "$1" --arg t "$2"
}
record_run() {   # head kind status started jobs
  local slot; slot="$(cat "$results/$1/run.tsv" "$results/$1"/bisect-*/run.tsv 2> /dev/null | awk -F'\t' '{ s += $3 } END { print s + 0 }')"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$(date '+%Y-%m-%d %H:%M')" "${1:0:10}" "$2" "$3" "$(( $(date +%s) - $4 ))" "$slot" "$5" >> "$results/runs.tsv"
  echo "$(( $(date +%s) - $4 )) s, $slot slot-seconds"
}
# H passed: main-green moves to it. A full run also records it as the last
# head where every check ran and passed.
go_green() {   # head green kind started jobs note
  local head="$1" green="$2" kind="$3" dir="$results/$1" cost
  echo green > "$dir/status"; rm -f "$dir/failed"
  [[ "$kind" != full ]] || { : > "$dir/full"; echo "$head" > "$results/full-green"; echo "$head" > "$results/full-head"; }
  cost="$(record_run "$head" "$kind" green "$4" "$5")"
  if [[ "$head" == "$green" ]]; then
    pl_say "after: green ($kind run$6); $pl_green stays at ${head:0:10} ($cost)"
  elif git push -q "$pl_remote" "$head:refs/heads/$pl_green" 2> /dev/null; then
    pl_say "after: green ($kind run$6); $pl_green is ${head:0:10} ($cost)"
    pages_of "$head"
  else
    pl_say "after: green ($kind run$6), but $pl_green could not move to ${head:0:10}"
  fi
}

run_head() {
  fetch || { pl_say "after: could not fetch $pl_remote"; return 1; }
  local head green full_green=""
  head="$(git rev-parse "$main_ref")"
  green="$(git rev-parse -q --verify "$green_ref" 2> /dev/null || true)"
  local dir="$results/$head"
  [[ ! -f "$results/full-green" ]] || full_green="$(cat "$results/full-green")"
  [[ -z "$full_green" ]] || git merge-base --is-ancestor "$full_green" "$head" 2> /dev/null || full_green=""
  # What a fast run compares against: main-green. With no main-green, the
  # head of the last full run, whose failures (known) are run again too: if
  # they and everything the change touched pass, the head is green. More than
  # OA_AFTER_KNOWN (8) known failures wait for a full run.
  local ref="$green" known=""
  if [[ -z "$green" && -f "$results/full-head" ]]; then
    ref="$(cat "$results/full-head")"
    if git merge-base --is-ancestor "$ref" "$head" 2> /dev/null && [[ -s "$results/$ref/failed" ]] &&
       (( $(wc -l < "$results/$ref/failed") <= ${OA_AFTER_KNOWN:-8} )); then known="$results/$ref/failed"
    else ref=""; fi
  fi
  # Which run: a full one when it is due (with no main-green, at most once an
  # hour), else a fast one on a head not checked yet.
  local kind="" full_gap="$gap"
  [[ -n "$green" ]] || full_gap=$(( gap < 3600 ? gap : 3600 ))
  if [[ ! -f "$dir/full" ]] && due last-full "$full_gap"; then kind=full
  elif [[ -n "$ref" && "$head" != "$ref" && ! -f "$dir/status" ]] && due last-fast "$fast_gap"; then kind=fast
  fi
  if [[ -z "$kind" ]]; then
    # main has not moved since a red run: its failures are still there.
    [[ "$(cat "$dir/status" 2> /dev/null)" != red ]] || still_red "$dir/failed" "$(date +%s)"
    return 0
  fi
  date +%s > "$results/last-$kind"
  [[ -f "$dir/status" ]] || rm -rf "$dir"
  mkdir -p "$dir"; rm -f "$dir/failed"; echo "$kind" > "$dir/kind"
  local started; started=$(date +%s)
  # What a failure is bisected against: what the run compares against, or for
  # a full run the last head a full run passed (the fast runs since may have
  # missed it).
  local base="$ref"
  if [[ "$kind" == full ]]; then base="$green"; known=""; [[ -z "$full_green" ]] || base="$full_green"; fi
  # The jobs.
  local jobs=() i changed=""
  if [[ "$kind" == fast ]]; then
    changed="$(git diff --name-only "$ref" "$head")"
    if [[ -n "$changed" ]] && grep -qvE '(\.md$|^docs/)' <<< "$changed"; then
      for (( i = 1; i <= fast_parts; i++ )); do jobs+=("since $ref $i/$fast_parts"); done
    fi
    [[ -z "$changed" ]] || ! grep -qv '\.md$' <<< "$changed" || jobs+=("web $ref" "room-since $ref")
    # The last full run's failures, each as its own check.
    if [[ -n "$known" ]]; then
      local item a j
      while read -r item; do
        a="$(args_for "$item")"; [[ -n "$a" ]] || continue
        for j in ${jobs[@]+"${jobs[@]}"}; do [[ "$j" != "$a" ]] || continue 2; done
        jobs+=("$a")
      done < "$known"
    fi
  else
    for (( i = 1; i <= parts; i++ )); do jobs+=("suites $i/$parts"); done
    for (( i = 1; i <= module_parts; i++ )); do jobs+=("modules $i/$module_parts"); done
    jobs+=("tools")
  fi
  local compiler=0
  case "$compiler_mode" in
    always) compiler=1 ;;
    auto) if [[ -z "$base" ]]; then compiler=1
          elif git diff --name-only "$base" "$head" | grep -Eq '^coax/(src|tests|bootstrap|lib/compiler)/'; then compiler=1; fi ;;
  esac
  (( compiler == 0 )) || for (( i = 1; i <= 8; i++ )); do jobs+=("compiler $i/8"); done
  pl_say "after: $kind run on ${head:0:10} ($(git log -1 --format=%s "$head" | cut -c1-60)), ${#jobs[@]} job(s), $pl_green $([[ -n "$green" ]] && echo "${green:0:10}" || echo none)$([[ "$kind" != fast || "$ref" == "$green" ]] || echo ", against the full run on ${ref:0:10}")"
  if (( ${#jobs[@]} == 0 )); then
    go_green "$head" "$green" "$kind" "$started" 0 ": only documents changed"
    return 0
  fi
  local pids=() names=() j
  for j in "${jobs[@]}"; do
    # shellcheck disable=SC2086
    check "$head" "$dir/${j//[ \/]/-}" $j & pids+=($!); names+=("$j")
  done
  local st=() k failed=() infra=0
  for k in "${!pids[@]}"; do wait "${pids[$k]}"; st+=($?); done
  # Per-commit compile-limit use and sizes.
  cat "$dir"/*/build/pipeline/limits.txt 2> /dev/null | sort -u > "$dir/limits.txt"
  cat "$dir"/*/build/pipeline/sizes.txt 2> /dev/null | sort -u > "$dir/sizes.txt"
  cat "$dir"/*/build/pipeline/warnings.txt 2> /dev/null | sort -u > "$dir/warnings.txt"
  local now; now=$(date +%s)
  awk -v c="${head:0:10}" -v t="$now" '{ printf "%s\t%s\tlimit\t%s:%s\t%s\t%s\n", c, t, $6, $3, $4, $5 }' "$dir/limits.txt" >> "$history"
  awk -v c="${head:0:10}" -v t="$now" '{ printf "%s\t%s\tsize\t%s\t%s\t%s\n", c, t, $1, $2, $3 }' "$dir/sizes.txt" >> "$history"
  local item x
  for k in "${!names[@]}"; do
    case "${st[$k]}" in
      97) infra=1 ;;
      1) local f="$dir/${names[$k]//[ \/]/-}/build/pipeline/failed"
         if [[ -s "$f" ]]; then
           while read -r item; do
             # A failing step whose items are listed (suites, compiler, modules) is those items.
             [[ "$item" == "step suites" || "$item" == "step compiler" || "$item" == "step modules" ]] && [[ $(wc -l < "$f") -gt 1 ]] && continue
             [[ "$item" == "step suites" || "$item" == "step compiler" || "$item" == "step modules" ]] && item="job ${names[$k]}"
             # One item once, whichever jobs name it.
             for x in ${failed[@]+"${failed[@]}"}; do [[ "${x%%|*}" != "$item" ]] || continue 2; done
             failed+=("$item|${names[$k]}")
           done < <(sort -u "$f")
         else failed+=("job ${names[$k]}|${names[$k]}"); fi ;;
    esac
  done
  # Entries this run shows fixed: their check ran and passed. A fast run runs
  # every check an entry names, since the entry's culprit changed its inputs
  # after main-green; the compiler's units run only when its sources changed.
  local open id chk issue
  open="$(jq -r '.[] | select(.status == "open") | "\(.id)|\(.check)|\(.issue // "")"' "$pl_red")"
  : > "$dir/fixed.tsv"
  while IFS='|' read -r id chk issue; do
    [[ -n "$id" ]] && (( infra == 0 )) || continue   # an unfinished job may be its check
    [[ "$chk" != compiler\ * ]] || (( compiler == 1 )) || { [[ -n "$known" ]] && grep -qxF "$chk" "$known"; } || continue
    local still=0
    for x in ${failed[@]+"${failed[@]}"}; do [[ "${x%%|*}" == "$chk" ]] && still=1; done
    (( still == 1 )) || printf '%s\t%s\t%s\n' "$id" "$chk" "$issue" >> "$dir/fixed.tsv"
  done <<< "$open"
  if [[ -s "$dir/fixed.tsv" ]]; then
    pl_red_update '($f | split("\n") | map(split("\t")[0])) as $ids | map(if .status == "open" and (.id as $i | $ids | index($i)) then .status = "fixed" else . end)' --rawfile f "$dir/fixed.tsv"
    # One comment an issue, and the issue closes when none of its entries is open.
    for issue in $(cut -f3 "$dir/fixed.tsv" | sed '/^$/d' | sort -u); do
      local n; n="$(awk -F'\t' -v i="$issue" '$3 == i' "$dir/fixed.tsv" | wc -l | tr -d ' ')"
      pl_issue_comment "$issue" "$n check(s) pass at ${head:0:10}:

$(awk -F'\t' -v i="$issue" '$3 == i { print "- `" $2 "`" }' "$dir/fixed.tsv" | head -n 30)$( (( n > 30 )) && echo "
- and $(( n - 30 )) more")"
      if [[ "$pl_issues" == 1 ]] && ! jq -e --arg i "$issue" '.[] | select(.status == "open" and .issue == $i)' "$pl_red" > /dev/null; then
        gh issue close "$issue" --repo "$pl_repo" > /dev/null 2>&1 || true
      fi
    done
    pl_say "after: $(wc -l < "$dir/fixed.tsv" | tr -d ' ') red-list entr(ies) fixed at ${head:0:10}"
  fi
  if (( ${#failed[@]} == 0 && infra == 0 )); then
    go_green "$head" "$green" "$kind" "$started" "${#jobs[@]}" ""
    return 0
  fi
  if (( ${#failed[@]} == 0 )); then
    pl_say "after: the hosts could not run every job on ${head:0:10}; it is checked again next time ($(record_run "$head" "$kind" unfinished "$started" "${#jobs[@]}"))"
    rm -f "$results/last-$kind"
    return 0
  fi
  # Red: bisect each failing check, unless the red list owns it.
  printf '%s\n' "${failed[@]%%|*}" > "$dir/failed"
  still_red "$dir/failed" "$started"
  pl_say "after: red at ${head:0:10}: $(printf '%s, ' "${failed[@]%%|*}" | cut -c1-600 | sed 's/, $//')"
  # Nothing known to pass (no main-green yet, or none before this head):
  # there is nothing to bisect against. Each failure goes on the red list
  # owned by nobody, under one issue, and later runs don't bisect it;
  # main-green starts at the first head where every check passes.
  if [[ -z "$base" || "$base" == "$head" ]]; then
    local new=()
    for x in "${failed[@]}"; do
      item="${x%%|*}"
      jq -e --arg c "$item" '.[] | select(.status == "open" and .check == $c)' "$pl_red" > /dev/null || new+=("$item")
    done
    if (( ${#new[@]} > 0 )); then
      issue=""
      if [[ "$pl_issues" == 1 ]]; then
        local url body
        body="The after-landing check (\`coax/remote/after.sh\`, docs/verse/green-main-pipeline.md) found ${#new[@]} failing check(s) on \`$pl_main\` at ${head:0:10} with no passing head before it to bisect against, so there is no culprit. \`$pl_green\` is set at the first head where every check passes.

$(printf -- '- \`%s\`\n' "${new[@]}" | head -n 200)

Each job's output: \`$dir/\` on the coordinator's Mac. Run one alone with \`coax/remote/remote.sh run -- coax/remote/check.sh only PATH\` (a suite) or \`check.sh reports\`."
        url="$(gh issue create --repo "$pl_repo" --title "Red main: ${#new[@]} check(s) fail with no green head to bisect against" --body "$body" 2> /dev/null)" && issue="${url##*/}"
      fi
      printf '%s\n' "${new[@]}" > "$dir/new"
      pl_red_update '. + ($f | split("\n") | map(select(. != "")) | map({id: ("none:" + .), check: ., culprit: "", subject: "fails with no green head before it", agent: "nobody", issue: (if $i == "" then null else $i end), opened: ($t | tonumber), seen: ($t | tonumber), status: "open", revert: ""}))' \
        --rawfile f "$dir/new" --arg i "$issue" --arg t "$(date +%s)"
      pl_say "after: no $pl_green to bisect against; ${#new[@]} failure(s) listed with no culprit${issue:+ (#$issue)}"
    fi
    echo red > "$dir/status"; [[ "$kind" != full ]] || { : > "$dir/full"; echo "$head" > "$results/full-head"; }
    pl_say "after: $kind run on ${head:0:10}: $(record_run "$head" "$kind" red "$started" "${#jobs[@]}")"
    return 0
  fi
  local bpids=() bitems=() n=0 job args rest=() owned=0 flaky_n=0 found=0
  for x in ${failed[@]+"${failed[@]}"}; do
    item="${x%%|*}"; job="${x#*|}"
    if jq -e --arg c "$item" '.[] | select(.status == "open" and .check == $c)' "$pl_red" > /dev/null; then
      pl_say "after: $item is already on the red list"; owned=1; continue
    fi
    if (( n < bisect_checks )); then
      args="$(args_for "$item" "$job")"
      # shellcheck disable=SC2086
      ( bisect "$dir/bisect-$n" "$base" "$head" $args > "$dir/bisect-$n.culprit" ) & bpids+=($!); bitems+=("$item"); n=$((n + 1))
    else rest+=("$item"); fi
  done
  (( ${#rest[@]} == 0 )) || pl_say "after: left for the next run (OA_BISECT_CHECKS=$bisect_checks): ${rest[*]}"
  for k in "${!bpids[@]}"; do
    wait "${bpids[$k]}"
    item="${bitems[$k]}"
    local culprit; culprit="$(cat "$dir/bisect-$k.culprit")"
    case "$culprit" in
      flaky) pl_say "after: $item failed at ${head:0:10} and passed when run again: flaky"
             printf '%s\t%s\n' "${head:0:10}" "$item" >> "$results/flaky.txt"; flaky_n=$((flaky_n + 1)); continue ;;
      infra) pl_say "after: the hosts could not finish the bisect of $item"; continue ;;
      "") pl_say "after: the bisect of $item found no culprit after ${base:0:10}"
          continue ;;
    esac
    local agent subject out
    agent="$(pl_agent_of "$culprit")"; [[ -n "$agent" ]] || agent="unknown"
    subject="$(git log -1 --format=%s "$culprit")"
    out="$(ls -d "$dir/bisect-$k"/r*-"${culprit:0:10}" 2> /dev/null | tail -1)/out"
    issue="$(file_issue "$item" "$culprit" "$agent" "$out")"
    pl_red_update '. + [{id: $id, check: $c, culprit: $s, subject: $sub, agent: $a, issue: (if $i == "" then null else $i end), opened: ($t | tonumber), seen: ($t | tonumber), status: "open", revert: ""}]' \
      --arg id "${culprit:0:10}:$item" --arg c "$item" --arg s "$culprit" --arg sub "$subject" --arg a "$agent" --arg i "$issue" --arg t "$(date +%s)"
    pl_message "$agent" "Your commit ${culprit:0:10} (\"$subject\") broke \`$item\` on $pl_main.${issue:+ Issue: https://github.com/$pl_repo/issues/$issue.}
Output: $out
Run it alone: coax/remote/remote.sh run -- coax/remote/check.sh $(args_for "$item" "$job")
Fixing it comes before new work. If it still fails 30 minutes from now, the integrator reverts the commit."
    pl_say "after: $item: culprit ${culprit:0:10} by $agent${issue:+ (#$issue)}"
    found=$((found + 1))
  done
  # Every failure passed when run again on H: H is green.
  if (( owned == 0 && ${#rest[@]} == 0 && flaky_n == ${#failed[@]} )); then
    go_green "$head" "$green" "$kind" "$started" "${#jobs[@]}" ", on a second run of $flaky_n flaky check(s)"
    return 0
  fi
  # A full run found what the fast runs passed: main-green goes back to the
  # last head a full run passed.
  if (( found > 0 )) && [[ -n "$green" && "$green" != "$base" ]]; then
    git push -q --force "$pl_remote" "$base:refs/heads/$pl_green" 2> /dev/null &&
      pl_say "after: a full run failed after fast runs passed; $pl_green is back at ${base:0:10}"
  fi
  echo red > "$dir/status"; [[ "$kind" != full ]] || { : > "$dir/full"; echo "$head" > "$results/full-head"; }
  pl_say "after: $kind run and bisects on ${head:0:10}: $(record_run "$head" "$kind" red "$started" "${#jobs[@]}")"
}

once() {
  exec 8> "$results/after.lock"
  flock -n 8 || { pl_say "after: a run is going"; return 0; }
  run_head
  exec 8>&-
}
if (( watch == 1 )); then
  pl_say "after: watching $pl_remote/$pl_main (full runs every $gap s, reverts $([[ $revert == 1 ]] && echo on || echo off))"
  while true; do
    once
    # The 30-minute revert, and any branch agents submitted: one integrator
    # round, beside the next run (it holds its own lock).
    if [[ "$revert" == 1 ]]; then
      ( OA_INTEGRATE_QUIET=1 "$here/integrate.sh" --once --revert >> "$pl_dir/logs/integrate.out" 2>&1 & )
    fi
    sleep "${OA_AFTER_POLL:-30}"
  done
else
  once
fi

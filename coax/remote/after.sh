#!/usr/bin/env bash
# Firm reason: 5. The green-main pipeline's after-landing check
# (docs/verse/green-main-pipeline.md, #13102): git, gh, jq and
# coax/remote/remote.sh. The checks run on the remote build hosts.
#
#   coax/remote/after.sh [--once]   a full run on the newest head of main, unless
#                                   one is running, that head was checked, or the
#                                   last run started under OA_AFTER_GAP seconds ago
#   coax/remote/after.sh --watch    the same every minute, for ever
#   coax/remote/after.sh red        the red list: failures on main and who owns them
#   coax/remote/after.sh history [NAME]
#                                   compile-limit use and wasm sizes per commit
#
# A full run checks the head H with these jobs side by side, each on its own
# host slot (coax/remote/check.sh): every suite in OA_AFTER_PARTS parts, every
# web module against its budget in OA_AFTER_MODULE_PARTS parts, the report checks (tools-check,
# everglade-parity, the committed shaders, the room build) and, when
# compiler sources changed since main-green, coax/tests/run.sh in 8 parts.
#
# Green: main-green moves to H, and the red-list entries H fixes close.
# Red: each failing check is bisected between main-green and H (several
# commits at once, OA_BISECT_WIDTH, running only that check). For the culprit
# it files an issue naming the commit, the check, its output and the agent in
# the commit's "Agent:" trailer, messages that agent, and adds an entry to the
# red list. A failure already on the red list is not bisected again. A check
# that fails on H and passes when run again is flaky: reported, no culprit.
# Before the first green run there is no main-green to bisect against: every
# failure goes on the red list owned by nobody, under one issue.
#
# Results: OA_PIPELINE_DIR/releases/after/COMMIT/ (each job's output and
# report, run.tsv with each job's status and seconds), history.tsv (commit,
# time, kind, name, value, limit) and red.json.
#
# Settings: OA_AFTER_PARTS (6), OA_AFTER_MODULE_PARTS (3), OA_AFTER_GAP (900), OA_AFTER_COMPILER
# (auto, always or never), OA_BISECT_WIDTH (4), OA_BISECT_CHECKS (4: failing
# checks bisected one by one; the rest share one bisect), OA_AFTER_TIMEOUT
# (3000 s a job), OA_AFTER_CHECK (a command run as CMD COMMIT OUTDIR CHECK-ARGS...
# in place of the remote check, for scratch runs), and coax/remote/pipeline-lib.sh's
# OA_PIPELINE_*.
set -uo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
source "$here/pipeline-lib.sh"
parts="${OA_AFTER_PARTS:-6}"
module_parts="${OA_AFTER_MODULE_PARTS:-3}"
gap="${OA_AFTER_GAP:-900}"
compiler_mode="${OA_AFTER_COMPILER:-auto}"
width="${OA_BISECT_WIDTH:-4}"
bisect_checks="${OA_BISECT_CHECKS:-4}"
limit="${OA_AFTER_TIMEOUT:-3000}"
results="$pl_dir/releases/after"
history="$results/history.tsv"

case "${1:-}" in
  red)
    jq -r '.[] | "\(.status)\t\(.check)\t\(if .culprit == "" then "-" else .culprit[0:10] end) \(.subject)\tagent \(.agent)\t#\(.issue // "-")\t\(.opened | strflocaltime("%Y-%m-%d %H:%M %Z"))"' "$pl_red"
    exit 0 ;;
  history)
    [[ -f "$history" ]] || exit 0
    if [[ -n "${2:-}" ]]; then awk -F'\t' -v n="$2" '$4 == n' "$history"; else cat "$history"; fi
    exit 0 ;;
esac
watch=0
while (($#)); do
  case "$1" in
    --once) shift ;;
    --watch) watch=1; shift ;;
    -h|--help) sed -n '2,38p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
# One check on one commit: 0 passed, 1 failed, 97 the hosts could not run it.
check() {   # commit outdir check-args...
  local commit="$1" dir="$2" s; shift 2
  mkdir -p "$dir"
  local t0; t0=$(date +%s)
  if [[ -n "${OA_AFTER_CHECK:-}" ]]; then
    # shellcheck disable=SC2086
    timeout "$limit" $OA_AFTER_CHECK "$commit" "$dir" "$@" > "$dir/out" 2>&1; s=$?
  else
    OA_REMOTE_TIMEOUT=$(( limit - 60 )) timeout "$limit" "$here/remote.sh" run --commit "$commit" --out "$dir" --no-fallback -- coax/remote/check.sh "$@" > "$dir/out" 2>&1; s=$?
  fi
  case "$s" in 0|1) ;; 124) echo "after: the check ran past $limit s" >> "$dir/out"; s=97 ;; 97|255) s=97 ;; *) s=1 ;; esac
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
    module\ *) echo "$*" ;;   # its part of the modules again
    step\ tools-check|step\ everglade-parity|step\ wgsl|step\ room-build) echo "tools" ;;
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

Fixing this comes before new work for \`$agent\`. If no fix lands within 30 minutes, the integrator reverts ${culprit:0:10}. Run the check alone with \`coax/remote/remote.sh run -- coax/remote/check.sh $(args_for "$check")\`."
  url="$(gh issue create --repo "$pl_repo" --title "Red main: $check fails since ${culprit:0:10}" --body "$body" 2> /dev/null)" || { echo ""; return; }
  echo "${url##*/}"
}

full_run() {
  fetch || { pl_say "after: could not fetch $pl_remote"; return 1; }
  local head green
  head="$(git rev-parse "$main_ref")"
  green="$(git rev-parse -q --verify "$green_ref" 2> /dev/null || true)"
  [[ "$head" != "$green" ]] || return 0
  local dir="$results/$head"
  [[ ! -f "$dir/status" ]] || return 0
  if [[ -f "$results/last-start" ]] && (( $(date +%s) - $(cat "$results/last-start") < gap )); then return 0; fi
  date +%s > "$results/last-start"
  rm -rf "$dir"; mkdir -p "$dir"
  local started; started=$(date +%s)
  pl_say "after: full run on ${head:0:10} ($(git log -1 --format=%s "$head" | cut -c1-60)), main-green ${green:0:10}"
  # The jobs.
  local jobs=() i
  for (( i = 1; i <= parts; i++ )); do jobs+=("suites $i/$parts"); done
  for (( i = 1; i <= module_parts; i++ )); do jobs+=("modules $i/$module_parts"); done
  jobs+=("tools")
  local compiler=0
  case "$compiler_mode" in
    always) compiler=1 ;;
    auto) [[ -n "$green" ]] && git diff --name-only "$green" "$head" | grep -Eq '^coax/(src|tests|bootstrap|lib/compiler)/' && compiler=1 ;;
  esac
  (( compiler == 0 )) || for (( i = 1; i <= 8; i++ )); do jobs+=("compiler $i/8"); done
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
  for k in "${!names[@]}"; do
    case "${st[$k]}" in
      97) infra=1 ;;
      1) local f="$dir/${names[$k]//[ \/]/-}/build/pipeline/failed"
         if [[ -s "$f" ]]; then
           while read -r item; do
             # A failing step whose items are listed (suites, compiler) is those items.
             [[ "$item" == "step suites" || "$item" == "step compiler" || "$item" == "step modules" ]] && [[ $(wc -l < "$f") -gt 1 ]] && continue
             [[ "$item" == "step suites" || "$item" == "step compiler" || "$item" == "step modules" ]] && item="job ${names[$k]}"
             failed+=("$item|${names[$k]}")
           done < "$f"
         else failed+=("job ${names[$k]}|${names[$k]}"); fi ;;
    esac
  done
  local secs=$(( $(date +%s) - started ))
  local slot_secs; slot_secs="$(awk -F'\t' '{ s += $3 } END { print s + 0 }' "$dir/run.tsv")"
  # Entries this run shows fixed: their check ran and passed.
  local open id chk
  open="$(jq -r '.[] | select(.status == "open") | "\(.id)|\(.check)|\(.issue // "")"' "$pl_red")"
  while IFS='|' read -r id chk issue; do
    [[ -n "$id" ]] || continue
    local still=0 x
    for x in ${failed[@]+"${failed[@]}"}; do [[ "${x%%|*}" == "$chk" ]] && still=1; done
    (( infra == 0 )) || continue   # its job may not have run
    if (( still == 0 )); then
      pl_red_update '(.[] | select(.id == $id) | .status) = "fixed"' --arg id "$id"
      pl_issue_comment "$issue" "\`$chk\` passes at ${head:0:10}."
      [[ "$pl_issues" != 1 || -z "$issue" ]] || gh issue close "$issue" --repo "$pl_repo" > /dev/null 2>&1 || true
      pl_say "after: $chk is fixed at ${head:0:10}"
    fi
  done <<< "$open"
  if (( ${#failed[@]} == 0 && infra == 0 )); then
    if git push -q "$pl_remote" "$head:refs/heads/$pl_green" 2> /dev/null; then
      pl_say "after: green; $pl_green is ${head:0:10} (${secs} s, ${slot_secs} slot-seconds)"
    else
      pl_say "after: green, but $pl_green could not move to ${head:0:10}"
    fi
    echo green > "$dir/status"
    return 0
  fi
  if (( ${#failed[@]} == 0 )); then
    pl_say "after: the hosts could not run every job on ${head:0:10}; it is checked again next time"
    rm -f "$results/last-start"
    return 0
  fi
  # Red: bisect each failing check, unless the red list owns it.
  printf '%s\n' "${failed[@]%%|*}" > "$dir/failed"
  pl_say "after: red at ${head:0:10}: $(printf '%s, ' "${failed[@]%%|*}" | sed 's/, $//') (${secs} s, ${slot_secs} slot-seconds)"
  # No main-green yet: nothing is known to pass, so there is nothing to bisect
  # against. Each failure goes on the red list owned by nobody, under one
  # issue, and later runs don't bisect it; main-green starts at the first head
  # where every check passes.
  if [[ -z "$green" ]]; then
    local new=() x item
    for x in "${failed[@]}"; do
      item="${x%%|*}"
      jq -e --arg c "$item" '.[] | select(.status == "open" and .check == $c)' "$pl_red" > /dev/null || new+=("$item")
    done
    if (( ${#new[@]} > 0 )); then
      local issue=""
      if [[ "$pl_issues" == 1 ]]; then
        local url body
        body="The after-landing check (\`coax/remote/after.sh\`, docs/verse/green-main-pipeline.md) found ${#new[@]} failing check(s) on \`$pl_main\` at ${head:0:10} before any run passed, so there is no \`$pl_green\` to bisect against and no culprit. \`$pl_green\` is first set at the head where every check passes.

$(printf -- '- \`%s\`\n' "${new[@]}" | head -n 200)

Each job's output: \`$dir/\` on the coordinator's Mac. Run one alone with \`coax/remote/remote.sh run -- coax/remote/check.sh only PATH\` (a suite) or \`check.sh tools\`."
        url="$(gh issue create --repo "$pl_repo" --title "Red main: ${#new[@]} check(s) fail before the first green run" --body "$body" 2> /dev/null)" && issue="${url##*/}"
      fi
      for item in "${new[@]}"; do
        pl_red_update '. + [{id: $id, check: $c, culprit: "", subject: "fails before the first green run", agent: "nobody", issue: (if $i == "" then null else $i end), opened: ($t | tonumber), status: "open", revert: ""}]' \
          --arg id "none:$item" --arg c "$item" --arg i "$issue" --arg t "$(date +%s)"
      done
      pl_say "after: no $pl_green yet; ${#new[@]} failure(s) listed with no culprit${issue:+ (#$issue)}"
    fi
    echo red > "$dir/status"
    return 0
  fi
  local base="$green"
  local bpids=() bitems=() n=0 x item job args rest=() owned=0 flaky_n=0
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
  (( ${#rest[@]} == 0 )) || pl_say "after: not bisected one by one (OA_BISECT_CHECKS=$bisect_checks): ${rest[*]}"
  for k in "${!bpids[@]}"; do
    wait "${bpids[$k]}"
    item="${bitems[$k]}"
    local culprit; culprit="$(cat "$dir/bisect-$k.culprit")"
    case "$culprit" in
      flaky) pl_say "after: $item failed at ${head:0:10} and passed when run again: flaky"
             printf '%s\t%s\n' "${head:0:10}" "$item" >> "$results/flaky.txt"; flaky_n=$((flaky_n + 1)); continue ;;
      infra) pl_say "after: the hosts could not finish the bisect of $item"; continue ;;
      "") pl_say "after: the bisect of $item found no culprit after main-green ${base:0:10}"
          continue ;;
    esac
    local agent subject out issue
    agent="$(pl_agent_of "$culprit")"; [[ -n "$agent" ]] || agent="unknown"
    subject="$(git log -1 --format=%s "$culprit")"
    out="$(ls -d "$dir/bisect-$k"/r*-"${culprit:0:10}" 2> /dev/null | tail -1)/out"
    issue="$(file_issue "$item" "$culprit" "$agent" "$out")"
    pl_red_update '. + [{id: $id, check: $c, culprit: $s, subject: $sub, agent: $a, issue: (if $i == "" then null else $i end), opened: ($t | tonumber), status: "open", revert: ""}]' \
      --arg id "${culprit:0:10}:$item" --arg c "$item" --arg s "$culprit" --arg sub "$subject" --arg a "$agent" --arg i "$issue" --arg t "$(date +%s)"
    pl_message "$agent" "Your commit ${culprit:0:10} (\"$subject\") broke \`$item\` on $pl_main.${issue:+ Issue: https://github.com/$pl_repo/issues/$issue.}
Output: $out
Fixing it comes before new work. If no fix lands within 30 minutes, the integrator reverts the commit."
    pl_say "after: $item: culprit ${culprit:0:10} by $agent${issue:+ (#$issue)}"
  done
  # Every failure passed when run again on H: H is green.
  if (( owned == 0 && ${#rest[@]} == 0 && flaky_n == ${#failed[@]} )); then
    git push -q "$pl_remote" "$head:refs/heads/$pl_green" 2> /dev/null && pl_say "after: green on a second run; $pl_green is ${head:0:10}"
    echo green > "$dir/status"
    return 0
  fi
  echo red > "$dir/status"
  pl_say "after: run and bisects on ${head:0:10}: $(( $(date +%s) - started )) s, $(cat "$dir/run.tsv" "$dir"/bisect-*/run.tsv 2> /dev/null | awk -F'\t' '{ s += $3 } END { print s + 0 }') slot-seconds"
}

once() {
  exec 8> "$results/after.lock"
  flock -n 8 || { pl_say "after: a full run is going"; return 0; }
  full_run
  exec 8>&-
}
if (( watch == 1 )); then
  while true; do once; sleep 60; done
else
  once
fi

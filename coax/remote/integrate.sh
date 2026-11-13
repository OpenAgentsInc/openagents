#!/usr/bin/env bash
# Firm reason: 5. The green-main pipeline's integrator (docs/metaverse/green-main-pipeline.md,
# #13102): git, gh and coax/remote/remote.sh. Its own work is light; the checks
# run on the remote build hosts.
#
#   coax/remote/integrate.sh            watch the queue: a round whenever a branch waits
#   coax/remote/integrate.sh --once     one round, then exit
#   coax/remote/integrate.sh status     the queue, the last rounds and the red list
#
# A round, for the queue B1 ... Bn (oldest newest-commit first, at most
# OA_INTEGRATE_MAX of them) on the head H of main:
#   1. Stacks the branches: Bk's commits go onto H plus B1 ... Bk-1, the head
#      it lands on if the branches ahead of it land. A branch that conflicts
#      with H goes back to its builder at once; one that conflicts only with a
#      branch ahead of it waits for the next round.
#   2. Checks every stacked branch at once, each on its own host slot:
#      coax/remote/check.sh branch, the suites whose sources the branch
#      changed (coax deps), the web build and its sizes when the branch touches
#      what it builds, and the compile-limit notes.
#   3. Lands the longest green prefix on main in one push, then starts
#      after.sh. A red branch goes to refs/heads/rejected/AGENT/TOPIC and its
#      agent gets a message; the branches behind it are checked again next
#      round, without it. A check the hosts could not run leaves its branch
#      queued.
#   4. With --revert (or OA_INTEGRATE_REVERT=1; after.sh --watch runs it so):
#      a red-list culprit that an after-landing run still saw failing
#      OA_REVERT_MINUTES (30) after its entry opened gets a revert, queued
#      ahead of everything. Its check is a branch's, and the check the culprit
#      broke must pass with the revert, so a wrong culprit is not reverted.
# Agents may push main directly (AGENTS.md) or submit. A landing that loses
# the race to a direct push is stacked and checked again on the new head.
#
# Settings: OA_INTEGRATE_MAX (6 branches a round), OA_INTEGRATE_POLL (30 s
# between looks at an empty queue), OA_INTEGRATE_TIMEOUT (2400 s a check),
# OA_INTEGRATE_CHECK (a command run as CMD COMMIT BASE OUTDIR in place of the
# remote check, for scratch runs), OA_INTEGRATE_AFTER=0 (start no after.sh),
# and coax/remote/pipeline-lib.sh's OA_PIPELINE_*.
set -uo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
source "$here/pipeline-lib.sh"
max="${OA_INTEGRATE_MAX:-6}"
poll="${OA_INTEGRATE_POLL:-30}"
limit="${OA_INTEGRATE_TIMEOUT:-2400}"
revert="${OA_INTEGRATE_REVERT:-0}"
revert_minutes="${OA_REVERT_MINUTES:-30}"
once=0
case "${1:-}" in
  status)
    "$here/submit.sh" status
    echo "last rounds ($pl_dir/logs/integrate.tsv: time, head, landed, rejected, waiting, seconds):"
    tail -n 10 "$pl_dir/logs/integrate.tsv" 2> /dev/null | sed 's/^/  /'
    exit 0 ;;
esac
while (($#)); do
  case "$1" in
    --once) once=1; shift ;;
    --revert) revert=1; shift ;;
    -h|--help) sed -n '2,38p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "integrate: unknown option $1" >&2; exit 2 ;;
  esac
done
root="$(git rev-parse --show-toplevel 2> /dev/null)" || { echo "integrate: not in a Git checkout" >&2; exit 2; }
cd "$root" || exit 2
qrefs="refs/remotes/$pl_remote/$pl_queue"
main_ref="refs/remotes/$pl_remote/$pl_main"

fetch() {
  git fetch -q "$pl_remote" "+refs/heads/$pl_main:$main_ref" "+refs/heads/$pl_queue/*:$qrefs/*" --prune 2> /dev/null ||
    git fetch -q "$pl_remote" "+refs/heads/$pl_main:$main_ref"
}
queue() {   # "sha name" for each queued branch, oldest newest-commit first
  git for-each-ref --format='%(committerdate:unix) %(objectname) %(refname)' "$qrefs/" | sed "s# $qrefs/# #" | sort -n -k1,1 -k3,3 | cut -d' ' -f2-
}
# A commit replayed onto TIP: the same change, author and message, no
# worktree (git merge-tree). Prints the new commit; fails on a conflict.
pick() {   # tip commit
  local tip="$1" c="$2" tree parent
  parent="$(git rev-parse "$c^" 2> /dev/null)" || return 1
  tree="$(git merge-tree --write-tree --merge-base="$parent" "$tip" "$c" 2> /dev/null | head -1)" || return 1
  [[ "$tree" =~ ^[0-9a-f]{40}$ ]] || return 1
  # A change main already has adds nothing: keep the tip.
  [[ "$tree" != "$(git rev-parse "$tip^{tree}")" ]] || { echo "$tip"; return 0; }
  GIT_AUTHOR_NAME="$(git log -1 --format=%an "$c")" GIT_AUTHOR_EMAIL="$(git log -1 --format=%ae "$c")" \
    GIT_AUTHOR_DATE="$(git log -1 --format=%aI "$c")" git commit-tree "$tree" -p "$tip" -F <(git log -1 --format=%B "$c")
}
stack() {   # tip sha: the branch's commits onto tip (fails on a conflict)
  local tip="$1" c
  for c in $(git rev-list --reverse --no-merges "$main_ref..$2"); do tip="$(pick "$tip" "$c")" || return 1; done
  echo "$tip"
}
run_check() {   # commit base outdir [also...]: 0 passed, 1 failed, 97 the hosts could not run it
  local commit="$1" base="$2" dir="$3" s; shift 3
  mkdir -p "$dir"
  if [[ -n "${OA_INTEGRATE_CHECK:-}" ]]; then
    # shellcheck disable=SC2086
    timeout "$limit" $OA_INTEGRATE_CHECK "$commit" "$base" "$dir" "$@" > "$dir/out" 2>&1; s=$?
  else
    OA_REMOTE_TIMEOUT=$(( limit - 60 )) timeout "$limit" "$here/remote.sh" run --commit "$commit" --out "$dir" --no-fallback -- coax/remote/check.sh branch "$base" "$@" > "$dir/out" 2>&1; s=$?
  fi
  case "$s" in 0|1) return "$s" ;; 124) echo "integrate: the check ran past $limit s" >> "$dir/out"; return 97 ;; 97|255) return 97 ;; *) return 1 ;; esac
}
# The check a culprit broke, on the head its revert would land on: 0 it
# passes (a fix landed first), 1 it still fails, 97 the hosts could not run it.
head_check() {   # commit outdir check-args...
  local commit="$1" dir="$2" s; shift 2
  [[ -z "${OA_INTEGRATE_CHECK:-}" ]] || return 1
  mkdir -p "$dir"
  OA_REMOTE_TIMEOUT=$(( limit - 60 )) timeout "$limit" "$here/remote.sh" run --commit "$commit" --out "$dir" --no-fallback -- coax/remote/check.sh "$@" > "$dir/out" 2>&1; s=$?
  case "$s" in 0|1) return "$s" ;; 124|97|255) return 97 ;; *) return 1 ;; esac
}
send_back() {   # name sha why logdir
  local name="$1" sha="$2" why="$3" dir="$4" agent="${1%%/*}"
  git push -q --force "$pl_remote" "$sha:refs/heads/$pl_rejected/$name" 2> /dev/null
  git push -q "$pl_remote" --force-with-lease="refs/heads/$pl_queue/$name:$sha" ":refs/heads/$pl_queue/$name" 2> /dev/null
  local failed=""
  [[ -z "$dir" || ! -s "$dir/build/pipeline/failed" ]] || failed="$(sed 's/^/  /' "$dir/build/pipeline/failed")"
  pl_message "$agent" "Your branch $pl_queue/$name (${sha:0:10}) did not land: $why.
${failed:+Failing:
$failed
}${dir:+Output: $dir/out and $dir/build/pipeline/
}It is kept as $pl_rejected/$name. Fix it on the newest $pl_main, then submit again with coax/remote/submit.sh."
  pl_say "sent back $name: $why"
}

# The check a red-list item names, as check.sh arguments ("" when it has none
# of its own): a revert must make it pass.
also_for() {   # item
  case "$1" in
    suite\ *) echo "only ${1#suite }" ;;
    module\ *) echo "module ${1#module }" ;;
    compiler\ *) echo "unit ${1#compiler }" ;;
    step\ wasm) echo "wasm" ;;
    step\ tools-check|step\ after-gates|step\ everglade-parity|step\ wire-check|step\ layer-check|step\ wgsl|step\ realm-pages|step\ changelog-page|step\ realm-cards) echo "reports" ;;
    step\ room-build) echo "room" ;;
    step\ check-visuals) echo "visuals" ;;
    # a raised allowlist count against main-green, Everglade II against production (#13266)
    count\ *) echo "counts $(git rev-parse -q --verify "refs/remotes/$pl_remote/$pl_green" 2> /dev/null)" ;;
    e2\ *) echo "e2 $(cat "$pl_dir/releases/after/production" 2> /dev/null)" ;;
  esac
}
# The revert step: a culprit that an after-landing run still saw failing
# (seen) the window after its entry opened gets a revert, queued ahead of
# every branch.
queue_reverts() {
  [[ "$revert" == 1 ]] || return 0
  jq -r --argjson w "$(( revert_minutes * 60 ))" '.[] | select(.status == "open" and (.revert // "") == "" and ((.seen // 0) - .opened) >= $w) | "\(.id)|\(.culprit)|\(.opened)|\(.issue // "")"' "$pl_red" |
  while IFS='|' read -r id culprit opened issue; do
    [[ -n "$culprit" ]] || continue   # failing before the first green run: nothing to revert
    [[ -n "$id" ]] || continue
    local head tree msg commit name check
    head="$(git rev-parse "$main_ref")"
    git merge-base --is-ancestor "$culprit" "$head" 2> /dev/null || continue
    check="$(jq -r --arg id "$id" '.[] | select(.id == $id) | .check' "$pl_red")"
    # merge-tree prints a tree on a conflict too, with the conflict markers in
    # its files, and exits 1: only status 0 is a clean revert. (On 2026-11-11
    # the revert of c4040e643d was queued with markers in build.sh, #13255.)
    local merged mstatus
    merged="$(git merge-tree --write-tree --merge-base="$culprit" "$head" "$culprit^" 2> /dev/null)"; mstatus=$?
    tree="$(head -1 <<< "$merged")"
    if (( mstatus != 0 )) || [[ ! "$tree" =~ ^[0-9a-f]{40}$ ]]; then
      pl_say "the revert of ${culprit:0:10} conflicts with $pl_main; leaving it to its agent"
      pl_red_update '(.[] | select(.id == $id) | .revert) = "conflict"' --arg id "$id"
      pl_issue_comment "$issue" "The 30-minute revert of ${culprit:0:10} conflicts with the newest $pl_main, so it was not queued. The failure is still open."
      continue
    fi
    msg="Revert \"$(git log -1 --format=%s "$culprit")\"

This reverts commit $culprit.

The after-landing check \`$check\` fails since this commit${issue:+ (#$issue)}, and no fix
landed within $revert_minutes minutes. To land the change again: fix the failure on the
newest $pl_main, then push it or submit it with coax/remote/submit.sh.

Agent: integrator"
    # Its commit time is the failure's, so the queue checks it first.
    commit="$(GIT_COMMITTER_DATE="@$opened" git commit-tree "$tree" -p "$head" -m "$msg")" || continue
    name="integrator/revert-${culprit:0:10}"
    if git push -q "$pl_remote" "$commit:refs/heads/$pl_queue/$name"; then
      pl_red_update '(.[] | select(.id == $id) | .revert) = $r' --arg id "$id" --arg r "$name"
      pl_issue_comment "$issue" "No fix landed within $revert_minutes minutes, so the integrator queued a revert of ${culprit:0:10} ($pl_queue/$name). It lands after its check passes."
      pl_say "queued $name"
    fi
  done
}
# After a revert lands: its red entry is reverted, and its issue stays open
# for the change to land again.
reverted() {   # queue name of a landed revert, landed commit
  local id issue
  IFS='|' read -r id issue < <(jq -r --arg r "$1" '.[] | select(.revert == $r) | "\(.id)|\(.issue // "")"' "$pl_red")
  [[ -n "$id" ]] || return 0
  pl_red_update '(.[] | select(.id == $id) | .status) = "reverted"' --arg id "$id"
  [[ "$pl_issues" != 1 || -z "$issue" ]] || gh issue reopen "$issue" --repo "$pl_repo" > /dev/null 2>&1 || true
  pl_issue_comment "$issue" "The revert landed as ${2:0:10}. Fix the failure, then land the change again."
  local agent culprit check
  IFS='|' read -r agent culprit check < <(jq -r --arg id "$id" '.[] | select(.id == $id) | "\(.agent)|\(.culprit)|\(.check)"' "$pl_red")
  pl_message "${agent:-unknown}" "Your commit ${culprit:0:10} was reverted on $pl_main as ${2:0:10}: \`$check\` still failed 30 minutes after it was reported.${issue:+ Issue: https://github.com/$pl_repo/issues/$issue.}
Fix the failure on the newest $pl_main, then land the change again."
}

# A queued revert whose check already passes on main: the revert is dropped
# and its red entry is fixed.
fixed_first() {   # queue name, queued sha, head
  local id check issue
  IFS='|' read -r id check issue < <(jq -r --arg r "$1" '.[] | select(.revert == $r) | "\(.id)|\(.check)|\(.issue // "")"' "$pl_red")
  git push -q "$pl_remote" --force-with-lease="refs/heads/$pl_queue/$1:$2" ":refs/heads/$pl_queue/$1" 2> /dev/null
  [[ -n "$id" ]] || return 0
  pl_red_update '(.[] | select(.id == $id)) |= (.status = "fixed" | .revert = "")' --arg id "$id"
  pl_issue_comment "$issue" "\`$check\` passes on $pl_main at ${3:0:10}: a fix landed before the revert, so nothing was reverted."
  [[ "$pl_issues" != 1 || -z "$issue" ]] || gh issue close "$issue" --repo "$pl_repo" > /dev/null 2>&1 || true
  pl_say "$1: $check already passes on ${3:0:10}; nothing is reverted"
}

round() {
  fetch || { pl_say "could not fetch $pl_remote"; return 2; }
  queue_reverts
  fetch
  local head started=$(date +%s)
  head="$(git rev-parse "$main_ref")"
  local names=() shas=() line sha name
  while read -r sha name; do
    [[ -n "$name" ]] || continue
    # Everything in it already on main (landed, or pushed directly): done.
    if [[ -z "$(git rev-list --no-merges "$main_ref..$sha" | head -1)" ]] ||
       ! git cherry "$main_ref" "$sha" | grep -q '^+'; then
      git push -q "$pl_remote" --force-with-lease="refs/heads/$pl_queue/$name:$sha" ":refs/heads/$pl_queue/$name" 2> /dev/null
      pl_say "$name is already on $pl_main"; continue
    fi
    names+=("$name"); shas+=("$sha")
    (( ${#names[@]} < max )) || break
  done < <(queue)
  (( ${#names[@]} > 0 )) || return 1
  local dir; dir="$pl_dir/logs/integrate-$(date +%Y%m%dT%H%M%S)"; mkdir -p "$dir"
  pl_say "round: ${#names[@]} branch(es) on ${head:0:10} (${dir})"
  # 1. The stack.
  local tip="$head" k cands=() bases=() keep=() rejected=0
  for k in "${!names[@]}"; do
    local c
    if c="$(stack "$tip" "${shas[$k]}")"; then
      cands+=("$c"); bases+=("$tip"); keep+=("$k"); tip="$c"
    elif ! git merge-tree --write-tree "$head" "${shas[$k]}" > /dev/null 2>&1; then
      send_back "${names[$k]}" "${shas[$k]}" "it conflicts with $pl_main (${head:0:10}): rebase it" ""
      rejected=$((rejected + 1))
    else
      pl_say "${names[$k]} conflicts with a branch ahead of it; it waits for the next round"
    fi
  done
  (( ${#cands[@]} > 0 )) || { printf '%s\t%s\t0\t%s\t%s\t%s\n' "$(date +%FT%T)" "${head:0:10}" "$rejected" "${#names[@]}" "$(( $(date +%s) - started ))" >> "$pl_dir/logs/integrate.tsv"; return 0; }
  # 2. The checks, side by side.
  local pids=() i
  for i in "${!cands[@]}"; do
    local also=""
    # A revert must make the check its culprit broke pass.
    [[ "${names[${keep[$i]}]}" != integrator/revert-* ]] ||
      also="$(also_for "$(jq -r --arg r "${names[${keep[$i]}]}" '[.[] | select(.revert == $r)][0].check // ""' "$pl_red")")"
    # A revert lands only while that check still fails on the head it lands
    # on: when a fix landed first, nothing is reverted (status 96).
    # shellcheck disable=SC2086
    ( if [[ -n "$also" ]]; then
        head_check "${bases[$i]}" "$dir/$((i + 1))-head" $also
        case $? in 0) exit 96 ;; 97) exit 97 ;; esac
      fi
      run_check "${cands[$i]}" "${bases[$i]}" "$dir/$((i + 1))" $also ) & pids+=($!)
  done
  local results=()
  for i in "${!pids[@]}"; do wait "${pids[$i]}"; results+=($?); done
  # 3. The green prefix lands.
  local land="" landed=0 waiting=0
  for i in "${!cands[@]}"; do
    k="${keep[$i]}"
    case "${results[$i]}" in
      0) if [[ $waiting == 0 ]]; then land="${cands[$i]}"; landed=$((i + 1)); fi ;;
      97) pl_say "${names[$k]}: the hosts could not run its check; it stays queued"; waiting=1 ;;
      96) fixed_first "${names[$k]}" "${shas[$k]}" "${bases[$i]}"; waiting=1 ;;
      *) if [[ $waiting == 0 ]]; then
           send_back "${names[$k]}" "${shas[$k]}" "its check failed on the head it would land on" "$dir/$((i + 1))"
           rejected=$((rejected + 1))
         fi
         waiting=1 ;;
    esac
  done
  if [[ -n "$land" ]]; then
    if git push -q "$pl_remote" "$land:refs/heads/$pl_main" 2> /dev/null; then
      pl_say "landed $landed branch(es): $pl_main is ${land:0:10}"
      for (( i = 0; i < landed; i++ )); do
        k="${keep[$i]}"
        git push -q "$pl_remote" --force-with-lease="refs/heads/$pl_queue/${names[$k]}:${shas[$k]}" ":refs/heads/$pl_queue/${names[$k]}" 2> /dev/null
        [[ "${names[$k]}" != integrator/revert-* ]] || reverted "${names[$k]}" "${cands[$i]}"
        [[ "${names[$k]}" == integrator/* ]] || pl_message "${names[$k]%%/*}" "Your branch $pl_queue/${names[$k]} landed on $pl_main as ${cands[$i]:0:10}."
      done
      if [[ "${OA_INTEGRATE_AFTER:-1}" == 1 ]]; then
        nohup "$here/after.sh" --once >> "$pl_dir/logs/after.out" 2>&1 < /dev/null &
      fi
    else
      pl_say "$pl_main moved under the round (a direct push): the green branches are stacked and checked again"
      landed=0
    fi
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$(date +%FT%T)" "${head:0:10}" "$landed" "$rejected" "$(( ${#names[@]} - landed - rejected ))" "$(( $(date +%s) - started ))" >> "$pl_dir/logs/integrate.tsv"
  return 0
}

main_loop() {
  if [[ "$once" == 1 ]]; then round; s=$?; (( s == 1 )) && [[ -z "${OA_INTEGRATE_QUIET:-}" ]] && pl_say "the queue is empty"; return 0; fi
  pl_say "integrator: watching $pl_remote $pl_queue/* for $pl_main (OA_INTEGRATE_MAX=$max, revert $([[ $revert == 1 ]] && echo "after $revert_minutes minutes" || echo off))"
  while true; do
    round; s=$?
    (( s == 0 )) || sleep "$poll"
  done
}
# One integrator at a time.
exec 9> "$pl_dir/integrate.lock"
if ! flock -n 9; then [[ -n "${OA_INTEGRATE_QUIET:-}" ]] || echo "integrate: another integrator is running ($pl_dir/integrate.lock)" >&2; exit 75; fi
main_loop

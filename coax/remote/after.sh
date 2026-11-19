#!/usr/bin/env bash
# Firm reason: 5. The green-main pipeline's after-landing check
# (docs/metaverse/green-main-pipeline.md, #13102): git, gh, jq and
# coax/remote/remote.sh. The checks run on the remote build hosts.
#
#   coax/remote/after.sh [--once]   one run on the newest head of main, unless one
#                                   is running or that head was checked
#   coax/remote/after.sh --watch    the same every minute, for ever, and the
#                                   30-minute revert (integrate.sh --once --revert;
#                                   --no-revert or OA_AFTER_REVERT=0: no reverts)
#   coax/remote/after.sh install    run the watcher under launchd (macOS) or as a
#                                   systemd user service (Linux): it starts at login
#                                   or boot, starts again whenever it stops, and takes
#                                   new pipeline scripts from main (uninstall: stop it).
#                                   OA_AFTER_STANDIN=1: a stand-in that stops and
#                                   uninstalls itself once another machine's watcher
#                                   publishes the shared state again (#13723)
#   coax/remote/after.sh state      the watcher, main, main-green, the last runs and
#                                   the red list
#   coax/remote/after.sh health     whether the watcher is running (status 1: it is not)
#   coax/remote/after.sh red        the red list: failures on main and who owns them
#   coax/remote/after.sh history [NAME]
#                                   compile-limit use, wasm sizes and allowlist counts per commit
#   coax/remote/after.sh flaky      the flaky checks: owner, issue, age (over 7 days: failing)
#
# Gates that compare (#13266): the allowlist counts (coax/remote/counts.sh: a
# head that raises one over the baseline since main-green is red, the culprit
# is the commit that raised it, found with git alone), and Everglade II against
# production (check.sh e2: the room's protocol pair on the head's room and on
# production's, OA_AFTER_PRODUCTION or the newest PASSED release in the bucket;
# a head worse than production beyond the noise margins is red, and when
# main-green is as bad the failure is reported, not red).
#
# Two runs, each a set of jobs side by side on host slots (coax/remote/check.sh):
#
# A fast run, on every new head H once main-green exists: only what changed
# since main-green G. The suites whose sources changed (check.sh since G, in
# OA_AFTER_FAST_PARTS parts), the web modules the change touched and the
# report checks (check.sh web G), the room build when the change reaches it
# (check.sh room-since G), the API suite in OA_AFTER_API_PARTS parts when the
# change reaches it (check.sh api-since G I/N), Coder's checks and smoke when it
# reaches Coder (check.sh coder-since G), and coax/tests/run.sh in 8
# parts when compiler sources changed. After them, and never part of the
# verdict, the mutation report: the mutants of what the change touched
# (check.sh mutate-since, mutants_of below). Everything else
# passed at G and reads no changed file, so it passes at H. A head that changes
# only documents is green with no job. A head that changes the compiler
# (coax/src, coax/bootstrap, coax/build.sh) gets a full run instead.
#
# With no main-green, a fast run compares against the head of the last full
# run and runs that run's failures again: when they and everything the change
# touched pass, the head is green. A new failure is bisected from that head.
#
# A full run, when there is no main-green (at most once an hour), and at most
# every OA_AFTER_GAP seconds after that: every suite in OA_AFTER_PARTS parts, every web module
# against its budget in OA_AFTER_MODULE_PARTS parts, the report checks and the
# room build (check.sh tools), the API suite in OA_AFTER_API_PARTS parts
# (check.sh api I/N), Coder's checks and smoke (check.sh coder), and coax/tests/run.sh in 8 parts when compiler
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
# to OA_AFTER_RETRIES times, before the run counts as unfinished. An unfinished
# run, or a bisect the hosts could not finish, leaves the head unjudged: the
# jobs that did not pass run again at once on the same head, up to
# OA_AFTER_UNFINISHED (3) runs, then every half hour, and it is reported.
#
# The pipeline reports its own trouble (pipeline-lib.sh's pl_alarm: the log,
# OA_PIPELINE_DIR/ALARM and one GitHub issue): the watcher was not running,
# main-green is OA_AFTER_STALE (5400) seconds or OA_AFTER_STALE_COMMITS (40)
# commits behind main (state prints a BEHIND line), or the hosts could not
# finish a head.
#
# Results: OA_PIPELINE_DIR/releases/after/COMMIT/ (each job's output and
# report, run.tsv with each job's status and seconds, kind and status, and
# mutants/ with the mutation report, report.md),
# history.tsv (commit, time, kind, name, value, limit), runs.tsv (time, commit,
# kind, status, seconds, slot-seconds, jobs) and red.json.
#
# Settings: OA_AFTER_PARTS (6), OA_AFTER_MODULE_PARTS (3), OA_AFTER_FAST_PARTS
# (3), OA_AFTER_API_PARTS (2), OA_AFTER_MUTATE (1: the mutation report; 0: none),
# OA_AFTER_MUTATE_PARTS (3), OA_AFTER_MUTATE_TIMEOUT (OA_AFTER_TIMEOUT), OA_AFTER_MUTATE_RECHECK (6: suites the report rechecks), OA_AFTER_GAP (10800: seconds between full
# runs), OA_AFTER_FAST_GAP (0: seconds between fast runs), OA_AFTER_COMPILER (auto, always or never),
# OA_BISECT_WIDTH (4), OA_BISECT_CHECKS (4: failing checks bisected one by
# one; the rest wait for the next run), OA_AFTER_TIMEOUT (3000 s a job's command),
# OA_AFTER_QUEUE (3600 s more to find a host slot),
# OA_AFTER_RETRIES (2), OA_AFTER_UNFINISHED (3), OA_AFTER_STALE (5400),
# OA_AFTER_STALE_COMMITS (40),
# OA_AFTER_FLAKY_TRIES (3), OA_AFTER_PAGES (0: no pages), OA_AFTER_PAGES_GAP
# (3600), OA_AFTER_FUZZ (1: a nightly fuzz; off by default), OA_AFTER_FUZZ_GAP (86400),
# OA_AFTER_FUZZ_HOURS (4), OA_AFTER_SOAK (1: a nightly soak; off by default), OA_AFTER_SOAK_GAP (86400),
# OA_AFTER_SOAK_SECONDS (7200), OA_AFTER_SOAK_PLAYERS (32: staging's one shard), OA_AFTER_CHECK (a command run as CMD COMMIT OUTDIR CHECK-ARGS... in
# place of the remote check, for scratch runs), and
# coax/remote/pipeline-lib.sh's OA_PIPELINE_*.
set -uo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
source "$here/pipeline-lib.sh"
source "$here/counts.sh"
parts="${OA_AFTER_PARTS:-6}"
module_parts="${OA_AFTER_MODULE_PARTS:-3}"
fast_parts="${OA_AFTER_FAST_PARTS:-3}"
api_parts="${OA_AFTER_API_PARTS:-2}"   # coax/web/api/test.sh --part: see docs/metaverse/remote-builds.md, "The API suite"
gap="${OA_AFTER_GAP:-10800}"   # a full run costs about $0.30 of host time (2026-11-09)
fast_gap="${OA_AFTER_FAST_GAP:-0}"
retries="${OA_AFTER_RETRIES:-2}"
pages_gap="${OA_AFTER_PAGES_GAP:-3600}"
fuzz_gap="${OA_AFTER_FUZZ_GAP:-86400}"
fuzz_hours="${OA_AFTER_FUZZ_HOURS:-4}"
soak_gap="${OA_AFTER_SOAK_GAP:-86400}"
soak_seconds="${OA_AFTER_SOAK_SECONDS:-7200}"
soak_players="${OA_AFTER_SOAK_PLAYERS:-32}"
soak_title="Nightly soak: staging's fleet does not hold"   # the one open issue a failing night opens or comments on
compiler_mode="${OA_AFTER_COMPILER:-auto}"
width="${OA_BISECT_WIDTH:-4}"
bisect_checks="${OA_BISECT_CHECKS:-4}"
limit="${OA_AFTER_TIMEOUT:-3000}"
# A job's wait for a host slot is not its command's time: the host stops the command at
# OA_AFTER_TIMEOUT less a minute (OA_REMOTE_TIMEOUT), and the job as a whole gets OA_AFTER_QUEUE
# more seconds (3600) to find a slot. Before #13603 the wait counted, so a full run's last jobs,
# queued behind the others and other agents' work, ran out with no command started.
queue="${OA_AFTER_QUEUE:-3600}"
max_unfinished="${OA_AFTER_UNFINISHED:-3}"
stale="${OA_AFTER_STALE:-5400}"
stale_commits="${OA_AFTER_STALE_COMMITS:-40}"
results="$pl_dir/releases/after"
history="$results/history.tsv"
label="${OA_AFTER_LABEL:-com.openagents.green-main}"
# Whether a watcher is alive, from the time it writes each minute (heartbeat).
# On a machine that isn't the watcher, the watcher's shared state first
# (pipeline-lib.sh's pl_mirror): the watcher elsewhere, and its host.
watcher_health() {
  local at=0 age where="" got=0 unread
  ! pl_mirror || where=" on $(cat "$results/watcher-host")"
  [[ ! -f "$results/heartbeat" ]] || at="$(cat "$results/heartbeat")"
  age=$(( $(date +%s) - at ))
  if (( age < 300 )); then echo "watcher     running$where, seen $age s ago"; return 0; fi
  # The watcher elsewhere is judged by the last copy of its state this machine
  # could read: the heartbeat's age when it was read. A copy that can't be read
  # now says nothing about the watcher, so it raises no alarm (#13525).
  if [[ -n "$where" ]]; then
    [[ ! -f "$pl_dir/mirror/read" ]] || got="$(cat "$pl_dir/mirror/read")"
    unread=$(( $(date +%s) - got ))
    if (( unread >= 60 )) && (( got - at < 300 )); then
      echo "watcher     running$where when its state was last read, $(( unread / 60 )) min ago; it can't be read now ($pl_shared/state.tar.gz)"
      return 0
    fi
    (( got == 0 )) || age=$(( got - at ))
  fi
  # A watcher on another machine that went quiet can't say so itself: the
  # machine that sees it does (the log, ALARM and the one GitHub issue).
  [[ -z "$where" ]] || (( age < 900 )) || pl_alarm "quiet-$at" "the watcher$where has not been seen for $(( age / 60 )) minutes" "The green-main watcher$where last wrote its heartbeat at $(pl_date "$at" '%Y-%m-%d %H:%M %Z'), so no head is checked and \`$pl_green\` does not move. Check it there: \`systemctl --user status com.openagents.green-main\` and \`~/.openagents/pipeline/logs/after.out\` (docs/metaverse/green-main-pipeline.md)." > /dev/null 2>&1
  echo "watcher     NOT RUNNING$where$( (( at == 0 )) || echo " since $(pl_date "$at" '%Y-%m-%d %H:%M')"): no head is checked and $pl_green does not move. $([[ -n "$where" ]] && echo "Check it there: systemctl --user status $label" || echo "Start it: coax/remote/after.sh install")"
  return 1
}

# Production's commit (the newest release that passed, as release.sh reads it), at most every
# 10 minutes from the bucket; OA_AFTER_PRODUCTION names one instead, and OA_AFTER_E2=0 turns
# Everglade II's comparison off. "" when it can't be read.
prod_bucket="${OA_AFTER_PROD_BUCKET:-openagentsgemini-openagents-web}"
production() {
  [[ "${OA_AFTER_E2:-1}" != 0 ]] || { echo ""; return; }
  if [[ -n "${OA_AFTER_PRODUCTION:-}" ]]; then git rev-parse -q --verify "${OA_AFTER_PRODUCTION}^{commit}" 2> /dev/null | tee "$results/production"; return; fi
  local f="$results/production" newest c
  if [[ ! -s "$f" ]] || (( $(date +%s) - $(stat -c %Y "$f" 2> /dev/null || stat -f %m "$f") > 600 )); then
    local cfg=(); [[ -n "${CLOUDSDK_CONFIG:-}" || ! -d "$HOME/work/.secrets/gcloud-sa-config" ]] || cfg=(env CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config")
    # A rolled-back release's folder has no PASSED: take the newest one that does.
    newest="$(${cfg[@]+"${cfg[@]}"} gcloud storage ls "gs://$prod_bucket/releases/" 2> /dev/null | grep -E '/releases/[0-9]{8}T[0-9]{6}Z/$' | sort -r | head -10)"
    c=""
    for r in $newest; do c="$(${cfg[@]+"${cfg[@]}"} gcloud storage cat "${r}PASSED" 2> /dev/null | awk 'NR == 1 { print $1 }')"; [[ -z "$c" ]] || break; done
    if [[ "$c" =~ ^[0-9a-f]{40}$ ]]; then echo "$c" > "$f"; else touch "$f"; fi
  fi
  c="$(cat "$f" 2> /dev/null)"
  git rev-parse -q --verify "$c^{commit}" > /dev/null 2>&1 && echo "$c" || echo ""
}
# The flaky list (OA_PIPELINE_DIR/releases/after/flaky.tsv, on the watcher's machine): one line a
# check that failed and passed when run again, "KEY FIRST-HEAD SINCE LAST TIMES OWNER ISSUE".
# KEY is the check with its base commit taken out. The owner is the agent of the last commit that
# changed the check's file (a suite or a compiler unit), else "pipeline"; the issue is one GitHub
# issue a key. An entry older than OA_AFTER_FLAKY_DAYS (7) counts as a failure in state. It leaves
# when its issue is closed (checked once an hour). The list before it, flaky.txt (HEAD and check,
# no owner), is kept as flaky-before-13266.txt.
flaky_tsv="$results/flaky.tsv"
[[ ! -f "$results/flaky.txt" || -e "$results/flaky-before-13266.txt" ]] || mv "$results/flaky.txt" "$results/flaky-before-13266.txt"
flaky_days="${OA_AFTER_FLAKY_DAYS:-7}"
flaky_key() { sed -E 's/[0-9a-f]{40}/BASE/g' <<< "$1"; }
flaky_owner() {   # item
  local path="" c
  case "$1" in
    suite\ *) path="${1#suite }" ;;
    compiler\ *) path="$(git ls-tree -r --name-only HEAD -- "coax/tests/${1#compiler }"* 2> /dev/null | head -1)" ;;
  esac
  [[ -n "$path" ]] && c="$(git log -1 --format=%H "$main_ref" -- "$path" 2> /dev/null)" && [[ -n "$c" ]] && pl_agent_of "$c" | grep . || echo pipeline
}
flaky_add() {   # head item: a flaky failure, its entry opened (with an owner and an issue) or stamped
  local key now owner issue="" url
  key="$(flaky_key "$2")"; now=$(date +%s)
  touch "$flaky_tsv"
  if awk -F'\t' -v k="$key" '$1 == k { f = 1 } END { exit !f }' "$flaky_tsv"; then
    awk -F'\t' -v OFS='\t' -v k="$key" -v t="$now" '$1 == k { $4 = t; $5 = $5 + 1 } { print }' "$flaky_tsv" > "$flaky_tsv.new" && mv "$flaky_tsv.new" "$flaky_tsv"
    return 0
  fi
  owner="$(flaky_owner "$2")"
  if [[ "$pl_issues" == 1 ]]; then
    url="$(gh issue create --repo "$pl_repo" --title "Flaky check: $key" --body "The after-landing check \`$2\` failed at ${1:0:10} and passed when run again on the same commit, so it has no culprit (docs/metaverse/green-main-pipeline.md, \"Flaky checks\").

Owner: \`$owner\` (the agent of the last commit that changed the check's file; \`pipeline\` when it has none). Make the check deterministic, or say here why it can't be. The entry counts as a failure in \`coax/remote/after.sh state\` $flaky_days days after it opened; it leaves the flaky list when this issue is closed." 2> /dev/null)" && issue="${url##*/}"
  fi
  printf '%s\t%s\t%s\t%s\t1\t%s\t%s\n' "$key" "${1:0:10}" "$now" "$now" "$owner" "${issue:--}" >> "$flaky_tsv"
  pl_message "$owner" "The check \`$2\` is flaky on $pl_main (it failed at ${1:0:10} and passed when run again). You own its entry on the flaky list${issue:+: https://github.com/$pl_repo/issues/$issue}. After $flaky_days days it counts as a failure."
}
flaky_prune() {   # once an hour: entries whose issue is closed leave
  [[ -s "$flaky_tsv" && "$pl_issues" == 1 ]] || return 0
  due last-flaky-prune 3600 || return 0
  date +%s > "$results/last-flaky-prune"
  local key issue keep=""
  while IFS=$'\t' read -r key _ _ _ _ _ issue; do
    [[ "$issue" =~ ^[0-9]+$ ]] || continue
    [[ "$(gh issue view "$issue" --repo "$pl_repo" --json state --jq .state 2> /dev/null)" == CLOSED ]] || continue
    awk -F'\t' -v k="$key" '$1 != k' "$flaky_tsv" > "$flaky_tsv.new" && mv "$flaky_tsv.new" "$flaky_tsv"
    pl_say "after: $key left the flaky list (#$issue is closed)"
  done < "$flaky_tsv"
}
flaky_show() {   # the entries, oldest first; status 1 when one is older than flaky_days
  [[ -s "$flaky_tsv" ]] || { echo "flaky list: empty"; return 0; }
  local now; now=$(date +%s)
  awk -F'\t' -v now="$now" -v days="$flaky_days" '
    { age = (now - $3) / 86400; over = age >= days; bad += over
      line[NR] = sprintf("  %s%s: owner %s, %s, %.1f days, %d time(s), last at %s", over ? "FAILING (over " days " days) " : "", $1, $6, ($7 == "-" ? "no issue" : "#" $7), age, $5, $2) }
    END { printf "flaky list: %d entr(ies), %d over %d days (each counts as a failure)\n", NR, bad, days; for (i = 1; i <= NR; i++) print line[i]; exit bad > 0 }' "$flaky_tsv"
}
# main-green's lag behind main (the oldest commit after it, in minutes), sampled every 5 minutes by
# the watcher into lag.tsv: state prints now, the last 7 days' worst and the share under 30 minutes.
lag_now() {   # head green: seconds
  local first; [[ -n "$2" && "$1" != "$2" ]] || { echo 0; return; }
  first="$(git rev-list --reverse --first-parent "$2..$1" | head -1)"
  [[ -n "$first" ]] || { echo 0; return; }
  echo $(( $(date +%s) - $(git log -1 --format=%ct "$first") ))
}
behind() {   # head green: "COMMITS SECONDS" main-green is behind main
  local first n
  [[ -n "$1" && -n "$2" && "$1" != "$2" ]] || { echo "0 0"; return; }
  n="$(git rev-list --count --first-parent "$2..$1" 2> /dev/null || echo 0)"
  first="$(git rev-list --reverse --first-parent "$2..$1" | head -1)"
  [[ -n "$first" ]] || { echo "$n 0"; return; }
  echo "$n $(( $(date +%s) - $(git log -1 --format=%ct "$first") ))"
}
lag_show() {
  [[ -s "$results/lag.tsv" ]] || return 0
  awk -F'\t' -v since="$(( $(date +%s) - 7 * 86400 ))" '$1 >= since { n++; if ($2 > worst) worst = $2; if ($2 <= 1800) ok++ }
    END { if (n) printf "main-green lag, last 7 days: worst %d min, under 30 min in %d of %d samples (%.0f%%)\n", worst / 60, ok, n, ok * 100 / n }' "$results/lag.tsv"
}
case "${1:-}" in
  flaky) flaky_show; exit $? ;;
  state)
    git fetch -q "$pl_remote" "+refs/heads/$pl_main:refs/remotes/$pl_remote/$pl_main" 2> /dev/null
    git fetch -q "$pl_remote" "+refs/heads/$pl_green:refs/remotes/$pl_remote/$pl_green" 2> /dev/null || git update-ref -d "refs/remotes/$pl_remote/$pl_green" 2> /dev/null
    h="$(git rev-parse -q --verify "refs/remotes/$pl_remote/$pl_main" 2> /dev/null || true)"
    g="$(git rev-parse -q --verify "refs/remotes/$pl_remote/$pl_green" 2> /dev/null || true)"
    watcher_health
    [[ ! -s "$pl_dir/ALARM" ]] || { echo "alarms (time, kind, what):"; tail -n 5 "$pl_dir/ALARM" | sed 's/^/  /'; }
    echo "$pl_main        ${h:0:10} $(git log -1 --format='%s' "$h" 2> /dev/null | cut -c1-70)"
    if [[ -z "$g" ]]; then echo "$pl_green  none: no run has passed yet"
    else echo "$pl_green  ${g:0:10} $(git log -1 --format='%cd' --date=format-local:'%Y-%m-%d %H:%M' "$g"), $(git rev-list --count "$g..$h" 2> /dev/null) commit(s) behind $pl_main, lag $(( $(lag_now "$h" "$g") / 60 )) min"
      read -r bn ba <<< "$(behind "$h" "$g")"
      (( ba < stale && bn < stale_commits )) || echo "BEHIND      $pl_green is $bn commit(s) and $(( ba / 60 )) min behind $pl_main: releases ship $pl_green, so they wait (the alarm: $(( stale / 60 )) min or $stale_commits commits)"
    fi
    lag_show
    # The allowlist counts (#13266): main's, main-green's (the baseline) and what a commit raised.
    if [[ -n "$h" ]]; then
      echo "allowlist counts ($pl_main, $pl_green; a head that raises one is red, coax/remote/counts.sh):"
      join -a 1 <(cn_show "$h" | sort) <( [[ -z "$g" ]] || cn_show "$g" | sort) | awk '{ printf "  %-20s %6s %6s%s\n", $1, $2, ($3 == "" ? "-" : $3), ($3 != "" && $2 > $3 ? "  RAISED" : "") }'
      [[ -z "$g" ]] || cn_check "$g" "$h" 2> /dev/null | awk '{ printf "  raised: %s by %s (baseline %s, now %s)\n", $2, substr($3, 1, 10), $4, $5 }'
    fi
    # Everglade II against production: the last run's numbers.
    p="$(cat "$results/production" 2> /dev/null)"
    e2="$(ls -t "$results"/*/*/build/pipeline/e2.tsv 2> /dev/null | head -1)"
    echo "Everglade II against production${p:+ (${p:0:10})}: $([[ -n "$e2" ]] && echo "last measured on ${e2#"$results/"}" | cut -c1-80 || echo "not measured yet")"
    [[ -z "$e2" ]] || awk -F'\t' '{ h[$1] = h[$1] $3; a[$1] = a[$1] " " $4; b[$1] = b[$1] " " $5; t[$1] = t[$1] " " $6 }
      END { for (s in h) printf "  %-4s holds %s  ack p95%s ms  bytes/s%s  tick p95%s us\n", s, h[s], a[s], b[s], t[s] }' "$e2" | sort
    if [[ -s "$results/runs.tsv" ]]; then
      echo "last runs (time, commit, kind, status, seconds, slot-seconds, jobs):"
      tail -n 5 "$results/runs.tsv" | awk -F'\t' '{ printf "  %s  %s  %-4s  %-10s %5s s  %6s slot-s  %s job(s)\n", $1, $2, $3, $4, $5, $6, $7 }'
    fi
    n="$(jq '[.[] | select(.status == "open")] | length' "$pl_red")"
    echo "red list: $n open"
    jq -r '.[] | select(.status == "open") | "  \(.check): \(if .culprit == "" then "no culprit" else .culprit[0:10] + " by " + .agent end)\(if .issue then " (#" + .issue + ")" else "" end)"' "$pl_red" | head -n 40
    flaky_show
    # The mutation report (#13610): its last head, what is still outstanding, and its issue.
    if [[ -f "$results/mutants-head" ]]; then
      echo "mutation report: last at $(cut -c1-10 "$results/mutants-head"), $(cat "$results/mutants-outstanding.tsv" 2> /dev/null | wc -l | tr -d " ") item(s) outstanding$([[ -f "$results/mutants-issue" ]] && echo " (#$(cat "$results/mutants-issue"))")"
      awk -F'\t' '{ printf "  %s %s %s line %s: %s\n", $1, $2, $3, $4, $5 }' "$results/mutants-outstanding.tsv" 2> /dev/null | head -n 20
    fi
    exit 0 ;;
  health) watcher_health; exit $? ;;
  install)
    # The watcher as a launchd job that starts at login and starts again
    # whenever it stops, in a checkout of its own that holds only coax/remote.
    root="$(git rev-parse --show-toplevel 2> /dev/null)" || { echo "after: not in a Git checkout" >&2; exit 2; }
    co="$pl_dir/checkout"; plist="$HOME/Library/LaunchAgents/$label.plist"; mref="refs/remotes/$pl_remote/$pl_main"
    git -C "$root" fetch -q "$pl_remote" "+refs/heads/$pl_main:$mref" || { echo "after: could not fetch $pl_remote" >&2; exit 1; }
    if [[ ! -e "$co/.git" ]]; then
      git -C "$root" worktree prune
      git -C "$root" worktree add -q --detach --no-checkout "$co" "$mref" || exit 1
      git -C "$co" sparse-checkout set --cone coax/remote || exit 1
    fi
    git -C "$co" checkout -q --detach "$mref" || exit 1
    keep="OA_REMOTE_STATE OA_REMOTE_DAILY_USD CLOUDSDK_CONFIG OA_PIPELINE_SHARED OA_PIPELINE_NOTIFY OA_AFTER_REVERT OA_AFTER_STANDIN"
    esc() { sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g' <<< "$1"; }
    cmd="cd '$co' || exit 1; if git fetch -q $pl_remote '+refs/heads/$pl_main:$mref'; then git checkout -q --detach '$mref'; fi; exec coax/remote/after.sh --watch"
    if [[ "$(uname -s)" == Linux ]]; then
      # A systemd user service on a Linux host: the same command, restarted a
      # minute after it stops (Restart=always also takes new pipeline scripts),
      # and its jobs' children left to finish (KillMode=process, as launchd's
      # AbandonProcessGroup). `loginctl enable-linger` keeps it running with
      # no one logged in.
      unit="$HOME/.config/systemd/user/$label.service"
      sd() { sed -e 's/%/%%/g' -e 's/\$/$$/g' <<< "$1"; }
      mkdir -p "$(dirname "$unit")"
      cat > "$unit" <<EOF
[Unit]
Description=Green-main pipeline watcher (coax/remote/after.sh --watch, docs/metaverse/green-main-pipeline.md)
After=network-online.target

[Service]
Type=simple
Environment="PATH=$(sd "$PATH")" "HOME=$(sd "$HOME")" OA_AFTER_SUPERVISED=1 "OA_PIPELINE_DIR=$(sd "$pl_dir")"$(for v in $keep; do [[ -z "${!v:-}" ]] || printf ' "%s=%s"' "$v" "$(sd "${!v}")"; done)
ExecStart=$(command -v bash) -c "$(sd "$cmd")"
Restart=always
RestartSec=60
KillMode=process
StandardOutput=append:$(sd "$pl_dir/logs/after.out")
StandardError=append:$(sd "$pl_dir/logs/after.out")

[Install]
WantedBy=default.target
EOF
      [[ "${OA_AFTER_INSTALL_PRINT:-0}" != 1 ]] || { cat "$unit"; exit 0; }
      systemctl --user daemon-reload && systemctl --user enable --now "$label.service" || { echo "after: systemd did not take $unit" >&2; exit 1; }
      [[ "$(loginctl show-user "$(id -un)" -p Linger --value 2> /dev/null)" == yes ]] ||
        echo "after: run 'sudo loginctl enable-linger $(id -un)' so the watcher runs with no one logged in" >&2
      echo "after: the watcher runs as the systemd user service $label from $co; its log is $pl_dir/logs/after.out"
      exit 0
    fi
    mkdir -p "$HOME/Library/LaunchAgents"
    cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key><array><string>$(esc "$(command -v bash)")</string><string>-c</string><string>$(esc "$cmd")</string></array>
  <key>EnvironmentVariables</key><dict>
    <key>PATH</key><string>$(esc "$PATH")</string>
    <key>HOME</key><string>$(esc "$HOME")</string>
    <key>OA_AFTER_SUPERVISED</key><string>1</string>
    <key>OA_PIPELINE_DIR</key><string>$(esc "$pl_dir")</string>
$(for v in $keep; do [[ -z "${!v:-}" ]] || printf '    <key>%s</key><string>%s</string>\n' "$v" "$(esc "${!v}")"; done)
  </dict>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ThrottleInterval</key><integer>60</integer>
  <key>AbandonProcessGroup</key><true/>
  <key>StandardOutPath</key><string>$(esc "$pl_dir/logs/after.out")</string>
  <key>StandardErrorPath</key><string>$(esc "$pl_dir/logs/after.out")</string>
</dict></plist>
EOF
    launchctl bootout "gui/$(id -u)/$label" 2> /dev/null
    for try in 1 2 3 4 5 6; do
      launchctl bootstrap "gui/$(id -u)" "$plist" 2> /dev/null && break
      sleep 2
    done
    launchctl print "gui/$(id -u)/$label" > /dev/null 2>&1 || { echo "after: launchd did not take $plist" >&2; exit 1; }
    echo "after: the watcher runs as $label from $co; its log is $pl_dir/logs/after.out"
    exit 0 ;;
  uninstall)
    if [[ "$(uname -s)" == Linux ]]; then
      systemctl --user disable --now "$label.service" 2> /dev/null
      rm -f "$HOME/.config/systemd/user/$label.service"; systemctl --user daemon-reload 2> /dev/null
    else
      launchctl bootout "gui/$(id -u)/$label" 2> /dev/null
      rm -f "$HOME/Library/LaunchAgents/$label.plist"
    fi
    rm -f "$results/heartbeat"
    echo "after: the watcher is stopped and will not start again"
    exit 0 ;;
  red)
    pl_mirror || true
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
    -h|--help) sed -n '2,84p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
# The host a job ran on, from remote.sh's last line in its output ("" when none).
host_of() { sed -nE 's/^remote: ([A-Za-z0-9._-]+) slot [0-9]+: .*/\1/p' "$1" 2> /dev/null | tail -1; }
# Whether a failed job's output judges the commit (#13256): the check said so
# itself. check.sh ends a failed check with "check MODE...: FAILED"; a job
# whose output is empty, or holds only remote.sh's own lines, or stops before
# that line (a host that filled up, dropped the connection or killed the job)
# says nothing of the commit. check-visuals.sh, run as itself, has only to say
# something.
judged() {   # outfile check-args...
  local f="$1"; shift
  grep -qv '^remote: ' "$f" 2> /dev/null || return 1
  [[ "$1" == visuals && -z "${OA_AFTER_CHECK:-}" ]] && return 0
  grep -Eq '^check .*: FAILED$' "$f"
}
# One check on one commit: 0 passed, 1 failed, 97 the hosts could not run it
# (after OA_AFTER_RETRIES more tries: a spot host can stop under a job). A job
# the host could not judge (97, or a failure with no verdict: judged) runs
# again on another host when there is one, with the slot's compiler installed
# again first (OA_REMOTE_AVOID, OA_REMOTE_FRESH_COMPILER), and is never
# counted as the commit's failure (#13256).
check() {   # commit outdir check-args...
  local commit="$1" dir="$2" s try=0 avoid="${OA_REMOTE_AVOID:-}" fresh="${OA_REMOTE_FRESH_COMPILER:-0}"; shift 2
  local t0; t0=$(date +%s)
  while true; do
    rm -rf "$dir"; mkdir -p "$dir"
    if [[ -n "${OA_AFTER_CHECK:-}" ]]; then
      # shellcheck disable=SC2086
      OA_REMOTE_AVOID="$avoid" OA_REMOTE_FRESH_COMPILER="$fresh" timeout "$(( limit + queue ))" $OA_AFTER_CHECK "$commit" "$dir" "$@" > "$dir/out" 2>&1; s=$?
    else
      # check-visuals runs as itself, so a bisect can run it on commits whose
      # check.sh has no such step.
      local cmd=(coax/remote/check.sh "$@")
      [[ "$1" != visuals ]] || cmd=(coax/web/check-visuals.sh apps/openagents-web)
      # Without the lock descriptors: nothing a job leaves behind holds the watcher's locks.
      OA_REMOTE_AVOID="$avoid" OA_REMOTE_FRESH_COMPILER="$fresh" OA_REMOTE_TIMEOUT=$(( limit - 60 )) timeout "$(( limit + queue ))" "$here/remote.sh" run --commit "$commit" --out "$dir" --no-fallback -- "${cmd[@]}" > "$dir/out" 2>&1 7>&- 8>&-; s=$?
    fi
    case "$s" in
      0) break ;;
      124) echo "after: the check ran past $(( limit + queue )) s, its wait for a host slot included" >> "$dir/out"; s=97; break ;;
      97|255) s=97 ;;
      *) if judged "$dir/out" "$@"; then s=1; break; fi
         echo "after: status $s with no verdict from the check (empty output, or it stopped first): the host could not judge it (97)" >> "$dir/out"; s=97 ;;
    esac
    (( try < retries )) || break
    try=$((try + 1)); cp "$dir/out" "$dir/../$(basename "$dir").try$try.out" 2> /dev/null
    # Again: elsewhere, with the slot's compiler installed again.
    avoid="$(host_of "$dir/out")"; [[ -n "$avoid" ]] || avoid="${OA_REMOTE_AVOID:-}"; fresh=1
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
    step\ tools-check|step\ after-gates|step\ everglade-parity|step\ wire-check|step\ layer-check|step\ editor-skin|step\ wgsl|step\ realm-pages|step\ changelog-page|step\ realm-cards) echo "reports" ;;
    step\ room-build) echo "room" ;;
    step\ coder-checks|step\ coder-smoke|step\ coder-desktop-evals) echo "coder" ;;
    step\ check-visuals) echo "visuals" ;;
    e2\ *) echo "e2 $(cat "$results/production" 2> /dev/null)" ;;
    *) echo "$*" ;;   # the whole job again
  esac
}

# A failing check run again on the head: "flaky" when it passes, "red" when
# it fails again, "infra" when the hosts could not run it. A check that was
# flaky before (flaky.txt) gets OA_AFTER_FLAKY_TRIES (3) runs, and one pass is
# enough.
# The run again goes to another host than the failure's (OA_REMOTE_AVOID, which
# the caller sets), so one bad host can't confirm its own failure (#13256).
confirm() {   # dir head item check-args...
  local dir="$1" head="$2" item="$3" tries=1 t; shift 3
  ! cut -f1 "$flaky_tsv" 2> /dev/null | grep -qxF "$(flaky_key "$item")" || tries="${OA_AFTER_FLAKY_TRIES:-3}"
  for (( t = 1; t <= tries; t++ )); do
    check "$head" "$dir/c$t-${head:0:10}" "$@"
    case $? in 0) echo flaky; return 0 ;; 97) echo infra; return 0 ;; esac
  done
  echo red
}
# Bisect one check between good (passes) and bad (fails: confirm ran it
# again). Prints the culprit, "infra" when the hosts could not run any commit
# of a round, or "" when it did not settle in 20 rounds. A commit the hosts
# could not run is left out of its round; the others still narrow the range.
bisect() {   # dir good bad check-args...
  local dir="$1" good="$2" bad="$3"; shift 3
  local list=() c
  while read -r c; do list+=("$c"); done < <(git rev-list --reverse --first-parent "$good..$bad")
  (( ${#list[@]} > 0 )) || { echo ""; return 0; }
  local lo=-1 hi=$(( ${#list[@]} - 1 )) round=0
  # lo: the index known to pass (-1: good), hi: the index known to fail.
  while (( hi - lo > 1 )); do
    round=$((round + 1))
    local points=() n=$(( hi - lo - 1 )) i
    if (( n <= width )); then for (( i = lo + 1; i < hi; i++ )); do points+=("$i"); done
    else for (( i = 1; i <= width; i++ )); do points+=("$(( lo + (n + 1) * i / (width + 1) ))"); done
    fi
    local pids=() p
    for p in "${points[@]}"; do check "${list[$p]}" "$dir/r$round-${list[$p]:0:10}" "$@" & pids+=($!); done
    local st=() k settled=0
    for k in "${!pids[@]}"; do wait "${pids[$k]}"; st+=($?); done
    # Lowest failing point, highest passing point under it.
    local newhi=$hi newlo=$lo
    for k in "${!points[@]}"; do
      p="${points[$k]}"
      case "${st[$k]}" in
        0) settled=1 ;;
        1) settled=1; (( p < newhi )) && newhi=$p ;;
      esac
    done
    (( settled == 1 )) || { echo infra; return 0; }
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

Bisected between \`$pl_green\` and the head by \`coax/remote/after.sh\` (docs/metaverse/green-main-pipeline.md).

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
  ( "$here/pages.sh" build "$1" > "$results/$1/pages.log" 2>&1 && pl_say "after: ${1:0:10}'s pages are in the shared store" ) 8>&- 7>&- &
}

# The room's nightly fuzz (#13283, coax/web/room/fuzz/fuzz.sh long): at most every OA_AFTER_FUZZ_GAP
# seconds, on a head that went green, for OA_AFTER_FUZZ_HOURS on one host slot, in the background. A
# crash, hang, leak or broken property files an issue with the log; the inputs it found come back in
# RESULTS/HEAD/fuzz (each crash under coax/web/room/build/fuzz/artifacts there).
fuzz_of() {   # head
  [[ "${OA_AFTER_FUZZ:-0}" != 0 && -z "${OA_AFTER_CHECK:-}" ]] || return 0
  due last-fuzz "$fuzz_gap" || return 0
  date +%s > "$results/last-fuzz"
  local head="$1" dir="$results/$1/fuzz"
  mkdir -p "$dir"
  (
    OA_REMOTE_TIMEOUT=$(( fuzz_hours * 3600 + 1800 )) "$here/remote.sh" run --commit "$head" --out "$dir" --no-fallback -- coax/remote/check.sh fuzz "$fuzz_hours" > "$dir/out" 2>&1
    s=$?
    if (( s == 1 )); then
      gh issue create --repo "$pl_repo" --title "Nightly fuzz: the room's fuzzers failed at ${head:0:10}" --body "\`coax/web/room/fuzz/fuzz.sh long $fuzz_hours\` on \`$head\` stopped on an input (#13283). Make it \`coax/web/room/fuzz/corpus/NAME/regress-WHAT\`, fix it, and add the case to wire-fuzz.coax when it is the room's. The inputs: \`$dir/\` on the coordinator's Mac.

\`\`\`
$(tail -n 40 "$dir/out" 2> /dev/null | cut -c1-300)
\`\`\`" > /dev/null 2>&1
      pl_say "after: the nightly fuzz failed at ${head:0:10} ($dir/out)"
    else
      pl_say "after: the nightly fuzz at ${head:0:10}: status $s"
    fi
  ) 8>&- 7>&- &
}

# The nightly soak (#13296, docs/deployment/soak.md, coax/remote/check.sh soak): at most every
# OA_AFTER_SOAK_GAP seconds, on a head that went green, OA_AFTER_SOAK_PLAYERS protocol bots play
# staging's rooms for OA_AFTER_SOAK_SECONDS from one host slot, in the background. The night's
# capacity record comes back in RESULTS/HEAD/soak, is kept by its date in RESULTS/soak/DAY.txt, and
# goes to the APIs (pipeline-lib.sh pl_soak_publish: the status page shows it). A run that fails
# (a slow tick, a leak, an unsaved checkpoint, slow tickets, lost seats) opens one issue with the
# record and the log's end, and each failing night after it comments there. Staging's alerts mail nobody.
soak_of() {   # head
  [[ "${OA_AFTER_SOAK:-0}" != 0 && -z "${OA_AFTER_CHECK:-}" ]] || return 0
  due last-soak "$soak_gap" || return 0
  date +%s > "$results/last-soak"
  local head="$1" dir="$results/$1/soak" day
  day="$(TZ=America/Chicago date +%F)"
  mkdir -p "$dir" "$results/soak"
  (
    # Staging's lock (#13398, #13384): the soak holds staging for its run, so a soak never starts while a
    # release is between its staging step and its promotion. Its lock gives way to a release or a deploy
    # (release-set-policy.coax rs_lock_yields): the soak then ends its run unjudged, and the next green
    # head tries again. OA_AFTER_SOAK_LOCK=0 runs it without the lock (a soak of another environment).
    vmd="$here/../web/room/vm-deploy.sh" tok="" put_off=0 me=$BASHPID   # this subshell holds the lock: it lives as long as the run
    if [[ "${OA_AFTER_SOAK_LOCK:-1}" != 0 ]]; then
      got="$(OA_STAGING_LOCK_PID=$me "$vmd" lock take "soak ${head:0:10}" $(( soak_seconds + 3000 )) 2> "$dir/lock")" || {
        pl_say "after: the nightly soak at ${head:0:10} is put off: staging is held ($(tail -n 1 "$dir/lock" 2> /dev/null | cut -c1-200))"
        rm -f "$results/last-soak"; exit 0; }
      tok="$(sed -n 's/^lock-token //p' <<<"$got")"
    fi
    OA_REMOTE_TIMEOUT=$(( soak_seconds + 2700 )) "$here/remote.sh" run --commit "$head" --out "$dir" --no-fallback -- coax/remote/check.sh soak "$soak_seconds" "$soak_players" "$day" > "$dir/out" 2>&1 &
    run=$!
    # once a minute: is staging still the soak's? (1: taken over. 2, the lock unread, is not held against the run)
    while kill -0 "$run" 2> /dev/null; do
      for _ in $(seq 1 60); do kill -0 "$run" 2> /dev/null || break; read -r -t 1 < /dev/zero || true; done
      [[ -n "$tok" ]] || continue
      st=0; "$vmd" lock mine "$tok" > /dev/null 2>&1 || st=$?
      [[ $st == 1 ]] || continue
      put_off=1; kill "$run" 2> /dev/null; break
    done
    s=0; wait "$run" || s=$?
    if (( put_off )); then
      pl_say "after: the nightly soak at ${head:0:10} gave staging to a release and is not judged; the next green head runs it"
      rm -f "$results/last-soak"; exit 0
    fi
    [[ -z "$tok" ]] || "$vmd" lock release "$tok" > /dev/null 2>&1
    rec="$dir/build/pipeline/capacity.txt"
    if [[ -s "$rec" ]]; then cp "$rec" "$results/soak/$day.txt"; pl_soak_publish "$rec"; fi
    if (( s == 1 )); then
      # One issue for the soak while it fails: the first failing night opens it, the next ones comment.
      body="\`coax/remote/check.sh soak $soak_seconds $soak_players\` on \`$head\` did not hold on $day (#13296, docs/deployment/soak.md). The verdict line names why: a slow tick, a leak, a checkpoint unsaved too long, slow tickets, a build the room refused, or seats lost. Staging's room and API are what it played; the log and the record are in \`$dir/\` on the watcher's machine.

The record:

\`\`\`
$(cat "$rec" 2> /dev/null || echo "none was written")
\`\`\`

The log's end:

\`\`\`
$(tail -n 30 "$dir/build/pipeline/soak.log" 2> /dev/null | cut -c1-300)
\`\`\`"
      n="$(gh issue list --repo "$pl_repo" --state open --search "\"$soak_title\" in:title" --json number,title --jq "[.[] | select(.title == \"$soak_title\")][0].number // empty" 2> /dev/null)"
      if [[ -n "$n" ]]; then gh issue comment "$n" --repo "$pl_repo" --body "$body" > /dev/null 2>&1
      else gh issue create --repo "$pl_repo" --title "$soak_title" --body "$body" > /dev/null 2>&1; fi
      pl_say "after: the nightly soak failed at ${head:0:10} ($dir/out)"
    else
      pl_say "after: the nightly soak at ${head:0:10}: status $s$([[ -s "$rec" ]] && echo ", $(grep -E '^(seated|verdict) ' "$rec" | tr '\n' ' ')")"
    fi
  ) 8>&- 7>&- &
}

# The mutation report (#13583, #13601, #13603): beside each fast run, the
# mutants of the procedures its change touched, in the #authority suites that
# reach them (check.sh mutate-since BASE I/N, in OA_AFTER_MUTATE_PARTS parts,
# OA_AFTER_MUTATE_TIMEOUT seconds a part). It is a report and never a verdict:
# it doesn't hold main-green, put anything on the red list, start a bisect or
# a revert, or count as flaky. The gate that blocks is the pre-push's and the
# integrator's (check.sh only PATH BASE, check.sh branch BASE). One report runs
# at a time; a head that comes while one runs gets none, and the next report
# starts from the head the last one covered, so no change goes unmutated. The
# report, RESULTS/HEAD/mutants/report.md, names each surviving mutant's file,
# procedure, lines and counts, each procedure no check reaches, and each part
# or suite the tool could not finish ("no summary", a timeout, a host that
# could not run it), each with the command that runs it alone from its base.
# Survivors in check units (files a suite loads only with `#load ... #test;`,
# which no compile ships) are one line a file, never an action item (#13610).
# The action items stay outstanding until a later report's recheck no longer
# finds them, and one GitHub issue is open exactly while any is outstanding
# (mutants_lifecycle, below; OA_AFTER_MUTATE=0: no report).
mutate_parts="${OA_AFTER_MUTATE_PARTS:-3}"
mutate_limit="${OA_AFTER_MUTATE_TIMEOUT:-$limit}"
mutants_title="Mutation report: survivors and tool problems after landing"
mutants_pid=""
mutants_path() {   # dir rel: DIR/REL with "." and ".." taken out
  local IFS=/ p out=() parts=()
  read -ra parts <<< "$1/$2"
  for p in "${parts[@]}"; do
    case "$p" in ""|.) ;; ..) (( ${#out[@]} == 0 )) || unset 'out[${#out[@]}-1]' ;; *) out+=("$p") ;; esac
  done
  echo "${out[*]}"
}
mutants_proc() {   # head path line: the procedure the line is in ("?" when none)
  git show "$1:$2" 2> /dev/null | awk -v l="$3" 'NR > l { exit } match($0, /^[ \t]*[A-Za-z_][A-Za-z0-9_]* :: [(]/) { s = substr($0, RSTART, RLENGTH); sub(/^[ \t]*/, "", s); sub(/ :: [(]$/, "", s); p = s } END { print (p == "" ? "?" : p) }'
}
# One part's log as records: S SUITE SUMMARY PATH:LINE:COL WHAT (a survivor),
# U SUITE SUMMARY NAMES (not reached), T SUITE WHY DETAIL (the tool did not finish),
# C SUITE SUMMARY PATH COUNT (survivors in a check unit, which only checks reach: counted, never
# an action item, #13610) and R SUITE SUMMARY (a suite whose mutants ran to a summary).
mutants_records() {   # log
  awk -F'\t' '
    function flush() { if (cur != "" && mode == "T") printf "T\t%s\t%s\t%s\n", cur, summ, (detail == "" ? "-" : detail); cur = ""; mode = ""; detail = "" }
    /^(ok   |FAIL )[^ ]+ \(mutate --changed [0-9a-f]+\): / {
      flush(); cur = $0; sub(/^(ok   |FAIL )/, "", cur); sub(/ .*/, "", cur)
      summ = $0; sub(/^(ok   |FAIL )[^ ]+ \(mutate --changed [0-9a-f]+\): /, "", summ); sub(/ \([0-9]+ s\)$/, "", summ)
      mode = (summ ~ /^[0-9]+ mutants?, [0-9]+ killed, [0-9]+ survived/ || summ ~ /^nothing to mutate/) ? "S" : "T"
      if (mode == "S") printf "R\t%s\t%s\n", cur, summ
      next }
    /^FAIL [^ ]+: the compiler did not start$/ { flush(); s = $0; sub(/^FAIL /, "", s); sub(/: .*/, "", s); printf "T\t%s\tthe compiler did not start\t-\n", s; next }
    /^(ok   |FAIL |suites: |== )/ { flush(); next }
    mode == "S" && /: mutant survived: / { l = $0; sub(/^ +/, "", l); w = l; sub(/: mutant survived: .*/, "", l); sub(/.*: mutant survived: /, "", w); printf "S\t%s\t%s\t%s\t%s\n", cur, summ, l, w; next }
    mode == "S" && /^ *not reached by any check: / { l = $0; sub(/^ *not reached by any check: /, "", l); printf "U\t%s\t%s\t%s\n", cur, summ, l; next }
    mode == "S" && /^ *check-only: [^ ]+: [0-9]+ / { l = $0; sub(/^ *check-only: /, "", l); p = l; sub(/: .*/, "", p); c = l; sub(/^[^ ]+: /, "", c); sub(/ .*/, "", c); printf "C\t%s\t%s\t%s\t%s\n", cur, summ, p, c; next }
    mode == "T" && detail == "" && /(error\[E[0-9]+\]|^coax: |still running)/ { d = $0; sub(/^ +/, "", d); detail = substr(d, 1, 240) }
    END { flush() }' "$1"
}
mutants_report() {   # head base dir status...: the report (markdown); status 1 when it has no action item
  local head="$1" base="$2" dir="$3" i=0 s n_s=0 n_u=0 n_t=0 n_c=0 n_o=0 line alone="coax/remote/remote.sh run -- coax/remote/check.sh"; shift 3
  : > "$dir/records.tsv"
  for s in "$@"; do
    i=$((i + 1))
    if [[ "$s" == 97 ]]; then
      printf 'P\tpart %s/%s\tthe hosts could not finish it\t%s\n' "$i" "$mutate_parts" "$(grep -v '^ *$' "$dir/part-$i/out" 2> /dev/null | tail -1 | cut -c1-240)" >> "$dir/records.tsv"
    else
      mutants_records "$dir/part-$i/build/pipeline/mutate-since.log" >> "$dir/records.tsv"
    fi
  done
  echo "Mutants of what changed from ${base:0:10} to ${head:0:10} on \`$pl_main\` (\`check.sh mutate-since $base I/$mutate_parts\`). A report only: it doesn't hold \`$pl_green\`, go on the red list or start a revert."
  # Survivors: one line a procedure.
  local suite summ at what p line proc key keys=() owner
  while IFS=$'\t' read -r _ suite summ at what; do
    p="$(mutants_path "$(dirname "$suite")" "${at%%:*}")"; line="${at#*:}"; line="${line%%:*}"
    proc="$(mutants_proc "$head" "$p" "$line")"
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$p" "$proc" "$line" "$suite" "$summ" "$what"
  done < <(awk -F'\t' '$1 == "S"' "$dir/records.tsv") | sort -t$'\t' -k1,1 -k2,2 -k3,3n > "$dir/survivors.all"
  # A survivor in a file the change didn't touch is outside it: coax mutate --changed counts the
  # procedures #import_proto generates as changed every time (#13614). One line a file, not an item.
  : > "$dir/survivors.tsv"; : > "$dir/outside.tsv"
  while IFS= read -r line; do
    if mutants_touched "$head" "$base" "${line%%$'\t'*}"; then echo "$line" >> "$dir/survivors.tsv"; else echo "$line" >> "$dir/outside.tsv"; fi
  done < "$dir/survivors.all"
  if [[ -s "$dir/survivors.tsv" ]]; then
    echo; echo "Surviving mutants (a check that fails for each, or \`// mutant: equivalent\` on a line that can't change behaviour):"
    while IFS=$'\t' read -r p proc; do
      n_s=$((n_s + 1))
      local rows lo hi n sts
      rows="$(awk -F'\t' -v p="$p" -v q="$proc" '$1 == p && $2 == q' "$dir/survivors.tsv")"
      lo="$(cut -f3 <<< "$rows" | head -1)"; hi="$(cut -f3 <<< "$rows" | tail -1)"; n="$(wc -l <<< "$rows" | tr -d ' ')"
      sts="$(cut -f4 <<< "$rows" | sort -u | tr '\n' ' ' | sed 's/ $//')"
      owner="$(git log -1 --format=%H "$base..$head" -- "$p" 2> /dev/null)"
      echo "- \`$p\` \`$proc\`, line$([[ "$lo" == "$hi" ]] && echo " $lo" || echo "s $lo-$hi"): $n mutant(s) survived; the suite: $(cut -f4,5 <<< "$rows" | sort -u | awk -F'\t' '{ printf "%s%s (%s)", (NR > 1 ? ", " : ""), $1, $2 }')${owner:+; changed by ${owner:0:10} (agent \`$(pl_agent_of "$owner")\`)}. Run alone: \`$alone only ${sts// /,} $base\`"
      cut -f3,6 <<< "$rows" | head -n 12 | awk -F'\t' '{ print "  - line " $1 ": " $2 }'
    done < <(cut -f1,2 "$dir/survivors.tsv" | awk '!seen[$0]++')
  fi
  if awk -F'\t' '$1 == "U"' "$dir/records.tsv" | grep -q .; then
    echo; echo "Changed procedures no check reaches (call each from a #test or #forall):"
    while IFS=$'\t' read -r _ suite summ what; do
      n_u=$((n_u + 1)); echo "- \`$suite\`: $what. Run alone: \`$alone only $suite $base\`"
    done < <(awk -F'\t' '$1 == "U"' "$dir/records.tsv")
  fi
  if awk -F'\t' '$1 == "T" || $1 == "P"' "$dir/records.tsv" | grep -q .; then
    echo; echo "Tool problems: the mutation tool did not finish these, so their mutants are not judged (not the commit's failure, not flaky):"
    while IFS=$'\t' read -r k suite summ what; do
      n_t=$((n_t + 1)); [[ "$what" != - ]] || what=""
      if [[ "$k" == P ]]; then echo "- $suite: $summ${what:+ (\`$what\`)}. Run alone: \`$alone mutate-since $base ${suite#part }\`"
      else echo "- \`$suite\`: $summ${what:+: \`$what\`}. Run alone: \`$alone only $suite $base\`"; fi
    done < <(awk -F'\t' '$1 == "T" || $1 == "P"' "$dir/records.tsv")
  fi
  if [[ -s "$dir/outside.tsv" ]]; then
    echo; echo "Survivors outside the change (not action items: their files did not change from ${base:0:10} to ${head:0:10}; coax mutate --changed also mutates the procedures #import_proto generates, #13614):"
    while IFS=$'\t' read -r p n; do n_o=$((n_o + 1)); echo "- \`$p\`: $n mutant(s) survived"; done < <(awk -F'\t' '{ n[$1]++; if (!($1 in s)) { s[$1] = 1; o[++k] = $1 } } END { for (i = 1; i <= k; i++) printf "%s\t%d\n", o[i], n[o[i]] }' "$dir/outside.tsv")
  fi
  # Check units: survivors in procedures only checks reach, one line a file, never an action item.
  if awk -F'\t' '$1 == "C"' "$dir/records.tsv" | grep -q .; then
    echo; echo "Check-only helper code (not action items: these procedures are in files the suite loads only with \`#load ... #test;\`, so no compile ships them, and a mutant there that still lets every check pass is not a gap in what ships; Chapter 11):"
    while IFS=$'\t' read -r p n; do
      n_c=$((n_c + 1)); echo "- \`$p\`: $n mutant(s) survived"
    done < <(awk -F'\t' '$1 == "C" { n[$4] += $5; if (!($4 in n0)) { n0[$4] = 1; o[++k] = $4 } } END { for (i = 1; i <= k; i++) printf "%s\t%d\n", o[i], n[o[i]] }' "$dir/records.tsv")
  fi
  echo; echo "Each part's output: \`$dir/\` on the watcher's machine."
  echo "$n_s $n_u $n_t $n_c" > "$dir/counts"
  (( n_s + n_u + n_t > 0 ))
}

# The report's lifecycle (#13610): the action items still open live in OUTSTANDING, one row each:
# KIND FILE PROC LINE WHAT SUITE BASE HEAD (S a surviving mutant in shipped code; U a changed
# procedure no check reaches, FILE the suite and PROC the names; T a suite the tool could not
# finish; P a part the hosts could not finish, FILE and SUITE "part I/N"). Each report first
# rechecks the earlier heads' items: their suites' mutants again at this head from the item's
# base (check.sh mutate-only SUITES BASE), at most OA_AFTER_MUTATE_RECHECK (6) suites, and up to
# OA_AFTER_MUTATE_PARTS parts the hosts could not finish, again at this head (check.sh mutate-range). An item a recheck no longer finds is cleared; one it can't judge
# stays. The issue is open while OUTSTANDING has a row: the same steps as the nightly sweep's
# issue (mutation-sweep.coax, ms_issue_step): nothing outstanding closes an open issue with a
# summary; an item reopens a closed one, or files one when there is none; the issue is the number
# kept in mutants-issue, else the newest with the title in any state. One comment a head at most.
mutants_out="$results/mutants-outstanding.tsv"
mutants_touched() {   # head base path: whether the change from base to head touched the file
  ! git diff --quiet "$2" "$1" -- "$3" 2> /dev/null
}
mutants_items() {   # head base records: the action items in records, as OUTSTANDING rows
  local head="$1" base="$2" k suite summ at what p line
  while IFS=$'\t' read -r k suite summ at what; do
    case "$k" in
      S) p="$(mutants_path "$(dirname "$suite")" "${at%%:*}")"; line="${at#*:}"; line="${line%%:*}"
         mutants_touched "$head" "$base" "$p" || continue
         printf 'S\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$p" "$(mutants_proc "$head" "$p" "$line")" "$line" "$what" "$suite" "$base" "$head" ;;
      U) printf 'U\t%s\t%s\t-\t-\t%s\t%s\t%s\n' "$suite" "$at" "$suite" "$base" "$head" ;;
      T|P) printf '%s\t%s\t%s\t-\t%s\t%s\t%s\t%s\n' "$k" "$suite" "$summ" "${at:--}" "$suite" "$base" "$head" ;;
    esac
  done < "$3"
}
# An item's key: a survivor is its procedure's; the rest are their suite's (or part's) at a base and head.
mutants_key_awk='function key() { return ($1 == "S") ? ("S\t" $2 "\t" $3) : ($1 "\t" $6 "\t" $7 ($1 == "P" ? "\t" $8 : "")) }'
mutants_list() {   # rows: the items as markdown lines, one a procedure (or suite)
  awk -F'\t' -v alone="coax/remote/remote.sh run -- coax/remote/check.sh" '
    $1 == "S" { k = $2 "\t" $3; if (!(k in n)) { o[++m] = k; f[k] = $2; p[k] = $3; s[k] = $6; b[k] = $7; h[k] = $8; lo[k] = $4 }
                n[k]++; hi[k] = $4; if (n[k] <= 3) w[k] = w[k] (n[k] > 1 ? "; " : "") "line " $4 ": " $5; next }
    { o[++m] = "#" NR; r["#" NR] = $0 }
    END { for (i = 1; i <= m; i++) { k = o[i]
      if (k in r) { split(r[k], x, "\t")
        if (x[1] == "U") printf "- `%s`: not reached by any check: %s (since %s). Run alone: `%s only %s %s`\n", x[2], x[3], substr(x[7], 1, 10), alone, x[2], x[7]
        else if (x[1] == "T") printf "- `%s`: the tool did not finish its mutants: %s (since %s). Run alone: `%s only %s %s`\n", x[2], x[3], substr(x[7], 1, 10), alone, x[2], x[7]
        else printf "- %s at %s: %s (since %s). Run alone: `%s mutate-range %s %s %s`\n", x[2], substr(x[8], 1, 10), x[3], substr(x[7], 1, 10), alone, x[7], x[8], substr(x[2], 6)
      } else printf "- `%s` `%s`, line%s: %d mutant(s) survived (%s%s; since %s, at %s). Run alone: `%s only %s %s`\n", f[k], p[k], (lo[k] == hi[k] ? " " lo[k] : "s " lo[k] "-" hi[k]), n[k], w[k], (n[k] > 3 ? "; ..." : ""), substr(b[k], 1, 10), substr(h[k], 1, 10), alone, s[k], b[k] } }' "$@"
}
mutants_issue() {   # "NUMBER STATE" of the report's one issue, nothing when there is none; status 1 when GitHub can't say
  local n s
  n="$(cat "$results/mutants-issue" 2> /dev/null)"
  if [[ -n "$n" ]] && s="$(gh issue view "$n" --repo "$pl_repo" --json state --jq .state 2> /dev/null)" && [[ -n "$s" ]]; then echo "$n $s"; return 0; fi
  gh issue list --repo "$pl_repo" --state all --search "\"$mutants_title\" in:title" --json number,title,state \
    --jq "[.[] | select(.title == \"$mutants_title\")] | first // empty | \"\\(.number) \\(.state)\"" 2> /dev/null
}
mutants_recheck() {   # head dir: recheck the earlier heads' items; the rows still outstanding into DIR/kept.tsv, the cleared into DIR/cleared.tsv
  local head="$1" dir="$2" max="${OA_AFTER_MUTATE_RECHECK:-6}" k=0 b list pids=() st=() i j=0 ph pb pp rd rlog s
  [[ -f "$mutants_out" ]] || : > "$mutants_out"
  : > "$dir/judged.tsv"; : > "$dir/again.tsv"; : > "$dir/cleared.tsv"
  # Each base's suites, oldest item first, at most max suites in all.
  awk -F'\t' -v h="$head" -v max="$max" '$8 != h && ($1 == "S" || $1 == "U" || $1 == "T") && !(($7 "\t" $6) in seen) && n < max { seen[$7 "\t" $6] = 1; n++
      if (!($7 in l)) o[++m] = $7; l[$7] = l[$7] (l[$7] == "" ? "" : ",") $6 }
    END { for (i = 1; i <= m; i++) printf "%s\t%s\n", o[i], l[o[i]] }' "$mutants_out" > "$dir/plan.tsv"
  while IFS=$'\t' read -r b list <&3; do
    k=$((k + 1)); check "$head" "$dir/recheck-$k" mutate-only "$list" "$b" < /dev/null & pids+=($!)
  done 3< "$dir/plan.tsv"
  # And the oldest parts the hosts could not finish, at most OA_AFTER_MUTATE_PARTS of them, again:
  # the same part of the suites BASE..TOP touched, at this head, so the compiler and tools are this
  # head's (an earlier head's tools could be why the part never finished: #13614's swollen
  # mutants at 7a2d5b2d5b).
  awk -F'\t' -v h="$head" '$1 == "P" && $8 != h { print $2 "\t" $7 "\t" $8 }' "$mutants_out" | head -n "$mutate_parts" > "$dir/parts.tsv"
  while IFS=$'\t' read -r pp pb ph <&3; do
    j=$((j + 1)); check "$head" "$dir/recheck-part-$j" mutate-range "$pb" "$ph" "${pp#part }" < /dev/null & pids+=($!)
  done 3< "$dir/parts.tsv"
  for i in ${pids[@]+"${pids[@]}"}; do wait "$i"; st+=($?); done
  i=0
  while IFS=$'\t' read -r b list <&3; do
    i=$((i + 1)); rd="$dir/recheck-$i"; rlog="$rd/build/pipeline/mutate-only.log"
    # Judged: the hosts finished it, the suites ran to their end, and the tool finished the suite.
    if [[ "${st[$((i - 1))]}" != 97 ]] && grep -q '^suites: ' "$rlog" 2> /dev/null; then
      mutants_records "$rlog" > "$rd.tsv"
      tr ',' '\n' <<< "$list" | while IFS= read -r s; do
        awk -F'\t' -v s="$s" '$1 == "T" && $2 == s { f = 1 } END { exit !f }' "$rd.tsv" || printf '%s\t%s\n' "$s" "$b"
      done >> "$dir/judged.tsv"
      mutants_items "$head" "$b" "$rd.tsv" >> "$dir/again.tsv"
    fi
  done 3< "$dir/plan.tsv"
  # An item in a judged suite stays only when the recheck finds it again (its new rows replace it).
  awk -F'\t' -v OFS='\t' -v kept="$dir/kept.tsv" -v cleared="$dir/cleared.tsv" -v found="$dir/found.tsv" "$mutants_key_awk"'
    FILENAME == ARGV[1] { j[$1 "\t" $2] = 1; next }
    FILENAME == ARGV[2] { k = key(); if (!(k in again) && (($6 "\t" $7) in j)) o[++m] = k; again[k] = again[k] $0 "\n"; next }
    !(($6 "\t" $7) in j) || $1 == "P" { print > kept; next }
    { k = key(); if (k in again) { if (!(k in done)) { printf "%s", again[k] > kept; done[k] = 1 } } else print > cleared }
    # what the recheck found that no item had: survivors the report could not list before (it lists 39 a suite)
    END { for (i = 1; i <= m; i++) if (!(o[i] in done)) printf "%s", again[o[i]] > found }' \
    "$dir/judged.tsv" "$dir/again.tsv" "$mutants_out"
  [[ -f "$dir/kept.tsv" ]] || : > "$dir/kept.tsv"
  [[ -f "$dir/found.tsv" ]] || : > "$dir/found.tsv"
  # A part: cleared once the hosts finish it, and what it found joins the items.
  j=0
  while IFS=$'\t' read -r pp pb ph <&3; do
    j=$((j + 1)); [[ "${st[$((i + j - 1))]}" != 97 ]] || continue
    awk -F'\t' -v p="$pp" -v h="$ph" -v OFS='\t' -v kept="$dir/kept.tsv.new" -v cleared="$dir/cleared.part" '!($1 == "P" && $2 == p && $8 == h) { print > kept; next } { print > cleared }' "$dir/kept.tsv"
    [[ -f "$dir/kept.tsv.new" ]] || : > "$dir/kept.tsv.new"
    mv "$dir/kept.tsv.new" "$dir/kept.tsv"; cat "$dir/cleared.part" >> "$dir/cleared.tsv" 2> /dev/null; rm -f "$dir/cleared.part"
    mutants_records "$dir/recheck-part-$j/build/pipeline/mutate-range.log" > "$dir/recheck-part-$j.tsv"
    mutants_items "$ph" "$pb" "$dir/recheck-part-$j.tsv" >> "$dir/kept.tsv"
  done 3< "$dir/parts.tsv"
}
mutants_lifecycle() {   # head base dir: OUTSTANDING after this report, and the one issue to match it
  local head="$1" base="$2" dir="$3" found n state new=0 cleared=0 out=0 nc url
  # The first report with a lifecycle takes the earlier reports' items (#13610).
  if [[ ! -f "$mutants_out" ]]; then
    : > "$mutants_out.seed"
    for d in "$results"/*/mutants; do
      [[ "$d" != "$dir" && -f "$d/records.tsv" && -f "$d/base" ]] || continue
      read -r a b c _ < "$d/counts" 2> /dev/null || continue
      (( ${a:-0} + ${b:-0} + ${c:-0} > 0 )) || continue
      mutants_items "$(basename "$(dirname "$d")")" "$(cat "$d/base")" "$d/records.tsv" >> "$mutants_out.seed"
    done
    mv "$mutants_out.seed" "$mutants_out"
  fi
  mutants_recheck "$head" "$dir"
  { mutants_items "$head" "$base" "$dir/records.tsv"; cat "$dir/found.tsv"; } > "$dir/items.tsv"
  # This head's items replace the same keys.
  awk -F'\t' "$mutants_key_awk"' FILENAME == ARGV[1] { n[key()] = 1; next } !(key() in n)' "$dir/items.tsv" "$dir/kept.tsv" > "$mutants_out.new"
  cat "$dir/items.tsv" >> "$mutants_out.new"; mv "$mutants_out.new" "$mutants_out"
  new="$(grep -c . "$dir/items.tsv")"; cleared="$(grep -c . "$dir/cleared.tsv")"; out="$(grep -c . "$mutants_out")"
  read -r _ _ _ nc < "$dir/counts" 2> /dev/null || nc=0
  pl_say "after: mutation report at ${head:0:10}: $new new item(s), $cleared cleared, $out outstanding"
  # The comment: this head's report when it has items, then what the recheck cleared and what is still open.
  {
    if (( new > 0 )); then cat "$dir/report.md"
    else
      echo "Nothing to act on in the change from ${base:0:10} to ${head:0:10}."
      (( ${nc:-0} == 0 )) || { echo; awk '/^Check-only helper code/ { on = 1 } on && /^$/ { exit } on' "$dir/report.md"; }
    fi
    if [[ -s "$dir/found.tsv" ]]; then echo; echo "Found by the recheck of earlier items' suites at ${head:0:10} (survivors an earlier report could not list):"; mutants_list "$dir/found.tsv"; fi
    if (( cleared > 0 )); then echo; echo "Cleared by the recheck at ${head:0:10} (killed, marked, or the tool finished):"; mutants_list "$dir/cleared.tsv"; fi
    if awk -F'\t' -v h="$head" '$8 != h' "$mutants_out" | grep -q .; then
      echo; echo "Still outstanding from earlier heads:"; awk -F'\t' -v h="$head" '$8 != h' "$mutants_out" | mutants_list
    fi
  } > "$dir/comment.md"
  [[ "$pl_issues" == 1 ]] || return 0
  found="$(mutants_issue)" || { pl_say "after: GitHub could not say where the mutation report's issue is; it follows at the next report"; return 0; }
  n="${found%% *}"; state="${found#* }"; [[ -n "$found" ]] || { n=""; state=""; }
  [[ -z "$n" ]] || echo "$n" > "$results/mutants-issue"
  if (( out == 0 )); then
    [[ "$state" == OPEN ]] || return 0
    gh issue close "$n" --repo "$pl_repo" --comment "**${head:0:10}**: nothing outstanding: every survivor in shipped code and every tool problem this issue listed is killed, marked equivalent or no longer reproduces. $(head -c 50000 "$dir/comment.md")

It reopens when a later report has something to act on." > /dev/null 2>&1 && pl_say "after: the mutation report's issue #$n is closed: nothing outstanding" || pl_say "could not close #$n"
  elif [[ "$state" == CLOSED ]]; then
    gh issue reopen "$n" --repo "$pl_repo" --comment "**${head:0:10}**: $(head -c 60000 "$dir/comment.md")" > /dev/null 2>&1 || pl_say "could not reopen #$n"
  elif [[ "$state" == OPEN ]]; then
    (( new + cleared == 0 )) || pl_issue_comment "$n" "**${head:0:10}**: $(head -c 60000 "$dir/comment.md")"
  else
    url="$(gh issue create --repo "$pl_repo" --title "$mutants_title" --body "The after-landing mutation report (\`coax/remote/after.sh\`, docs/metaverse/green-main-pipeline.md): each fast run's change, mutated. It never holds \`$pl_green\` or reverts anything; each comment is one head's report. It closes itself when nothing is outstanding.

**${head:0:10}**: $(head -c 60000 "$dir/comment.md")" 2> /dev/null)" && echo "${url##*/}" > "$results/mutants-issue" || pl_say "could not open the mutation report's issue"
  fi
}
mutants_of() {   # head base: the head's report, in the background
  [[ "${OA_AFTER_MUTATE:-1}" != 0 ]] || return 0
  local head="$1" base="$2" last=""
  [[ ! -d "$results/$head/mutants" ]] || return 0
  [[ ! -f "$results/mutants-head" ]] || last="$(cat "$results/mutants-head")"
  if [[ -n "$last" && "$last" != "$head" ]] && git merge-base --is-ancestor "$base" "$last" 2> /dev/null &&
     git merge-base --is-ancestor "$last" "$head" 2> /dev/null; then base="$last"; fi
  git diff --name-only "$base" "$head" | grep -qvE '(\.md$|^docs/)' || return 0
  (
    flock -n 6 || { pl_say "after: a mutation report is running; ${head:0:10}'s change goes in the next one"; exit 0; }
    dir="$results/$head/mutants"; mkdir -p "$dir"; echo "$base" > "$dir/base"
    limit="$mutate_limit"
    pids=() st=()
    for (( i = 1; i <= mutate_parts; i++ )); do check "$head" "$dir/part-$i" mutate-since "$base" "$i/$mutate_parts" & pids+=($!); done
    for p in "${pids[@]}"; do wait "$p"; st+=($?); done
    echo "$head" > "$results/mutants-head"
    if mutants_report "$head" "$base" "$dir" "${st[@]}" > "$dir/report.md"; then
      read -r ns nu nt nc < "$dir/counts"
      pl_say "after: mutation report at ${head:0:10}: $ns procedure(s) with survivors, $nu unreached, $nt tool problem(s), $nc check-only file(s) ($dir/report.md)"
    else
      pl_say "after: mutation report at ${head:0:10}: no survivor in shipped code"
    fi
    mutants_lifecycle "$head" "$base" "$dir"
  ) 6> "$results/mutants.lock" 8>&- 7>&- &
  mutants_pid=$!
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
# The hosts could not finish H's run: what did not finish runs again at once
# (the jobs that passed are kept), OA_AFTER_UNFINISHED (3) times. Then it is
# reported, and H waits half an hour or for a new head.
unfinished() {   # head kind started jobs why
  local dir="$results/$1" n cost off title
  # remote.sh's own line when the pool is off (#13472): it names the cause
  # (the cap, or no host could start) and the fix, so the issue says that.
  off="$(cat "$dir"/*/out "$dir"/*.out "$dir"/bisect-*/*/out 2> /dev/null | sed -n 's/^remote: \(the pool is off: .*\)/\1/p' | tail -1)"
  [[ -z "$off" ]] || set -- "$1" "$2" "$3" "$4" "${off%%. Fix:*}"
  n=$(( $(cat "$dir/tries" 2> /dev/null || echo 0) + 1 )); echo "$n" > "$dir/tries"
  cost="$(record_run "$1" "$2" unfinished "$3" "$4")"
  if (( n < max_unfinished )); then
    rm -f "$results/last-$2"
    pl_say "after: $5 on ${1:0:10}; what did not finish runs again (try $n of $max_unfinished; $cost)"
    return 0
  fi
  echo unfinished > "$dir/status"; date +%s > "$results/last-unfinished"
  pl_say "after: $5 on ${1:0:10}, $n times; it is tried again in half an hour or on a new head ($cost)"
  title="the hosts could not finish the check of ${1:0:10}"
  [[ -z "$off" ]] || title="${5%%: the hosts cost*}: no check of ${1:0:10} could run"
  pl_alarm unfinished "$title" "$5 on ${1:0:10}, $n times in a row, so \`$pl_green\` has not moved. The pipeline tries this head again in half an hour, and a new head at once.
${off:+
remote.sh: $off
}
Each job's status and seconds (97: the hosts could not run it):

\`\`\`
$(tail -n 30 "$dir/run.tsv" 2> /dev/null)
\`\`\`

The hosts: \`coax/remote/remote.sh status\`. Each job's output: \`$dir/\` on the coordinator's Mac."
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
    pl_alarm_clear "\`$pl_green\` moved to ${head:0:10} after a green $kind run."
    pages_of "$head"
    fuzz_of "$head"
    soak_of "$head"
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
  # A head the hosts could not finish is tried again half an hour on.
  if [[ "$(cat "$dir/status" 2> /dev/null)" == unfinished ]] && due last-unfinished 1800; then rm -f "$dir/status"; fi
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
  elif [[ -n "$ref" && "$head" != "$ref" && ! -f "$dir/status" ]] && due last-fast "$fast_gap"; then
    kind=fast
    # A changed compiler can change any check, so that head gets a full run.
    if [[ ! -f "$dir/full" ]] && git diff --name-only "$ref" "$head" | grep -Eq '^coax/(src|bootstrap)/|^coax/(build|provenance)\.sh$|^coax/web/native-check\.c$'; then kind=full; fi
  fi
  if [[ -z "$kind" ]]; then
    # main has not moved since a red run: its failures are still there.
    [[ "$(cat "$dir/status" 2> /dev/null)" != red ]] || still_red "$dir/failed" "$(date +%s)"
    return 0
  fi
  date +%s > "$results/last-$kind"
  # An unfinished run on this head left the jobs that passed (passed): they
  # don't run again.
  if [[ -f "$dir/status" ]]; then rm -f "$dir/passed" "$dir/tries"; elif [[ ! -f "$dir/tries" ]]; then rm -rf "$dir"; fi
  mkdir -p "$dir"; rm -f "$dir/failed"; echo "$kind" > "$dir/kind"
  local started; started=$(date +%s)
  # What a failure is bisected against: what the run compares against, or for
  # a full run the last head a full run passed (the fast runs since may have
  # missed it).
  local base="$ref"
  if [[ "$kind" == full ]]; then base="$green"; known=""; [[ -z "$full_green" ]] || base="$full_green"; fi
  # The jobs.
  local jobs=() i changed="" prod
  # Production's commit, for Everglade II's comparison ("" leaves it out).
  prod="$(production)"
  if [[ "$kind" == fast ]]; then
    changed="$(git diff --name-only "$ref" "$head")"
    if [[ -n "$changed" ]] && grep -qvE '(\.md$|^docs/)' <<< "$changed"; then
      for (( i = 1; i <= fast_parts; i++ )); do jobs+=("since $ref $i/$fast_parts"); done
    fi
    [[ -z "$changed" ]] || ! grep -qv '\.md$' <<< "$changed" || jobs+=("web $ref" "room-since $ref${prod:+ $prod}" "coder-since $ref")
    # The API suite's parts only when the change reaches it (check.sh's api_inputs; it checks again).
    if grep -v '\.md$' <<< "$changed" | grep -Eq '^(coax/web/api/|coax/web/agent/|coax/lib/|coax/platform/|coax/src/|coax/bootstrap/|apps/openagents-web/docs/limits/)'; then
      for (( i = 1; i <= api_parts; i++ )); do jobs+=("api-since $ref $i/$api_parts"); done
    fi
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
    jobs+=("tools${prod:+ $prod}" "coder")
    for (( i = 1; i <= api_parts; i++ )); do jobs+=("api $i/$api_parts"); done
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
    names+=("$j")
    if grep -qxF "$j" "$dir/passed" 2> /dev/null; then pids+=(0); continue; fi
    # shellcheck disable=SC2086
    check "$head" "$dir/${j//[ \/]/-}" $j & pids+=($!)
  done
  [[ ! -s "$dir/passed" ]] || pl_say "after: $(wc -l < "$dir/passed" | tr -d ' ') of the job(s) passed on ${head:0:10} before and don't run again"
  local st=() k failed=() infra=0
  for k in "${!pids[@]}"; do if [[ "${pids[$k]}" == 0 ]]; then st+=(0); else wait "${pids[$k]}"; st+=($?); fi; done
  for k in "${!names[@]}"; do [[ "${st[$k]}" != 0 ]] || echo "${names[$k]}"; done > "$dir/passed"
  # Then the change's mutants, a report and never part of this run's verdict: they start once
  # its jobs are done, so they never take a slot its jobs wait for.
  [[ "$kind" != fast || -z "$changed" ]] || mutants_of "$head" "$ref"
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
  # The allowlist counts (#13266), here with git alone: against main-green (with none, what the
  # run compares against). A raised count is a failure whose culprit is already known.
  local cbase="${green:-$ref}"
  : > "$dir/count-culprits"
  cn_show "$head" | awk -v c="${head:0:10}" -v t="$now" '{ printf "%s\t%s\tcount\t%s\t%s\t-\n", c, t, $1, $2 }' >> "$history"
  if [[ -n "$cbase" && "$cbase" != "$head" ]]; then
    cn_check "$cbase" "$head" > "$dir/count-culprits" 2> /dev/null
    while read -r _ x c _; do
      [[ -n "$x" ]] || continue
      failed+=("count $x|counts")
    done < "$dir/count-culprits"
  fi
  # Entries this run shows fixed: their check ran and passed. A fast run runs
  # every check an entry names, since the entry's culprit changed its inputs
  # after main-green; the compiler's units run only when its sources changed.
  local open id chk issue
  open="$(jq -r '.[] | select(.status == "open") | "\(.id)|\(.check)|\(.issue // "")"' "$pl_red")"
  : > "$dir/fixed.tsv"
  while IFS='|' read -r id chk issue; do
    [[ -n "$id" ]] && (( infra == 0 )) || continue   # an unfinished job may be its check
    [[ "$chk" != compiler\ * ]] || (( compiler == 1 )) || { [[ -n "$known" ]] && grep -qxF "$chk" "$known"; } || continue
    # Everglade II's entries are judged only by a run that compared with production.
    [[ "$chk" != e2\ * ]] || [[ -n "$prod" ]] || continue
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
    unfinished "$head" "$kind" "$started" "${#jobs[@]}" "the hosts could not run every job"
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
        body="The after-landing check (\`coax/remote/after.sh\`, docs/metaverse/green-main-pipeline.md) found ${#new[@]} failing check(s) on \`$pl_main\` at ${head:0:10} with no passing head before it to bisect against, so there is no culprit. \`$pl_green\` is set at the first head where every check passes.

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
  local bpids=() bitems=() bjobs=() n=0 job args rest=() owned=0 flaky_n=0 found=0 infra_b=0
  for x in ${failed[@]+"${failed[@]}"}; do
    item="${x%%|*}"; job="${x#*|}"
    if jq -e --arg c "$item" '.[] | select(.status == "open" and .check == $c)' "$pl_red" > /dev/null; then
      pl_say "after: $item is already on the red list"; owned=1; continue
    fi
    if [[ "$item" == count\ * ]]; then
      # A raised count: its culprit is the commit that raised it (counts.sh), no bisect.
      awk -v x="${item#count }" '$2 == x { print $3; exit }' "$dir/count-culprits" > "$dir/bisect-$n.culprit"
      bpids+=(0); bitems+=("$item"); bjobs+=("$job"); n=$((n + 1))
    elif (( n < bisect_checks )); then
      args="$(args_for "$item" "$job")"
      # shellcheck disable=SC2086
      ( export OA_REMOTE_AVOID; OA_REMOTE_AVOID="$(host_of "$dir/${job//[ \/]/-}/out")"
        c="$(confirm "$dir/bisect-$n" "$head" "$item" $args)"
        # Worse than production at main-green too: it was already there (it shipped, or will), so
        # it is reported, not red, and no commit of this run is blamed for it.
        if [[ "$c" == red && "$item" == e2\ * ]]; then
          check "$base" "$dir/bisect-$n/base-${base:0:10}" $args
          case $? in 1) c=prior ;; 97) c=infra ;; esac
        fi
        [[ "$c" != red ]] || c="$(bisect "$dir/bisect-$n" "$base" "$head" $args)"
        # A culprit before the head failed once, on one host: it runs again on
        # another before anyone is blamed (the head already failed twice, the
        # second time elsewhere). Passing there, it is not filed (#13256).
        if [[ "$c" =~ ^[0-9a-f]{40}$ && "$c" != "$head" ]]; then
          OA_REMOTE_AVOID="$(host_of "$(ls -td "$dir/bisect-$n"/r*-"${c:0:10}" 2> /dev/null | head -1)/out")"
          check "$c" "$dir/bisect-$n/v-${c:0:10}" $args
          case $? in 0) c="unconfirmed ${c}" ;; 97) c=infra ;; esac
        fi
        echo "$c" > "$dir/bisect-$n.culprit" ) & bpids+=($!); bitems+=("$item"); bjobs+=("$job"); n=$((n + 1))
    else rest+=("$item"); fi
  done
  (( ${#rest[@]} == 0 )) || pl_say "after: left for the next run (OA_BISECT_CHECKS=$bisect_checks): ${rest[*]}"
  local counted=0 culprits=()
  for k in "${!bpids[@]}"; do
    [[ "${bpids[$k]}" == 0 ]] || wait "${bpids[$k]}"
    item="${bitems[$k]}"
    local culprit; culprit="$(cat "$dir/bisect-$k.culprit")"
    case "$culprit" in
      flaky) pl_say "after: $item failed at ${head:0:10} and passed when run again: flaky"
             flaky_add "$head" "$item"; flaky_n=$((flaky_n + 1)); continue ;;
      prior) pl_say "after: $item: worse than production at ${base:0:10} too, so no commit of this run caused it; reported, not red"
             printf '%s\t%s\t%s\t%s\n' "$(date '+%Y-%m-%d %H:%M')" "${head:0:10}" "${base:0:10}" "$item" >> "$results/e2-reports.tsv"
             flaky_n=$((flaky_n + 1)); continue ;;
      infra) pl_say "after: the hosts could not finish the bisect of $item"; infra_b=$((infra_b + 1)); continue ;;
      "") pl_say "after: the bisect of $item found no culprit after ${base:0:10}"
          continue ;;
      unconfirmed\ *) pl_say "after: the bisect of $item named ${culprit#unconfirmed }, but it passed when run again on another host; nothing filed"
          continue ;;
    esac
    local agent subject out
    agent="$(pl_agent_of "$culprit")"; [[ -n "$agent" ]] || agent="unknown"
    subject="$(git log -1 --format=%s "$culprit")"
    # The culprit's own failing run: a bisect round's, or, when the culprit is
    # the head, the run again (c*) or the job itself. Before #13256 a culprit
    # at the head had no round of its own, and its issue showed empty output.
    out="$(ls -td "$dir/bisect-$k"/r*-"${culprit:0:10}" "$dir/bisect-$k"/c*-"${culprit:0:10}" 2> /dev/null | head -1)"
    if [[ -n "$out" ]]; then out="$out/out"; else out="$dir/${bjobs[$k]//[ \/]/-}/out"; fi
    issue="$(file_issue "$item" "$culprit" "$agent" "$out")"
    pl_red_update '. + [{id: $id, check: $c, culprit: $s, subject: $sub, agent: $a, issue: (if $i == "" then null else $i end), opened: ($t | tonumber), seen: ($t | tonumber), status: "open", revert: ""}]' \
      --arg id "${culprit:0:10}:$item" --arg c "$item" --arg s "$culprit" --arg sub "$subject" --arg a "$agent" --arg i "$issue" --arg t "$(date +%s)"
    pl_message "$agent" "Your commit ${culprit:0:10} (\"$subject\") broke \`$item\` on $pl_main.${issue:+ Issue: https://github.com/$pl_repo/issues/$issue.}
Output: $out
Run it alone: coax/remote/remote.sh run -- coax/remote/check.sh $(args_for "$item" "$job")
Fixing it comes before new work. If it still fails 30 minutes from now, the integrator reverts the commit."
    # The issues the culprit's subject names hear of it too, before any revert.
    local r
    for r in $(grep -oE '#[0-9]+' <<< "$subject" | tr -d '#' | sort -u); do
      [[ "$r" == "$issue" ]] || pl_issue_comment "$r" "The after-landing check \`$item\` fails on \`$pl_main\` since ${culprit:0:10} (\"$subject\"), by agent \`$agent\`${issue:+: #$issue}. If it still fails 30 minutes from now, the integrator queues a revert of ${culprit:0:10}; a fix on \`$pl_main\` before then means nothing is reverted. Run the check alone with \`coax/remote/remote.sh run -- $([[ "$(args_for "$item" "$job")" == visuals ]] && echo "coax/web/check-visuals.sh apps/openagents-web" || echo "coax/remote/check.sh $(args_for "$item" "$job")")\`."
    done
    pl_say "after: $item: culprit ${culprit:0:10} by $agent${issue:+ (#$issue)}"
    # A count's baseline is main-green, not the last full green run: it never moves main-green back.
    if [[ "$item" == count\ * ]]; then counted=$((counted + 1)); else found=$((found + 1)); culprits+=("$culprit"); fi
  done
  # Every failure passed when run again on H: H is green.
  if (( infra == 0 && infra_b == 0 && owned == 0 && ${#rest[@]} == 0 && flaky_n == ${#failed[@]} )); then
    go_green "$head" "$green" "$kind" "$started" "${#jobs[@]}" ", on a second run of $flaky_n flaky check(s)"
    return 0
  fi
  # A full run found what the fast runs passed: main-green goes back to just
  # before the oldest culprit it holds, never further. Until #13256 it went
  # back to the last head a full run passed: on 2026-11-11 twice to 38daeb78d0,
  # 3 and 6 hours back, though each culprit was hours newer (and the first
  # three were a host fault).
  if (( found > 0 )) && [[ -n "$green" && "$green" != "$base" ]]; then
    local back="" c2
    for c2 in "${culprits[@]}"; do
      git merge-base --is-ancestor "$c2" "$green" 2> /dev/null || continue
      [[ -z "$back" ]] || git merge-base --is-ancestor "$c2" "$back" 2> /dev/null || continue
      back="$c2"
    done
    if [[ -n "$back" ]] && back="$(git rev-parse -q --verify "$back^" 2> /dev/null)" &&
       git push -q --force "$pl_remote" "$back:refs/heads/$pl_green" 2> /dev/null; then
      pl_say "after: a full run found a culprit in $pl_green's history; $pl_green is back at ${back:0:10}, before it"
    elif [[ -z "$back" ]]; then
      pl_say "after: a full run's culprits are all after $pl_green ${green:0:10}; it stays"
    fi
  fi
  # A job or a bisect the hosts could not finish: the head is not red yet, and
  # main-green must not wait for a new head. What did not finish runs again.
  if (( infra == 1 || infra_b > 0 )); then
    unfinished "$head" "$kind" "$started" "${#jobs[@]}" "the hosts could not finish every job and bisect"
    return 0
  fi
  echo red > "$dir/status"; [[ "$kind" != full ]] || { : > "$dir/full"; echo "$head" > "$results/full-head"; }
  pl_say "after: $kind run and bisects on ${head:0:10}: $(record_run "$head" "$kind" red "$started" "${#jobs[@]}")"
}
# main-green far behind main is reported, whatever the reason: releases ship it.
# Far is OA_AFTER_STALE seconds (5400: 90 minutes; 3 hours before #13256) since
# the oldest commit after it landed, or OA_AFTER_STALE_COMMITS (40) commits.
# The report is the log, the ALARM file (the first lines of after.sh state) and
# a comment on the one "Green-main pipeline needs attention" issue.
stale_check() {
  local head green n age
  head="$(git rev-parse -q --verify "$main_ref" 2> /dev/null || true)"
  green="$(git rev-parse -q --verify "$green_ref" 2> /dev/null || true)"
  read -r n age <<< "$(behind "$head" "$green")"
  (( age >= stale || n >= stale_commits )) || return 0
  pl_alarm stale "$pl_green is $n commit(s) and $(( age / 3600 )) h $(( age % 3600 / 60 )) min behind $pl_main" "\`$pl_green\` is ${green:0:10}, and the oldest of the $n commit(s) after it landed $(( age / 60 )) minutes ago (the alarm: $(( stale / 60 )) minutes or $stale_commits commits). Releases ship \`$pl_green\`, so they are that far behind.

\`\`\`
$("$here/after.sh" state 2>&1 | head -n 40)
\`\`\`"
}

once() {
  exec 8> "$results/after.lock"
  flock -n 8 || { pl_say "after: a run is going"; return 0; }
  run_head
  exec 8>&-
}
# The other machine whose watcher published the shared state after this one
# last did (status 1: none).
standin_superseded() {
  [[ "$pl_shared" != 0 ]] || return 1
  local t host at
  t="$(mktemp -d "${TMPDIR:-/tmp}/oa-pl-standin.XXXXXXXX")" || return 1
  pl_gcloud storage cp "$pl_shared/state.tar.gz" "$t/state.tar.gz" > /dev/null 2>&1 &&
    read -r host at < <(tar -xzOf "$t/state.tar.gz" watcher 2> /dev/null)
  rm -rf "$t"
  [[ -n "${host:-}" && "$host" != "$(pl_host)" && "${at:-0}" =~ ^[0-9]+$ ]] || return 1
  (( at > $(cat "$results/published" 2> /dev/null || echo 0) )) || return 1
  echo "$host"
}
# The stand-in stops for good: as `after.sh uninstall`, and the watcher with it.
standin_stop() {   # other-host
  pl_say "after: the watcher on $1 is publishing again; this stand-in stops and uninstalls itself"
  rm -f "$results/heartbeat" "$results/published"
  if [[ -n "${OA_AFTER_SUPERVISED:-}" && "$(uname -s)" == Linux ]]; then
    rm -f "$HOME/.config/systemd/user/$label.service"
    systemctl --user disable --now "$label.service" > /dev/null 2>&1
  elif [[ -n "${OA_AFTER_SUPERVISED:-}" ]]; then
    rm -f "$HOME/Library/LaunchAgents/$label.plist"
    launchctl bootout "gui/$(id -u)/$label" > /dev/null 2>&1
  fi
  kill "$$" 2> /dev/null
}
if (( watch == 1 )); then
  exec 7> "$results/watch.lock"
  flock -n 7 || { pl_say "after: another watcher is running ($results/watch.lock)"; exit 75; }
  pl_say "after: watching $pl_remote/$pl_main (full runs every $gap s, reverts $([[ $revert == 1 ]] && echo on || echo off))"
  # A gap in the heartbeat is a time when nothing was checked: say so.
  if [[ -f "$results/heartbeat" ]]; then
    was="$(cat "$results/heartbeat")"; down=$(( $(date +%s) - was ))
    (( down < 900 )) || pl_alarm "down-$was" "the watcher was not running for $(( down / 60 )) minutes" "No after-landing check ran between $(pl_date "$was" '%Y-%m-%d %H:%M') and $(date '+%Y-%m-%d %H:%M %Z'), so \`$pl_green\` did not move. The watcher is running again and checks the newest head now."
  fi
  # This machine is the watcher now; other machines read its state (pl_publish).
  rm -f "$results/watcher-host"
  # Beside it, the service alerts passed on to the agents (pipeline-lib.sh pl_service_watch, #13297).
  # A stand-in (OA_AFTER_STANDIN=1, #13723) steps aside when another machine's
  # watcher publishes the shared state after it did: that watcher is back, and
  # two would race on main-green.
  date +%s > "$results/published"
  ( while kill -0 $$ 2> /dev/null; do
      date +%s > "$results/heartbeat"; pl_service_watch
      if [[ "${OA_AFTER_STANDIN:-0}" == 1 ]] && other="$(standin_superseded)"; then standin_stop "$other"; break; fi
      pl_publish; date +%s > "$results/published"; sleep 60
    done ) 7>&- &
  while true; do
    once
    stale_check
    # main-green's lag, every 5 minutes (state's week of it), and the flaky list's closed issues.
    if due last-lag 300; then
      date +%s > "$results/last-lag"
      printf '%s\t%s\n' "$(date +%s)" "$(lag_now "$(git rev-parse -q --verify "$main_ref")" "$(git rev-parse -q --verify "$green_ref" 2> /dev/null)")" >> "$results/lag.tsv"
    fi
    flaky_prune
    # Under launchd (after.sh install): new pipeline scripts on main are taken
    # by stopping here; launchd starts the watcher again on them.
    if [[ -n "${OA_AFTER_SUPERVISED:-}" ]] && ! git diff --quiet HEAD "$main_ref" -- coax/remote 2> /dev/null; then
      pl_say "after: the pipeline's scripts changed on $pl_main; starting again on them"
      exit 0
    fi
    # The 30-minute revert, and any branch agents submitted: one integrator
    # round, beside the next run (it holds its own lock).
    if [[ "$revert" == 1 ]]; then
      ( OA_INTEGRATE_QUIET=1 "$here/integrate.sh" --once --revert >> "$pl_dir/logs/integrate.out" 2>&1 7>&- & )
    fi
    sleep "${OA_AFTER_POLL:-30}"
  done
else
  once
  # One run: its mutation report too, before it ends.
  [[ -z "$mutants_pid" ]] || wait "$mutants_pid"
fi

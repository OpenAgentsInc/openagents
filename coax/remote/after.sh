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
#   coax/remote/after.sh install    run the watcher under launchd (macOS) or as a
#                                   systemd user service (Linux): it starts at login
#                                   or boot, starts again whenever it stops, and takes
#                                   new pipeline scripts from main (uninstall: stop it)
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
# change reaches it (check.sh api-since G I/N), and coax/tests/run.sh in 8
# parts when compiler sources changed. Everything else
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
# (check.sh api I/N), and coax/tests/run.sh in 8 parts when compiler
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
# main-green is OA_AFTER_STALE (10800) seconds behind main, or the hosts could
# not finish a head.
#
# Results: OA_PIPELINE_DIR/releases/after/COMMIT/ (each job's output and
# report, run.tsv with each job's status and seconds, kind and status),
# history.tsv (commit, time, kind, name, value, limit), runs.tsv (time, commit,
# kind, status, seconds, slot-seconds, jobs) and red.json.
#
# Settings: OA_AFTER_PARTS (6), OA_AFTER_MODULE_PARTS (3), OA_AFTER_FAST_PARTS
# (3), OA_AFTER_API_PARTS (2), OA_AFTER_GAP (10800: seconds between full
# runs), OA_AFTER_FAST_GAP (0: seconds between fast runs), OA_AFTER_COMPILER (auto, always or never),
# OA_BISECT_WIDTH (4), OA_BISECT_CHECKS (4: failing checks bisected one by
# one; the rest wait for the next run), OA_AFTER_TIMEOUT (3000 s a job),
# OA_AFTER_RETRIES (2), OA_AFTER_UNFINISHED (3), OA_AFTER_STALE (10800),
# OA_AFTER_FLAKY_TRIES (3), OA_AFTER_PAGES (0: no pages), OA_AFTER_PAGES_GAP
# (3600), OA_AFTER_CHECK (a command run as CMD COMMIT OUTDIR CHECK-ARGS... in
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
api_parts="${OA_AFTER_API_PARTS:-2}"   # coax/web/api/test.sh --part: see docs/verse/remote-builds.md, "The API suite"
gap="${OA_AFTER_GAP:-10800}"   # a full run costs about $0.30 of host time (2026-11-09)
fast_gap="${OA_AFTER_FAST_GAP:-0}"
retries="${OA_AFTER_RETRIES:-2}"
pages_gap="${OA_AFTER_PAGES_GAP:-3600}"
compiler_mode="${OA_AFTER_COMPILER:-auto}"
width="${OA_BISECT_WIDTH:-4}"
bisect_checks="${OA_BISECT_CHECKS:-4}"
limit="${OA_AFTER_TIMEOUT:-3000}"
max_unfinished="${OA_AFTER_UNFINISHED:-3}"
stale="${OA_AFTER_STALE:-10800}"
results="$pl_dir/releases/after"
history="$results/history.tsv"
label="${OA_AFTER_LABEL:-com.openagents.green-main}"
# Whether a watcher is alive, from the time it writes each minute (heartbeat).
# On a machine that isn't the watcher, the watcher's shared state first
# (pipeline-lib.sh's pl_mirror): the watcher elsewhere, and its host.
watcher_health() {
  local at=0 age where=""
  ! pl_mirror || where=" on $(cat "$results/watcher-host")"
  [[ ! -f "$results/heartbeat" ]] || at="$(cat "$results/heartbeat")"
  age=$(( $(date +%s) - at ))
  if (( age < 300 )); then echo "watcher     running$where, seen $age s ago"; return 0; fi
  # A watcher on another machine that went quiet can't say so itself: the
  # machine that sees it does (the log, ALARM and the one GitHub issue).
  [[ -z "$where" ]] || (( age < 900 )) || pl_alarm "quiet-$at" "the watcher$where has not been seen for $(( age / 60 )) minutes" "The green-main watcher$where last wrote its heartbeat at $(pl_date "$at" '%Y-%m-%d %H:%M %Z'), so no head is checked and \`$pl_green\` does not move. Check it there: \`systemctl --user status com.openagents.green-main\` and \`~/.openagents/pipeline/logs/after.out\` (docs/verse/green-main-pipeline.md)." > /dev/null 2>&1
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
    newest="$(${cfg[@]+"${cfg[@]}"} gcloud storage ls "gs://$prod_bucket/releases/" 2> /dev/null | grep -E '/releases/[0-9]{8}T[0-9]{6}Z/$' | sort | tail -1)"
    c="$([[ -z "$newest" ]] || ${cfg[@]+"${cfg[@]}"} gcloud storage cat "${newest}PASSED" 2> /dev/null | awk 'NR == 1 { print $1 }')"
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
    url="$(gh issue create --repo "$pl_repo" --title "Flaky check: $key" --body "The after-landing check \`$2\` failed at ${1:0:10} and passed when run again on the same commit, so it has no culprit (docs/verse/green-main-pipeline.md, \"Flaky checks\").

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
    else echo "$pl_green  ${g:0:10} $(git log -1 --format='%cd' --date=format-local:'%Y-%m-%d %H:%M' "$g"), $(git rev-list --count "$g..$h" 2> /dev/null) commit(s) behind $pl_main, lag $(( $(lag_now "$h" "$g") / 60 )) min"; fi
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
Description=Green-main pipeline watcher (coax/remote/after.sh --watch, docs/verse/green-main-pipeline.md)
After=network-online.target

[Service]
Type=simple
Environment="PATH=$(sd "$PATH")" "HOME=$(sd "$HOME")" OA_AFTER_SUPERVISED=1 "OA_PIPELINE_DIR=$(sd "$pl_dir")"$(for v in OA_REMOTE_STATE OA_REMOTE_DAILY_USD CLOUDSDK_CONFIG OA_PIPELINE_SHARED OA_PIPELINE_NOTIFY; do [[ -z "${!v:-}" ]] || printf ' "%s=%s"' "$v" "$(sd "${!v}")"; done)
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
      # check-visuals runs as itself, so a bisect can run it on commits whose
      # check.sh has no such step.
      local cmd=(coax/remote/check.sh "$@")
      [[ "$1" != visuals ]] || cmd=(coax/web/check-visuals.sh apps/openagents-web)
      # Without the lock descriptors: nothing a job leaves behind holds the watcher's locks.
      OA_REMOTE_TIMEOUT=$(( limit - 60 )) timeout "$limit" "$here/remote.sh" run --commit "$commit" --out "$dir" --no-fallback -- "${cmd[@]}" > "$dir/out" 2>&1 7>&- 8>&-; s=$?
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
    step\ tools-check|step\ after-gates|step\ everglade-parity|step\ wire-check|step\ layer-check|step\ wgsl|step\ realm-pages|step\ changelog-page|step\ realm-cards) echo "reports" ;;
    step\ room-build) echo "room" ;;
    step\ check-visuals) echo "visuals" ;;
    e2\ *) echo "e2 $(cat "$results/production" 2> /dev/null)" ;;
    *) echo "$*" ;;   # the whole job again
  esac
}

# A failing check run again on the head: "flaky" when it passes, "red" when
# it fails again, "infra" when the hosts could not run it. A check that was
# flaky before (flaky.txt) gets OA_AFTER_FLAKY_TRIES (3) runs, and one pass is
# enough.
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
  ( "$here/pages.sh" build "$1" > "$results/$1/pages.log" 2>&1 && pl_say "after: ${1:0:10}'s pages are in the shared store" ) 8>&- 7>&- &
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
  local dir="$results/$1" n cost
  n=$(( $(cat "$dir/tries" 2> /dev/null || echo 0) + 1 )); echo "$n" > "$dir/tries"
  cost="$(record_run "$1" "$2" unfinished "$3" "$4")"
  if (( n < max_unfinished )); then
    rm -f "$results/last-$2"
    pl_say "after: $5 on ${1:0:10}; what did not finish runs again (try $n of $max_unfinished; $cost)"
    return 0
  fi
  echo unfinished > "$dir/status"; date +%s > "$results/last-unfinished"
  pl_say "after: $5 on ${1:0:10}, $n times; it is tried again in half an hour or on a new head ($cost)"
  pl_alarm unfinished "the hosts could not finish the check of ${1:0:10}" "$5 on ${1:0:10}, $n times in a row, so \`$pl_green\` has not moved. The pipeline tries this head again in half an hour, and a new head at once.

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
    [[ -z "$changed" ]] || ! grep -qv '\.md$' <<< "$changed" || jobs+=("web $ref" "room-since $ref${prod:+ $prod}")
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
    jobs+=("tools${prod:+ $prod}")
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
  local bpids=() bitems=() n=0 job args rest=() owned=0 flaky_n=0 found=0 infra_b=0
  for x in ${failed[@]+"${failed[@]}"}; do
    item="${x%%|*}"; job="${x#*|}"
    if jq -e --arg c "$item" '.[] | select(.status == "open" and .check == $c)' "$pl_red" > /dev/null; then
      pl_say "after: $item is already on the red list"; owned=1; continue
    fi
    if [[ "$item" == count\ * ]]; then
      # A raised count: its culprit is the commit that raised it (counts.sh), no bisect.
      awk -v x="${item#count }" '$2 == x { print $3; exit }' "$dir/count-culprits" > "$dir/bisect-$n.culprit"
      bpids+=(0); bitems+=("$item"); n=$((n + 1))
    elif (( n < bisect_checks )); then
      args="$(args_for "$item" "$job")"
      # shellcheck disable=SC2086
      ( c="$(confirm "$dir/bisect-$n" "$head" "$item" $args)"
        # Worse than production at main-green too: it was already there (it shipped, or will), so
        # it is reported, not red, and no commit of this run is blamed for it.
        if [[ "$c" == red && "$item" == e2\ * ]]; then
          check "$base" "$dir/bisect-$n/base-${base:0:10}" $args
          case $? in 1) c=prior ;; 97) c=infra ;; esac
        fi
        [[ "$c" != red ]] || c="$(bisect "$dir/bisect-$n" "$base" "$head" $args)"
        echo "$c" > "$dir/bisect-$n.culprit" ) & bpids+=($!); bitems+=("$item"); n=$((n + 1))
    else rest+=("$item"); fi
  done
  (( ${#rest[@]} == 0 )) || pl_say "after: left for the next run (OA_BISECT_CHECKS=$bisect_checks): ${rest[*]}"
  local counted=0
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
    # The issues the culprit's subject names hear of it too, before any revert.
    local r
    for r in $(grep -oE '#[0-9]+' <<< "$subject" | tr -d '#' | sort -u); do
      [[ "$r" == "$issue" ]] || pl_issue_comment "$r" "The after-landing check \`$item\` fails on \`$pl_main\` since ${culprit:0:10} (\"$subject\"), by agent \`$agent\`${issue:+: #$issue}. If it still fails 30 minutes from now, the integrator queues a revert of ${culprit:0:10}; a fix on \`$pl_main\` before then means nothing is reverted. Run the check alone with \`coax/remote/remote.sh run -- $([[ "$(args_for "$item" "$job")" == visuals ]] && echo "coax/web/check-visuals.sh apps/openagents-web" || echo "coax/remote/check.sh $(args_for "$item" "$job")")\`."
    done
    pl_say "after: $item: culprit ${culprit:0:10} by $agent${issue:+ (#$issue)}"
    # A count's baseline is main-green, not the last full green run: it never moves main-green back.
    if [[ "$item" == count\ * ]]; then counted=$((counted + 1)); else found=$((found + 1)); fi
  done
  # Every failure passed when run again on H: H is green.
  if (( infra == 0 && infra_b == 0 && owned == 0 && ${#rest[@]} == 0 && flaky_n == ${#failed[@]} )); then
    go_green "$head" "$green" "$kind" "$started" "${#jobs[@]}" ", on a second run of $flaky_n flaky check(s)"
    return 0
  fi
  # A full run found what the fast runs passed: main-green goes back to the
  # last head a full run passed.
  if (( found > 0 )) && [[ -n "$green" && "$green" != "$base" ]]; then
    git push -q --force "$pl_remote" "$base:refs/heads/$pl_green" 2> /dev/null &&
      pl_say "after: a full run failed after fast runs passed; $pl_green is back at ${base:0:10}"
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
stale_check() {
  local head green first age
  head="$(git rev-parse -q --verify "$main_ref" 2> /dev/null || true)"
  green="$(git rev-parse -q --verify "$green_ref" 2> /dev/null || true)"
  [[ -n "$head" && -n "$green" && "$head" != "$green" ]] || return 0
  first="$(git rev-list --reverse --first-parent "$green..$head" | head -1)"
  [[ -n "$first" ]] || return 0
  age=$(( $(date +%s) - $(git log -1 --format=%ct "$first") ))
  (( age >= stale )) || return 0
  pl_alarm stale "$pl_green is $(( age / 3600 )) h $(( age % 3600 / 60 )) min behind $pl_main" "\`$pl_green\` is ${green:0:10}, and the oldest of the $(git rev-list --count "$green..$head") commit(s) after it landed $(( age / 60 )) minutes ago. Releases ship \`$pl_green\`, so they are that far behind.

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
  ( while kill -0 $$ 2> /dev/null; do date +%s > "$results/heartbeat"; pl_publish; sleep 60; done ) 7>&- &
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
fi

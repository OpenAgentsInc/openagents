#!/usr/bin/env bash
# Firm reason: 5. Settings and helpers shared by the green-main pipeline's
# scripts (docs/verse/green-main-pipeline.md, #13102): submit.sh,
# integrate.sh and after.sh source it. Git, gh and the files under the
# pipeline's directory.
#
# Settings (environment):
#   OA_PIPELINE_REMOTE   the Git remote (default origin)
#   OA_PIPELINE_MAIN     the branch the integrator lands on (default main)
#   OA_PIPELINE_GREEN    the last commit that passed the full check (default main-green)
#   OA_PIPELINE_QUEUE    the queue's ref prefix under refs/heads/ (default queue)
#   OA_PIPELINE_REJECTED where a red branch goes back to, under refs/heads/ (default rejected)
#   OA_PIPELINE_DIR      logs, results and the red list (default ~/.openagents/pipeline)
#   OA_PIPELINE_ISSUES   0: file no GitHub issues and write no comments (scratch runs)
#   OA_PIPELINE_NOTIFY   a command run as CMD AGENT MESSAGE-FILE for each message to an agent
#   OA_SERVICE_AGENT     who hears the service alerts the watcher passes on (default foreman)
#   OA_SERVICE_ORIGINS   the sites whose alerts it reads (default production and staging); 0: none
#
# A scratch run points every name elsewhere, for example
#   OA_PIPELINE_MAIN=scratch/gm/main OA_PIPELINE_GREEN=scratch/gm/main-green \
#   OA_PIPELINE_QUEUE=scratch/gm/queue OA_PIPELINE_REJECTED=scratch/gm/rejected \
#   OA_PIPELINE_DIR=/tmp/gm OA_PIPELINE_ISSUES=0
pl_remote="${OA_PIPELINE_REMOTE:-origin}"
pl_main="${OA_PIPELINE_MAIN:-main}"
pl_green="${OA_PIPELINE_GREEN:-main-green}"
pl_queue="${OA_PIPELINE_QUEUE:-queue}"
pl_rejected="${OA_PIPELINE_REJECTED:-rejected}"
pl_dir="${OA_PIPELINE_DIR:-$HOME/.openagents/pipeline}"
pl_issues="${OA_PIPELINE_ISSUES:-1}"
pl_repo="${OA_PIPELINE_GH_REPO:-OpenAgentsInc/openagents}"
pl_here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
mkdir -p "$pl_dir/logs" "$pl_dir/inbox" "$pl_dir/releases/after"
pl_red="$pl_dir/releases/after/red.json"
[[ -f "$pl_red" ]] || echo '[]' > "$pl_red"

pl_say() { echo "$(date +%H:%M:%S) $*" >&2; }
pl_date() {   # UNIX-TIME FORMAT: the time in local time, on macOS (date -r) and Linux (date -d @)
  date -r "$1" "+$2" 2> /dev/null || date -d "@$1" "+$2"
}
# The watcher's state, shared (#13255): the watcher runs on one machine
# (coderos-4080 since 2026-11-11), and every other machine reads its heartbeat,
# red list, last runs, alarms and agents' inboxes from one object in the build
# cache's bucket. The watcher writes it each minute (pl_publish); after.sh
# state, red and health on any other machine copy it into the same files under
# OA_PIPELINE_DIR first (pl_mirror), so everything that reads those files
# (prepush.coax, status-data.coax, submit.sh status) reads the watcher's.
# OA_PIPELINE_SHARED: the object's prefix, 0 for none. A scratch run (another
# main or main-green, or OA_PIPELINE_ISSUES=0) shares nothing unless set.
pl_shared="${OA_PIPELINE_SHARED:-gs://openagentsgemini-coax-build-cache/pipeline/state}"
if [[ -z "${OA_PIPELINE_SHARED:-}" ]] && [[ -n "${OA_PIPELINE_MAIN:-}${OA_PIPELINE_GREEN:-}" || "$pl_issues" != 1 ]]; then pl_shared=0; fi
pl_gcloud() {   # gcloud as the automation account (as remote.sh), at most 60 s
  local sa="$HOME/work/.secrets/gcloud-sa-config"
  if [[ -z "${CLOUDSDK_CONFIG:-}" && -d "$sa" ]]; then CLOUDSDK_CONFIG="$sa" timeout 60 gcloud "$@"; else timeout 60 gcloud "$@"; fi
}
pl_host() { hostname -s 2> /dev/null || hostname; }
# The watcher's files other machines read, as one tarball: run each minute
# beside the heartbeat.
pl_publish() {
  [[ "$pl_shared" != 0 ]] || return 0
  local t f files=() e2
  t="$(mktemp -d "${TMPDIR:-/tmp}/oa-pl-pub.XXXXXXXX")" || return 0
  for f in heartbeat red.json runs.tsv flaky.txt flaky.tsv lag.tsv production full-green full-head; do
    [[ ! -f "$pl_dir/releases/after/$f" ]] || files+=("releases/after/$f")
  done
  # The newest Everglade II measurement, at its own path (after.sh state lists it).
  e2="$(cd "$pl_dir" && ls -t releases/after/*/*/build/pipeline/e2.tsv 2> /dev/null | head -1)"
  [[ -z "$e2" ]] || files+=("$e2")
  [[ ! -f "$pl_dir/ALARM" ]] || files+=(ALARM)
  for f in "$pl_dir"/inbox/*.log; do [[ ! -f "$f" ]] || files+=("inbox/${f##*/}"); done
  echo "$(pl_host) $(date +%s)" > "$t/watcher"
  tar -czf "$t/state.tar.gz" -C "$pl_dir" ${files[@]+"${files[@]}"} -C "$t" watcher 2> /dev/null &&
    pl_gcloud storage cp "$t/state.tar.gz" "$pl_shared/state.tar.gz" > /dev/null 2>&1 ||
    pl_say "could not publish the pipeline's state to $pl_shared"
  rm -rf "$t"
}
# On a machine whose own watcher isn't running: the shared state into the
# local files, at most every 20 s. Status 0 when they now show a watcher on
# another machine (its name in releases/after/watcher-host).
pl_mirror() {
  [[ "$pl_shared" != 0 ]] || return 1
  # This machine's watcher writes the heartbeat and no watcher-host: its own
  # files are the state. (Not watch.lock: a stopped watcher's jobs still hold it
  # until they end.)
  local m="$pl_dir/mirror" t host at hb=0
  [[ ! -f "$pl_dir/releases/after/heartbeat" ]] || hb="$(cat "$pl_dir/releases/after/heartbeat")"
  [[ -s "$pl_dir/releases/after/watcher-host" ]] || (( $(date +%s) - hb >= 300 )) || return 1
  mkdir -p "$m"
  if [[ -f "$m/fetched" ]] && (( $(date +%s) - $(cat "$m/fetched") < 20 )); then
    [[ -s "$pl_dir/releases/after/watcher-host" ]]; return
  fi
  t="$(mktemp -d "${TMPDIR:-/tmp}/oa-pl-mirror.XXXXXXXX")" || return 1
  if ! pl_gcloud storage cp "$pl_shared/state.tar.gz" "$t/state.tar.gz" > /dev/null 2>&1 ||
     ! mkdir "$t/s" || ! tar -xzf "$t/state.tar.gz" -C "$t/s" 2> /dev/null || [[ ! -s "$t/s/watcher" ]]; then
    rm -rf "$t"; date +%s > "$m/fetched"; [[ -s "$pl_dir/releases/after/watcher-host" ]]; return
  fi
  read -r host at < "$t/s/watcher"
  if [[ "$host" == "$(pl_host)" ]]; then rm -rf "$t"; return 1; fi
  rm -f "$t/s/watcher" "$t/s/ALARM.keep"; [[ ! -f "$t/s/ALARM" ]] || mv "$t/s/ALARM" "$t/s/ALARM.keep"
  (cd "$t/s" && find . -type f ! -name ALARM.keep -print0 | while IFS= read -r -d '' f; do mkdir -p "$pl_dir/${f%/*}" && cp "$f" "$pl_dir/$f"; done)
  if [[ -f "$t/s/ALARM.keep" ]]; then cp "$t/s/ALARM.keep" "$pl_dir/ALARM"; else rm -f "$pl_dir/ALARM"; fi
  echo "$host" > "$pl_dir/releases/after/watcher-host"
  rm -rf "$t"; date +%s > "$m/fetched"
  return 0
}
pl_lock() {   # FILE CMD...: CMD under an exclusive lock
  if command -v flock > /dev/null; then flock "$@"; else lockf -k "$@"; fi
}
pl_trylock() {   # FILE CMD...: CMD under the lock, or status 75 when another holds it
  if command -v flock > /dev/null; then flock -n -E 75 "$@"; else lockf -k -t 0 "$@" || return 75; fi
}
# A commit's agent, from its last "Agent:" trailer ("" when none).
pl_agent_of() {
  git log -1 --format='%(trailers:key=Agent,valueonly,separator=%x0a)' "$1" 2> /dev/null | sed '/^$/d' | tail -1 | tr -d '[:space:]'
}
pl_valid_name() { [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$ ]]; }

# A message to an agent: its inbox file (OA_PIPELINE_DIR/inbox/AGENT.log, which
# `submit.sh status` prints), and OA_PIPELINE_NOTIFY when set.
pl_message() {   # agent text
  local agent="${1:-unknown}" file
  file="$(mktemp "${TMPDIR:-/tmp}/oa-pipeline-msg.XXXXXXXX")"
  printf '%s\n' "$2" > "$file"
  { echo "== $(date '+%Y-%m-%d %H:%M %Z')"; cat "$file"; echo; } >> "$pl_dir/inbox/$agent.log"
  if [[ -n "${OA_PIPELINE_NOTIFY:-}" ]]; then
    # shellcheck disable=SC2086
    $OA_PIPELINE_NOTIFY "$agent" "$file" || pl_say "OA_PIPELINE_NOTIFY failed for $agent"
  fi
  rm -f "$file"
}

# The service alerts (#13297, docs/deployment/service-levels.md). The API opens and resolves them
# every minute (coax/web/api/service.c) and mails the owner; the watcher passes each one opened or
# resolved on to the agents' coordinator as a message (its inbox and OA_PIPELINE_NOTIFY), and says
# so when a site's service health stops answering for 3 minutes. Run each minute beside the
# heartbeat, on the watcher's machine only. Its memory: OA_PIPELINE_DIR/service-ENV (key TAB title).
pl_service_watch() {
  local origin env f j open n key title agent="${OA_SERVICE_AGENT:-foreman}"
  [[ "${OA_SERVICE_ORIGINS:-}" != 0 ]] || return 0
  for origin in ${OA_SERVICE_ORIGINS:-https://openagents.com https://staging.openagents.com}; do
    env="${origin#*://}"; env="${env%%.*}"; [[ "$env" != openagents ]] || env=production
    f="$pl_dir/service-$env"
    if ! j="$(curl -fsS --max-time 20 "$origin/api/playtest/service-health" 2> /dev/null)" ||
       ! open="$(jq -r '.alerts[] | "\(.key)\t\(.title)"' <<< "$j" 2> /dev/null)"; then
      n=$(( $(cat "$f.down" 2> /dev/null || echo 0) + 1 )); echo "$n" > "$f.down"
      (( n != 3 )) || pl_message "$agent" "Service alert ($env): $origin/api/playtest/service-health has not answered for 3 minutes, so its alerts can't be read. Check oa-api's health (docs/deployment/service-levels.md)."
      continue
    fi
    if [[ -f "$f.down" ]] && (( $(cat "$f.down") >= 3 )); then pl_message "$agent" "Service alert ($env): $origin/api/playtest/service-health answers again."; fi
    rm -f "$f.down"; touch "$f"
    while IFS=$'\t' read -r key title; do
      [[ -n "$key" ]] || continue
      cut -f1 "$f" | grep -qxF -- "$key" || pl_message "$agent" "Service alert ($env) opened: $title. Runbook: docs/deployment/service-levels.md. Now: $origin/api/playtest/service-health"
    done <<< "$open"
    while IFS=$'\t' read -r key title; do
      [[ -n "$key" ]] || continue
      printf '%s\n' "$open" | cut -f1 | grep -qxF -- "$key" || pl_message "$agent" "Service alert ($env) resolved: $title."
    done < "$f"
    printf '%s\n' "$open" | sed '/^$/d' > "$f"
  done
}

# A night's capacity record (#13296, docs/deployment/soak.md) to each API in OA_SOAK_PUBLISH
# (production's and staging's; 0: none), so the status page shows it: coax/web/room/soak.coax publish
# posts it as a room's `capacity` write, signed with shard 1's write key (Secret Manager
# oa-room-write-key-s1) or, while the API still takes it, the shared seat-ticket key. The key goes
# from Secret Manager into the tool's environment and nowhere else. Without gcloud, a key or this
# checkout's compiler, the record stays in the results (releases/after/soak/DAY.txt) and a line says so.
pl_soak_publish() {   # record-file
  local rec="$1" origin root key="" name
  [[ "${OA_SOAK_PUBLISH:-}" != 0 && -s "$rec" ]] || return 0
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
  [[ -x "$root/coax/bin/coax" ]] || { pl_say "soak: this checkout has no compiler: the record is not published ($rec)"; return 0; }
  for name in oa-room-write-key-s1 oa-room-ticket-key; do
    key="$(CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}" gcloud secrets versions access latest --secret "$name" --project "${OA_REMOTE_PROJECT:-openagentsgemini}" 2> /dev/null)" && [[ -n "$key" ]] && break
    key=""
  done
  [[ -n "$key" ]] || { pl_say "soak: no room key could be read: the record is not published ($rec)"; return 0; }
  for origin in ${OA_SOAK_PUBLISH:-https://openagents.com https://staging.openagents.com}; do
    if [[ "$name" == oa-room-write-key-s1 ]]; then
      (cd "$root" && OA_ROOM_WRITE_KEY="$key" OA_ROOM_SHARD=1 coax/web/tool.sh coax/web/room/soak.coax publish "$rec" "$origin") > /dev/null 2>&1
    else
      (cd "$root" && OA_ROOM_TICKET_KEY="$key" coax/web/tool.sh coax/web/room/soak.coax publish "$rec" "$origin") > /dev/null 2>&1
    fi && pl_say "soak: the night's record is on $origin" || pl_say "soak: $origin did not take the night's record"
  done
}

# The red list (releases/after/red.json): one entry per failing check on main
# that has a culprit. Fields: id, check, culprit, subject, agent, issue,
# opened (Unix time), status (open, fixed or reverted), revert (the revert
# branch, once queued).
pl_red_update() {   # JQ-PROGRAM [ARGS...]: rewrite the red list under its lock
  local prog="$1"; shift
  pl_lock "$pl_red.lock" bash -c '
    f="$1"; prog="$2"; shift 2
    jq "$@" "$prog" "$f" > "$f.new" && mv "$f.new" "$f"' _ "$pl_red" "$prog" "$@"
}

pl_issue_comment() {   # issue-number text
  [[ "$pl_issues" == 1 && -n "$1" ]] || return 0
  gh issue comment "$1" --repo "$pl_repo" --body "$2" > /dev/null 2>&1 || pl_say "could not comment on #$1"
}

# A loud report that the pipeline itself needs attention (the watcher was not
# running, main-green is stale, a head could not be checked): the log, the
# ALARM file under the pipeline's directory (after.sh state prints it), and a
# comment on one GitHub issue, opened when none is open. One report per KEY
# every OA_PIPELINE_ALARM_GAP seconds (7200).
pl_alarm() {   # key title text
  local key="${1//[^A-Za-z0-9._-]/-}" gap="${OA_PIPELINE_ALARM_GAP:-7200}" n="" url
  mkdir -p "$pl_dir/alarms"
  if [[ -f "$pl_dir/alarms/$key" ]] && (( $(date +%s) - $(cat "$pl_dir/alarms/$key") < gap )); then return 0; fi
  date +%s > "$pl_dir/alarms/$key"
  pl_say "ALARM: $2"
  printf '%s\t%s\t%s\n' "$(date '+%Y-%m-%d %H:%M %Z')" "$key" "$2" >> "$pl_dir/ALARM"
  [[ "$pl_issues" == 1 ]] || return 0
  [[ ! -f "$pl_dir/alarm-issue" ]] || n="$(cat "$pl_dir/alarm-issue")"
  # One issue for every machine: the watcher's and a reader's (pl_watcher_alarm).
  [[ -n "$n" ]] || n="$(pl_alarm_open)"
  if [[ -n "$n" && "$(gh issue view "$n" --repo "$pl_repo" --json state --jq .state 2> /dev/null)" == OPEN ]]; then
    gh issue comment "$n" --repo "$pl_repo" --body "**$2**

$3" > /dev/null 2>&1 || pl_say "could not comment on #$n"
  else
    url="$(gh issue create --repo "$pl_repo" --title "Green-main pipeline needs attention: $2" --body "$3

Reported by \`coax/remote/after.sh\` (docs/verse/green-main-pipeline.md). \`coax/remote/after.sh state\` shows the pipeline now. This issue closes itself when \`$pl_green\` next moves." 2> /dev/null)" &&
      echo "${url##*/}" > "$pl_dir/alarm-issue" || pl_say "could not open an issue for the alarm"
  fi
}
# The open alarm issue, if any (another machine may have opened it).
pl_alarm_open() {
  gh issue list --repo "$pl_repo" --state open --search 'in:title "Green-main pipeline needs attention"' --json number --jq '.[0].number // empty' 2> /dev/null
}
# main-green moved: the alarms are over. An issue another machine opened is
# looked for at most every 30 minutes.
pl_alarm_clear() {   # text
  if [[ ! -f "$pl_dir/ALARM" && ! -f "$pl_dir/alarm-issue" && "$pl_issues" == 1 ]] &&
     { [[ ! -f "$pl_dir/alarm-looked" ]] || (( $(date +%s) - $(cat "$pl_dir/alarm-looked") >= 1800 )); }; then
    date +%s > "$pl_dir/alarm-looked"
    local o; o="$(pl_alarm_open)"; [[ -z "$o" ]] || echo "$o" > "$pl_dir/alarm-issue"
  fi
  [[ -f "$pl_dir/ALARM" || -f "$pl_dir/alarm-issue" ]] || return 0
  rm -rf "$pl_dir/ALARM" "$pl_dir/alarms"
  local n=""; [[ ! -f "$pl_dir/alarm-issue" ]] || n="$(cat "$pl_dir/alarm-issue")"
  rm -f "$pl_dir/alarm-issue"
  [[ "$pl_issues" == 1 && -n "$n" ]] || return 0
  gh issue close "$n" --repo "$pl_repo" --comment "$1" > /dev/null 2>&1 || true
}

#!/usr/bin/env bash
# Firm reason: 5. Runs a heavy command on a remote build host instead of this
# computer (docs/metaverse/remote-builds.md, #13077): gcloud, ssh, git and tar,
# and the Mac's build slots (~/.openagents/build-slots/heavy.sh) when no host
# can take the job.
#
#   coax/remote/remote.sh run [--local] [--no-fallback] -- CMD [ARGS...]
#                              run CMD from this directory of this checkout on a
#                              host: the checkout's commit and its uncommitted
#                              changes (untracked files too) go there, and the
#                              files CMD writes come back
#   coax/remote/remote.sh run --commit REV [--out DIR] [--no-fallback] -- CMD ...
#                              run CMD from the root of commit REV instead (the
#                              green-main pipeline's checks): no snapshot, and
#                              the files CMD writes go to DIR (or nowhere)
#   coax/remote/remote.sh up      start (or create) a host and wait until it is ready
#   coax/remote/remote.sh status  hosts, free slots, and spend
#   coax/remote/remote.sh cost    spend over the last 24 hours and 30 days, from the ledger
#   coax/remote/remote.sh down    stop every host (the disks, and their caches, stay)
#   coax/remote/remote.sh delete  delete every host and its disk
#
# Settings (environment):
#   OA_REMOTE=0                  never use a host: run on this computer's slots
#   OA_REMOTE_DAILY_USD          the most the hosts may cost over 24 hours (default 100)
#   OA_REMOTE_MAX_HOSTS          hosts at once (default 16)
#   OA_REMOTE_MACHINE            machine type (default c3d-highcpu-30, spot)
#   OA_REMOTE_IDLE_MINUTES       a host stops itself after this long idle (default 15)
#   OA_REMOTE_SLOTS              jobs at once on one host (default 3)
#   OA_REMOTE_ZONES              zones to create in, in turn (default us-central1-a,b,c,f)
#   OA_REMOTE_TIMEOUT            seconds the command may run on the host (default no limit)
#   OA_REMOTE_WAIT               seconds to look for a free slot on any host before
#                                waiting on one (default 1800)
#   OA_REMOTE_HOST               run on this host only (a running one), waiting for its
#                                slot: for measuring one host against another
#   OA_REMOTE_AVOID              a host to use only when no other running one has a free slot (the
#                                pipeline runs a failed check again elsewhere, #13256)
#   OA_REMOTE_FRESH_COMPILER=1   the slot reinstalls its compiler before the command
#                                (the pipeline's retry of a check the host could not judge)
#   CLOUDSDK_CONFIG              gcloud's configuration (default the automation account's)
set -uo pipefail
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd -P)"
project="${OA_REMOTE_PROJECT:-openagentsgemini}"
state="${OA_REMOTE_STATE:-$HOME/.openagents/remote}"
daily="${OA_REMOTE_DAILY_USD:-110}"
max_hosts="${OA_REMOTE_MAX_HOSTS:-10}"
machine="${OA_REMOTE_MACHINE:-c3d-highcpu-30}"
idle="${OA_REMOTE_IDLE_MINUTES:-15}"
slots="${OA_REMOTE_SLOTS:-3}"
zones="${OA_REMOTE_ZONES:-us-central1-a,us-central1-b,us-central1-c,us-central1-f}"
sa_config="$HOME/work/.secrets/gcloud-sa-config"
[[ -n "${CLOUDSDK_CONFIG:-}" || ! -d "$sa_config" ]] || export CLOUDSDK_CONFIG="$sa_config"
# List price per hour (Cloud Billing catalog, us-central1, 2026-11-09): spot
# C3D $0.00711 a vCPU and $0.00096 a GiB; c3d-highcpu-30 is 30 vCPU and 59 GiB,
# plus $0.005 for its external address. The 200 GB pd-balanced disk is $20 a
# month whether the host runs or not.
case "$machine" in
  c3d-highcpu-30) rate=0.275 ;;
  c3d-highcpu-16) rate=0.150 ;;
  c3d-highcpu-60) rate=0.545 ;;
  *) rate="${OA_REMOTE_USD_PER_HOUR:?OA_REMOTE_USD_PER_HOUR: no list price for $machine}" ;;
esac
rate="${OA_REMOTE_USD_PER_HOUR:-$rate}"
mkdir -p "$state"; chmod 700 "$state"
key="$state/ssh_ed25519"; known="$state/known_hosts"; ledger="$state/ledger.tsv"
[[ -f "$key" ]] || ssh-keygen -q -t ed25519 -N '' -C coax-remote -f "$key"
say() { echo "remote: $*" >&2; }
now() { date +%s; }

# The ledger: one line per host start or creation and per job, "time kind host
# seconds usd note". Spend is estimated from it, high rather than low: every
# job is billed at the whole host's price for its wall time on this computer,
# and every start adds the idle minutes before the host stops itself.
record() {   # kind host seconds note: one ledger line, its cost the seconds at the host's rate
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$(now)" "$1" "$2" "$3" "$(awk -v s="$3" -v r="$rate" 'BEGIN { printf "%.4f", s / 3600 * r }')" "$4" >> "$ledger"
}
# One cap for every machine (#13255): each ledger is copied to the bucket as
# ledgers/HOST-ID.tsv, and the others come back into $state/ledgers/, at most
# every 5 minutes and in the background, so spent() counts every machine's
# jobs (OA_REMOTE_SHARED_LEDGERS=0: this ledger alone).
shared_ledgers="${OA_REMOTE_SHARED_LEDGERS:-gs://openagentsgemini-coax-build-cache/pipeline/ledgers}"
ledger_id="$(hostname -s 2> /dev/null || hostname)-$(printf '%s' "$state" | shasum -a 256 | cut -c1-8)"
ledger_sync() {
  [[ "$shared_ledgers" != 0 ]] && command -v gcloud > /dev/null || return 0
  mkdir -p "$state/ledgers"
  [[ ! -f "$state/ledgers/.synced" ]] || (( $(now) - $(cat "$state/ledgers/.synced") >= 300 )) || return 0
  now > "$state/ledgers/.synced"
  ( [[ ! -f "$ledger" ]] || timeout 60 gcloud storage cp -q "$ledger" "$shared_ledgers/$ledger_id.tsv"
    mkdir -p "$state/ledgers/all" && timeout 60 gcloud storage rsync -q "$shared_ledgers" "$state/ledgers/all"
  ) < /dev/null > /dev/null 2>&1 7>&- 8>&- 9>&- &
}
spent() {   # seconds: the estimated spend over that window, on every machine
  ledger_sync
  local others=() f
  for f in "$state/ledgers/all/"*.tsv; do [[ ! -f "$f" || "$(basename "$f" .tsv)" == "$ledger_id" ]] || others+=("$f"); done
  [[ -f "$ledger" || ${#others[@]} -gt 0 ]] || { echo 0; return; }
  cat ${ledger:+"$ledger"} ${others[@]+"${others[@]}"} 2> /dev/null |
    awk -F'\t' -v since="$(( $(now) - $1 ))" '$1 >= since { s += $5 } END { printf "%.2f", s }'
}
under_budget() { awk -v s="$(spent 86400)" -v d="$daily" 'BEGIN { exit !(s < d) }'; }

lock() {   # FILE CMD...: CMD under an exclusive lock
  if command -v flock > /dev/null; then flock "$@"; else lockf -k "$@"; fi
}
# SSH's shared connections: a socket path over about 104 bytes (with ssh's own
# suffix) fails, and every ssh_to with it, so a long OA_REMOTE_STATE keeps them
# in /tmp (the watcher's ~/.openagents/pipeline/remote on coderos, #13255).
cm_dir="$state"
if (( ${#state} > 45 )); then cm_dir="/tmp/oa-remote-cm-$(id -u)"; mkdir -p "$cm_dir" && chmod 700 "$cm_dir"; fi
ssh_to() {   # NAME IP CMD...
  local name="$1" ip="$2"; shift 2
  ssh -i "$key" -o IdentitiesOnly=yes -o BatchMode=yes -o UserKnownHostsFile="$known" -o HostKeyAlias="$name" \
    -o StrictHostKeyChecking=accept-new -o ConnectTimeout=15 -o ServerAliveInterval=20 -o ServerAliveCountMax=6 \
    -o ControlMaster=auto -o ControlPath="$cm_dir/cm-%C" -o ControlPersist=300 -o LogLevel=ERROR "coax@$ip" "$@"
}
hosts() {   # name zone status ip, one host a line
  gcloud compute instances list --project "$project" --filter=labels.oa-remote=build \
    --format='value(name,zone.basename(),status,networkInterfaces[0].accessConfigs[0].natIP)' 2> /dev/null | sort
}
ready() { ssh_to "$1" "$2" 'test -f /var/lib/oa-remote-ready && touch ~/busy' < /dev/null 2> /dev/null; }
wait_ready() {   # name zone: the host's address once its setup is done (first boot: Nix, Bun, the clone)
  local name="$1" zone="$2" ip="" until=$(( $(now) + 1500 ))
  while (( $(now) < until )); do
    ip="$(gcloud compute instances describe "$name" --zone "$zone" --project "$project" --format='value(networkInterfaces[0].accessConfigs[0].natIP)' 2> /dev/null)"
    [[ -n "$ip" ]] && ready "$name" "$ip" && { echo "$ip"; return 0; }
    sleep 10
  done
  return 1
}
free_slots() {   # name ip: how many of the host's job slots no job holds
  ssh_to "$1" "$2" "n=0; for s in \$(seq 1 $slots); do mkdir -p ~/slots/\$s; flock -n ~/slots/\$s/lock true && n=\$((n + 1)); done; echo \$n" < /dev/null 2> /dev/null || echo 0
}
create() {   # name: a new spot host, in each zone in turn
  local name="$1" zone
  ssh-keygen -q -R "$name" -f "$known" > /dev/null 2>&1 || true
  for zone in ${zones//,/ }; do
    say "creating $name ($machine, spot) in $zone"
    if gcloud compute instances create "$name" --project "$project" --zone "$zone" --machine-type "$machine" \
        --provisioning-model=SPOT --instance-termination-action=STOP --max-run-duration=12h \
        --image-family=debian-12 --image-project=debian-cloud --boot-disk-size=200GB --boot-disk-type=pd-balanced \
        --no-service-account --no-scopes --shielded-secure-boot --labels=oa-remote=build \
        --metadata-from-file=startup-script="$here/host-startup.sh" \
        --metadata="ssh-keys=coax:$(cat "$key.pub"),block-project-ssh-keys=TRUE,oa-remote-idle-minutes=$idle" > /dev/null 2>&1; then
      record create "$name" 900 "first boot and setup"   # billed as 15 minutes of setup
      echo "$zone"; return 0
    fi
  done
  return 1
}
# A host with a free slot: a running one, else a stopped one started, else a
# new one (within OA_REMOTE_MAX_HOSTS and the budget). Prints "name ip".
acquire() {
  local list name zone status ip best="" stopped="" count=0 free avoided=""
  list="$(hosts)" || return 1
  while read -r name zone status ip; do
    [[ -n "$name" ]] || continue
    count=$((count + 1))
    if [[ "$status" == RUNNING && -n "$ip" ]] && ready "$name" "$ip"; then
      free="$(free_slots "$name" "$ip")"
      # OA_REMOTE_AVOID: another host first, this one only when none has a slot.
      if (( free > 0 )) && [[ "$name" == "${OA_REMOTE_AVOID:-}" ]]; then avoided="$name $ip"; continue; fi
      if (( free > 0 )); then echo "$name $ip"; return 0; fi
      [[ -n "$best" ]] || best="$name $ip"
    elif [[ "$status" == TERMINATED || "$status" == STOPPED ]]; then
      [[ -n "$stopped" ]] || stopped="$name $zone"
    fi
  done <<< "$list"
  [[ -z "$avoided" ]] || { echo "$avoided"; return 0; }
  if ! under_budget; then
    say "the hosts cost about \$$(spent 86400) over 24 hours, the most OA_REMOTE_DAILY_USD allows is \$$daily: starting none"
    [[ -n "$best" ]] && { echo "$best"; return 0; }
    return 1
  fi
  if [[ -n "$stopped" ]]; then
    read -r name zone <<< "$stopped"
    say "starting $name in $zone"
    if gcloud compute instances start "$name" --zone "$zone" --project "$project" > /dev/null 2>&1; then
      record start "$name" "$(( idle * 60 ))" "idle minutes after the last job"
      ip="$(wait_ready "$name" "$zone")" && { echo "$name $ip"; return 0; }
    fi
    say "could not start $name (spot capacity?)"
  elif (( count < max_hosts )); then
    name="oa-build-$((count + 1))"
    if zone="$(create "$name")"; then
      record start "$name" "$(( idle * 60 ))" "idle minutes after the last job"
      say "waiting for $name's first boot (Nix, Bun and the clone; about 10 minutes)"
      ip="$(wait_ready "$name" "$zone")" && { echo "$name $ip"; return 0; }
    fi
    say "could not create a host"
  fi
  [[ -n "$best" ]] && { echo "$best"; return 0; }
  return 1
}
named_host() {   # name: "name ip" of that running host
  local ip
  ip="$(gcloud compute instances list --project "$project" --filter="name=$1 AND status=RUNNING" --format='value(networkInterfaces[0].accessConfigs[0].natIP)' 2> /dev/null)"
  [[ -n "$ip" ]] && ready "$1" "$ip" && echo "$1 $ip"
}
# The last host used, if it still answers: no gcloud call for most jobs.
cached_host() {
  local name ip free
  [[ -f "$state/host" ]] && read -r name ip < "$state/host" || return 1
  [[ "$name" != "${OA_REMOTE_AVOID:-}" ]] || return 1
  [[ -n "$ip" ]] && ready "$name" "$ip" || return 1
  free="$(free_slots "$name" "$ip")"
  (( free > 0 )) || return 1
  echo "$name $ip"
}

local_run() {   # the Mac's own slots
  say "running on this computer${1:+: $1}"
  shift
  export OA_REMOTE=0
  if [[ -x "$HOME/.openagents/build-slots/heavy.sh" ]]; then exec "$HOME/.openagents/build-slots/heavy.sh" "$@"; fi
  exec "$@"
}

# What one call made, removed: its scratch folder, its bundle's ref and its
# token refresher. Each call's own (the names hold its folder and its pid).
tmp=""; bref=""; refresher=""; cleanup_root=""
cleanup() {
  [[ -z "$refresher" ]] || { pkill -P "$refresher" 2> /dev/null; kill "$refresher" 2> /dev/null; refresher=""; }
  [[ -z "$bref" || -z "$cleanup_root" ]] || git -C "$cleanup_root" update-ref -d "$bref" 2> /dev/null
  bref=""
  [[ -z "$tmp" ]] || rm -rf "${tmp:?}"
  tmp=""
}

run() {
  local fallback=1 force_local=0 at_commit="" out_dir=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --local) force_local=1; shift ;;
      --no-fallback) fallback=0; shift ;;
      --commit) at_commit="$2"; shift 2 ;;
      --out) out_dir="$2"; shift 2 ;;
      --) shift; break ;;
      *) break ;;
    esac
  done
  [[ $# -gt 0 ]] || { echo "usage: remote.sh run [--local] [--no-fallback] -- CMD [ARGS...]" >&2; exit 2; }
  if [[ -n "$at_commit" ]]; then
    # A commit's own files only run on a host: the Mac's slots would need a
    # checkout of it, so no fallback.
    fallback=0
  elif [[ "$force_local" == 1 || "${OA_REMOTE:-}" == 0 ]]; then local_run "" "$@"; fi
  fail() {   # why CMD...: the Mac's slots run CMD (or, with --no-fallback, exit 97)
    local why="$1"; shift
    cleanup   # local_run execs: the traps go with this shell
    if [[ "$fallback" == 1 ]]; then local_run "$why" "$@"; else say "$why"; exit 97; fi
  }
  local root rel started
  started=$(now)
  root="$(git rev-parse --show-toplevel 2> /dev/null)" || { say "not in a Git checkout"; exit 2; }
  rel="$(git rev-parse --show-prefix)"; rel="${rel%/}"; [[ -n "$rel" ]] || rel=.
  # This call's scratch folder (#13181): never inside the checkout, where the
  # snapshot would take it and every later call would send it again, and
  # removed when the call ends however it ends (done, failed, Ctrl-C, kill).
  # Many calls run side by side: each removes only its own folder and ref.
  local base="${TMPDIR:-/tmp}"; base="$(cd "$base" 2> /dev/null && pwd -P)" || base=/tmp
  case "$base/" in "$(cd "$root" && pwd -P)"/*) base=/tmp ;; esac
  tmp="$(mktemp -d "$base/oa-remote.XXXXXXXX")" || { say "could not make a scratch folder in $base"; exit 2; }
  trap cleanup EXIT
  trap 'exit 130' INT; trap 'exit 143' TERM; trap 'exit 129' HUP
  local tree commit
  if [[ -n "$at_commit" ]]; then
    rel=.
    commit="$(git -C "$root" rev-parse --verify -q "$at_commit^{commit}")" || fail "no commit $at_commit" "$@"
    tree="$(git -C "$root" rev-parse "$commit^{tree}")"
  else
    # The snapshot: the checkout's files as they are, untracked ones too (not
    # ignored ones), as a commit on HEAD made with a scratch index, so the
    # checkout's own index and refs don't change. Its tree is the job's source
    # fingerprint.
    # build-remote/ is scratch of remote builds (a wrapper's temp folders, at
    # any depth): never part of a job's sources (#13181). It is ignored here
    # on top of the checkout's own ignores and this user's global ones.
    local big global
    global="$(git -C "$root" config --path core.excludesFile 2> /dev/null)" || global="${XDG_CONFIG_HOME:-$HOME/.config}/git/ignore"
    { [[ ! -f "$global" ]] || cat "$global"; echo; echo 'build-remote/'; } > "$tmp/exclude"
    big="$(cd "$root" && git -c core.excludesFile="$tmp/exclude" ls-files -o --exclude-standard -z | xargs -0 du -k 2> /dev/null | awk '{ s += $1 } END { print s * 1024 }')"
    (( big < 200000000 )) || fail "untracked files hold $((big / 1000000)) MB (more than 200 MB): too much to send" "$@"
    cp "$(git -C "$root" rev-parse --path-format=absolute --git-path index)" "$tmp/index" 2> /dev/null
    tree="$(cd "$root" && GIT_INDEX_FILE="$tmp/index" git -c core.excludesFile="$tmp/exclude" add -A . && GIT_INDEX_FILE="$tmp/index" git write-tree)" || fail "could not snapshot the checkout" "$@"
    commit="$(cd "$root" && GIT_AUTHOR_NAME=remote GIT_AUTHOR_EMAIL=remote@localhost GIT_COMMITTER_NAME=remote GIT_COMMITTER_EMAIL=remote@localhost \
      GIT_AUTHOR_DATE='2026-01-01T00:00:00Z' GIT_COMMITTER_DATE='2026-01-01T00:00:00Z' git commit-tree "$tree" -p HEAD -m 'remote.sh snapshot')" || fail "could not snapshot the checkout" "$@"
  fi
  # What the host lacks: the commits since origin/main, or, for a commit main
  # already has, that commit alone (its parents come with the host's fetch of
  # main). The ref is named for this process too, so jobs on one commit side by
  # side don't delete each other's.
  local since=(origin/main)
  bref="refs/oa-remote/$$/$commit"; cleanup_root="$root"
  if git -C "$root" merge-base --is-ancestor "$commit" origin/main 2> /dev/null; then
    read -r -a since <<< "$(git -C "$root" rev-parse "$commit^@")"
  fi
  (cd "$root" && git update-ref "$bref" "$commit" && \
    { git bundle create "$tmp/bundle" "$bref" --not "${since[@]}" > /dev/null 2>&1 || \
      git bundle create "$tmp/bundle" "$bref" > /dev/null 2>&1; }; s=$?; git update-ref -d "$bref"; exit $s) \
    || fail "could not bundle the snapshot" "$@"
  bref=""
  local bundle_kb=$(( ($(wc -c < "$tmp/bundle") + 1023) / 1024 ))
  # OA_REMOTE_DRY_RUN=1: the snapshot and its bundle only, nothing sent (the
  # cleanup check, and measuring what a call would send).
  if [[ "${OA_REMOTE_DRY_RUN:-}" == 1 ]]; then echo "snapshot $tree bundle $bundle_kb KB"; exit 0; fi
  # The command, with this checkout's path written @ROOT@ (the host's tree
  # replaces it), and the Coax build settings it would have seen here.
  local argv=() a env_vars=() v
  for a in "$@"; do argv+=("${a//"$root"/@ROOT@}"); done
  while IFS= read -r v; do
    case "${v%%=*}" in
      COAX|COAXIAL_CLANG|COAXIAL_SYSROOT|COAXIAL_RESOURCES|COAXIAL_WASM_OPT|COAXIAL_WASM_CACHE|COAXIAL_TOOLCHAIN|COAX_CHECK_CACHE|COAX_RELEASE_CACHE) ;;
      COAX_SHARED_CACHE_TOKEN|COAX_SHARED_CACHE_STATS|COAX_SHARED_CACHE_TOOL|COAX_SHARED_CACHE_KEY|COAX_JOB_MARKER) ;;
      COAX_*|COAXIAL_*) env_vars+=("$v") ;;
    esac
  done < <(env | grep -E '^COAX(IAL)?_' | sort)
  local job
  job="$( { echo coax-remote-job-1; echo "$tree"; echo "$rel"; printf '%s\n' "${argv[@]}"; echo --; printf '%s\n' ${env_vars[@]+"${env_vars[@]}"}; } | shasum -a 256 | cut -c1-32)"
  # A token for the shared build cache's bucket alone (shared-cache.sh, #13119):
  # the host's wasm.sh reads and writes the bucket with it. Not in the key.
  local shared_token=""
  [[ "${COAX_SHARED_CACHE:-}" == 0 ]] || shared_token="$("$here/shared-cache.sh" token 2> /dev/null)" || shared_token=""
  { printf 'key=%q\ncommit=%q\nrelcwd=%q\nshared_token=%q\n' "$job" "$commit" "$rel" "$shared_token"
    printf 'argv=('; printf '%q ' "${argv[@]}"; printf ')\n'
    printf 'env_vars=('; [[ ${#env_vars[@]} -eq 0 ]] || printf '%q ' "${env_vars[@]}"; printf ')\n'; } > "$tmp/spec"
  under_budget || fail "the hosts cost about \$$(spent 86400) over 24 hours (OA_REMOTE_DAILY_USD is \$$daily)" "$@"
  local host name ip inbox="$job.$$" sent attempt=1 nowait status
  # nixpkgs.rev: the toolchain revision host-job.sh pins the host's nixpkgs to (#13256).
  cp "$here/host-job.sh" "$here/shared-cache.sh" "$here/nixpkgs.rev" "$tmp/"
  # A host whose slots filled since it was picked sends the job back (96), and
  # the hosts are asked again until one has a free slot.
  while true; do
    if [[ -n "${OA_REMOTE_HOST:-}" ]]; then host="$(named_host "$OA_REMOTE_HOST")" || fail "$OA_REMOTE_HOST isn't running" "$@"
    elif (( attempt == 1 )); then host="$(cached_host || lock "$state/acquire.lock" "$0" _acquire)"
    else host="$(lock "$state/acquire.lock" "$0" _acquire)"; fi || fail "no build host could take the job" "$@"
    read -r name ip <<< "$host"; [[ -n "${OA_REMOTE_HOST:-}" ]] || echo "$name $ip" > "$state/host"
    (cd "$tmp" && COPYFILE_DISABLE=1 tar --no-xattrs -cf - spec bundle host-job.sh shared-cache.sh nixpkgs.rev) | ssh_to "$name" "$ip" "mkdir -p ~/inbox/$inbox && tar --warning=no-unknown-keyword -xf - -C ~/inbox/$inbox && touch ~/busy" \
      || fail "could not send the job to $name" "$@"
    sent=$(now)
    # Every host full: look again every 10 s for the first slot any host frees,
    # for up to OA_REMOTE_WAIT seconds (default 1800), then wait on this host.
    nowait=1; (( $(now) - started < ${OA_REMOTE_WAIT:-1800} )) && [[ -z "${OA_REMOTE_HOST:-}" ]] || nowait=0
    # The cache token lasts an hour, and the job may run longer or have waited
    # for a slot: a new one a minute in, then every 15 minutes (host-job.sh
    # and shared-cache.sh read ~/tokens/JOB first).
    refresher=""
    if [[ -n "$shared_token" ]]; then
      ( pause=60
        while sleep "$pause"; do
          kill -0 $$ 2> /dev/null || exit 0
          t="$("$here/shared-cache.sh" token 2> /dev/null)" && [[ -n "$t" ]] &&
            printf '%s\n' "$t" | ssh_to "$name" "$ip" "umask 077; mkdir -p ~/tokens && cat > ~/tokens/$inbox.new && mv -f ~/tokens/$inbox.new ~/tokens/$inbox"
          pause=900
        done ) > /dev/null 2>&1 &
      refresher=$!
    fi
    ssh_to "$name" "$ip" "OA_REMOTE_SLOTS=$slots OA_REMOTE_NO_WAIT=$nowait OA_REMOTE_TIMEOUT=${OA_REMOTE_TIMEOUT:-0} OA_REMOTE_FRESH_COMPILER=${OA_REMOTE_FRESH_COMPILER:-0} bash ~/inbox/$inbox/host-job.sh run $inbox" < /dev/null
    status=$?
    # Its sleep too: an orphaned sleep keeps the caller's descriptors, and with
    # them a lock such as after.sh's (#13102).
    [[ -z "$refresher" ]] || { pkill -P "$refresher" 2> /dev/null; kill "$refresher" 2> /dev/null; wait "$refresher" 2> /dev/null; refresher=""; }
    [[ $status == 96 && $nowait == 1 ]] || break
    # A command of its own that exited 96 left its result.
    ssh_to "$name" "$ip" "grep -qx 96 ~/results/$job/status" < /dev/null 2> /dev/null && break
    (( attempt > 1 )) || say "$name's slots are full; waiting for a free slot on any host"
    rm -f "$state/host"; attempt=$((attempt + 1)); sleep 10
  done
  if [[ $status == 255 || $status == 97 ]]; then
    record job "$name" "$(( $(now) - started ))" "failed to run: $*"
    fail "$name could not run the job (ssh status $status)" "$@"
  fi
  # The files the command wrote, into this checkout.
  local dest="$root"
  [[ -z "$at_commit" ]] || dest="$out_dir"
  if [[ -n "$dest" ]]; then
    mkdir -p "$dest"
    ssh_to "$name" "$ip" "cat ~/results/$job/files.tar" < /dev/null | tar -xf - -C "$dest" 2> /dev/null \
      || say "could not bring the files back from $name"
  fi
  local back; back="$(ssh_to "$name" "$ip" "cat ~/results/$job/times" < /dev/null 2> /dev/null)"
  local total=$(( $(now) - started ))
  record job "$name" "$total" "status $status: $*"
  say "$name: $total s in all (snapshot and send $((sent - started)) s, $bundle_kb KB; ${back:-no timings}); about \$$(awk -v s="$total" -v r="$rate" 'BEGIN { printf "%.3f", s / 3600 * r }')"
  exit "$status"
}

case "${1:-}" in
  run) shift; run "$@" ;;
  _acquire) acquire ;;
  up) host="$(lock "$state/acquire.lock" "$0" _acquire)" && { echo "$host"; echo "$host" > "$state/host"; } ;;
  status)
    hosts | while read -r name zone status ip; do
      free="-"; [[ "$status" == RUNNING && -n "$ip" ]] && free="$(free_slots "$name" "$ip")"
      echo "$name $zone $status ${ip:--} free slots: $free"
    done
    echo "about \$$(spent 86400) over the last 24 hours (the most: \$$daily), \$$(spent 2592000) over 30 days; disks \$20 a month each" ;;
  cost) echo "about \$$(spent 86400) over the last 24 hours (the most: \$$daily), \$$(spent 2592000) over 30 days" ;;
  down) hosts | while read -r name zone status ip; do
          [[ "$status" == RUNNING ]] && gcloud compute instances stop "$name" --zone "$zone" --project "$project" --async --quiet > /dev/null && echo "stopping $name"
        done; rm -f "$state/host" ;;
  delete) hosts | while read -r name zone status ip; do
            gcloud compute instances delete "$name" --zone "$zone" --project "$project" --quiet > /dev/null && echo "deleted $name"
          done; rm -f "$state/host" ;;
  *) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac

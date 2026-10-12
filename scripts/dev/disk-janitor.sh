#!/usr/bin/env bash
# disk-janitor: keeps a Mac shared by many coding agents from filling its disk.
#
# Every run (launchd starts it every 10 minutes; see install-disk-janitor.sh):
#   - quick check: when more than JANITOR_LOW_GB is free and the last full run
#     was under JANITOR_EVERY_MIN ago, it exits at once;
#   - Cargo build folders: deletes a whole folder that no process uses and
#     nothing wrote for JANITOR_TARGET_IDLE_HOURS; in a folder still in use but
#     not building right now, deletes incremental caches older than a day and
#     dependency outputs older than JANITOR_DEPS_DAYS;
#   - git worktrees of the known repos (not Coder's own chat, task, or lease
#     checkouts under ~/.openagents/{host,worktrees,leases}): removes one only when no process uses
#     it, nothing changed for JANITOR_WORKTREE_IDLE_HOURS, `git status` is
#     empty, and every commit on HEAD is on a remote; then `git worktree prune`;
#   - agent scratch (~/.openagents/scratch): removes a session's folder once its
#     process is gone and nothing changed for JANITOR_SCRATCH_DAYS, unless it
#     holds unsaved git work or is named in the keep file;
#   - exports and traces: past JANITOR_EXPORT_CAP_MB, files older than 7 days;
#   - below JANITOR_LOW_GB free: also old Xcode DerivedData, unavailable
#     simulators, the kache store past 3 days, and core dumps.
#
# Never touched: uncommitted or unpushed work, .git folders, secrets, Coder
# chats (sessions/*.atif.json), ~/.codex, ~/.claude, ~/work/android-apks, and
# anything named in ~/.openagents/disk-janitor.keep (one path per line).
#
# Every removal is a line in ~/.openagents/logs/disk-janitor.log.
#
# Usage: disk-janitor.sh [--now] [--dry-run] [--low]
#   --now      skip the quick check and run in full
#   --dry-run  log what it would remove; remove nothing
#   --low      act as if free space were below JANITOR_LOW_GB
#
# Turn it off: touch ~/.openagents/disk-janitor.off (or run
# scripts/dev/install-disk-janitor.sh --uninstall).

set -uo pipefail

H=${JANITOR_HOME:-$HOME}
OA=$H/.openagents
LOW_GB=${JANITOR_LOW_GB:-150}
EMERGENCY_GB=${JANITOR_EMERGENCY_GB:-40}
EVERY_MIN=${JANITOR_EVERY_MIN:-60}
TARGET_IDLE_HOURS=${JANITOR_TARGET_IDLE_HOURS:-12}
DEPS_DAYS=${JANITOR_DEPS_DAYS:-2}
WORKTREE_IDLE_HOURS=${JANITOR_WORKTREE_IDLE_HOURS:-24}
SCRATCH_DAYS=${JANITOR_SCRATCH_DAYS:-2}
EXPORT_CAP_MB=${JANITOR_EXPORT_CAP_MB:-2048}
LOG=${JANITOR_LOG:-$OA/logs/disk-janitor.log}
KEEP_FILE=${JANITOR_KEEP_FILE:-$OA/disk-janitor.keep}
OFF_FILE=$OA/disk-janitor.off
STAMP=$OA/logs/disk-janitor.last
# Space-separated extra repos whose worktrees are swept.
EXTRA_REPOS=${JANITOR_REPOS:-}
# Test hook: a fixed free-space figure in GB instead of df.
FREE_GB_OVERRIDE=${JANITOR_FREE_GB:-}

DRY=0
NOW=0
FORCE_LOW=0
for arg in "$@"; do
  case $arg in
    --dry-run) DRY=1 ;;
    --now) NOW=1 ;;
    --low) FORCE_LOW=1 ;;
    -h | --help)
      sed -n '2,40p' "$0"
      exit 0
      ;;
    *)
      echo "disk-janitor: unknown argument $arg" >&2
      exit 2
      ;;
  esac
done

mkdir -p "$(dirname "$LOG")" "$OA/locks"
[ -e "$OFF_FILE" ] && exit 0

log() { printf '%s %s\n' "$(date '+%Y-%m-%dT%H:%M:%S')" "$*" >>"$LOG"; }

free_gb() {
  if [ -n "$FREE_GB_OVERRIDE" ]; then
    echo "$FREE_GB_OVERRIDE"
    return
  fi
  df -k "$H" | awk 'NR==2 {printf "%d", $4/1048576}'
}

# One run at a time.
LOCK=$OA/locks/disk-janitor.lock
if ! mkdir "$LOCK" 2>/dev/null; then
  holder=$(cat "$LOCK/pid" 2>/dev/null || true)
  if [ -n "$holder" ] && kill -0 "$holder" 2>/dev/null; then
    exit 0
  fi
  rm -rf "$LOCK"
  mkdir "$LOCK" 2>/dev/null || exit 0
fi
echo $$ >"$LOCK/pid"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/disk-janitor.XXXXXX")
trap 'rm -rf "$LOCK" "$WORK"' EXIT

FREE_START=$(free_gb)
LOW=0
EMERGENCY=0
[ "$FORCE_LOW" = 1 ] || [ "$FREE_START" -lt "$LOW_GB" ] && LOW=1
[ "$FREE_START" -lt "$EMERGENCY_GB" ] && EMERGENCY=1

if [ "$NOW" = 0 ] && [ "$LOW" = 0 ] && [ -e "$STAMP" ] &&
  [ -z "$(find "$STAMP" -mmin +"$EVERY_MIN" 2>/dev/null)" ]; then
  exit 0
fi

if [ "$EMERGENCY" = 1 ]; then
  TARGET_IDLE_HOURS=1
  WORKTREE_IDLE_HOURS=6
  SCRATCH_DAYS=1
elif [ "$LOW" = 1 ]; then
  [ "$TARGET_IDLE_HOURS" -gt 3 ] && TARGET_IDLE_HOURS=3
  [ "$WORKTREE_IDLE_HOURS" -gt 12 ] && WORKTREE_IDLE_HOURS=12
fi

log "start free=${FREE_START}GB low=$LOW emergency=$EMERGENCY dry=$DRY"
FREED_KB=0

# --- what is in use -------------------------------------------------------
# Every process's command line, and every open file / working directory.
ps -axww -o pid=,command= 2>/dev/null | awk -v me=$$ '$1 != me {$1=""; print}' >"$WORK/ps"
if [ -n "${JANITOR_LSOF_FILE:-}" ]; then
  cp "$JANITOR_LSOF_FILE" "$WORK/lsof"
else
  lsof -nP -w -Fn 2>/dev/null | sed -n 's/^n//p' | grep -E "^$H/|^/private/tmp/|^/tmp/" | sort -u >"$WORK/lsof"
fi

# in_use PATH: a process names PATH in its command line, or has a file or
# its working directory under it.
in_use() {
  local p=$1
  grep -qF -- "$p" "$WORK/ps" && return 0
  grep -qE -- "^$(printf '%s' "$p" | sed 's/[][\.*^$+?(){}|]/\\&/g')(/|$)" "$WORK/lsof" && return 0
  return 1
}

# building PATH: a cargo, rustc, or kache process names PATH.
building() {
  grep -F -- "$1" "$WORK/ps" | grep -qE '(^|/| )(cargo|rustc|kache|clippy-driver|rustdoc)( |$)'
}

kept_by_file() {
  local p=$1 line
  [ -f "$KEEP_FILE" ] || return 1
  while IFS= read -r line || [ -n "$line" ]; do
    line=${line%%#*}
    line=$(printf '%s' "$line" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
    [ -z "$line" ] && continue
    line=${line/#\~/$H}
    case $p/ in "$line"/*) return 0 ;; esac
    case $line/ in "$p"/*) return 0 ;; esac
  done <"$KEEP_FILE"
  return 1
}

# Paths that are never removed, whatever else holds.
protected() {
  case $1/ in
    "$H"/ | "$OA"/ | "$H/work"/ | "$H/code"/) return 0 ;;
    "$H/work/.secrets"/* | "$H/.codex"/* | "$H/.claude"/* | "$H/work/android-apks"/*) return 0 ;;
    */sessions/*.atif.json/) return 0 ;;
  esac
  kept_by_file "$1"
}

# recent PATH MINUTES [DEPTH]: something under PATH changed within MINUTES.
recent() {
  [ -n "$(find "$1" -maxdepth "${3:-3}" -mmin -"$2" -print -quit 2>/dev/null)" ]
}

remove() {
  local kind=$1 path=$2 kb
  protected "$path" && {
    log "keep $kind $path (protected)"
    return 1
  }
  kb=$(du -sk "$path" 2>/dev/null | awk '{print $1}')
  kb=${kb:-0}
  if [ "$DRY" = 1 ]; then
    log "would-remove $kind ${kb}KB $path"
  else
    rm -rf "$path" && log "removed $kind ${kb}KB $path"
  fi
  FREED_KB=$((FREED_KB + kb))
}

# --- git safety -----------------------------------------------------------
# git_saved DIR: DIR is a git checkout with nothing uncommitted and no commit
# on HEAD that is missing from every remote.
git_saved() {
  local dir=$1 status ahead
  status=$(GIT_OPTIONAL_LOCKS=0 git -C "$dir" status --porcelain --untracked-files=normal 2>/dev/null) || return 1
  [ -n "$status" ] && return 1
  ahead=$(git -C "$dir" log --oneline -1 HEAD --not --remotes 2>/dev/null) || return 1
  [ -n "$ahead" ] && return 1
  return 0
}

# --- 1. Cargo build folders -----------------------------------------------
target_dirs() {
  local d
  for d in "$H"/work/openagents-target-* "$OA"/coder/target "$OA"/coder-one/target \
    "$OA"/targets/*-slot-[0-9]* "$H"/.cache/openagents/target-* \
    "$H"/work/*/target "$H"/code/*/target; do
    [ -d "$d" ] && [ ! -L "$d" ] || continue
    case $d in *.lock | *.json) continue ;; esac
    # In-tree folders count only when Cargo made them.
    case $d in
      "$H"/work/*/target | "$H"/code/*/target)
        [ -e "$d/CACHEDIR.TAG" ] || [ -d "$d/debug" ] || [ -d "$d/release" ] || continue
        ;;
    esac
    echo "$d"
  done
}

prune_target() {
  local t=$1 profile dir
  for profile in "$t"/*/ "$t"/*/*/; do
    profile=${profile%/}
    [ -d "$profile/deps" ] || [ -d "$profile/incremental" ] || continue
    for dir in "$profile"/incremental "$profile"/incremental.kache-auto; do
      [ -d "$dir" ] || continue
      if [ "$LOW" = 1 ]; then
        remove incremental "$dir"
      else
        while IFS= read -r entry; do
          remove incremental "$entry"
        done < <(find "$dir" -mindepth 1 -maxdepth 1 -mtime +0 2>/dev/null)
      fi
    done
    if [ -d "$profile/deps" ]; then
      local kb
      kb=$(find "$profile/deps" -type f -ctime +"$DEPS_DAYS" -print0 2>/dev/null | xargs -0 du -ck 2>/dev/null | awk 'END {print $1+0}')
      if [ "${kb:-0}" -gt 0 ]; then
        if [ "$DRY" = 1 ]; then
          log "would-remove deps ${kb}KB $profile/deps (older than ${DEPS_DAYS}d)"
        else
          find "$profile/deps" -type f -ctime +"$DEPS_DAYS" -delete 2>/dev/null
          log "removed deps ${kb}KB $profile/deps (older than ${DEPS_DAYS}d)"
        fi
        FREED_KB=$((FREED_KB + kb))
      fi
    fi
    for dir in "$profile"/build "$profile"/.fingerprint; do
      [ -d "$dir" ] || continue
      while IFS= read -r entry; do
        remove build-output "$entry"
      done < <(find "$dir" -mindepth 1 -maxdepth 1 -type d -mtime +"$DEPS_DAYS" 2>/dev/null)
    done
  done
}

while IFS= read -r t; do
  protected "$t" && continue
  if building "$t"; then
    log "keep target $t (building now)"
    continue
  fi
  if ! in_use "$t" && ! recent "$t" $((TARGET_IDLE_HOURS * 60)) 3; then
    remove target "$t"
    continue
  fi
  prune_target "$t"
done < <(target_dirs)

# --- 2. git worktrees -----------------------------------------------------
repos=()
add_repo() {
  local common top r
  common=$(git -C "$1" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || return
  top=$(dirname "$common")
  [ "$(basename "$common")" = ".git" ] || return
  for r in "${repos[@]:-}"; do [ "$r" = "$top" ] && return; done
  repos+=("$top")
}
for r in "$H/work/openagents" "$H/code/openagents" $EXTRA_REPOS; do
  [ -d "$r" ] && add_repo "$r"
done
for d in "$H"/work/* "$H"/code/*; do
  [ -f "$d/.git" ] && add_repo "$d"
done

for repo in "${repos[@]:-}"; do
  [ -n "$repo" ] || continue
  git -C "$repo" worktree list --porcelain 2>/dev/null | awk '
    /^worktree / {if (path != "") print path "\t" flags; path = substr($0, 10); flags = ""}
    /^locked/ {flags = flags "locked "}
    /^prunable/ {flags = flags "prunable "}
    END {if (path != "") print path "\t" flags}
  ' | tail -n +2 >"$WORK/worktrees"
  while IFS=$'\t' read -r wt flags; do
    [ -d "$wt" ] || continue
    case $flags in *locked*) continue ;; esac
    # Checkouts Coder itself owns (chat studios, task worktrees, lease
    # artifacts) are its own disk monitor's to judge: a chat may resume.
    case $wt/ in "$OA"/host/* | "$OA"/worktrees/* | "$OA"/leases/*) continue ;; esac
    protected "$wt" && continue
    in_use "$wt" && continue
    gitdir=$(git -C "$wt" rev-parse --path-format=absolute --git-dir 2>/dev/null) || continue
    mins=$((WORKTREE_IDLE_HOURS * 60))
    recent "$wt" "$mins" 2 && continue
    [ -n "$(find "$gitdir" -maxdepth 1 \( -name HEAD -o -name index \) -mmin -"$mins" 2>/dev/null)" ] && continue
    git_saved "$wt" || {
      log "keep worktree $wt (unsaved work)"
      continue
    }
    kb=$(du -sk "$wt" 2>/dev/null | awk '{print $1}')
    if [ "$DRY" = 1 ]; then
      log "would-remove worktree ${kb:-0}KB $wt"
    else
      git -C "$repo" worktree remove --force "$wt" 2>>"$LOG" && log "removed worktree ${kb:-0}KB $wt"
    fi
    FREED_KB=$((FREED_KB + ${kb:-0}))
  done <"$WORK/worktrees"
  [ "$DRY" = 1 ] || git -C "$repo" worktree prune 2>/dev/null
done

# --- 3. agent scratch -----------------------------------------------------
scratch_saved() {
  # Every git checkout inside the folder must be saved; a linked worktree
  # is removed through its repo so the repo forgets it.
  local d=$1 marker wt common
  while IFS= read -r marker; do
    wt=$(dirname "$marker")
    git_saved "$wt" || return 1
  done < <(find "$d" -maxdepth 4 -name .git 2>/dev/null)
  [ "$DRY" = 1 ] && return 0
  while IFS= read -r marker; do
    wt=$(dirname "$marker")
    common=$(git -C "$wt" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || continue
    [ "$(basename "$common")" = ".git" ] || continue
    git -C "$(dirname "$common")" worktree remove --force "$wt" 2>/dev/null
  done < <(find "$d" -maxdepth 4 -name .git -type f 2>/dev/null)
  return 0
}

EMPTY_SCRATCH=0
if [ -d "$OA/scratch" ]; then
  # Every Coder process makes a `<name>-<pid>` folder holding only its
  # `.session` marker; tens of thousands pile up. One pass finds those whose
  # process is gone and that are over an hour old; they go uncounted in size.
  find "$OA/scratch" -mindepth 1 -maxdepth 1 -type d -name '*-[0-9]*' -mmin +60 | sort >"$WORK/marker-all"
  find "$OA/scratch" -mindepth 2 -maxdepth 2 ! -name .session 2>/dev/null | sed 's#/[^/]*$##' | sort -u >"$WORK/marker-busy"
  comm -23 "$WORK/marker-all" "$WORK/marker-busy" >"$WORK/marker-only"
  while IFS= read -r s; do
    pid=${s##*-}
    case $pid in '' | *[!0-9]*) continue ;; esac
    kill -0 "$pid" 2>/dev/null && continue
    protected "$s" && continue
    [ "$DRY" = 1 ] || rm -rf "$s"
    EMPTY_SCRATCH=$((EMPTY_SCRATCH + 1))
  done <"$WORK/marker-only"
  find "$OA/scratch" -mindepth 1 -maxdepth 1 >"$WORK/scratch"
  while IFS= read -r s; do
    protected "$s" && continue
    name=$(basename "$s")
    pid=${name##*-}
    case $pid in '' | *[!0-9]*) pid= ;; esac
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then continue; fi
    # Cheapest checks first: there can be tens of thousands of these.
    [ -n "$(find "$s" -mtime -"$SCRATCH_DAYS" -print -quit 2>/dev/null)" ] && continue
    in_use "$s" && continue
    if [ -d "$s" ] && ! scratch_saved "$s"; then
      log "keep scratch $s (unsaved git work)"
      continue
    fi
    remove scratch "$s"
  done <"$WORK/scratch"
  [ "$EMPTY_SCRATCH" -gt 0 ] && log "removed scratch-markers $EMPTY_SCRATCH empty session folders"
fi

# --- 4. exports and traces ------------------------------------------------
for dir in "$OA/exports" "$OA/traces"; do
  [ -d "$dir" ] || continue
  mb=$(du -sm "$dir" 2>/dev/null | awk '{print $1}')
  [ "${mb:-0}" -gt "$EXPORT_CAP_MB" ] || continue
  while IFS= read -r f; do
    remove export "$f"
  done < <(find "$dir" -type f -mtime +7 ! -name '*.atif.json' 2>/dev/null)
done

# --- 5. caches, only when low ---------------------------------------------
if [ "$LOW" = 1 ]; then
  dd=$H/Library/Developer/Xcode/DerivedData
  if [ -d "$dd" ]; then
    while IFS= read -r d; do
      in_use "$d" || remove derived-data "$d"
    done < <(find "$dd" -mindepth 1 -maxdepth 1 -type d -mtime +3 2>/dev/null)
  fi
  if [ "$DRY" = 0 ] && command -v xcrun >/dev/null 2>&1 && [ -z "$FREE_GB_OVERRIDE" ]; then
    xcrun simctl delete unavailable >/dev/null 2>&1 && log "ran simctl delete unavailable"
  fi
  if [ "$DRY" = 0 ] && command -v kache >/dev/null 2>&1 && [ -z "$FREE_GB_OVERRIDE" ]; then
    kache gc --max-age 3d >/dev/null 2>&1 && log "ran kache gc --max-age 3d"
  fi
  for f in /cores/core.*; do
    [ -f "$f" ] && [ -O "$f" ] && remove core-dump "$f"
  done
fi

[ "$DRY" = 1 ] || touch "$STAMP"
log "done freed~$((FREED_KB / 1048576))GB free=$(free_gb)GB"
exit 0

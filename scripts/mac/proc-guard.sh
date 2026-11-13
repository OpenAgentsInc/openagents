#!/bin/bash
# Firm reason: 5 (launchd runs this every 30 seconds while the Mac may be near
# its process limit: it must start without building anything and fork almost
# nothing, so it is a few lines of sh over ps and awk, like fd-guard.sh.)
#
# Keeps a runaway from filling the Mac with processes. On 2026-11-13 a check
# that relaunched itself made 347 copies and about 590 headless Chromes; the
# Mac reached 3,375 processes and a load of 250 and was unusable
# (docs/audits/2026-11-13-build-hosts.md, #13472).
#
# Each run counts this user's processes by program (the executable's name):
# 1. A program with more copies than `copies` is a runaway. Its count and the
#    process groups that hold the most of it (with each group leader's
#    command) go to ~/.openagents/logs/proc-guard/guard.log.
# 2. A runaway's processes whose command line matches a `stop=` pattern of the
#    settings are stopped: TERM, then KILL 3 s later for any left. Nothing
#    else is ever stopped; with no `stop=` line the guard only logs.
# 3. Above `warn_total` processes (all users), the 25 biggest programs and
#    groups go to a snapshot file (once every 5 minutes) and a notification
#    names the biggest.
#
# Settings: ~/.openagents/proc-guard.conf (written with the defaults at
# install, never overwritten), one key=value a line, # for comments:
#   warn_total=2000    copies=150    stop=EXTENDED-REGEX (repeatable)
#
# Install (opt-in): scripts/mac/proc-guard.sh --install
# Remove:           launchctl bootout gui/$(id -u)/com.openagents.proc-guard
set -u
logs="${PROC_GUARD_LOGS:-$HOME/.openagents/logs/proc-guard}"
conf="${PROC_GUARD_CONF:-$HOME/.openagents/proc-guard.conf}"
label=com.openagents.proc-guard

default_conf() {
  cat <<'EOF'
# proc-guard settings (scripts/mac/proc-guard.sh). One key=value a line.
# Above this many processes (all users): a snapshot of the biggest and a notification.
warn_total=2000
# A program with more copies than this (this user's) is a runaway: logged with its groups.
copies=150
# A runaway's processes whose command line matches one of these extended regexes are
# stopped (TERM, then KILL). Nothing else is stopped. Delete every stop= line to only log.
# Headless Chrome that our CDP tools start (never the owner's own Chrome):
stop=--headless=new.*--user-data-dir=
EOF
}

if [[ "${1:-}" == --install ]]; then
  mkdir -p "$HOME/.openagents/bin" "$logs"
  [[ -f "$conf" ]] || default_conf > "$conf"
  cp "$0" "$HOME/.openagents/bin/proc-guard.sh" && chmod 755 "$HOME/.openagents/bin/proc-guard.sh"
  plist="$HOME/Library/LaunchAgents/$label.plist"
  cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key><array><string>$HOME/.openagents/bin/proc-guard.sh</string></array>
  <key>StartInterval</key><integer>30</integer>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>StandardErrorPath</key><string>$logs/launchd.log</string>
</dict></plist>
EOF
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null
  launchctl bootstrap "gui/$(id -u)" "$plist" && echo "proc-guard: installed, runs every 30 s; settings $conf; log $logs/guard.log"
  exit $?
fi

mkdir -p "$logs"
now() { date '+%F %T'; }
warn_total=2000; copies=150; stops=()
if [[ -f "$conf" ]]; then
  while IFS= read -r line; do
    case "$line" in
      warn_total=*) warn_total="${line#*=}" ;;
      copies=*) copies="${line#*=}" ;;
      stop=?*) stops+=("${line#*=}") ;;
    esac
  done < "$conf"
fi

uid=$(id -u)
total=$(ps -ax -o pid= | wc -l | tr -d ' ')
# "pid pgid name", this user's processes; the name is the executable's file name.
mine="$(ps -U "$uid" -o pid=,pgid=,comm= | awk '{ p = $1; g = $2; $1 = ""; $2 = ""; sub(/^ +/, ""); n = split($0, a, "/"); print p "\t" g "\t" a[n] }')"
runaways="$(awk -F'\t' -v c="$copies" '{ n[$3]++ } END { for (k in n) if (n[k] > c) print n[k] "\t" k }' <<< "$mine" | sort -rn)"

stopped=()
while IFS=$'\t' read -r count name; do
  [[ -n "$name" ]] || continue
  groups="$(awk -F'\t' -v k="$name" '$3 == k { g[$2]++ } END { for (x in g) print g[x], x }' <<< "$mine" | sort -rn | head -5 |
    while read -r n g; do printf '%s in group %s (%s); ' "$n" "$g" "$(ps -o command= -p "$g" 2>/dev/null | cut -c1-160)"; done)"
  echo "$(now) processes $total: runaway: $count copies of '$name' (more than $copies): $groups" >> "$logs/guard.log"
  (( ${#stops[@]} )) || continue
  pids=()
  while read -r pid; do
    cmd="$(ps -o command= -p "$pid" 2>/dev/null)" || continue
    for re in "${stops[@]}"; do
      if [[ "$cmd" =~ $re ]]; then pids+=("$pid"); break; fi
    done
  done < <(awk -F'\t' -v k="$name" '$3 == k { print $1 }' <<< "$mine")
  (( ${#pids[@]} )) || { echo "$(now) runaway '$name': no copy matches a stop= pattern; not stopped" >> "$logs/guard.log"; continue; }
  kill -TERM "${pids[@]}" 2>/dev/null
  stopped+=("${pids[@]}")
  echo "$(now) runaway '$name': stopped ${#pids[@]} matching a stop= pattern (TERM)" >> "$logs/guard.log"
done <<< "$runaways"
if (( ${#stopped[@]} )); then
  sleep "${PROC_GUARD_KILL_AFTER:-3}"
  left=(); for p in "${stopped[@]}"; do kill -0 "$p" 2>/dev/null && left+=("$p"); done
  (( ${#left[@]} == 0 )) || { kill -KILL "${left[@]}" 2>/dev/null; echo "$(now) KILL for ${#left[@]} still running after TERM" >> "$logs/guard.log"; }
fi

if (( total > warn_total )); then
  stamp="$logs/.last-snapshot"
  if [[ ! -e "$stamp" ]] || (( $(date +%s) - $(stat -f %m "$stamp") >= 300 )); then
    touch "$stamp"
    out="$logs/$(date +%Y%m%d-%H%M%S).txt"
    { echo "$(now) processes $total (warn_total $warn_total)"; echo "copies program (all users)"
      ps -ax -o comm= | awk '{ n = split($0, a, "/"); c[a[n]]++ } END { for (k in c) print c[k], k }' | sort -rn | head -25
      echo "processes group: leader's command"
      ps -ax -o pgid= | sort | uniq -c | sort -rn | head -25 | while read -r n g; do echo "$n $g: $(ps -o command= -p "$g" 2>/dev/null | cut -c1-160)"; done
    } > "$out"
    echo "$(now) processes $total (more than $warn_total): biggest in $out" >> "$logs/guard.log"
    if [[ "${PROC_GUARD_NOTIFY:-1}" == 1 ]]; then
      top="$(ps -ax -o comm= | awk '{ n = split($0, a, "/"); c[a[n]]++ } END { for (k in c) print c[k] " " k }' | sort -rn | head -1)"
      osascript -e "display notification \"$total processes; the most: $top. Details: ~/.openagents/logs/proc-guard\" with title \"Mac process count high\"" 2>/dev/null
    fi
  fi
fi
exit 0

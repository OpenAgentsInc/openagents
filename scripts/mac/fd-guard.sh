#!/bin/bash
# Firm reason: 5 (launchd runs this every 30 seconds while the system file
# table may be nearly full: it must start without building anything and open
# almost nothing, so it is a few lines of sh over ps, sysctl and lsof.)
#
# Keeps the Mac's system-wide open-file table (kern.maxfiles) from filling.
# On 2026-11-07 it filled at 12:11 CST and every new process failed with
# ENFILE until a hard reboot (docs/postmortems/2026-11-07-mac-file-table.md).
#
# Each run:
# 1. Kills orphaned headless Chromes: the main process of a Chrome that our
#    CDP tools started (--headless=new with its own --user-data-dir) whose
#    parent has died (parent pid 1). Its helpers exit with it. A tool that is
#    killed before cdp_close leaves one behind, holding ~265 files.
# 2. At 50% full or more, writes the 25 processes holding the most files to
#    ~/.openagents/logs/fd-guard/ (once every 5 minutes).
# 3. At 80% full or more, shows a notification.
#
# Install: scripts/mac/fd-guard.sh --install
set -u
logs="$HOME/.openagents/logs/fd-guard"
label=com.openagents.fd-guard

if [[ "${1:-}" == --install ]]; then
  mkdir -p "$HOME/.openagents/bin" "$logs"
  cp "$0" "$HOME/.openagents/bin/fd-guard.sh" && chmod 755 "$HOME/.openagents/bin/fd-guard.sh"
  plist="$HOME/Library/LaunchAgents/$label.plist"
  cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key><array><string>$HOME/.openagents/bin/fd-guard.sh</string></array>
  <key>StartInterval</key><integer>30</integer>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>StandardErrorPath</key><string>$logs/launchd.log</string>
</dict></plist>
EOF
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null
  launchctl bootstrap "gui/$(id -u)" "$plist" && echo "fd-guard: installed, runs every 30 s; log $logs/guard.log"
  exit $?
fi

mkdir -p "$logs"
n=$(sysctl -n kern.num_files); max=$(sysctl -n kern.maxfiles)
pct=$(( n * 100 / max ))
now() { date '+%F %T'; }

reaped=""
while read -r pid ppid; do
  [[ "$ppid" == 1 ]] || continue
  kill -KILL "$pid" 2>/dev/null && reaped="$reaped $pid"
done < <(ps -axo pid=,ppid=,command= | awk '/--headless=new/ && /--user-data-dir=/ && !/--type=/ {print $1, $2}')
[[ -n "$reaped" ]] && echo "$(now) files $n/$max: killed orphaned headless Chrome$reaped" >> "$logs/guard.log"

if (( pct >= 50 )); then
  stamp="$logs/.last-snapshot"
  if [[ ! -e "$stamp" ]] || (( $(date +%s) - $(stat -f %m "$stamp") >= 300 )); then
    touch "$stamp"
    out="$logs/$(date +%Y%m%d-%H%M%S).txt"
    { echo "$(now) files $n of $max ($pct%)"; echo "files pid command";
      lsof -nP -w 2>/dev/null | awk 'NR > 1 { c[$2 " " $1]++ } END { for (k in c) print c[k], k }' | sort -rn | head -25; } > "$out"
    echo "$(now) files $n/$max ($pct%): holders in $out" >> "$logs/guard.log"
  fi
fi

if (( pct >= 80 )); then
  osascript -e "display notification \"$pct% of the open-file table is in use ($n of $max). Holders: ~/.openagents/logs/fd-guard\" with title \"Mac file table nearly full\"" 2>/dev/null
fi
exit 0

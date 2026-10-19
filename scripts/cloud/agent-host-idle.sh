#!/bin/sh
# Installed as /usr/local/bin/oa-agent-host-idle on the agent host.
set -eu

state=/run/oa-agent-host
install -d "$state"
busy=$state/busy
for p in cargo rustc claude codex grok devin microcoder node python3; do
    if pgrep -u coder -x "$p" >/dev/null; then
        touch "$busy"
        exit 0
    fi
done
if pgrep -f '^sshd(-session)?: coder@' >/dev/null ||
    [ -e /home/coder/.openagents/keep-awake ] || [ ! -e "$busy" ]; then
    touch "$busy"
    exit 0
fi
idle=$(curl -sf -m 2 -H Metadata-Flavor:Google http://metadata.google.internal/computeMetadata/v1/instance/attributes/oa-agent-host-idle-minutes || echo 60)
case "$idle" in ''|*[!0-9]*) idle=60 ;; esac
[ "$idle" -gt 0 ] || exit 0
age=$(( $(date +%s) - $(stat -c %Y "$busy") ))
[ "$age" -ge "$((idle * 60))" ] || exit 0
logger -t oa-agent-host "OA_AGENT_HOST_IDLE_STOP idle_seconds=$age limit_minutes=$idle"
systemctl poweroff

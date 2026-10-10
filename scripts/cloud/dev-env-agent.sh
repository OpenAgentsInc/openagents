#!/usr/bin/env bash
# The cloud development environment's own agent (#11227,
# docs/cloud/dogfood-dev-on-prod.md gaps 12 and 15):
#
#   sudo scripts/cloud/dev-env-agent.sh install   # once per environment; survives reboots
#   scripts/cloud/dev-env-agent.sh check          # busy or idle, and why
#
# install sets up two systemd units:
#
# - oa-dev-env-idle.timer: every minute, is anything running? A job (cargo,
#   rustc, claude, codex, microcoder, a Coder run, the landing queue's busy
#   marker), an ssh session running a shell or command, or
#   ~/.openagents/keep-awake. After
#   `oa-dev-env-idle-minutes` (instance metadata, default 30; 0 never stops)
#   with none of them, the VM powers itself off: a stopped GCE instance
#   bills only its disk. Starting it again (`gcloud compute instances start`,
#   or `openagents land submit` when the integrator is down) brings
#   everything back: the session file and login lines are on the disk, and
#   the integrator restarts by itself.
# - oa-land-worker.service: the landing queue's integrator
#   (`openagents land work`) as user coder in ~/openagents, signed in by
#   dev-env-session.sh at each start. It uses ~/.openagents/bin/openagents
#   when present (a build from main), else the image's.
#
# Set the idle time from anywhere:
#   gcloud compute instances add-metadata oa-dev-env-1 --zone us-central1-b \
#     --metadata oa-dev-env-idle-minutes=60
set -euo pipefail

user=${OA_DEV_ENV_USER:-coder}
home=$(getent passwd "$user" | cut -d: -f6)
home=${home:-/home/$user}

check() {
  local why=()
  local p
  for p in cargo rustc claude codex microcoder; do
    pgrep -x "$p" >/dev/null 2>&1 && why+=("job: $p")
  done
  pgrep -f 'openagents .*(chat work|task execute|cloud up)' >/dev/null 2>&1 && why+=("job: a Coder run")
  local marker=$home/.openagents/land-queue/busy
  if [[ -f $marker ]] && kill -0 "$(cat "$marker" 2>/dev/null)" 2>/dev/null; then
    why+=("job: the landing queue is landing an entry")
  fi
  # An ssh session counts while it runs something (a shell or a command).
  # IAP leaves dead connections open with nothing under them; those do not
  # count, and sshd's keepalive (installed below) reaps them.
  local ssh=0 pid
  for pid in $(pgrep -f '^sshd(-session)?: [^ ]+@' 2>/dev/null); do
    pgrep -P "$pid" >/dev/null 2>&1 && ssh=$((ssh + 1))
  done
  ((ssh > 0)) && why+=("ssh: $ssh session(s)")
  [[ -e $home/.openagents/keep-awake ]] && why+=("keep-awake: $home/.openagents/keep-awake")
  if ((${#why[@]})); then
    printf 'busy: %s\n' "${why[@]}"
    return 0
  fi
  echo "idle"
  return 1
}

if [[ ${1:-} == check ]]; then
  check || true
  exit 0
fi

if [[ ${1:-} == tick ]]; then
  # Run by the timer as root.
  md() { curl -sf -m 2 -H Metadata-Flavor:Google "http://metadata.google.internal/computeMetadata/v1/$1"; }
  state=/var/lib/oa-dev-env
  install -d "$state"
  if check >"$state/last" 2>&1; then
    touch "$state/busy"
    exit 0
  fi
  idle=$(md instance/attributes/oa-dev-env-idle-minutes || echo 30)
  case "$idle" in ''|*[!0-9]*) idle=30 ;; esac
  ((idle > 0)) || exit 0
  now=$(date +%s)
  since=$(stat -c %Y "$state/busy" 2>/dev/null || echo 0)
  booted=$(( now - $(cut -d. -f1 /proc/uptime) ))
  ((since > booted)) || since=$booted
  age=$(( now - since ))
  ((age >= idle * 60)) || exit 0
  msg="OA_DEV_ENV_IDLE_STOP idle_seconds=$age limit_minutes=$idle"
  echo "$msg" >/dev/ttyS0 2>/dev/null || true
  logger -t oa-dev-env "$msg"
  systemctl poweroff
  exit 0
fi

if [[ ${1:-} != install ]]; then
  sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//' >&2
  exit 2
fi

[[ $(id -u) == 0 ]] || { echo "install needs root: sudo $0 install" >&2; exit 1; }
here=$(cd "$(dirname "$0")" && pwd)
install -m 755 "$here/dev-env-agent.sh" /usr/local/bin/oa-dev-env-agent

cat >/etc/systemd/system/oa-dev-env-idle.service <<UNIT
[Unit]
Description=OpenAgents dev environment: power off when idle
[Service]
Type=oneshot
Environment=OA_DEV_ENV_USER=$user
ExecStart=/usr/local/bin/oa-dev-env-agent tick
UNIT
cat >/etc/systemd/system/oa-dev-env-idle.timer <<'UNIT'
[Unit]
Description=OpenAgents dev environment: idle check every minute
[Timer]
OnBootSec=60
OnUnitActiveSec=60
[Install]
WantedBy=timers.target
UNIT
cat >/etc/systemd/system/oa-land-worker.service <<UNIT
[Unit]
Description=OpenAgents landing queue integrator (openagents land work)
After=network-online.target
Wants=network-online.target
[Service]
User=$user
WorkingDirectory=$home/openagents
Environment=PATH=$home/.cargo/bin:/usr/local/bin:/usr/bin:/bin
ExecStartPre=/bin/bash -lc 'scripts/cloud/dev-env-session.sh >/dev/null'
ExecStart=/bin/bash -lc 'export PATH=\$HOME/.cargo/bin:\$PATH; set -a; . \$HOME/.openagents/dev-env.env; set +a; bin=\$HOME/.openagents/bin/openagents; [ -x "\$bin" ] || bin=openagents; exec "\$bin" land work'
Restart=always
RestartSec=30
[Install]
WantedBy=multi-user.target
UNIT
# Reap ssh connections whose client is gone (IAP keeps them open).
install -d /etc/ssh/sshd_config.d
printf 'ClientAliveInterval 60\nClientAliveCountMax 3\n' >/etc/ssh/sshd_config.d/oa-dev-env.conf
systemctl reload ssh 2>/dev/null || systemctl reload sshd 2>/dev/null || true
systemctl daemon-reload
systemctl enable --now oa-dev-env-idle.timer
systemctl enable --now oa-land-worker.service
echo "installed: oa-dev-env-idle.timer, oa-land-worker.service"

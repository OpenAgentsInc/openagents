#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: install-cloud-node.sh [--dry-run]

Installs the source Pylon v0.3 RC as a headless systemd service on a Linux VM.

Environment:
  PYLON_INSTALL_DIR                  default: /opt/openagents-pylon
  PYLON_HOME                         default: /var/lib/openagents-pylon
  PYLON_SERVICE_USER                 default: pylon
  PYLON_SERVICE_NAME                 default: openagents-pylon
  PYLON_OPENAGENTS_BASE_URL          default: https://openagents.com
  PYLON_CONTROL_PORT                 default: 4716
  PYLON_RESOURCE_MODE                default: background_20
  PYLON_DISPLAY_NAME                 default: Cloud Pylon
  PYLON_REF                          optional stable pylon ref
  PYLON_HOME_ARCHIVE                 optional tar.gz of an isolated PYLON_HOME to restore
  OPENAGENTS_AGENT_TOKEN             optional owner-granted agent token
  ANTHROPIC_API_KEY                  optional BYOK Claude Agent credential
  PYLON_ENABLE_ASSIGNMENT_WORKER     set 0 to install node without worker loop
  PYLON_ENABLE_CODEX_SUPERVISOR      set 1 to install the Codex own-capacity supervisor
  PYLON_ENABLE_CLAUDE_SUPERVISOR     set 1 to install the Claude own-capacity supervisor
  SUP_PYLON_REF                      optional target Pylon ref for supervisors
  SUP_MAX_SLOTS                      optional supervisor slot ceiling
  SUP_PER_ACCOUNT                    optional per-account parallelism
USAGE
}

dry_run=0
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
elif [[ "${1:-}" == "--dry-run" ]]; then
  dry_run=1
elif [[ $# -gt 0 ]]; then
  usage >&2
  exit 2
fi

run() {
  if [[ "$dry_run" == "1" ]]; then
    printf '+ %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

write_file() {
  local path="$1"
  if [[ "$dry_run" == "1" ]]; then
    echo "+ write ${path}"
    cat
  else
    install -m "${3:-0644}" /dev/stdin "$path"
  fi
}

if [[ "$dry_run" != "1" && "$(uname -s)" != "Linux" ]]; then
  echo "cloud Pylon systemd install requires Linux" >&2
  exit 1
fi

if [[ "$dry_run" != "1" && "${EUID}" -ne 0 ]]; then
  echo "run as root, for example: sudo -E $0" >&2
  exit 1
fi

install_dir="${PYLON_INSTALL_DIR:-/opt/openagents-pylon}"
pylon_home="${PYLON_HOME:-/var/lib/openagents-pylon}"
service_user="${PYLON_SERVICE_USER:-pylon}"
service_name="${PYLON_SERVICE_NAME:-openagents-pylon}"
base_url="${PYLON_OPENAGENTS_BASE_URL:-https://openagents.com}"
control_port="${PYLON_CONTROL_PORT:-4716}"
resource_mode="${PYLON_RESOURCE_MODE:-background_20}"
display_name="${PYLON_DISPLAY_NAME:-Cloud Pylon}"
assignment_worker="${PYLON_ENABLE_ASSIGNMENT_WORKER:-1}"
codex_supervisor="${PYLON_ENABLE_CODEX_SUPERVISOR:-0}"
claude_supervisor="${PYLON_ENABLE_CLAUDE_SUPERVISOR:-0}"
env_file="/etc/${service_name}.env"
unit_file="/etc/systemd/system/${service_name}.service"

if ! id -u "$service_user" >/dev/null 2>&1; then
  run useradd --system --create-home --home-dir "$pylon_home" --shell /usr/sbin/nologin "$service_user"
fi
run mkdir -p "$install_dir" "$pylon_home"
if [[ -n "${PYLON_HOME_ARCHIVE:-}" ]]; then
  if [[ "$dry_run" != "1" && ! -f "$PYLON_HOME_ARCHIVE" ]]; then
    echo "PYLON_HOME_ARCHIVE not found: $PYLON_HOME_ARCHIVE" >&2
    exit 2
  fi
  run tar -xzf "$PYLON_HOME_ARCHIVE" -C "$pylon_home" --strip-components=1 --no-same-owner
fi
run chown -R "$service_user:$service_user" "$pylon_home"

if [[ -d "${install_dir}/.git" ]]; then
  run git -C "$install_dir" fetch --depth 1 origin main
  run git -C "$install_dir" reset --hard origin/main
else
  run git clone --depth 1 https://github.com/OpenAgentsInc/openagents "$install_dir"
fi

run chown -R "$service_user:$service_user" "$install_dir"
run sudo -u "$service_user" env HOME="$pylon_home" PYLON_HOME="$pylon_home" bun install --cwd "$install_dir"

bootstrap_args=(
  bootstrap
  --register-openagents
  --resource-mode "$resource_mode"
  --display-name "$display_name"
  --capability-ref capability.pylon.assignment_ready
  --capability-ref capability.pylon.local_claude_agent
  --capability-ref capability.pylon.local_codex
  --json
)
if [[ -n "${PYLON_REF:-}" ]]; then
  bootstrap_args+=(--pylon-ref "$PYLON_REF")
fi
run sudo -u "$service_user" env HOME="$pylon_home" PYLON_HOME="$pylon_home" bun "$install_dir/apps/pylon/src/index.ts" "${bootstrap_args[@]}"

{
  echo "PYLON_HOME=${pylon_home}"
  echo "PYLON_OPENAGENTS_BASE_URL=${base_url}"
  echo "PYLON_CONTROL_PORT=${control_port}"
  echo "PYLON_ASSIGNMENT_WORKER=${assignment_worker}"
  echo "PYLON_ASSIGNMENT_WORKER_INTERVAL_SECONDS=${PYLON_ASSIGNMENT_WORKER_INTERVAL_SECONDS:-30}"
  echo "PYLON_DISABLE_DAEMON_ROUTING=1"
  if [[ -n "${SUP_PYLON_REF:-}" ]]; then
    echo "SUP_PYLON_REF=${SUP_PYLON_REF}"
  elif [[ -n "${PYLON_REF:-}" ]]; then
    echo "SUP_PYLON_REF=${PYLON_REF}"
  fi
  if [[ -n "${SUP_MAX_SLOTS:-}" ]]; then
    echo "SUP_MAX_SLOTS=${SUP_MAX_SLOTS}"
  fi
  if [[ -n "${SUP_PER_ACCOUNT:-}" ]]; then
    echo "SUP_PER_ACCOUNT=${SUP_PER_ACCOUNT}"
  fi
  if [[ -n "${OPENAGENTS_AGENT_TOKEN:-}" ]]; then
    if [[ "$dry_run" == "1" ]]; then
      echo "OPENAGENTS_AGENT_TOKEN=<redacted>"
    else
      echo "OPENAGENTS_AGENT_TOKEN=${OPENAGENTS_AGENT_TOKEN}"
    fi
  fi
  if [[ -n "${ANTHROPIC_API_KEY:-}" ]]; then
    if [[ "$dry_run" == "1" ]]; then
      echo "ANTHROPIC_API_KEY=<redacted>"
    else
      echo "ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}"
    fi
  fi
} | write_file "$env_file" root 0600

cat <<UNIT | write_file "$unit_file" root 0644
[Unit]
Description=OpenAgents Pylon headless node
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${service_user}
WorkingDirectory=${install_dir}
EnvironmentFile=${env_file}
ExecStart=/usr/bin/env bun ${install_dir}/apps/pylon/src/index.ts node
Restart=always
RestartSec=10
KillSignal=SIGTERM

[Install]
WantedBy=multi-user.target
UNIT

write_supervisor_unit() {
  local kind="$1"
  local script_path="$install_dir/apps/pylon/scripts/${kind}-supervisor/${kind}-supervisor.sh"
  local supervisor_service="openagents-${kind}-supervisor"
  local supervisor_unit="/etc/systemd/system/${supervisor_service}.service"
  local state_dir="%h/.${kind}-supervisor"

  cat <<UNIT | write_file "$supervisor_unit" root 0644
[Unit]
Description=OpenAgents ${kind} own-capacity supervisor
After=network-online.target ${service_name}.service
Wants=network-online.target

[Service]
Type=simple
User=${service_user}
WorkingDirectory=${install_dir}
EnvironmentFile=${env_file}
Environment=PYLON_HOME=${pylon_home}
Environment=SUP_STATE_DIR=${state_dir}
Environment=SUP_LOG=${state_dir}/supervisor.log
ExecStart=/usr/bin/env bash ${script_path}
Restart=always
RestartSec=10
KillSignal=SIGTERM

[Install]
WantedBy=multi-user.target
UNIT
}

if [[ "$codex_supervisor" == "1" ]]; then
  write_supervisor_unit codex
fi
if [[ "$claude_supervisor" == "1" ]]; then
  write_supervisor_unit claude
fi

run systemctl daemon-reload
run systemctl enable --now "$service_name"
if [[ "$codex_supervisor" == "1" ]]; then
  run systemctl enable --now openagents-codex-supervisor
fi
if [[ "$claude_supervisor" == "1" ]]; then
  run systemctl enable --now openagents-claude-supervisor
fi

cat <<EOF
Installed ${service_name}.

Next checks:
  systemctl status ${service_name} --no-pager
  systemctl status openagents-codex-supervisor --no-pager
  systemctl status openagents-claude-supervisor --no-pager
  journalctl -u ${service_name} -f
  sudo -u ${service_user} env PYLON_HOME=${pylon_home} PYLON_OPENAGENTS_BASE_URL=${base_url} bun ${install_dir}/apps/pylon/src/index.ts presence heartbeat --base-url ${base_url}
  sudo -u ${service_user} env PYLON_HOME=${pylon_home} bun ${install_dir}/apps/pylon/src/index.ts status --json
EOF

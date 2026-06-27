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
  PYLON_ACCOUNT_ARCHIVE              optional .tar.gz of an isolated PYLON_HOME account tree
  OPENAGENTS_AGENT_TOKEN             optional owner-granted agent token
  ANTHROPIC_API_KEY                  optional BYOK Claude Agent credential
  PYLON_ENABLE_ASSIGNMENT_WORKER     set 0 to install node without worker loop
  PYLON_ENABLE_CODEX_SUPERVISOR      set 1 to install the Codex own-capacity supervisor service
  PYLON_ENABLE_CLAUDE_SUPERVISOR     set 1 to install the Claude own-capacity supervisor service
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
account_archive="${PYLON_ACCOUNT_ARCHIVE:-}"
codex_supervisor="${PYLON_ENABLE_CODEX_SUPERVISOR:-0}"
claude_supervisor="${PYLON_ENABLE_CLAUDE_SUPERVISOR:-0}"
env_file="/etc/${service_name}.env"
unit_file="/etc/systemd/system/${service_name}.service"

restore_account_archive() {
  local archive="$1"
  if [[ -z "$archive" ]]; then
    return
  fi
  if [[ "$dry_run" == "1" ]]; then
    echo "+ validate account archive ${archive}"
    echo "+ tar -xzf ${archive} -C ${pylon_home}"
    return
  fi
  if [[ ! -f "$archive" ]]; then
    echo "account archive not found: $archive" >&2
    exit 2
  fi
  if tar -tzf "$archive" | grep -E '(^/|(^|/)\.\.(/|$))' >/dev/null; then
    echo "account archive contains unsafe absolute or parent paths" >&2
    exit 2
  fi
  run tar -xzf "$archive" -C "$pylon_home"
  run chown -R "$service_user:$service_user" "$pylon_home"
}

write_supervisor_unit() {
  local lane="$1"
  local supervisor_script="$2"
  local supervisor_service="${service_name}-${lane}-supervisor"
  local supervisor_unit="/etc/systemd/system/${supervisor_service}.service"
  cat <<UNIT | write_file "$supervisor_unit" root 0644
[Unit]
Description=OpenAgents Pylon ${lane} own-capacity supervisor
After=network-online.target ${service_name}.service
Wants=network-online.target ${service_name}.service

[Service]
Type=simple
User=${service_user}
WorkingDirectory=${install_dir}
EnvironmentFile=${env_file}
Environment=SUP_PYLON_REF=${PYLON_REF:-}
ExecStart=/usr/bin/env bash ${install_dir}/${supervisor_script}
Restart=always
RestartSec=15
KillSignal=SIGTERM

[Install]
WantedBy=multi-user.target
UNIT
}

if ! id -u "$service_user" >/dev/null 2>&1; then
  run useradd --system --create-home --home-dir "$pylon_home" --shell /usr/sbin/nologin "$service_user"
fi
run mkdir -p "$install_dir" "$pylon_home"
run chown -R "$service_user:$service_user" "$pylon_home"
restore_account_archive "$account_archive"

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
  echo "SUP_PYLON_REF=${PYLON_REF:-}"
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

if [[ "$codex_supervisor" == "1" ]]; then
  write_supervisor_unit codex apps/pylon/scripts/codex-supervisor/codex-supervisor.sh
fi
if [[ "$claude_supervisor" == "1" ]]; then
  write_supervisor_unit claude apps/pylon/scripts/claude-supervisor/claude-supervisor.sh
fi

run systemctl daemon-reload
run systemctl enable --now "$service_name"
if [[ "$codex_supervisor" == "1" ]]; then
  run systemctl enable --now "${service_name}-codex-supervisor"
fi
if [[ "$claude_supervisor" == "1" ]]; then
  run systemctl enable --now "${service_name}-claude-supervisor"
fi

cat <<EOF
Installed ${service_name}.

Next checks:
  systemctl status ${service_name} --no-pager
  journalctl -u ${service_name} -f
  sudo -u ${service_user} env PYLON_HOME=${pylon_home} PYLON_OPENAGENTS_BASE_URL=${base_url} bun ${install_dir}/apps/pylon/src/index.ts presence heartbeat --base-url ${base_url}
  sudo -u ${service_user} env PYLON_HOME=${pylon_home} bun ${install_dir}/apps/pylon/src/index.ts status --json
EOF
if [[ "$codex_supervisor" == "1" ]]; then
  echo "  systemctl status ${service_name}-codex-supervisor --no-pager"
fi
if [[ "$claude_supervisor" == "1" ]]; then
  echo "  systemctl status ${service_name}-claude-supervisor --no-pager"
fi

#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: install-codex-supervisor-systemd.sh [--dry-run]

Installs the Codex own-capacity supervisor as a Linux systemd service. This is
the GCE/Linux counterpart to apps/pylon/scripts/supervisor-launchd/.

Environment:
  PYLON_INSTALL_DIR                  default: /opt/openagents-pylon
  PYLON_HOME                         default: /var/lib/openagents-pylon
  PYLON_SERVICE_USER                 default: pylon
  PYLON_SERVICE_NAME                 default: openagents-codex-supervisor
  PYLON_OPENAGENTS_BASE_URL          default: https://openagents.com
  OPENAGENTS_AGENT_TOKEN             required at runtime, usually via env file
  SUP_PYLON_REF                      optional; supervisor can resolve when wrapper starts
  SUP_MAX_SLOTS                      default: 1 on GCE account-per-VM hosts
  SUP_PER_ACCOUNT                    default: 1 on GCE account-per-VM hosts
  SUP_REPO                           default: OpenAgentsInc/openagents
  SUP_ISSUES                         public issue list for the supervisor
  SUP_VERIFY                         verification command for delegated work

The service sources /etc/openagents-pylon.env when present, then runs the
existing codex-supervisor.sh in the foreground so systemd can restart it.
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
  local owner="${2:-root}"
  local mode="${3:-0644}"
  if [[ "$dry_run" == "1" ]]; then
    echo "+ write ${path}"
    cat
  else
    install -m "$mode" -o "$owner" -g "$owner" /dev/stdin "$path"
  fi
}

if [[ "$dry_run" != "1" && "$(uname -s)" != "Linux" ]]; then
  echo "codex supervisor systemd install requires Linux" >&2
  exit 1
fi

if [[ "$dry_run" != "1" && "${EUID}" -ne 0 ]]; then
  echo "run as root, for example: sudo -E $0" >&2
  exit 1
fi

install_dir="${PYLON_INSTALL_DIR:-/opt/openagents-pylon}"
pylon_home="${PYLON_HOME:-/var/lib/openagents-pylon}"
service_user="${PYLON_SERVICE_USER:-pylon}"
service_name="${PYLON_SERVICE_NAME:-openagents-codex-supervisor}"
base_url="${PYLON_OPENAGENTS_BASE_URL:-https://openagents.com}"
env_file="/etc/openagents-pylon.env"
service_env_file="/etc/${service_name}.env"
wrapper="/usr/local/bin/${service_name}"
unit_file="/etc/systemd/system/${service_name}.service"
state_dir="${SUP_STATE_DIR:-${pylon_home}/.codex-supervisor}"

emit_env() {
  local key="$1"
  local value="$2"
  printf '%s=%q\n' "$key" "$value"
}

if ! id -u "$service_user" >/dev/null 2>&1; then
  run useradd --system --create-home --home-dir "$pylon_home" --shell /usr/sbin/nologin "$service_user"
fi
run mkdir -p "$pylon_home" "$state_dir"
run chown -R "$service_user:$service_user" "$pylon_home"

{
  emit_env "PYLON_HOME" "$pylon_home"
  emit_env "PYLON_OPENAGENTS_BASE_URL" "$base_url"
  emit_env "PYLON_DISABLE_DAEMON_ROUTING" "1"
  emit_env "SUP_STATE_DIR" "$state_dir"
  emit_env "SUP_LOG" "${state_dir}/supervisor.log"
  emit_env "SUP_MAX_SLOTS" "${SUP_MAX_SLOTS:-1}"
  emit_env "SUP_PER_ACCOUNT" "${SUP_PER_ACCOUNT:-1}"
  emit_env "SUP_REPO" "${SUP_REPO:-OpenAgentsInc/openagents}"
  emit_env "SUP_HEARTBEAT_SECS" "${SUP_HEARTBEAT_SECS:-45}"
  emit_env "SUP_BACKOFF_MIN" "${SUP_BACKOFF_MIN:-15}"
  emit_env "SUP_BACKOFF_MAX" "${SUP_BACKOFF_MAX:-300}"
  emit_env "SUP_STALL_REFUSALS" "${SUP_STALL_REFUSALS:-20}"
  emit_env "SUP_SELFHEAL_COOLDOWN_SECS" "${SUP_SELFHEAL_COOLDOWN_SECS:-300}"
  emit_env "SUP_SELFHEAL_CHECK_SECS" "${SUP_SELFHEAL_CHECK_SECS:-30}"
  if [[ -n "${SUP_PYLON_REF:-}" ]]; then
    emit_env "SUP_PYLON_REF" "$SUP_PYLON_REF"
  fi
  if [[ -n "${SUP_ISSUES:-}" ]]; then
    emit_env "SUP_ISSUES" "$SUP_ISSUES"
  fi
  if [[ -n "${SUP_VERIFY:-}" ]]; then
    emit_env "SUP_VERIFY" "$SUP_VERIFY"
  fi
} | write_file "$service_env_file" root 0600

cat <<'WRAPPER' | write_file "$wrapper" root 0755
#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="${PYLON_INSTALL_DIR:-/opt/openagents-pylon}"
PYLON_HOME="${PYLON_HOME:-/var/lib/openagents-pylon}"
BASE_URL="${PYLON_OPENAGENTS_BASE_URL:-https://openagents.com}"

if [[ -f /etc/openagents-pylon.env ]]; then
  set -a
  # shellcheck disable=SC1091
  . /etc/openagents-pylon.env
  set +a
fi

if [[ -f /etc/openagents-codex-supervisor.env ]]; then
  set -a
  # shellcheck disable=SC1091
  . /etc/openagents-codex-supervisor.env
  set +a
fi

export PATH="$HOME/.bun/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export PYLON_HOME="${PYLON_HOME:-/var/lib/openagents-pylon}"
export PYLON_OPENAGENTS_BASE_URL="${PYLON_OPENAGENTS_BASE_URL:-$BASE_URL}"
export PYLON_DISABLE_DAEMON_ROUTING="${PYLON_DISABLE_DAEMON_ROUTING:-1}"
export SUP_STATE_DIR="${SUP_STATE_DIR:-$PYLON_HOME/.codex-supervisor}"
export SUP_LOG="${SUP_LOG:-$SUP_STATE_DIR/supervisor.log}"
export SUP_MAX_SLOTS="${SUP_MAX_SLOTS:-1}"
export SUP_PER_ACCOUNT="${SUP_PER_ACCOUNT:-1}"

mkdir -p "$SUP_STATE_DIR"

if [[ -z "${OPENAGENTS_AGENT_TOKEN:-}" ]]; then
  echo "OPENAGENTS_AGENT_TOKEN is required for ${0##*/}" >&2
  exit 1
fi

if [[ -z "${SUP_PYLON_REF:-}" ]]; then
  live_ref="$(
    bun "$REPO_ROOT/apps/pylon/src/index.ts" provider go-online --json 2>/dev/null \
      | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('pylonRef') or d.get('pylon',{}).get('ref') or '')" 2>/dev/null \
      || true
  )"
  if [[ -n "$live_ref" ]]; then
    export SUP_PYLON_REF="$live_ref"
  fi
fi

exec bash "$REPO_ROOT/apps/pylon/scripts/codex-supervisor/codex-supervisor.sh"
WRAPPER

cat <<UNIT | write_file "$unit_file" root 0644
[Unit]
Description=OpenAgents Codex own-capacity supervisor
After=network-online.target openagents-pylon.service
Wants=network-online.target

[Service]
Type=simple
User=${service_user}
WorkingDirectory=${install_dir}
Environment=PYLON_INSTALL_DIR=${install_dir}
EnvironmentFile=-${env_file}
EnvironmentFile=-${service_env_file}
ExecStart=${wrapper}
Restart=always
RestartSec=15
KillSignal=SIGTERM

[Install]
WantedBy=multi-user.target
UNIT

run systemctl daemon-reload
run systemctl enable --now "$service_name"

cat <<EOF
Installed ${service_name}.

Next checks:
  systemctl status ${service_name} --no-pager
  journalctl -u ${service_name} -f
  sudo -u ${service_user} env PYLON_HOME=${pylon_home} bun ${install_dir}/apps/pylon/src/index.ts codex accounts list --json
EOF

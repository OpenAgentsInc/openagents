#!/usr/bin/env bash
set -euo pipefail

CONFIG_FILE="/etc/modprobe.d/nvidia-profiler.conf"
PARAM_FILE="/proc/driver/nvidia/params"
OPTION_LINE="options nvidia NVreg_RestrictProfilingToAdminUsers=0"

usage() {
  cat <<'EOF'
Usage:
  crates/psionic/scripts/enable-nvidia-profiling.sh enable
  crates/psionic/scripts/enable-nvidia-profiling.sh disable
  crates/psionic/scripts/enable-nvidia-profiling.sh status

Commands:
  enable   Allow non-root Nsight/NCU profiling after reboot.
  disable  Restore the default admin-only profiling behavior after reboot.
  status   Show the live driver setting and whether the config file exists.
EOF
}

status() {
  if [[ -r "$PARAM_FILE" ]]; then
    echo "Live driver state:"
    rg -n "RmProfilingAdminOnly" "$PARAM_FILE" || true
  else
    echo "Live driver state: unavailable ($PARAM_FILE not readable)"
  fi

  if [[ -f "$CONFIG_FILE" ]]; then
    echo
    echo "Configured override:"
    cat "$CONFIG_FILE"
  else
    echo
    echo "Configured override: none"
  fi
}

enable() {
  cat <<EOF | sudo tee "$CONFIG_FILE" >/dev/null
$OPTION_LINE
EOF
  sudo mkinitcpio -P
  echo "Enabled non-admin NVIDIA profiling."
  echo "Reboot required. After reboot, run:"
  echo "  crates/psionic/scripts/enable-nvidia-profiling.sh status"
}

disable() {
  sudo rm -f "$CONFIG_FILE"
  sudo mkinitcpio -P
  echo "Removed the NVIDIA profiling override."
  echo "Reboot required. After reboot, run:"
  echo "  crates/psionic/scripts/enable-nvidia-profiling.sh status"
}

main() {
  local command="${1:-enable}"

  case "$command" in
    enable)
      enable
      ;;
    disable)
      disable
      ;;
    status)
      status
      ;;
    -h|--help|help)
      usage
      ;;
    *)
      echo "Unknown command: $command" >&2
      usage >&2
      exit 1
      ;;
  esac
}

main "$@"

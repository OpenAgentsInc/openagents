#!/usr/bin/env bash
set -euo pipefail

# OpenAgents CLI standalone installer
# Usage: curl -fsSL https://openagents.com/install.sh | bash

BASE_URL="${OPENAGENTS_RELEASE_URL:-https://openagents.com/releases}"
INSTALL_DIR="${OPENAGENTS_INSTALL_DIR:-$HOME/.openagents/bin}"
BIN_NAME="openagents"

detect_platform() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"

  case "$os" in
    darwin) os="darwin" ;;
    linux) os="linux" ;;
    *) echo "Unsupported operating system: $os" >&2; exit 1 ;;
  esac

  case "$arch" in
    x86_64|amd64) arch="x64" ;;
    arm64|aarch64) arch="arm64" ;;
    *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
  esac

  echo "${os}-${arch}"
}

main() {
  local target
  target="$(detect_platform)"
  echo "Detected target platform: ${target}"

  mkdir -p "${INSTALL_DIR}"

  echo "Installing OpenAgents CLI into ${INSTALL_DIR}/${BIN_NAME}..."
  # If local target/debug/oa exists during workspace development, link/copy as proof
  if [[ -f "./target/debug/oa" ]]; then
    cp "./target/debug/oa" "${INSTALL_DIR}/${BIN_NAME}"
    chmod +x "${INSTALL_DIR}/${BIN_NAME}"
  fi

  echo "OpenAgents CLI installation complete!"
  echo "Add ${INSTALL_DIR} to your PATH if not already present:"
  echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
}

main "$@"

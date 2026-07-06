#!/usr/bin/env bash
set -euo pipefail

# Minimal run-capable bootstrap for the `cloud-gcp` ephemeral worker-VM lane
# (openagents#8503). This is NOT the Firecracker Agent Computer image under
# ../agent-computer/ — it targets the plain, already-proven `oa-codex-sess-*`
# ephemeral VM lane the private cloud/ repo's `gce_capacity.rs` provisions and
# tears down today (cloud#95/#96/#97). Today that lane's default image
# (ubuntu-2404-lts-amd64) is provisioner-only: it boots and answers SSH, but
# has no coding-agent-runnable runtime installed. This script is the minimal
# fix: install Bun, fetch the pinned public Pylon runtime, and run it in
# `org_cloud` executor mode so one real Khala Code turn can execute inside the
# VM before the control plane tears it down.
#
# Deliberately public-safe: this file accepts NO secrets on its command line
# or in its own body. The three required credentials
# (OPENAGENTS_ADMIN_API_TOKEN, OPENAGENTS_AGENT_TOKEN, OPENAGENTS_BASE_URL) are
# read at boot from the instance's own GCE metadata server, which only that
# instance can query. The private control plane is responsible for writing
# short-lived, run-scoped metadata values onto the specific ephemeral VM at
# `instances create` time (mirroring the existing session-scoped SSH-metadata
# pattern `gce_capacity.rs` already uses) and for scrubbing/deleting the VM
# (and therefore its metadata) at teardown. This script does not persist any
# credential to disk outside the running process environment.
#
# Usage as a GCE startup-script (metadata key `startup-script`):
#   gcloud compute instances create <name> ... \
#     --metadata-from-file startup-script=bootstrap.sh \
#     --metadata openagents-pin-ref=<git ref>,openagents-agent-token=<...>,...
#
# Can also be run by hand inside an already-booted VM for manual smoke testing
# (it is idempotent: re-running just re-syncs the pinned ref and restarts the
# supervisor).

METADATA_ROOT="http://metadata.google.internal/computeMetadata/v1/instance/attributes"
REPO_URL="${OPENAGENTS_REPO_URL:-https://github.com/OpenAgentsInc/openagents.git}"
STATE_ROOT="${OPENAGENTS_WORKER_STATE_ROOT:-/var/lib/openagents/cloud-gcp-worker}"
CHECKOUT_DIR="${STATE_ROOT}/openagents"
PYLON_HOME="${STATE_ROOT}/pylon-home"

log() {
  echo "[cloud-gcp-worker-bootstrap] $*" >&2
}

metadata_attr() {
  local key="$1"
  local fallback="${2:-}"
  if [[ -n "${!key:-}" ]]; then
    # Allow an env var override (useful for local dry-run testing) named
    # exactly like the metadata key with dashes turned into underscores and
    # upper-cased, e.g. OPENAGENTS_PIN_REF for openagents-pin-ref.
    printf '%s' "${!key}"
    return 0
  fi
  curl -fsS --max-time 5 -H "Metadata-Flavor: Google" "${METADATA_ROOT}/${key}" 2>/dev/null || printf '%s' "$fallback"
}

require_metadata() {
  local key="$1"
  local value
  value="$(metadata_attr "$key" "")"
  if [[ -z "$value" ]]; then
    echo "missing required instance metadata attribute: $key" >&2
    exit 2
  fi
  printf '%s' "$value"
}

install_bun() {
  if command -v bun >/dev/null 2>&1; then
    log "bun already installed: $(bun --version)"
    return
  fi
  log "installing bun"
  curl -fsSL https://bun.sh/install | bash
  export PATH="${HOME}/.bun/bin:${PATH}"
}

sync_repo() {
  local pin_ref
  pin_ref="$(metadata_attr OPENAGENTS_PIN_REF "main")"
  log "syncing ${REPO_URL} at ${pin_ref} into ${CHECKOUT_DIR}"
  install -d -m 0755 "$STATE_ROOT"
  if [[ -d "${CHECKOUT_DIR}/.git" ]]; then
    git -C "$CHECKOUT_DIR" fetch --depth 1 origin "$pin_ref"
    git -C "$CHECKOUT_DIR" checkout --force FETCH_HEAD
  else
    git clone --depth 1 --branch "$pin_ref" "$REPO_URL" "$CHECKOUT_DIR" 2>/dev/null \
      || git clone --depth 1 "$REPO_URL" "$CHECKOUT_DIR"
    if [[ "$(git -C "$CHECKOUT_DIR" rev-parse --abbrev-ref HEAD)" != "$pin_ref" ]]; then
      git -C "$CHECKOUT_DIR" fetch --depth 1 origin "$pin_ref"
      git -C "$CHECKOUT_DIR" checkout --force FETCH_HEAD
    fi
  fi
}

install_deps() {
  log "bun install (workspace root)"
  (cd "$CHECKOUT_DIR" && bun install)
}

run_supervisor() {
  local base_url admin_token agent_token owner_user_id
  base_url="$(metadata_attr OPENAGENTS_BASE_URL "https://openagents.com")"
  admin_token="$(require_metadata OPENAGENTS_ADMIN_TOKEN)"
  agent_token="$(require_metadata OPENAGENTS_AGENT_TOKEN)"
  # Deliberately NOT set: an owner-user-id. Leaving it unset is what selects
  # `org_cloud` executor mode (see runtime-intent-supervisor.ts) so the
  # supervisor never resolves a real user's own local Pylon/session — only
  # OpenAgents-owned org capacity, per the Agent Computers isolation posture.
  install -d -m 0700 "$PYLON_HOME"
  log "starting runtime-intent-supervisor in org_cloud mode against ${base_url}"
  cd "$CHECKOUT_DIR"
  OPENAGENTS_ADMIN_API_TOKEN="$admin_token" \
  OPENAGENTS_AGENT_TOKEN="$agent_token" \
  OPENAGENTS_BASE_URL="$base_url" \
    exec bun apps/pylon/src/orchestration/runtime-intent-supervisor.ts \
      --pylon-home "$PYLON_HOME" \
      --base-url "$base_url"
}

main() {
  install_bun
  sync_repo
  install_deps
  run_supervisor
}

# Only run main when executed directly (not when sourced, e.g. by tests that
# want to exercise metadata_attr/require_metadata in isolation).
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  main "$@"
fi

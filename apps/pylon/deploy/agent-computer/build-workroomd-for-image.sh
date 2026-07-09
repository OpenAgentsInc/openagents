#!/usr/bin/env bash
# Build in-repo oa-workroomd for Agent Computer guest images (#8591 Phase 4).
#
# Produces a linux-x86_64 release binary from crates/oa-workroomd and stages it
# under the path the guest image bake expects. No secrets are accepted.
#
# Usage:
#   apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh
#   apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh --out /path/to/oa-workroomd
#   apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh --docker
#
# --docker uses docker/cloud/oa-workroomd.Dockerfile (linux guest target).
# Default builds with cargo for the host triple (fine for local smoke; use
# --docker when baking the real Firecracker rootfs on a nested-virt host).

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  build-workroomd-for-image.sh [--out PATH] [--docker] [--tag TAG]

Builds crates/oa-workroomd from this monorepo for Agent Computer images.
Does not accept tokens, project ids, or wallet material.
USAGE
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
out_dir="${OPENAGENTS_AGENT_COMPUTER_STAGING:-${repo_root}/var/agent-computer/staging}"
out_bin=""
use_docker="false"
tag="${OPENAGENTS_CLOUD_IMAGE_TAG:-agent-computer-local}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out)
      out_bin="${2:-}"
      shift 2
      ;;
    --docker)
      use_docker="true"
      shift
      ;;
    --tag)
      tag="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

mkdir -p "$out_dir"
if [[ -z "$out_bin" ]]; then
  out_bin="${out_dir}/oa-workroomd"
fi

if [[ "$use_docker" == "true" ]]; then
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required for --docker" >&2
    exit 1
  fi
  image="openagents-cloud/oa-workroomd:${tag}"
  docker build \
    -f "${repo_root}/docker/cloud/oa-workroomd.Dockerfile" \
    -t "$image" \
    --build-arg "IMAGE_CREATED=$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --build-arg "IMAGE_REVISION=$(git -C "$repo_root" rev-parse --short=12 HEAD 2>/dev/null || printf unknown)" \
    --build-arg "IMAGE_VERSION=${tag}" \
    "$repo_root"
  # Extract binary from the image without running the entrypoint.
  cid="$(docker create "$image")"
  trap 'docker rm -f "$cid" >/dev/null 2>&1 || true' EXIT
  docker cp "${cid}:/usr/local/bin/oa-workroomd" "$out_bin"
  docker rm -f "$cid" >/dev/null
  trap - EXIT
else
  (
    cd "$repo_root"
    cargo build --release -p oa-workroomd
  )
  cp "${repo_root}/target/release/oa-workroomd" "$out_bin"
fi

chmod 755 "$out_bin"
# Public-safe digest for the manifest / bake log (not a signature).
if command -v shasum >/dev/null 2>&1; then
  digest="$(shasum -a 256 "$out_bin" | awk '{print $1}')"
elif command -v sha256sum >/dev/null 2>&1; then
  digest="$(sha256sum "$out_bin" | awk '{print $1}')"
else
  digest="unknown"
fi

meta="${out_dir}/oa-workroomd.staging.json"
cat >"$meta" <<EOF
{
  "schema": "openagents.agent_computer.workroomd_staging.v1",
  "binaryPath": "${out_bin}",
  "guestInstallPath": "/usr/local/bin/oa-workroomd",
  "sourceCrate": "crates/oa-workroomd",
  "dockerfile": "docker/cloud/oa-workroomd.Dockerfile",
  "sha256": "${digest}",
  "builtAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "gitRevision": "$(git -C "$repo_root" rev-parse HEAD 2>/dev/null || printf unknown)"
}
EOF

echo "staged oa-workroomd -> ${out_bin}"
echo "sha256:${digest}"
echo "guest path: /usr/local/bin/oa-workroomd"
echo "meta: ${meta}"

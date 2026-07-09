#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/build-cloud-images.sh [--registry REGISTRY] [--tag TAG] [--push] [--load]

Builds reproducible local images for:
  - oa-node
  - oa-workroomd
  - oa-codex-control

No secrets are accepted as arguments. Runtime credentials must be mounted,
brokered, or provided through scoped platform identity outside the image.
USAGE
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
registry="openagents-cloud"
tag="${OPENAGENTS_CLOUD_IMAGE_TAG:-local}"
push="false"
load="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --registry)
      registry="${2:-}"
      shift 2
      ;;
    --tag)
      tag="${2:-}"
      shift 2
      ;;
    --push)
      push="true"
      shift
      ;;
    --load)
      load="true"
      shift
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

if [[ -z "$registry" || -z "$tag" ]]; then
  echo "--registry and --tag must not be empty" >&2
  exit 2
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required to build cloud images" >&2
  exit 1
fi

created="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
revision="$(git -C "$repo_root" rev-parse --short=12 HEAD 2>/dev/null || printf unknown)"
version="${tag}"

if docker buildx version >/dev/null 2>&1; then
  builder=(docker buildx build)
  output_args=()
  if [[ "$push" == "true" ]]; then
    output_args+=(--push)
  elif [[ "$load" == "true" ]]; then
    output_args+=(--load)
  fi
else
  builder=(docker build)
  output_args=()
  if [[ "$push" == "true" ]]; then
    echo "--push requires docker buildx" >&2
    exit 2
  fi
fi

build_image() {
  local name="$1"
  local dockerfile="$2"
  local image="${registry}/${name}:${tag}"

  "${builder[@]}" \
    "${output_args[@]}" \
    --build-arg "IMAGE_CREATED=${created}" \
    --build-arg "IMAGE_REVISION=${revision}" \
    --build-arg "IMAGE_VERSION=${version}" \
    --label "org.opencontainers.image.created=${created}" \
    --label "org.opencontainers.image.revision=${revision}" \
    --label "org.opencontainers.image.version=${version}" \
    -f "$repo_root/${dockerfile}" \
    -t "$image" \
    "$repo_root"

  printf '%s\n' "$image"
}

build_image oa-node docker/oa-node.Dockerfile
build_image oa-workroomd docker/oa-workroomd.Dockerfile
build_image oa-codex-control docker/oa-codex-control.Dockerfile

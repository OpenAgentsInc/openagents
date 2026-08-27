#!/usr/bin/env bash
set -euo pipefail

# Reproducible build recipe for the OpenAgents harbor-runner image.
# Builds from the repository root so the Dockerfile can COPY bench/ assets.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION_FILE="${REPO_ROOT}/docker/harbor-runner/VERSION"
VERSION="${HARBOR_RUNNER_VERSION:-$(cat "${VERSION_FILE}")}"
TAG="harbor-runner:${VERSION}"
DOCKERFILE="${REPO_ROOT}/docker/harbor-runner/Dockerfile"

# The base image is pinned by digest in the Dockerfile. This script builds the
# image, tags it, and prints the local image id for the caller to record.
docker build \
  -f "${DOCKERFILE}" \
  -t "${TAG}" \
  --build-arg "HARBOR_RUNNER_VERSION=${VERSION}" \
  "${REPO_ROOT}"

docker image inspect "${TAG}" --format='{{.Id}}'

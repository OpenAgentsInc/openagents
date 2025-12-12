#!/bin/bash
# Build the OpenAgents agent container image
#
# Usage:
#   ./docker/agent/build.sh [VERSION]
#
# Examples:
#   ./docker/agent/build.sh           # Build :latest
#   ./docker/agent/build.sh v1.0.0    # Build :v1.0.0

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

IMAGE_NAME="openagents/agent"
VERSION="${1:-latest}"

echo "Building ${IMAGE_NAME}:${VERSION}"
echo "Context: ${REPO_ROOT}"
echo "Dockerfile: ${SCRIPT_DIR}/Dockerfile"

docker build \
    -t "${IMAGE_NAME}:${VERSION}" \
    -f "${SCRIPT_DIR}/Dockerfile" \
    "${REPO_ROOT}"

echo ""
echo "Successfully built ${IMAGE_NAME}:${VERSION}"
echo ""
echo "To run:"
echo "  docker run -it -v \$(pwd):/workspace ${IMAGE_NAME}:${VERSION}"

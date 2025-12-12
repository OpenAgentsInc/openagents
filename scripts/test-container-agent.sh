#!/bin/bash
# Test script for container-based parallel agent execution
#
# Prerequisites:
#   1. Docker installed and running
#   2. Agent image built: ./docker/agent/build.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Container-Based Parallel Agent Test ==="
echo ""

# Step 1: Check Docker is available
echo "Step 1: Checking Docker..."
if ! command -v docker &> /dev/null; then
    echo "ERROR: Docker not found. Install Docker first."
    exit 1
fi

if ! docker info &> /dev/null; then
    echo "ERROR: Docker daemon not running. Start Docker first."
    exit 1
fi
echo "✓ Docker is available"
echo ""

# Step 2: Build agent image if not exists
echo "Step 2: Checking agent image..."
if ! docker image inspect openagents/agent:latest &> /dev/null; then
    echo "Building agent image..."
    "$REPO_ROOT/docker/agent/build.sh"
else
    echo "✓ Agent image exists"
fi
echo ""

# Step 3: Create test workspace
echo "Step 3: Creating test workspace..."
WORKSPACE=$(mktemp -d)
echo "Workspace: $WORKSPACE"

# Clone a simple test repo (or use current repo)
echo "Cloning test repository..."
git clone --depth 1 "$REPO_ROOT" "$WORKSPACE/repo" 2>/dev/null || {
    # Fallback: just create a basic git repo
    mkdir -p "$WORKSPACE/repo"
    cd "$WORKSPACE/repo"
    git init
    echo "# Test" > README.md
    git add .
    git commit -m "Initial"
}
echo "✓ Test repository ready"
echo ""

# Step 4: Run container with mounted workspace
echo "Step 4: Running container..."
CONTAINER_ID=$(docker run -d \
    -v "$WORKSPACE/repo:/workspace" \
    -e AGENT_ID="test-agent-0" \
    -e TASK_ID="test-task-1" \
    -e GIT_BRANCH="agent/test-agent-0" \
    openagents/agent:latest \
    sleep 300)

echo "Container ID: ${CONTAINER_ID:0:12}"
echo ""

# Step 5: Execute commands inside container
echo "Step 5: Testing container environment..."
echo ""

echo "--- Git version ---"
docker exec "$CONTAINER_ID" git --version

echo ""
echo "--- Bun version ---"
docker exec "$CONTAINER_ID" bun --version

echo ""
echo "--- Rust version ---"
docker exec "$CONTAINER_ID" rustc --version

echo ""
echo "--- Environment variables ---"
docker exec "$CONTAINER_ID" env | grep -E "^(AGENT_ID|TASK_ID|GIT_BRANCH)="

echo ""
echo "--- Git config ---"
docker exec "$CONTAINER_ID" git config --global --list | grep -E "^user\."

echo ""
echo "--- Workspace contents ---"
docker exec "$CONTAINER_ID" ls -la /workspace

# Step 6: Test git operations
echo ""
echo "Step 6: Testing git operations in container..."
docker exec "$CONTAINER_ID" bash -c '
    cd /workspace
    git checkout -b agent/test-agent-0 2>/dev/null || git checkout agent/test-agent-0
    echo "Test change from container agent" >> test-agent.txt
    git add test-agent.txt
    git commit -m "Test commit from agent"
    git log --oneline -1
'
echo "✓ Git operations work"

# Cleanup
echo ""
echo "Step 7: Cleaning up..."
docker stop "$CONTAINER_ID" > /dev/null
docker rm "$CONTAINER_ID" > /dev/null
sudo rm -rf "$WORKSPACE" 2>/dev/null || rm -rf "$WORKSPACE"
echo "✓ Cleanup complete"

echo ""
echo "=== All tests passed! ==="
echo ""
echo "The container environment is ready for parallel agent execution."
echo ""
echo "Next steps:"
echo "  1. Integrate with ContainerManager (crates/parallel/src/container_manager.rs)"
echo "  2. Use orchestrator to spawn multiple containers"
echo "  3. Each container gets its own branch and pushes work"

#!/bin/bash
# Run a TB2 task with tbench and save ATIF trajectories
#
# Usage:
#   ./scripts/tb2-run.sh regex-log                      # Default model
#   ./scripts/tb2-run.sh regex-log --model claude-opus-4-5-20251101
#   ./scripts/tb2-run.sh regex-log --stream             # Stream events to stdout
#
# Environment:
#   TB2_ROOT - Path to terminal-bench-2 repo (default: ~/code/terminal-bench-2)
#   ANTHROPIC_API_KEY - Required for Claude Code

set -e

# Configuration
TASK_ID="${1:-regex-log}"
shift || true

# Parse remaining args
MODEL=""
STREAM=""
TIMEOUT="900"
while [[ $# -gt 0 ]]; do
    case $1 in
        --model)
            MODEL="$2"
            shift 2
            ;;
        --stream)
            STREAM="--stream"
            shift
            ;;
        --timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Paths
TB2_ROOT="${TB2_ROOT:-$HOME/code/terminal-bench-2}"
TASK_DIR="${TB2_ROOT}/${TASK_ID}"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
SESSION_ID="${TIMESTAMP}-$(openssl rand -hex 4)"
OUTPUT_DIR="results/trajectories/${TASK_ID}/${SESSION_ID}"
WORKSPACE=$(mktemp -d)

# Verify task exists
if [[ ! -d "${TASK_DIR}" ]]; then
    echo "Error: Task directory not found: ${TASK_DIR}"
    echo "Available tasks:"
    ls -1 "${TB2_ROOT}" 2>/dev/null | head -20
    exit 1
fi

# Load instruction
INSTRUCTION_FILE="${TASK_DIR}/instruction.md"
if [[ ! -f "${INSTRUCTION_FILE}" ]]; then
    echo "Error: Instruction file not found: ${INSTRUCTION_FILE}"
    exit 1
fi
INSTRUCTION=$(cat "${INSTRUCTION_FILE}")

# Get docker image from task.toml
DOCKER_IMAGE=$(grep 'docker_image' "${TASK_DIR}/task.toml" 2>/dev/null | cut -d'"' -f2 || echo "")
if [[ -z "${DOCKER_IMAGE}" ]]; then
    echo "Warning: No docker_image found in task.toml"
fi

# Build tbench if needed
TBENCH_BIN="target/release/tbench"
if [[ ! -f "${TBENCH_BIN}" ]]; then
    echo "Building tbench..."
    cargo build -p harbor --release
fi

# Setup directories
mkdir -p "${OUTPUT_DIR}"
mkdir -p "${WORKSPACE}/app"  # Create /app for TB2 tasks that expect it

# Copy environment files to workspace (so agent has something to work with)
ENV_DIR="${TASK_DIR}/environment"
if [[ -d "${ENV_DIR}" ]]; then
    # Copy files from environment/tests/ (common pattern) or environment/ directly
    if [[ -d "${ENV_DIR}/tests" ]]; then
        # Some tasks store input files in environment/tests/
        cp -r "${ENV_DIR}/tests"/* "${WORKSPACE}/app/" 2>/dev/null || true
    fi
    # Also copy any files directly in environment/ (excluding Dockerfile)
    for f in "${ENV_DIR}"/*; do
        if [[ -f "$f" && "$(basename "$f")" != "Dockerfile" && "$(basename "$f")" != "docker-compose.yaml" ]]; then
            cp "$f" "${WORKSPACE}/app/" 2>/dev/null || true
        fi
    done
fi

# Get absolute paths for Docker volumes
OUTPUT_DIR_ABS="$(cd "$(dirname "${OUTPUT_DIR}")" 2>/dev/null && pwd)/$(basename "${OUTPUT_DIR}")"
mkdir -p "${OUTPUT_DIR_ABS}"

# Start Docker container for agent to use during development
CONTAINER_NAME="tb2-${TASK_ID}-${SESSION_ID:0:8}"
if [[ -n "${DOCKER_IMAGE}" ]]; then
    echo "Starting Docker container for agent access..."
    docker run --rm -d \
        --name "${CONTAINER_NAME}" \
        -v "${WORKSPACE}/app:/app" \
        -w /app \
        "${DOCKER_IMAGE}" \
        tail -f /dev/null >/dev/null 2>&1

    # Add container info to instruction
    INSTRUCTION="${INSTRUCTION}

## Docker Environment

A Docker container is running with the task environment. Use it to run tools:

\`\`\`bash
docker exec ${CONTAINER_NAME} <command>
\`\`\`

Examples:
- \`docker exec ${CONTAINER_NAME} ls /app\`
- \`docker exec ${CONTAINER_NAME} pdflatex -interaction=nonstopmode main.tex\`
- \`docker exec ${CONTAINER_NAME} python3 script.py\`

The container has all required tools installed. Your workspace at /app is shared."
fi

echo "========================================"
echo "TB2 Run: ${TASK_ID}"
echo "========================================"
echo "Session:    ${SESSION_ID}"
echo "Workspace:  ${WORKSPACE}"
echo "Output:     ${OUTPUT_DIR}"
echo "Docker:     ${DOCKER_IMAGE:-none}"
echo "Container:  ${CONTAINER_NAME:-none}"
echo "Model:      ${MODEL:-default}"
echo "Timeout:    ${TIMEOUT}s"
echo "========================================"
echo ""

# Build tbench args
# Use the /app subdirectory as CWD since TB2 tasks expect to write to /app/
TBENCH_ARGS=(
    --instruction "${INSTRUCTION}"
    --output-dir "${OUTPUT_DIR_ABS}"
    --cwd "${WORKSPACE}/app"
    --timeout "${TIMEOUT}"
    --max-turns 300
)

if [[ -n "${MODEL}" ]]; then
    TBENCH_ARGS+=(--model "${MODEL}")
fi

if [[ -n "${STREAM}" ]]; then
    TBENCH_ARGS+=(--stream)
fi

# Run tbench
echo "Running tbench..."
"${TBENCH_BIN}" "${TBENCH_ARGS[@]}"
TBENCH_EXIT=$?

echo ""
echo "========================================"
echo "tbench completed (exit code: ${TBENCH_EXIT})"
echo "========================================"
echo ""

# Stop the development container before verification
if [[ -n "${CONTAINER_NAME}" ]]; then
    docker stop "${CONTAINER_NAME}" >/dev/null 2>&1 || true
fi

# Run TB2 verification if docker image is available
if [[ -n "${DOCKER_IMAGE}" ]]; then
    echo "Running TB2 verification..."

    # Create verifier output dir with absolute path
    VERIFIER_DIR="${OUTPUT_DIR_ABS}/verifier"
    mkdir -p "${VERIFIER_DIR}"

    # Run verification - mount workspace/app as /app
    docker run --rm \
        -v "${WORKSPACE}/app:/app" \
        -v "${VERIFIER_DIR}:/logs/verifier" \
        -v "${TASK_DIR}/tests:/tests:ro" \
        -w /app \
        "${DOCKER_IMAGE}" \
        bash /tests/test.sh 2>&1 | tee "${VERIFIER_DIR}/output.txt"

    # Check result
    if [[ -f "${VERIFIER_DIR}/reward.txt" ]]; then
        REWARD=$(cat "${VERIFIER_DIR}/reward.txt")
        echo ""
        echo "========================================"
        echo "TB2 Result: ${REWARD}"
        echo "========================================"
    else
        echo ""
        echo "Warning: No reward.txt found"
    fi
fi

echo ""
echo "ATIF trajectory saved to: ${OUTPUT_DIR_ABS}/trajectory.json"
echo "Metrics saved to: ${OUTPUT_DIR_ABS}/metrics.json"
echo "Workspace: ${WORKSPACE}/app"
echo ""

# Cleanup workspace
# rm -rf "${WORKSPACE}"  # Uncomment to auto-cleanup

exit ${TBENCH_EXIT}

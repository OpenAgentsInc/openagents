#!/bin/bash
# Terminal-Bench Demo Run Script
# Runs a single task (regex-log) via Harbor with either Claude Code or Pi agent

set -e

# Defaults
AGENT="${1:-claude}"  # claude or pi
TASK="${2:-regex-log}"
MODEL="${3:-anthropic/claude-haiku-4-5-20251001}"
OUTPUT_DIR="./results/tbench-demo-$(date +%Y%m%d-%H%M%S)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Terminal-Bench Demo Run ===${NC}"
echo "Agent: $AGENT"
echo "Task: $TASK"
echo "Model: $MODEL"
echo "Output: $OUTPUT_DIR"
echo ""

# Check prerequisites
if ! command -v harbor &> /dev/null; then
    echo -e "${RED}Error: Harbor not installed${NC}"
    echo "Install with: uv tool install harbor"
    exit 1
fi

if ! docker info &> /dev/null 2>&1; then
    echo -e "${RED}Error: Docker not running${NC}"
    echo "Please start Docker and try again"
    exit 1
fi

# Check for API key
if [ -z "$ANTHROPIC_API_KEY" ] && [ -z "$ANTHROPIC_OAUTH_TOKEN" ]; then
    echo -e "${YELLOW}Warning: No ANTHROPIC_API_KEY or ANTHROPIC_OAUTH_TOKEN set${NC}"
    echo "Attempting to export from Mac Keychain..."

    # Try to export OAuth credentials from Keychain
    if [ "$(uname)" = "Darwin" ]; then
        CRED=$(security find-generic-password -s "Claude Code-credentials" -g 2>&1 | grep '^password:' | sed 's/^password: "//' | sed 's/"$//')
        if [ -n "$CRED" ]; then
            echo -e "${GREEN}Found credentials in Keychain${NC}"
            # Write to temp file for container use
            CRED_DIR=$(mktemp -d)
            echo "$CRED" | sed 's/\\"/"/g' | sed 's/\\\\/\\/g' > "$CRED_DIR/.credentials.json"
            chmod 600 "$CRED_DIR/.credentials.json"
            export CLAUDE_CREDENTIALS_DIR="$CRED_DIR"
            echo "Credentials exported to: $CRED_DIR"
        else
            echo -e "${RED}No credentials found in Keychain${NC}"
            echo "Please set ANTHROPIC_API_KEY environment variable"
            exit 1
        fi
    else
        echo -e "${RED}Please set ANTHROPIC_API_KEY environment variable${NC}"
        exit 1
    fi
fi

# Navigate to harbor package
cd "$(dirname "$0")/../harbor"

# Create virtual environment if needed
if [ ! -d ".venv" ]; then
    echo "Creating virtual environment..."
    uv venv
fi

# Activate and install
source .venv/bin/activate
uv pip install -e ".[dev]"

# Apply Harbor fix if needed (upload_dir bug)
echo "Checking Harbor patch status..."
PATCHED=$(python -c "from harbor.environments.docker.docker import DockerEnvironment; import inspect; print('PATCHED' if 'rstrip' in inspect.getsource(DockerEnvironment.upload_dir) else 'NOT_PATCHED')" 2>/dev/null || echo "ERROR")

if [ "$PATCHED" = "NOT_PATCHED" ]; then
    echo -e "${YELLOW}Applying Harbor upload_dir patch...${NC}"
    python -c "
import harbor.environments.docker.docker as m
p = m.__file__
t = open(p).read()
old = '''    async def upload_dir(self, source_dir: Path | str, target_dir: str):
        await self._run_docker_compose_command(
            [
                \"cp\",
                str(source_dir),
                f\"main:{target_dir}\",
            ],
            check=True,
        )'''
new = '''    async def upload_dir(self, source_dir: Path | str, target_dir: str):
        # Append /. to source to copy contents, not the directory itself
        source = str(source_dir).rstrip('/') + '/.'
        await self._run_docker_compose_command(
            [
                \"cp\",
                source,
                f\"main:{target_dir}\",
            ],
            check=True,
        )'''
if old in t:
    open(p, 'w').write(t.replace(old, new))
    print('Patch applied successfully')
else:
    print('Already patched or file structure changed')
"
elif [ "$PATCHED" = "ERROR" ]; then
    echo -e "${YELLOW}Warning: Could not check Harbor patch status${NC}"
fi

# Select agent import path
if [ "$AGENT" = "pi" ]; then
    IMPORT_PATH="openagents_harbor:PiAgent"
    echo -e "${GREEN}Using Pi agent${NC}"
else
    IMPORT_PATH="openagents_harbor:ClaudeCodeAgent"
    echo -e "${GREEN}Using Claude Code agent with testgen skill${NC}"
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo ""
echo -e "${GREEN}Starting Harbor run...${NC}"
echo "harbor run -d terminal-bench@2.0 --agent-import-path $IMPORT_PATH -m $MODEL -t $TASK -o $OUTPUT_DIR"
echo ""

# Run Harbor
harbor run \
    -d terminal-bench@2.0 \
    --agent-import-path "$IMPORT_PATH" \
    -m "$MODEL" \
    -t "$TASK" \
    -o "$OUTPUT_DIR"

# Check results
echo ""
echo -e "${GREEN}=== Results ===${NC}"
if [ -f "$OUTPUT_DIR/results.json" ]; then
    echo "Results saved to: $OUTPUT_DIR/results.json"
    cat "$OUTPUT_DIR/results.json" | python -m json.tool 2>/dev/null || cat "$OUTPUT_DIR/results.json"
else
    echo -e "${YELLOW}No results.json found${NC}"
fi

if [ -f "$OUTPUT_DIR/$TASK/verification.txt" ]; then
    echo ""
    echo "Verification output:"
    cat "$OUTPUT_DIR/$TASK/verification.txt"
fi

# Cleanup credentials if we created them
if [ -n "$CRED_DIR" ]; then
    rm -rf "$CRED_DIR"
fi

echo ""
echo -e "${GREEN}Demo complete!${NC}"

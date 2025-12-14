# OpenAgents Harbor

Harbor agent adapters for Terminal-Bench evaluations. Supports both Claude Code CLI and Pi agents.

## Installation

```bash
# Install uv if not already installed
curl -LsSf https://astral.sh/uv/install.sh | sh

# Install harbor
uv tool install harbor

# Install this package
cd harbor
uv venv --python python3.12
source .venv/bin/activate
uv pip install -e ".[dev]"
```

## Usage

### Claude Code Agent (with TestGen skill)

```bash
harbor run \
    -d terminal-bench@2.0 \
    --agent-import-path openagents_harbor:ClaudeCodeAgent \
    -m anthropic/claude-haiku-4-5-20251001 \
    --task-ids regex-log \
    -o results/
```

### Pi Agent

```bash
harbor run \
    -d terminal-bench@2.0 \
    --agent-import-path openagents_harbor:PiAgent \
    -m anthropic/claude-haiku-4-5-20251001 \
    --task-ids regex-log \
    -o results/
```

## Authentication

### API Key (Recommended for containers)

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

### OAuth Token (Mac Keychain)

OAuth tokens from Claude Code CLI work if credentials are exported from Mac Keychain.
The ClaudeCodeAgent automatically exports credentials if ANTHROPIC_API_KEY is not set.

## Harbor Patch

There is a bug in Harbor's `upload_dir` function. Apply the patch:

```bash
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
    print('Patch applied')
else:
    print('Already patched')
"
```

# Claude Code Container Image

Default container image for running Claude Code CLI inside the OpenAgents runtime.

## Why this image exists

- Provides a reproducible, minimal runtime for Claude CLI.
- Keeps credentials out of the image; auth is injected at runtime.
- Works with both Docker and Apple Container (macOS 26+).

## Build

Docker:

```bash
docker build -f docker/claude/Dockerfile -t openagents/claude-code:latest .
```

Apple Container:

```bash
container build -t openagents/claude-code:latest -f docker/claude/Dockerfile .
```

If Apple Container build is unavailable or slow, build with Docker and load the
OCI tar:

```bash
docker buildx build -f docker/claude/Dockerfile \
  -t openagents/claude-code:latest \
  --output type=oci,dest=/tmp/openagents-claude-code.oci.tar .
container image load -i /tmp/openagents-claude-code.oci.tar
```

## Run (manual smoke tests)

Version check:

```bash
container run --rm -i openagents/claude-code:latest claude --version
```

Docker equivalent:

```bash
docker run --rm openagents/claude-code:latest
```

Credential smoke tests (local only):

Docker supports file mounts, so you can mount just the credentials file without
masking the bundled CLI:

```bash
docker run --rm -i \
  -v ~/.claude/.credentials.json:/home/agent/.claude/.credentials.json:ro \
  openagents/claude-code:latest \
  claude --print "Reply with OK" --max-budget-usd 0.10
```

Apple Container requires directory mounts and will reject single-file mounts.
Use a temp directory and point `CLAUDE_CONFIG_DIR` at it so the CLI can write
its state without masking `/home/agent/.claude/bin/claude`:

```bash
mkdir -p /tmp/claude
cp ~/.claude/.credentials.json /tmp/claude/
container run --rm -i \
  -v /tmp/claude:/tmp/claude \
  -e CLAUDE_CONFIG_DIR=/tmp/claude \
  openagents/claude-code:latest \
  /home/agent/.claude/bin/claude --print "Reply with OK" --max-budget-usd 0.10
```

Notes:

- Keep the Apple Container mount writable; the CLI writes `.claude.json` in the
  config directory.
- Avoid mounting a host directory onto `/home/agent/.claude` or the `claude`
  binary gets hidden.

Helper script:

```bash
scripts/claude-container-smoke-test.sh "Reply with OK"
OPENAGENTS_CLAUDE_CONTAINER_RUNTIME=docker scripts/claude-container-smoke-test.sh "Reply with OK"
```

The script sources credentials from `CLAUDE_CREDENTIALS_JSON`, from
`~/.claude/.credentials.json`, or from macOS Keychain (`Claude Code-credentials`)
and cleans up its temp directory after the run.

## Runtime wiring

The runtime reads these environment variables to wrap Claude in a container:

- `OPENAGENTS_CLAUDE_CONTAINER_IMAGE` (required)
- `OPENAGENTS_CLAUDE_CONTAINER_RUNTIME` (optional: `apple`, `docker`, `auto`)
- `OPENAGENTS_CLAUDE_CONTAINER_COMMAND` (optional: defaults to `claude`)
- `OPENAGENTS_CLAUDE_PROXY_URL` (optional: sets `HTTP_PROXY`, `HTTPS_PROXY`, `NODE_USE_ENV_PROXY=1`)

Example:

```bash
export OPENAGENTS_CLAUDE_CONTAINER_IMAGE=openagents/claude-code:latest
export OPENAGENTS_CLAUDE_CONTAINER_RUNTIME=apple
export OPENAGENTS_CLAUDE_PROXY_URL=http://192.168.64.1:8080
```

## Notes

- No credentials are baked into the image.
- Use proxy-based auth (preferred) or mount `~/.claude` for local testing.
- The CLI binary is downloaded from the official Claude Code distribution
  bucket and verified against the published manifest checksums.

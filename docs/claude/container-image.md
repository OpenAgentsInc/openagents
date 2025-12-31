# Claude Code Container Image

This document describes the default container image used to run Claude Code CLI
inside the OpenAgents runtime.

## Overview

The runtime can isolate Claude sessions by executing the CLI inside a container.
This image provides a repeatable, minimal environment with the official
`claude` launcher installed and common developer tooling available.

Key properties:

- No credentials baked into the image.
- Works with Apple Container (macOS 26+) and Docker.
- Designed for the runtime's containerized CLI wrapper.

## Image source

Dockerfile: `docker/claude/Dockerfile`

The Dockerfile:

- Installs dependencies required to download and verify the Claude binary
  (`bash`, `curl`, `jq`, `sha256sum` via coreutils).
- Downloads the official Claude Code binary and verifies it against the
  published manifest checksums.
- Creates a non-root `agent` user to avoid root-owned workspace files.
- Adds common tooling (`git`, `rg`, `fd`, `jq`, `ssh`) used by Claude.

## Build

Docker:

```bash
docker build -f docker/claude/Dockerfile -t openagents/claude-code:latest .
```

Apple Container:

```bash
container build -t openagents/claude-code:latest -f docker/claude/Dockerfile .
```

If Apple Container build is unavailable or slow, you can build with Docker and
load the OCI tar into Apple Container:

```bash
docker buildx build -f docker/claude/Dockerfile \
  -t openagents/claude-code:latest \
  --output type=oci,dest=/tmp/openagents-claude-code.oci.tar .
container image load -i /tmp/openagents-claude-code.oci.tar
```

## Test locally

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

## Runtime integration

Set these environment variables on the runtime host:

- `OPENAGENTS_CLAUDE_CONTAINER_IMAGE` (required): image tag to run
- `OPENAGENTS_CLAUDE_CONTAINER_RUNTIME` (optional): `apple`, `docker`, or `auto`
- `OPENAGENTS_CLAUDE_CONTAINER_COMMAND` (optional): default `claude`
- `OPENAGENTS_CLAUDE_PROXY_URL` (optional): HTTP proxy URL

Example:

```bash
export OPENAGENTS_CLAUDE_CONTAINER_IMAGE=openagents/claude-code:latest
export OPENAGENTS_CLAUDE_CONTAINER_RUNTIME=apple
export OPENAGENTS_CLAUDE_PROXY_URL=http://192.168.64.1:8080
```

When `policy.isolation_mode = container`, the runtime wraps the local and cloud
Claude providers by invoking:

```
container run --rm -i <image> claude ...
```

`network_mode = none` disables networking with `--network none`. `proxy_only`
uses the provided proxy URL and sets `NODE_USE_ENV_PROXY=1` inside the container.

## Security notes

- Credentials are never stored in the image.
- Prefer proxy-based auth (tunnel or local proxy) so the container has no
  direct access to API keys.
- If you mount `~/.claude`, treat it as sensitive data and keep it read-write
  only for local testing.

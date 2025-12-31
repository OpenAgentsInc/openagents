# Dockerfile for the default Claude Code CLI runtime image.
#
# Build (Docker):
#   docker build -f docker/claude/Dockerfile -t openagents/claude-code:latest .
#
# Build (Apple Container):
#   container build -t openagents/claude-code:latest -f docker/claude/Dockerfile .
#
# Notes:
# - No credentials are baked into the image. Provide auth via proxy
#   (OPENAGENTS_CLAUDE_PROXY_URL) or mount ~/.claude at runtime.
# - This image is intentionally minimal but includes common tooling
#   that Claude frequently uses (git, rg, fd, jq, ssh).

FROM debian:bookworm-slim

# Install dependencies for the Claude CLI installer and common tooling.
RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    ca-certificates \
    curl \
    fd-find \
    git \
    jq \
    openssh-client \
    procps \
    ripgrep \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user to match host file permissions.
ARG HOST_UID=1000
RUN useradd -m -u ${HOST_UID} -s /bin/bash agent
RUN mkdir -p /workspace && chown -R agent:agent /workspace

USER agent
WORKDIR /home/agent

# Install Claude Code CLI (official distribution).
# We mirror the installer logic but skip shell integration to keep builds lean.
ARG CLAUDE_CHANNEL=stable
RUN /bin/bash -lc '\
  set -e; \
  GCS_BUCKET="https://storage.googleapis.com/claude-code-dist-86c565f3-f756-42ad-8dfa-d59b1c096819/claude-code-releases"; \
  VERSION="$(curl -fsSL "$GCS_BUCKET/${CLAUDE_CHANNEL}")"; \
  ARCH="$(uname -m)"; \
  case "$ARCH" in \
    x86_64|amd64) PLATFORM="linux-x64" ;; \
    arm64|aarch64) PLATFORM="linux-arm64" ;; \
    *) echo "Unsupported arch: $ARCH" >&2; exit 1 ;; \
  esac; \
  MANIFEST="$(curl -fsSL "$GCS_BUCKET/$VERSION/manifest.json")"; \
  CHECKSUM="$(echo "$MANIFEST" | jq -r ".platforms[\"$PLATFORM\"].checksum")"; \
  if [ -z "$CHECKSUM" ] || [ "$CHECKSUM" = "null" ]; then \
    echo "Platform $PLATFORM not found in manifest" >&2; exit 1; \
  fi; \
  curl -fsSL -o /home/agent/claude "$GCS_BUCKET/$VERSION/$PLATFORM/claude"; \
  echo "$CHECKSUM  /home/agent/claude" | sha256sum -c -; \
  chmod +x /home/agent/claude; \
  mkdir -p /home/agent/.claude/bin; \
  mv /home/agent/claude /home/agent/.claude/bin/claude; \
  '

# Ensure the Claude launcher is on PATH.
ENV PATH="/home/agent/.claude/bin:${PATH}"

# Default workdir for mounted projects.
WORKDIR /workspace

# Default command prints version (override in runtime).
CMD ["claude", "--version"]

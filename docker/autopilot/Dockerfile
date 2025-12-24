# Dockerfile for parallel autopilot agents
#
# IMPORTANT: Build the binaries on the host first:
#   cargo build --release -p autopilot -p issues-mcp
#   mkdir -p docker/autopilot/bin
#   cp target/release/autopilot docker/autopilot/bin/
#   cp target/release/issues-mcp docker/autopilot/bin/
#
# Then build the image:
#   docker-compose -f docker/autopilot/docker-compose.yml build
#
# Run:
#   docker-compose -f docker/autopilot/docker-compose.yml up -d

# Using trixie (testing) for glibc 2.39+ compatibility with Arch-built binaries
FROM debian:trixie-slim

# Install runtime dependencies (including procps for healthcheck pgrep)
RUN apt-get update && apt-get install -y \
    git \
    curl \
    ripgrep \
    fd-find \
    jq \
    openssh-client \
    ca-certificates \
    procps \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user with configurable UID to match host
ARG HOST_UID=1000
RUN useradd -m -u ${HOST_UID} -s /bin/bash agent

# Copy pre-built binaries from host
# Build on host with: cargo build --release -p autopilot -p issues-mcp
COPY docker/autopilot/bin/autopilot /usr/local/bin/autopilot
COPY docker/autopilot/bin/issues-mcp /usr/local/bin/issues-mcp
RUN chmod +x /usr/local/bin/autopilot /usr/local/bin/issues-mcp

# Switch to non-root user
USER agent
WORKDIR /home/agent

# Install Claude Code CLI
# Note: The actual installation URL should be verified
RUN curl -fsSL https://claude.ai/install.sh | bash || true

# Add Claude to PATH
ENV PATH="/home/agent/.claude/bin:${PATH}"

# Set working directory to mounted workspace
WORKDIR /workspace

# Environment variables
ENV AUTOPILOT_MODEL=sonnet
ENV AUTOPILOT_FULL_AUTO=true
ENV RUST_LOG=info

# Healthcheck
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD pgrep -x autopilot || exit 1

# Run autopilot in full-auto mode with issues
# The prompt is required but in full-auto mode it just processes issues
# --issues-db points to the shared database mounted from host
ENTRYPOINT ["autopilot", "run", "--full-auto", "--with-issues", "--issues-db", "/shared/autopilot.db", "Process issues from database"]

# Dockerfile for parallel autopilot agents
#
# IMPORTANT: Build the binary on the host first:
#   cargo build --release -p autopilot
#   mkdir -p docker/autopilot/bin
#   cp target/release/autopilot docker/autopilot/bin/
#
# Then build the image:
#   docker-compose -f docker/autopilot/docker-compose.yml build
#
# Run:
#   docker-compose -f docker/autopilot/docker-compose.yml up -d

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    git \
    curl \
    ripgrep \
    fd-find \
    jq \
    openssh-client \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user with configurable UID to match host
ARG HOST_UID=1000
RUN useradd -m -u ${HOST_UID} -s /bin/bash agent

# Copy pre-built autopilot binary from host
# Build on host with: cargo build --release -p autopilot
COPY docker/autopilot/bin/autopilot /usr/local/bin/autopilot
RUN chmod +x /usr/local/bin/autopilot

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
ENTRYPOINT ["autopilot", "run", "--full-auto", "--with-issues"]

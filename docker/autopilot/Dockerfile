# Dockerfile for parallel autopilot agents
# Build: docker build -t autopilot-agent --build-arg HOST_UID=$(id -u) .
# Run: docker-compose up -d

# Stage 1: Build autopilot binary
FROM rust:1.83-bookworm AS builder

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binary
RUN cargo build --release -p autopilot

# Stage 2: Runtime image
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

# Copy autopilot binary
COPY --from=builder /build/target/release/autopilot /usr/local/bin/autopilot
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

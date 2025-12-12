# OpenAgents Agent Container
# Base image with essential tools for autonomous agent execution
#
# Features:
# - Ubuntu 22.04 LTS base
# - Bun (JavaScript/TypeScript runtime)
# - Rust toolchain
# - Git configured for agent commits
# - Claude CLI (placeholder - requires installation)
#
# Build:
#   docker build -t openagents/agent:latest -f docker/agent/Dockerfile .
#
# Usage:
#   docker run -v /path/to/workspace:/workspace openagents/agent:latest

FROM ubuntu:22.04

# Prevent interactive prompts during installation
ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    unzip \
    jq \
    ripgrep \
    fd-find \
    && rm -rf /var/lib/apt/lists/*

# Install Bun (JavaScript/TypeScript runtime)
RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:$PATH"

# Install Rust (for Rust-based projects)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:$PATH"

# Install Node.js (for compatibility)
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install Claude CLI
# Note: Replace with actual install command once available
# For now, we assume Claude CLI will be mounted or the claude command exists
RUN echo '#!/bin/bash\necho "Claude CLI placeholder - mount real CLI at /usr/local/bin/claude"' > /usr/local/bin/claude-placeholder \
    && chmod +x /usr/local/bin/claude-placeholder

# Configure git for agent commits
RUN git config --global user.email "noreply@openagents.com" \
    && git config --global user.name "OpenAgents MechaCoder" \
    && git config --global init.defaultBranch main \
    && git config --global push.autoSetupRemote true \
    && git config --global --add safe.directory '*'

# Create workspace directory
WORKDIR /workspace

# Default environment variables
ENV AGENT_ID=""
ENV TASK_ID=""
ENV GIT_BRANCH=""

# Health check - verify essential tools are available
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD git --version && bun --version && rustc --version

# Default command
CMD ["bash"]

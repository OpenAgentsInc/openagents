# Build stage for nostr-htmx frontend
FROM node:18 as frontend-builder
WORKDIR /app
# Install just command runner and esbuild
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin
RUN npm install -g esbuild
# Copy frontend files
COPY static/ .
# Install dependencies and build
RUN npm install
RUN just build

# Rust builder stage
FROM lukemathwalker/cargo-chef:latest-rust-1.81.0 as chef
WORKDIR /app
RUN apt update && apt install lld clang -y

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin openagents

# Runtime stage
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install certificates and DNS utilities first
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends \
       openssl \
       ca-certificates \
       dnsutils \
    && update-ca-certificates \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

# Verify DNS resolution works
RUN dig +short nostr-db-do-user-17766209-0.h.db.ondigitalocean.com || echo "DNS check failed"

# Copy the built executable
COPY --from=builder /app/target/release/openagents openagents
# Copy static files including the built JS
COPY --from=frontend-builder /app /app/static
# Copy configuration files
COPY configuration /app/configuration/

# Set environment variables
ENV APP_ENVIRONMENT=production
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Create a non-root user and switch to it
RUN useradd -m -u 1001 -U app
RUN chown -R app:app /app
USER app

# List contents for verification
RUN ls -la /app/static

ENTRYPOINT ["./openagents"]

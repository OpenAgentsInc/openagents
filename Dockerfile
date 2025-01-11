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
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

# Copy the built executable
COPY --from=builder /app/target/release/openagents openagents
# Copy static files including the built JS
COPY --from=frontend-builder /app /app/static
# Copy configuration files
COPY --from=builder /app/configuration configuration/

ENV APP_ENVIRONMENT production
ENTRYPOINT ["./openagents"]
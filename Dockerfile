FROM lukemathwalker/cargo-chef:latest-rust-1.81.0 as chef
WORKDIR /app/backend
RUN apt update && apt install lld clang -y

FROM chef as planner
COPY backend/ .
# Compute a lock-like file for our project
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
WORKDIR /app/backend
COPY --from=planner /app/backend/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json
COPY backend/ .
ENV SQLX_OFFLINE true
# Build our project
RUN cargo build --release --bin openagents

# Add Node.js build stage for frontend
FROM node:20-alpine AS frontend-builder
WORKDIR /app/frontend
# Copy frontend files and install dependencies
COPY frontend/ ./
RUN if [ -f package-lock.json ]; then npm ci; else npm install; fi
RUN npm run build

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/backend/target/release/openagents openagents
COPY --from=builder /app/backend/assets assets
COPY --from=frontend-builder /app/frontend/dist frontend/dist
COPY backend/configuration configuration
ENV APP_ENVIRONMENT production
ENTRYPOINT ["./openagents"]

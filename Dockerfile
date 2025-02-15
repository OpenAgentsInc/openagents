FROM lukemathwalker/cargo-chef:latest-rust-1.81.0 as chef
WORKDIR /app
RUN apt update && apt install lld clang -y

FROM chef as planner
COPY . .
# Compute a lock-like file for our project
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
ENV SQLX_OFFLINE true
# Build our project
RUN cargo build --release --bin openagents

# Add Node.js build stage for chat app
FROM node:18 AS chat-builder
WORKDIR /app
COPY chat/package.json chat/yarn.lock ./
RUN yarn install --frozen-lockfile
COPY chat/ .
# Install expo-cli globally and build
RUN yarn global add expo-cli
RUN npx expo export:web

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/openagents openagents
COPY --from=builder /app/assets assets
COPY --from=chat-builder /app/web-build chat/web-build
COPY configuration configuration
ENV APP_ENVIRONMENT production
ENTRYPOINT ["./openagents"]

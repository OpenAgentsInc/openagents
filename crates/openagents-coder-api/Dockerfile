# Build from the workspace root:
#   docker build -f crates/openagents-coder-api/Dockerfile .
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/openagents-coder-api crates/openagents-coder-api
COPY crates/openagents-coder-contract crates/openagents-coder-contract
# Shrink the workspace so unused members do not have to be copied.
RUN python3 -c "from pathlib import Path; import re; p=Path('Cargo.toml'); t=p.read_text(); t=re.sub(r'members = \\[[\\s\\S]*?\\]', 'members = [\"crates/openagents-coder-api\", \"crates/openagents-coder-contract\"]', t, count=1); p.write_text(t)"
RUN cargo build --release -p openagents-coder-api

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/openagents-coder-api /usr/local/bin/openagents-coder-api
ENV PORT=8080
EXPOSE 8080
USER nobody
ENTRYPOINT ["/usr/local/bin/openagents-coder-api"]

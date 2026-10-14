# oa-computers: the lifecycle layer for every computer (crates/oa-computers,
# #11624). Built by deploy/oa-computers/cloudbuild.yaml from a GitHub
# commit; runs as the Cloud Run service `oa-computers`. Runbook:
# docs/cloud/computers.md.
FROM rust:1.97.1-slim-trixie AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY crates ./crates
COPY nips ./nips
COPY tests ./tests
COPY vendor ./vendor
RUN cargo build --release -p oa-computers \
    && strip target/release/oa-computers

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --create-home \
        --home-dir /home/oa-computers --shell /usr/sbin/nologin oa-computers
COPY --from=builder /build/target/release/oa-computers /usr/local/bin/oa-computers

USER 10001:10001
# OA_COMPUTERS_DATABASE_URL, OA_COMPUTERS_TOKEN and BOAT_API_KEY come from
# Secret Manager on the service; BOAT_API_BASE names oa-boat.
ENTRYPOINT ["/usr/local/bin/oa-computers"]

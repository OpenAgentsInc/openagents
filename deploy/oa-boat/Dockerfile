# oa-boat: our own Boat-compatible sandbox service on GCE (crates/oa-boat,
# #11256). Built by deploy/oa-boat/cloudbuild.yaml from a GitHub commit;
# runs as the Cloud Run service `oa-boat`. Runbook: docs/cloud/oa-boat.md.
FROM rust:1.97.1-slim-trixie AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY crates ./crates
COPY nips ./nips
COPY tests ./tests
RUN cargo build --release -p oa-boat \
    && strip target/release/oa-boat

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates openssh-client \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --create-home \
        --home-dir /home/oa-boat --shell /usr/sbin/nologin oa-boat
COPY --from=builder /build/target/release/oa-boat /usr/local/bin/oa-boat

USER 10001:10001
ENV OA_BOAT_STATE=/tmp/oa-boat
# OA_BOAT_TOKEN and OA_BOAT_SSH_KEY come from Secret Manager
# (`oa-boat-api-key`, `oa-boat-ssh-key`) on the service.
ENTRYPOINT ["/usr/local/bin/oa-boat"]

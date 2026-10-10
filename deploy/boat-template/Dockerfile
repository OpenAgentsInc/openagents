# The daily Boat template job (crates/boat-template, issue #10219). Built by
# deploy/boat-template/cloudbuild.yaml from the repository root; runs as the
# Cloud Run job `oa-boat-template`. Runbook: docs/deployment/boat-template.md.
FROM rust:1.97.1-slim-trixie AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY crates ./crates
COPY nips ./nips
COPY tests ./tests
COPY vendor ./vendor
# boat-template embeds the fork-ready script (#10251).
COPY scripts/cloud/boat-fork-ready.sh ./scripts/cloud/boat-fork-ready.sh
# No --locked: main's Cargo.lock is sometimes stale, and this binary only
# needs boat's few dependencies.
RUN cargo build --release -p boat-template \
    && strip target/release/boat-template

FROM debian:13-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --no-create-home \
        --home-dir /nonexistent --shell /usr/sbin/nologin boat-template
COPY --from=builder /build/target/release/boat-template /usr/local/bin/boat-template

USER 10001:10001
# BOAT_API_KEY comes from Secret Manager `boat-api-key` on the job.
ENTRYPOINT ["/usr/local/bin/boat-template"]
CMD ["build"]

# The daily decision-throughput job (docs/inference/decision-throughput.md):
# the harness, Python, git, and `oa-att` for the sealed lanes. Built by
# cloudbuild.yaml from a `git archive` of one commit, run as the Cloud Run
# job `decision-throughput-daily` (setup.sh).
ARG RUST_IMAGE=rust:1.97.1-slim-trixie@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c
ARG BASE_IMAGE=python:3.13-slim-trixie

FROM ${RUST_IMAGE} AS builder
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --locked --release -p oa-att --features net --bin oa-att \
    && strip --strip-all target/release/oa-att && cp target/release/oa-att /oa-att

FROM ${BASE_IMAGE}
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /oa-att /usr/local/bin/oa-att
COPY scripts/bench/decision-throughput /app
WORKDIR /app
ENV OA_ATT=/usr/local/bin/oa-att PYTHONUNBUFFERED=1
ENTRYPOINT ["/app/cloud/run-daily.sh"]

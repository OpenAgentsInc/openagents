# syntax=docker/dockerfile:1.7

# Base image is pinned by digest. Update the digest when the base image tag
# moves; the build script records the digest that was actually used.
#
# Digest recorded for python:3.12-slim-bookworm on 2026-08-27:
# sha256:0f5b26b9518d002b6173fd61daad821fa340635ebfec5bba471013f9ca114579
ARG BASE_IMAGE=python:3.12-slim-bookworm@sha256:0f5b26b9518d002b6173fd61daad821fa340635ebfec5bba471013f9ca114579
ARG HARBOR_RUNNER_VERSION=0.1.0

FROM ${BASE_IMAGE} AS runtime

ARG HARBOR_RUNNER_VERSION
ARG BASE_IMAGE

LABEL org.opencontainers.image.title="OpenAgents harbor-runner" \
      org.opencontainers.image.description="Harbor harness image for OpenAgents Gym runs" \
      org.opencontainers.image.vendor="OpenAgents" \
      org.opencontainers.image.source="https://github.com/OpenAgentsInc/openagents" \
      org.opencontainers.image.version="${HARBOR_RUNNER_VERSION}"

RUN apt-get update \
  && apt-get install -y --no-install-recommends git ca-certificates \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /tmp/install
COPY docker/harbor-runner/requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

WORKDIR /opt/bench
COPY bench/adapters/openagents_coder.py adapters/openagents_coder.py
COPY bench/post_gym_run.py post_gym_run.py
COPY bench/run-suite.sh run-suite.sh
COPY bench/suites suites/
ENV PYTHONPATH=/opt/bench

RUN chmod 0755 /opt/bench/run-suite.sh \
  && harbor --version \
  && python3 /opt/bench/adapters/openagents_coder.py --help

ENTRYPOINT ["harbor"]
CMD ["--version"]

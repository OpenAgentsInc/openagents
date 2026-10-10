# The NIP-ATT attested decision provider (docs/security/private-inference.md):
# Psionic's Clef lane and the pylon's attested serve mode, run as a Google
# Confidential Space workload on Intel TDX. Built by deploy/att/cloudbuild.yaml
# from a `git archive` of one commit, so the image digest is a function of
# the source: base images pinned by digest, --locked builds at fixed paths
# with remapped path prefixes, SOURCE_DATE_EPOCH, and BuildKit's
# rewrite-timestamp. The final image is the pinned Debian base plus two
# binaries and this configuration; nothing is installed at build time.
ARG RUST_IMAGE=rust:1.97.1-slim-trixie@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c
ARG BASE_IMAGE=debian:13-slim@sha256:a29215f6a35e51e22adffa17f89e9d2ef06214e64a2bad10d765c46aea49f11f

FROM ${RUST_IMAGE} AS builder
ARG SOURCE_DATE_EPOCH=0
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH} \
    CARGO_INCREMENTAL=0 \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_DEBUG=0 \
    RUSTFLAGS="--remap-path-prefix=/build=/src --remap-path-prefix=/usr/local/cargo=/cargo -C link-arg=-Wl,--build-id=none"
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
# Psionic is its own workspace (crates/psionic). With no CUDA toolkit in
# the builder, its CUDA backend compiles to stubs: this image runs Clef on
# the CPU only.
RUN cd crates/psionic \
    && cargo build --locked --release -p psionic-serve --bin psionic-openai-server \
    && strip --strip-all target/release/psionic-openai-server \
    && cp target/release/psionic-openai-server /psionic-openai-server
RUN cargo build --locked --release -p pylon --bin pylon \
    && strip --strip-all target/release/pylon \
    && cp target/release/pylon /pylon \
    && sha256sum /psionic-openai-server /pylon

FROM ${BASE_IMAGE}
COPY --from=builder /psionic-openai-server /usr/local/bin/psionic-openai-server
COPY --from=builder /pylon /usr/local/bin/pylon
# Confidential Space launch policy: the operator may set only the release
# ID (the client checks the release against the image digest anyway), and
# the workload's stdout goes to Cloud Logging. The pylon logs no prompt or
# answer.
LABEL "tee.launch_policy.allow_env_override"="OA_ATT_RELEASE" \
      "tee.launch_policy.log_redirect"="always" \
      "org.opencontainers.image.source"="https://github.com/OpenAgentsInc/openagents"
ENV OA_ATT_PUBLISHER=77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e \
    OPENAGENTS_PYLON_HOME=/tmp/pylon
ENTRYPOINT ["/usr/local/bin/pylon", "serve", "--attested", \
    "--weights-url", "https://huggingface.co/ggml-org/Clef-Flash-GGUF/resolve/4a192915ef971886004b5b13294f2b4c7a7fc39d/Clef-Flash-Q4_K_M.gguf", \
    "--weights-sha256", "fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c", \
    "--weights", "/tmp/Clef-Flash-Q4_K_M.gguf", \
    "--psionic", "/usr/local/bin/psionic-openai-server", \
    "--pylon", "att-tdx", "--slots", "1", "--rate", "6"]

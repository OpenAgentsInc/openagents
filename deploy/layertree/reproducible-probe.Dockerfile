# Rebuild the signet research probe from source at fixed paths.
FROM rust@sha256:39f68a3e8e3ff425f8945ffa91128e60ff930d53e17fbb5214e95824bdd46f1b AS builder
ARG SOURCE_DATE_EPOCH
ARG SOURCE_COMMIT
ARG PROBE_EXAMPLE=attestation_probe
ARG PROBE_FEATURES=events
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH} \
    LAYERTREE_SOURCE_COMMIT=${SOURCE_COMMIT} \
    CARGO_INCREMENTAL=0 \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_DEBUG=0 \
    CARGO_PROFILE_RELEASE_STRIP=symbols \
    RUSTFLAGS="--remap-path-prefix=/build=/src --remap-path-prefix=/usr/local/cargo=/cargo -C link-arg=-Wl,--build-id=none"
WORKDIR /build
COPY crates/layertree-tee-lab/ .
RUN --mount=type=cache,id=layertree-repro-cargo,target=/usr/local/cargo/registry \
    --mount=type=cache,id=layertree-repro-target,target=/build/target \
    cargo build --locked --release --features "$PROBE_FEATURES" --example "$PROBE_EXAMPLE" \
    && cp "target/release/examples/$PROBE_EXAMPLE" /attestation_probe \
    && sha256sum /attestation_probe

FROM debian@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
ARG SOURCE_COMMIT
LABEL org.opencontainers.image.source="https://github.com/OpenAgentsInc/openagents" \
    org.opencontainers.image.revision="${SOURCE_COMMIT}" \
    tee.launch_policy.log_redirect="always" \
    tee.launch_policy.allow_env_override="LAYERTREE_SEALED_BLOB"
COPY --from=builder /attestation_probe /usr/local/bin/attestation_probe
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/attestation_probe"]

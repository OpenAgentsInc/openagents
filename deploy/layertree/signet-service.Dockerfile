# The image includes its own signet validator and immutable public tree config.
FROM bitcoin/bitcoin:30.2@sha256:2b1d8a8d23c67b426afa8cd5b3ffd66b8c4ebbbceef3e214df8627aaead1f517 AS bitcoin
FROM rust@sha256:39f68a3e8e3ff425f8945ffa91128e60ff930d53e17fbb5214e95824bdd46f1b AS builder
ARG SOURCE_DATE_EPOCH
ARG SOURCE_COMMIT
ARG SERVICE_CONFIG=service.json
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH} \
    LAYERTREE_SOURCE_COMMIT=${SOURCE_COMMIT} \
    CARGO_TARGET_DIR=/build/target \
    CARGO_INCREMENTAL=0 \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_DEBUG=0 \
    CARGO_PROFILE_RELEASE_STRIP=symbols \
    RUSTFLAGS="--remap-path-prefix=/build=/src --remap-path-prefix=/usr/local/cargo=/cargo -C link-arg=-Wl,--build-id=none"
WORKDIR /build
COPY crates/layertree-tee-lab/ .
RUN --mount=type=cache,id=layertree-repro-cargo,target=/usr/local/cargo/registry \
    --mount=type=cache,id=layertree-repro-target,target=/build/target \
    cp "server/${SERVICE_CONFIG}" /build-selected-service.json \
    && cp /build-selected-service.json server/service.json \
    && cargo build --locked --release --manifest-path server/Cargo.toml --bin layertree-signet-service \
    && cp target/release/layertree-signet-service /signet-service

FROM debian@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
ARG SOURCE_COMMIT
LABEL org.opencontainers.image.source="https://github.com/OpenAgentsInc/openagents" \
    org.opencontainers.image.revision="${SOURCE_COMMIT}" \
    tee.launch_policy.log_redirect="always"
COPY --from=builder /signet-service /usr/local/bin/signet-service
COPY --from=bitcoin /opt/bitcoin-30.2/bin/bitcoind /usr/local/bin/bitcoind
RUN mkdir /state && chown 65532:65532 /state
USER 65532:65532
EXPOSE 8443
ENTRYPOINT ["/usr/local/bin/signet-service"]

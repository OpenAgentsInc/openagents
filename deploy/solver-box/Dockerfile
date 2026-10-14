# The solver container (#11560, spec docs/cloud/solver-on-gcp.md): runs
# `coder-new solver N --engine coder` against one GitHub issue on a box the
# dispatcher (crates/solver-box, `oa-solver-box`) made. Built by
# deploy/solver-box/cloudbuild.yaml from a GitHub commit; the entrypoint is
# deploy/solver-box/solver-box-run. See deploy/solver-box/README.md.
FROM rust:1.97.1-slim-trixie AS builder

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        build-essential pkg-config clang cmake protobuf-compiler libprotobuf-dev \
        libssl-dev libasound2-dev libudev-dev git ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
# The commit `coder --version` names (the build context has no .git).
ARG CODER_BUILD_COMMIT=unknown
ENV CODER_BUILD_COMMIT=$CODER_BUILD_COMMIT CODER_BUILD_DIRTY=0
# The whole checkout: coder-new and its crates read files from across the
# repository at compile time (include_str!).
COPY . .
# No --locked: main's Cargo.lock is sometimes a commit behind.
RUN cargo build --release -p coder-new --bin coder-new \
    && strip target/release/coder-new \
    && mkdir -p /out/tools/scripts/bench \
    && cp target/release/coder-new /out/coder-new \
    && cp -r scripts/filefind scripts/coder /out/tools/scripts/ \
    && cp -r scripts/bench/briefed-ab scripts/bench/traces /out/tools/scripts/bench/ \
    && cp deploy/solver-box/solver-box-run /out/solver-box-run

# The runtime keeps the pinned toolchain (rust-toolchain.toml): the
# solver's harness runs `cargo check`/`cargo test` for the crates a change
# touches.
FROM rust:1.97.1-slim-trixie

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        ca-certificates curl git gh jq procps ripgrep bubblewrap \
        python3 python3-numpy \
        build-essential pkg-config clang cmake protobuf-compiler libprotobuf-dev \
        libssl-dev libasound2-dev libudev-dev \
    && rm -rf /var/lib/apt/lists/* \
    && rustup component add rustfmt clippy \
    && rm -rf /usr/local/rustup/downloads /usr/local/rustup/tmp
COPY --from=builder /out/coder-new /usr/local/bin/coder-new
COPY --from=builder /out/tools /opt/oa-tools
COPY --from=builder /out/solver-box-run /usr/local/bin/solver-box-run
RUN ln -s /usr/local/bin/coder-new /usr/local/bin/coder \
    && chmod 0755 /usr/local/bin/solver-box-run

# Where the solver's briefing scripts are (coder-new issue_run::tools_root).
ENV CODER_ISSUE_RUN_TOOLS=/opt/oa-tools \
    CODER_ISSUE_RUN_CAPTURE=0 \
    CODER_UPDATE=off \
    CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=never
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/solver-box-run"]

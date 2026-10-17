FROM rust:1.97.1-slim-trixie AS build
WORKDIR /build
COPY . .
RUN cargo build --release -p oa-computers --features space --bin computer-credential-broker \
    && strip target/release/computer-credential-broker

FROM debian:13-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --no-create-home --shell /usr/sbin/nologin credential-broker
COPY --from=build /build/target/release/computer-credential-broker /usr/local/bin/computer-credential-broker
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/computer-credential-broker"]

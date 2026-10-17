FROM rust:1.97.1-slim-trixie AS build
WORKDIR /build
COPY . .
RUN cargo build --locked --release --manifest-path deploy/space-credentials/fixture/Cargo.toml \
    && strip deploy/space-credentials/fixture/target/release/space-credential-fixture
FROM debian:13-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --user-group --no-create-home --shell /usr/sbin/nologin fixture \
    && mkdir -p /srv/fixture/work \
    && git init --initial-branch=main /srv/fixture/work \
    && echo 'Synthetic private repository for credential qualification.' >/srv/fixture/work/README.md \
    && git -C /srv/fixture/work add README.md \
    && git -C /srv/fixture/work -c user.name=Qualification -c user.email=qualification@openagents.invalid commit -m 'Add synthetic fixture' \
    && git clone --bare /srv/fixture/work /srv/fixture/repo.git \
    && rm -rf /srv/fixture/work \
    && chown -R 10001:10001 /srv/fixture
COPY --from=build /build/deploy/space-credentials/fixture/target/release/space-credential-fixture /usr/local/bin/space-credential-fixture
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/space-credential-fixture"]

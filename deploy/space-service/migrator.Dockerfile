FROM public.ecr.aws/docker/library/rust:1-bookworm AS build
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config protobuf-compiler libprotobuf-dev && rm -rf /var/lib/apt/lists/*
COPY . .
ENV CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_LTO=false
RUN rustup toolchain install && cargo build --locked --release -p pplx-space-db-migrator
# Retain the qualified API image's libraries and nonroot identity.
FROM us-central1-docker.pkg.dev/openagentsgemini/oa-space-service/native/api@sha256:6c621a723c91746475b7844be817638b29df13eb7ae04ac06e00d7d3254ea09d
COPY --from=build /src/target/release/space-db-migrator /space/bin/space-db-migrator
ARG OA_COMMIT
LABEL org.opencontainers.image.revision="de87b436c35dac056564bdd5b2a113c9046e189b" com.openagents.patch-source="${OA_COMMIT}" com.openagents.patch="migrator-tls-v1"

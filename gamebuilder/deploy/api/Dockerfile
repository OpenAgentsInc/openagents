# The hosted levels API (docs/programmatic-access.md, C6 and C7): `gamebuilder serve --accounts`
# with each account's level files on the mounted volume /data, and socat taking the outside's
# connections (the server binds 127.0.0.1 only; docs/cli.md, "Hosted: serve --accounts").
# The context is the monorepo subset deploy/api/deploy.sh stages from HEAD.
FROM ubuntu:24.04 AS build
ENV DEBIAN_FRONTEND=noninteractive COAX_MUTATION=0
RUN apt-get update -q && apt-get install -yq --no-install-recommends \
      build-essential clang pkg-config libssl-dev git ca-certificates curl perl rsync \
  && rm -rf /var/lib/apt/lists/*
COPY . /w
WORKDIR /w
RUN coax/build.sh \
  && gamebuilder/cli/build.sh --force

FROM ubuntu:24.04
RUN apt-get update -q && apt-get install -yq --no-install-recommends ca-certificates libssl3 socat \
  && rm -rf /var/lib/apt/lists/*
COPY --from=build /w/gamebuilder/build/cli/gamebuilder /usr/local/bin/gamebuilder
COPY gamebuilder/deploy/api/run.sh /usr/local/bin/gamebuilder-api
# Cloud Run gives PORT; the server itself listens on 127.0.0.1:8811.
ENV PORT=8080 GB_ORIGIN=https://gamebuilder.org GB_ACCOUNTS=/data
CMD ["/usr/local/bin/gamebuilder-api"]

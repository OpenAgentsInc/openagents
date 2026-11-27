# First-party GCP oa-boat Cloud Run service image (#14128).
# Pure-Coax generated policy/wire C + thin C11 GCE/HTTP effects driver.
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    clang libcurl4-openssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY src/ ./src/
COPY gen/ ./gen/
RUN clang -std=c11 -O2 -Wall -Wextra -Werror -D_POSIX_C_SOURCE=200809L \
    -I./gen -o /oa-boat ./src/oa-boat.c ./gen/boat-service-wire.generated.c \
    -lcurl -lpthread

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    libcurl4 ca-certificates openssh-client && rm -rf /var/lib/apt/lists/*
COPY --from=build /oa-boat /usr/local/bin/oa-boat
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/oa-boat"]

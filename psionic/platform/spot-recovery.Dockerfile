FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY spot-recovery.generated.c spot-recovery.h spot-recovery-face.c ./
RUN cc -std=c11 -O2 -ffp-contract=off -fno-fast-math spot-recovery.generated.c spot-recovery-face.c -lm -o /spot-recovery
# Infrastructure HTTP effects; no Python, Google SDK or model runtime.
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends curl jq ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /spot-recovery /app/spot-recovery
COPY spot-recovery-run.sh /app/run.sh
ENV PYLON_RECOVERY_DRY_RUN=1
ENTRYPOINT ["/app/run.sh"]

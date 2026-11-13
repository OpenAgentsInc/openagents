# The Coaxial production server, built once: serve --static over a site that
# the platform mounts at /site (a storage bucket on Cloud Run). The page's
# files are not in the image; deploy.sh uploads them to the bucket.
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev && rm -rf /var/lib/apt/lists/*
# server/ is serve.c, ws.c and every part coax/web/serve-parts.sh writes, and
# nothing else: all its C files are the server (#13447). check.sh reports runs
# this line on a build host (serve-parts.sh --check).
COPY server/ /src/
RUN gcc -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE -o /serve /src/*.c -lm

FROM debian:bookworm-slim
COPY --from=build /serve /usr/local/bin/serve
USER nobody
CMD ["serve", "/site", "--static", "--host-dir", "/site/coaxial", "--cache", "0"]

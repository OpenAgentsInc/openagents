# The Coaxial production server: serve --static over a built app. deploy.sh
# stages the context (server sources, host files, the app with its wasm).
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev && rm -rf /var/lib/apt/lists/*
COPY server/ /src/
RUN gcc -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE -o /serve /src/serve.c /src/ws.c

FROM debian:bookworm-slim
COPY --from=build /serve /usr/local/bin/serve
COPY coaxial/ /srv/coaxial/
COPY app/ /srv/app/
USER nobody
CMD ["serve", "/srv/app", "--static", "--host-dir", "/srv/coaxial"]

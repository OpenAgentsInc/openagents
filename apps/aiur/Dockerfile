# Aiur on Cloud Run (CFG-11, #8526).
#
# The build happens on the deploy machine (`bun run build:cloudrun`):
# - dist/client   — Vite client build + prerendered SPA shell (_shell.html)
# - dist/cloudrun — self-contained `bun build --target=bun` server bundle
#   (src/cloudrun/server.ts; only node builtins stay external)
# so this image needs no node_modules and no workspace context.

FROM oven/bun:1

WORKDIR /app

COPY dist/client ./dist/client
COPY dist/cloudrun ./dist/cloudrun

ENV AIUR_CLIENT_DIST=/app/dist/client

EXPOSE 8080

CMD ["bun", "run", "dist/cloudrun/server.js"]

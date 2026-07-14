# Aiur on Cloud Run (CFG-11, #8526).
#
# The build happens on the deploy machine (`pnpm run build:cloudrun`):
# - dist/client   — Vite client build + prerendered SPA shell (_shell.html)
# - dist/cloudrun — self-contained Vite Plus server bundle
#   (src/cloudrun/server.ts; only Node builtins stay external)
# so this image needs no node_modules and no workspace context.

FROM node:24.13.1-bookworm-slim

WORKDIR /app

COPY dist/client ./dist/client
COPY dist/cloudrun ./dist/cloudrun

ENV AIUR_CLIENT_DIST=/app/dist/client

EXPOSE 8080

CMD ["node", "dist/cloudrun/server.mjs"]

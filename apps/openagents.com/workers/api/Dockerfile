# openagents.com monolith on Cloud Run (CFG-9, #8524).
#
# The build happens on the deploy machine (scripts/deploy-cloudrun.sh):
#   dist-cloudrun/server.mjs  — self-contained `vp pack` Node bundle
#                               bundle of src/cloudrun/server.ts
#   dist-cloudrun/preload.js  — bundled cloudflare:workers virtual-module
#                               plugin (src/cloudrun/preload.ts)
#   dist-cloudrun/web-dist/   — the built apps/web SPA assets
# so this image needs no node_modules and no workspace context.

FROM node:24.13.1-bookworm-slim

WORKDIR /app

# Vite Plus may split dynamic imports into sibling .mjs chunks. Copy the whole
# executable module set, not just the two entry files.
COPY dist-cloudrun/*.mjs ./dist-cloudrun/
COPY dist-cloudrun/web-dist ./web-dist
# Sarah removed at owner direction 2026-07-10 (epic #8610): the former
# sarah-ui / sarah-agent / sarah-clips image layers and SARAH_* env are gone.
# #8652 PORTAL-1: /portal Effect Native bundle for openagents.com/portal
COPY dist-cloudrun/portal-ui ./portal-ui
# #8634/#8635 scope 5: /forum* Effect Native bundle for openagents.com/forum
COPY dist-cloudrun/forum-ui ./forum-ui

ENV OPENAGENTS_WEB_DIST=/app/web-dist
ENV PORTAL_UI_DIR=/app/portal-ui
ENV FORUM_UI_DIR=/app/forum-ui

EXPOSE 8080

CMD ["node", "--import", "./dist-cloudrun/preload.mjs", "dist-cloudrun/server.mjs"]

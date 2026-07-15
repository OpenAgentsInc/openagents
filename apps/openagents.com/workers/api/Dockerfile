# openagents.com monolith on Cloud Run (CFG-9, #8524).
#
# The build happens on the deploy machine (scripts/deploy-cloudrun.sh):
#   dist-cloudrun/server.mjs  — self-contained `vp pack` Node bundle
#                               bundle of src/cloudrun/server.ts
#   dist-cloudrun/start-client/ — TanStack Start browser assets
#   dist-cloudrun/start-server/ — TanStack Start SSR handler and chunks
#   dist-cloudrun/astro-ui/ — static Astro landing candidate mounted at /astro
# so this image needs no node_modules and no workspace context.

FROM node:24.13.1-bookworm-slim

WORKDIR /app

# Vite Plus may split dynamic imports into sibling .mjs chunks. Copy the whole
# executable module set, not just the two entry files.
COPY dist-cloudrun/*.mjs ./dist-cloudrun/
COPY dist-cloudrun/node_modules ./node_modules
COPY dist-cloudrun/start-client ./start-client
COPY dist-cloudrun/start-server ./start-server
COPY dist-cloudrun/astro-ui ./astro-ui
# Sarah removed at owner direction 2026-07-10 (epic #8610): the former
# sarah-ui / sarah-agent / sarah-clips image layers and SARAH_* env are gone.
# #8652 PORTAL-1: /portal Effect Native bundle for openagents.com/portal
COPY dist-cloudrun/portal-ui ./portal-ui
# #8634/#8635 scope 5: /forum* Effect Native bundle for openagents.com/forum
COPY dist-cloudrun/forum-ui ./forum-ui

ENV OPENAGENTS_WEB_DIST=/app/start-client
ENV OPENAGENTS_START_CLIENT_DIR=/app/start-client
ENV OPENAGENTS_START_SERVER_ENTRY=/app/start-server/server.js
ENV ASTRO_UI_DIR=/app/astro-ui
ENV PORTAL_UI_DIR=/app/portal-ui
ENV FORUM_UI_DIR=/app/forum-ui

EXPOSE 8080

CMD ["node", "dist-cloudrun/server.mjs"]

# openagents.com monolith on Cloud Run (CFG-9, #8524).
#
# The build happens on the deploy machine (scripts/deploy-cloudrun.sh):
#   dist-cloudrun/server.js   — self-contained `bun build --target=bun`
#                               bundle of src/cloudrun/server.ts
#   dist-cloudrun/preload.js  — bundled cloudflare:workers virtual-module
#                               plugin (src/cloudrun/preload.ts)
#   dist-cloudrun/web-dist/   — the built apps/web SPA assets
# so this image needs no node_modules and no workspace context.

FROM oven/bun:1

WORKDIR /app

COPY dist-cloudrun/server.js ./dist-cloudrun/server.js
COPY dist-cloudrun/preload.js ./dist-cloudrun/preload.js
COPY dist-cloudrun/web-dist ./web-dist
# #8594: Sarah path-mount assets (UI + agent persona) for openagents.com/sarah
COPY dist-cloudrun/sarah-ui ./sarah-ui
COPY dist-cloudrun/sarah-agent ./sarah-agent
# #8610: shippable pre-rendered clip tier (MIT Hallo2 openers-v2 only).
COPY dist-cloudrun/sarah-clips ./sarah-clips
# #8652 PORTAL-1: /portal Effect Native bundle for openagents.com/portal
COPY dist-cloudrun/portal-ui ./portal-ui

ENV OPENAGENTS_WEB_DIST=/app/web-dist
ENV PORTAL_UI_DIR=/app/portal-ui
ENV SARAH_UI_DIR=/app/sarah-ui
ENV SARAH_AGENT_DIR=/app/sarah-agent
ENV SARAH_CLIPS_DIR=/app/sarah-clips
ENV SARAH_PUBLIC_BASE_URL=https://openagents.com/sarah

EXPOSE 8080

CMD ["bun", "--preload", "./dist-cloudrun/preload.js", "dist-cloudrun/server.js"]

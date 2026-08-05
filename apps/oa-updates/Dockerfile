FROM node:24.13.1-bookworm-slim

WORKDIR /app

COPY dist-server ./dist-server
# Pylon release seed: signed Node executables plus a manifest staged by
# scripts/publish-pylon-release.ts. Served at /pylon/<channel>/<platform>/feed.json
# when OA_PYLON_RELEASES_DIST=/app/pylon-dist is set (powers Pylon default-on OTA).
COPY pylon-dist ./pylon-dist

EXPOSE 8080

CMD ["node", "dist-server/serve.mjs"]

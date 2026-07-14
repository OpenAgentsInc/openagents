FROM node:24.13.1-bookworm-slim

WORKDIR /app

COPY dist-server ./dist-server
# Seed bundle: a real `expo export` placed in the build context at deploy time
# (gitignored). Served at startup when OA_SEED_DIST=/app/dist is set.
COPY dist ./dist
# Desktop release seed: full archives plus BSDIFF deltas staged by
# scripts/publish-desktop-release.ts. Served when
# OA_DESKTOP_RELEASES_DIST=/app/desktop-dist is set.
COPY desktop-dist ./desktop-dist
# Pylon release seed: signed Node executables plus a manifest staged by
# scripts/publish-pylon-release.ts. Served at /pylon/<channel>/<platform>/feed.json
# when OA_PYLON_RELEASES_DIST=/app/pylon-dist is set (powers Pylon default-on OTA).
COPY pylon-dist ./pylon-dist
COPY desktop-ota ./desktop-ota
# Independent signed Electron feed. The directory is intentionally empty in
# source; release automation stages only the bounded descriptor + signed
# manifest envelope before a deliberate release deploy.
COPY openagents-desktop-dist ./openagents-desktop-dist

EXPOSE 8080

CMD ["node", "dist-server/serve.mjs"]

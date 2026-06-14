FROM oven/bun:1

WORKDIR /app

COPY package.json ./
COPY src ./src
# Seed bundle: a real `expo export` placed in the build context at deploy time
# (gitignored). Served at startup when OA_SEED_DIST=/app/dist is set.
COPY dist ./dist
# Desktop release seed: full archives plus BSDIFF deltas staged by
# scripts/publish-desktop-release.ts. Served when
# OA_DESKTOP_RELEASES_DIST=/app/desktop-dist is set.
COPY desktop-dist ./desktop-dist

EXPOSE 8080

CMD ["bun", "run", "src/serve.ts"]

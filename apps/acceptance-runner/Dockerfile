# Build from the repository root. Vite Plus bundles owned TypeScript while
# Playwright remains a normal production dependency beside its browser image.
FROM node:24.13.1-bookworm-slim AS build
WORKDIR /workspace
RUN corepack enable && corepack prepare pnpm@11.10.0 --activate
COPY . .
RUN pnpm install --frozen-lockfile
RUN pnpm exec vp pack apps/acceptance-runner/src/service.ts \
      --out-dir /bundle --format esm --platform node --target node24 \
      --deps.never-bundle playwright --deps.never-bundle playwright-core \
  && pnpm --filter @openagentsinc/acceptance-runner deploy --legacy --prod /runtime

FROM mcr.microsoft.com/playwright:v1.61.0-jammy
COPY --from=build /usr/local /usr/local
WORKDIR /app
COPY --from=build /bundle ./bundle
COPY --from=build /runtime/node_modules ./node_modules
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright
ENTRYPOINT ["node", "bundle/service.mjs"]

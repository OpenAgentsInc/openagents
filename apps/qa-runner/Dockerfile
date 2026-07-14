# Build from the repository root. The final image contains Node, the bundled QA
# entrypoint, production dependencies, and Playwright's matched browser only.
FROM node:24.13.1-bookworm-slim AS build
WORKDIR /workspace
RUN corepack enable && corepack prepare pnpm@11.10.0 --activate
COPY . .
RUN pnpm install --frozen-lockfile
RUN pnpm exec vp pack apps/qa-runner/src/demo-login.ts \
      --out-dir /bundle --format esm --platform node --target node24 \
      --deps.never-bundle playwright --deps.never-bundle playwright-core \
  && pnpm --filter @openagentsinc/qa-runner deploy --legacy --prod /runtime

FROM mcr.microsoft.com/playwright:v1.61.0-jammy
COPY --from=build /usr/local /usr/local
WORKDIR /app
COPY --from=build /bundle ./bundle
COPY --from=build /runtime/node_modules ./node_modules
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright
ENTRYPOINT ["node", "bundle/demo-login.mjs"]

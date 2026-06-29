# Out-of-Worker Khala acceptance-verdict runner (EPIC #6017).
#
# A long-running Bun service that leases acceptance jobs from the openagents.com gateway,
# runs the REAL Playwright/chromium headless acceptance suite OUT of the Worker, and POSTs
# the verdict back to the authenticated callback. Chromium NEVER runs in the Worker — it
# runs HERE. INERT without its host secrets (see docs/DEPLOY.md): with the required env
# unset the service refuses to start and never invents work.
#
# Build context is the REPO ROOT (the service bridges into apps/openagents.com worker
# source), e.g.:  docker build -f apps/acceptance-runner/Dockerfile -t oa-acceptance-runner .

# Playwright's official image ships a matched chromium + all system deps already.
FROM mcr.microsoft.com/playwright:v1.61.0-jammy

# Bun (the service + the bridged worker source are Bun/Effect TypeScript).
RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

WORKDIR /app

# Copy the workspace manifests first for a cached install layer, then the source the
# service needs: its own dir + the bridged worker inference source + the catalog root.
COPY package.json bun.lock* ./
COPY apps/acceptance-runner ./apps/acceptance-runner
COPY apps/openagents.com/package.json ./apps/openagents.com/package.json
COPY apps/openagents.com/workers/api ./apps/openagents.com/workers/api

# Install workspace deps (effect, playwright, the api worker's deps the bridge pulls).
RUN bun install --frozen-lockfile || bun install

# Use the chromium the Playwright base image already installed (do not re-download).
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright

# Cloud Run / GCE health: the daemon logs structured JSON to stdout; it has no HTTP
# server of its own (it PULLS work). For a Cloud Run deployment that requires a listening
# port, front it with a tiny health endpoint or run it as a GCE/Compute service instead
# (see docs/DEPLOY.md — GCE is the recommended host).
ENTRYPOINT ["bun", "run", "apps/acceptance-runner/src/service.ts"]

# Khala autonomous-QA runner (EPIC #6174).
# Build context is the REPO ROOT:
#   docker build -f apps/qa-runner/Dockerfile -t oa-qa-runner .
#
# This image runs the LOCAL backend (real chromium on this container). The
# cloudVmBackend (per-run OpenAgents Cloud firecracker / sek8s microVM) is a
# separate owner-gated provisioner wired in `cloud`, not baked here.

# Playwright's official image ships a matched chromium + all system deps, plus
# ffmpeg for the webm->mp4 transcode.
FROM mcr.microsoft.com/playwright:v1.61.0-jammy

# Bun (the app + the bridged probe-runtime are Bun/Effect TypeScript).
RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

WORKDIR /app

# Manifests first for a cached install layer, then the source the app needs:
# its own dir + the probe runtime it bridges + the workspace root.
COPY package.json bun.lock* ./
COPY apps/qa-runner ./apps/qa-runner
COPY packages/probe ./packages/probe

# Install workspace deps (effect, playwright, probe-runtime).
RUN bun install --ignore-scripts --frozen-lockfile || bun install --ignore-scripts

# Use the chromium the Playwright base image already installed.
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright

# Default to the headline demo (Khala /login regression on openagents.com).
# Override CMD to run-once against a different target.
ENTRYPOINT ["bun", "run", "apps/qa-runner/src/demo-login.ts"]

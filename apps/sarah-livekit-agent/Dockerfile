FROM node:24.13.1-bookworm-slim@sha256:85a395c77b811fa7f5b5e4aa69cd6eb4c3b80c7f1a8e34704dc0ce061e5b404e AS build

ARG IMAGE_REVISION=unknown
LABEL org.opencontainers.image.revision="${IMAGE_REVISION}"

WORKDIR /workspace
RUN apt-get update \
  && apt-get install --yes --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
RUN corepack enable && corepack prepare pnpm@11.10.0 --activate
COPY . .
RUN pnpm install --frozen-lockfile --ignore-scripts
RUN pnpm --dir apps/sarah-livekit-agent build \
  && pnpm --filter @openagentsinc/sarah-livekit-agent deploy \
    --legacy --prod --ignore-scripts --config.allowUnusedPatches=true /runtime

FROM node:24.13.1-bookworm-slim@sha256:85a395c77b811fa7f5b5e4aa69cd6eb4c3b80c7f1a8e34704dc0ce061e5b404e

ARG IMAGE_REVISION=unknown
LABEL org.opencontainers.image.revision="${IMAGE_REVISION}"

RUN apt-get update \
  && apt-get install --yes --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && groupadd --system --gid 10001 sarah \
  && useradd --system --uid 10001 --gid 10001 --create-home sarah
WORKDIR /app
COPY --from=build --chown=sarah:sarah \
  /workspace/apps/sarah-livekit-agent/dist ./dist
COPY --from=build --chown=sarah:sarah /runtime/node_modules ./node_modules
USER 10001:10001

ENTRYPOINT ["node", "dist/agent.mjs"]
CMD ["start"]

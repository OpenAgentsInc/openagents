FROM node:24.13.1-bookworm-slim@sha256:85a395c77b811fa7f5b5e4aa69cd6eb4c3b80c7f1a8e34704dc0ce061e5b404e AS build

ARG IMAGE_REVISION=unknown
LABEL org.opencontainers.image.revision="${IMAGE_REVISION}"

WORKDIR /workspace
RUN corepack enable && corepack prepare pnpm@11.10.0 --activate
COPY . .
RUN pnpm install --frozen-lockfile
RUN pnpm --dir apps/sarah-livekit-agent build \
  && pnpm --filter @openagentsinc/sarah-livekit-agent deploy \
    --legacy --prod /runtime

FROM node:24.13.1-bookworm-slim@sha256:85a395c77b811fa7f5b5e4aa69cd6eb4c3b80c7f1a8e34704dc0ce061e5b404e

ARG IMAGE_REVISION=unknown
LABEL org.opencontainers.image.revision="${IMAGE_REVISION}"

RUN groupadd --system sarah \
  && useradd --system --gid sarah --create-home sarah
WORKDIR /app
COPY --from=build --chown=sarah:sarah \
  /workspace/apps/sarah-livekit-agent/dist ./dist
COPY --from=build --chown=sarah:sarah /runtime/node_modules ./node_modules
USER sarah

ENTRYPOINT ["node", "dist/agent.mjs"]
CMD ["start"]

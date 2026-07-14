# khala-capture on Cloud Run (#8554).
#
# The build happens on the deploy machine (`pnpm run build:cloudrun`):
# dist/server.mjs is a self-contained Vite Plus bundle
# (src/server.ts + the bundled @openagentsinc/khala-sync-server capture daemon
# and its `postgres` driver; only Node builtins stay external), so this
# image needs no node_modules and no workspace context.

FROM node:24.13.1-bookworm-slim

WORKDIR /app

COPY dist/server.mjs ./dist/server.mjs

EXPOSE 8080

CMD ["node", "dist/server.mjs"]

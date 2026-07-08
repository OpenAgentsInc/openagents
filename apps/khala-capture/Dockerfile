# khala-capture on Cloud Run (#8554).
#
# The build happens on the deploy machine (`bun run build:cloudrun`):
# dist/server.js is a self-contained `bun build --target=bun` bundle
# (src/server.ts + the bundled @openagentsinc/khala-sync-server capture daemon
# and its `postgres` driver; only bun/node builtins stay external), so this
# image needs no node_modules and no workspace context.

FROM oven/bun:1

WORKDIR /app

COPY dist/server.js ./dist/server.js

EXPOSE 8080

CMD ["bun", "run", "dist/server.js"]

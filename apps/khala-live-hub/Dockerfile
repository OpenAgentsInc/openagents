# khala-live-hub on Cloud Run (CFG-5, #8520).
#
# The build happens on the deploy machine (`pnpm run build:cloudrun`):
# dist/server.mjs is a self-contained Vite Plus bundle
# (src/server.ts; only Node builtins stay external), so this image
# needs no node_modules and no workspace context.

FROM node:24.13.1-bookworm-slim

WORKDIR /app

COPY dist/server.mjs ./dist/server.mjs

EXPOSE 8080

CMD ["node", "dist/server.mjs"]

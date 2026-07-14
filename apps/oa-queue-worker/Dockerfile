# oa-queue-worker on Cloud Run (CFG-7, #8522).
#
# The build happens on the deploy machine (`pnpm run build`):
#   dist/main.mjs — self-contained Vite Plus bundle
# so this image needs no node_modules and no workspace context.

FROM node:24.13.1-bookworm-slim

WORKDIR /app

COPY dist ./dist

EXPOSE 8080

CMD ["node", "dist/main.mjs"]

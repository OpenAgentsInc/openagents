# oa-queue-worker on Cloud Run (CFG-7, #8522).
#
# The build happens on the deploy machine (`bun run build`):
#   dist/server.js — self-contained `bun build --target=bun` bundle
# so this image needs no node_modules and no workspace context.

FROM oven/bun:1

WORKDIR /app

COPY dist ./dist

EXPOSE 8080

CMD ["bun", "run", "dist/server.js"]

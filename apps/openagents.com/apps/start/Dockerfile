FROM oven/bun:1

WORKDIR /app

COPY dist/server ./dist/server
COPY dist/client ./dist/client
COPY cloudrun/server.mjs ./cloudrun/server.mjs

ENV OPENAGENTS_START_CLIENT_DIR=/app/dist/client

EXPOSE 8080

CMD ["bun", "cloudrun/server.mjs"]

FROM node:24.13.1-bookworm-slim

WORKDIR /app

COPY dist/server ./dist/server
COPY dist/client ./dist/client
COPY dist/cloudrun ./dist/cloudrun

ENV OPENAGENTS_START_CLIENT_DIR=/app/dist/client

EXPOSE 8080

CMD ["node", "dist/cloudrun/server.mjs"]

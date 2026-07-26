FROM node:24.13.1-bookworm-slim@sha256:a81a03dd965b4052269a57fac857004022b522a4bf06e7a739e25e18bce45af2

RUN apt-get update \
  && apt-get install -y --no-install-recommends git ca-certificates \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY dist ./dist

WORKDIR /app/dist

ENV PORT=8080
ENV FORGE_GIT_REPOSITORY_ROOT=/var/lib/forge/repositories

EXPOSE 8080

CMD ["node", "--import", "tsx", "main.mjs"]

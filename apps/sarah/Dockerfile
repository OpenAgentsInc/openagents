# syntax=docker/dockerfile:1.7
FROM oven/bun:1.3 AS runtime
WORKDIR /app
COPY package.json bun.lock ./
COPY apps/sarah ./apps/sarah
COPY packages ./packages
RUN bun install --production
ENV SARAH_PORT=8080
EXPOSE 8080
CMD ["bun", "apps/sarah/src/server.ts"]

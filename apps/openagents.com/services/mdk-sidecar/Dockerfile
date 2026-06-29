FROM oven/bun:1.3.1

WORKDIR /app

COPY package.json bun.lock* ./
RUN bun install --production

COPY src ./src
COPY scripts ./scripts

ENV NODE_ENV=production
EXPOSE 8080

CMD ["bun", "run", "start"]

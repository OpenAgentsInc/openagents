FROM oven/bun:1

WORKDIR /app

COPY package.json ./
COPY src ./src

EXPOSE 8080

CMD ["bun", "run", "src/serve.ts"]

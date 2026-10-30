# The API's Spark wallet (wallet.cjs): Node with the Breez Spark SDK, pinned.
FROM node:22.20.0-bookworm AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --omit=dev --no-audit --no-fund
COPY wallet.cjs ./

FROM node:22.20.0-bookworm-slim
WORKDIR /app
COPY --from=build /app /app
USER node
ENV NODE_ENV=production SPARK_DIR=/tmp/spark
CMD ["node", "wallet.cjs"]

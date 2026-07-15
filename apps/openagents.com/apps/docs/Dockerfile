FROM node:24.13.1-bookworm-slim

WORKDIR /app
ENV NODE_ENV=production
ENV PORT=8080

COPY server.mjs ./server.mjs
COPY dist ./dist

USER node
EXPOSE 8080
CMD ["node", "server.mjs"]

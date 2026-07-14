FROM node:24.13.1-bookworm-slim
WORKDIR /app
COPY dist/main.mjs ./dist/main.mjs
EXPOSE 8080
CMD ["node", "dist/main.mjs"]

FROM node:20-alpine AS development-dependencies-env
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM node:20-alpine AS build-env
WORKDIR /app
COPY . .
COPY --from=development-dependencies-env /app/node_modules ./node_modules
RUN npm run build

FROM node:20-alpine AS runtime
WORKDIR /app
COPY --from=build-env /app/dist ./dist
COPY package.json package-lock.json ./
RUN npm ci --omit=dev
CMD ["npm", "run", "start"]

ARG NODE_VERSION=lts-bookworm-slim
ARG NGINX_VERSION=alpine

FROM node:${NODE_VERSION} AS builder

WORKDIR /app

RUN git clone https://github.com/Blockstream/esplora.git .

RUN npm ci

ENV API_URL=http://localhost:4002/api
ENV SITE_TITLE="Bitcoin regtest explorer"

RUN npm run dist

RUN mv dist dist-bitcoin

ENV API_URL=http://localhost:4003/api
ENV IS_ELEMENTS=1
ENV NATIVE_ASSET_ID=5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225
ENV NATIVE_ASSET_LABEL=L-BTC
ENV SITE_TITLE="Liquid regtest explorer"

RUN npm run dist

FROM nginx:${NGINX_VERSION}

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist-bitcoin /usr/share/nginx/bitcoin
COPY --from=builder /app/dist /usr/share/nginx/liquid

EXPOSE 4002

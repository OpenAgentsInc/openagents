FROM docker.io/cloudflare/sandbox:0.7.0

ENV NODE_VERSION=22.13.1
RUN ARCH="$(dpkg --print-architecture)" \
    && case "${ARCH}" in \
         amd64) NODE_ARCH="x64" ;; \
         arm64) NODE_ARCH="arm64" ;; \
         *) echo "Unsupported architecture: ${ARCH}" >&2; exit 1 ;; \
       esac \
    && apt-get update && apt-get install -y xz-utils ca-certificates rsync \
    && curl -fsSLk https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${NODE_ARCH}.tar.xz -o /tmp/node.tar.xz \
    && tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1 \
    && rm /tmp/node.tar.xz \
    && node --version \
    && npm --version

RUN npm install -g clawdbot@2026.1.24-3 \
    && clawdbot --version \
    && ln -sf "$(command -v clawdbot)" /usr/local/bin/openclaw

RUN mkdir -p /root/.clawdbot \
    && mkdir -p /root/clawd/skills \
    && mkdir -p /data/openclaw

COPY start-openclaw.sh /usr/local/bin/start-openclaw.sh
RUN chmod +x /usr/local/bin/start-openclaw.sh

WORKDIR /root/clawd
EXPOSE 18789

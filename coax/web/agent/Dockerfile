# The project agent's runner (runner.c, with /chat's general mode in
# chat-runner.c and hosted seats in seat-runner.c) and engines (turn.coax,
# chat.coax and seat.coax, compiled to C by deploy.sh before the build), with
# the claude CLI the engines drive, git and ssh with GitHub's host keys pinned,
# and Chromium for hosted seats' headless pages (docs/verse/realm-seat.md). No
# shell tools reach the model.
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev libsecp256k1-dev libssl-dev libcurl4-openssl-dev && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
RUN gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I/src -o /oa-agent /src/runner.c /src/runner-sign.c -lsecp256k1 -lcrypto -lcurl -lm \
 && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -o /oa-turn /src/turn.c /src/process-native.c /src/engine-io.c -lm \
 && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -o /oa-chat-turn /src/chat.c /src/process-native.c /src/engine-io.c -lm \
 && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -I/src/seat -o /oa-seat /src/seat/seat.c /src/seat/tool-native.c /src/seat/process-native.c \
      /src/seat/store-file-native.c /src/seat/read-file-native.c /src/seat/socket-native.c /src/seat/entropy-native.c \
      /src/seat/entropy-foreign.c /src/seat/entropy.c /src/engine-io.c -lpthread -lm

FROM node:22-bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends git openssh-client ca-certificates tzdata libsecp256k1-1 libssl3 libcurl4 \
      chromium fonts-dejavu-core \
 && rm -rf /var/lib/apt/lists/* && npm install -g @anthropic-ai/claude-code@2.1.312 && npm cache clean --force
COPY --from=build /oa-agent /usr/local/bin/oa-agent
COPY --from=build /oa-turn /usr/local/bin/oa-turn
COPY --from=build /oa-chat-turn /usr/local/bin/oa-chat-turn
COPY --from=build /oa-seat /usr/local/bin/oa-seat
COPY known_hosts /etc/oa-agent/known_hosts
# Hosted seats: Debian's chromium wrapper adds CHROMIUM_FLAGS, and CDP_WEBGPU makes cdp.coax ask for
# SwiftShader's WebGPU. Cloud Run has no GPU and no user namespaces for Chromium's sandbox, so the
# sandbox is off: the only page it opens is our own site's realm.
ENV OA_AGENT_ENGINE=/usr/local/bin/oa-turn OA_AGENT_WORK=/tmp/oa-agent OA_KNOWN_HOSTS=/etc/oa-agent/known_hosts OA_GIT=/usr/bin/git DISABLE_AUTOUPDATER=1 \
    OA_CHAT_ENGINE=/usr/local/bin/oa-chat-turn OA_CLAUDE=/usr/local/bin/claude \
    OA_SEAT_ENGINE=/usr/local/bin/oa-seat CHROME=/usr/bin/chromium CDP_WEBGPU=swiftshader \
    CHROMIUM_FLAGS="--no-sandbox"
USER node
CMD ["oa-agent", "--listen", "0.0.0.0:8080"]

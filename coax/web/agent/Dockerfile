# The project agent's runner (runner.c, with /chat's general mode in
# chat-runner.c) and engines (turn.coax and chat.coax, compiled to C by
# deploy.sh before the build), with the claude CLI the engines drive, git
# and ssh with GitHub's host keys pinned. No shell tools reach the model.
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev libsecp256k1-dev libssl-dev libcurl4-openssl-dev && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
RUN gcc -std=c11 -O2 -D_DEFAULT_SOURCE -o /oa-agent /src/runner.c -lsecp256k1 -lcrypto -lcurl \
 && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -o /oa-turn /src/turn.c /src/process-native.c /src/engine-io.c -lm \
 && gcc -std=c11 -O2 -D_DEFAULT_SOURCE -o /oa-chat-turn /src/chat.c /src/process-native.c /src/engine-io.c -lm

FROM node:22-bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends git openssh-client ca-certificates tzdata libsecp256k1-1 libssl3 libcurl4 \
 && rm -rf /var/lib/apt/lists/* && npm install -g @anthropic-ai/claude-code@2.1.312 && npm cache clean --force
COPY --from=build /oa-agent /usr/local/bin/oa-agent
COPY --from=build /oa-turn /usr/local/bin/oa-turn
COPY --from=build /oa-chat-turn /usr/local/bin/oa-chat-turn
COPY known_hosts /etc/oa-agent/known_hosts
ENV OA_AGENT_ENGINE=/usr/local/bin/oa-turn OA_AGENT_WORK=/tmp/oa-agent OA_KNOWN_HOSTS=/etc/oa-agent/known_hosts OA_GIT=/usr/bin/git DISABLE_AUTOUPDATER=1 \
    OA_CHAT_ENGINE=/usr/local/bin/oa-chat-turn OA_CLAUDE=/usr/local/bin/claude
USER node
CMD ["oa-agent", "--listen", "0.0.0.0:8080"]

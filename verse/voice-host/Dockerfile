# The voice host on Cloud Run (verse/voice-host/deploy.sh stages src/: the C
# sources, verse/platform/voice.c and .h, and the Coax engine already turned
# into C by coax/bin/coax). Google's speech and Vertex AI are called with the
# service's own account, from the metadata server (OA_VOICE_TOKEN=metadata).
FROM debian:bookworm-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev libcurl4-openssl-dev libssl-dev && rm -rf /var/lib/apt/lists/*
COPY src/ /src/
WORKDIR /src
RUN cp -r /src/platform /platform
RUN for c in gen/*.c; do gcc -std=c11 -O2 -w -Igen -c "$c" -o "${c%.c}.o"; done \
 && f="-std=c11 -O2 -D_GNU_SOURCE -Wall -Wno-unused-parameter -Wno-unused-function -Wno-unused-variable -Wno-sign-compare -Igen -Iplatform" \
 && gcc $f -c platform/voice.c -o voice.o \
 && gcc $f -o /voice-host host.c ws.c wav.c speech.c voice.o gen/*.o -lcurl -lcrypto -lpthread -lm

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libcurl4 libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /voice-host /usr/local/bin/voice-host
ENV OA_VOICE_TOKEN=metadata HOME=/tmp
USER nobody
CMD ["sh", "-c", "exec voice-host --listen 0.0.0.0:${PORT:-8080} --provider google --agent gemini --max-speakers 16"]

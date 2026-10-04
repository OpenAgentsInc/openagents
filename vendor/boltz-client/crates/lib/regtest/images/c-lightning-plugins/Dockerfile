ARG VERSION=latest
ARG UBUNTU_VERSION=24.04

FROM ubuntu:${UBUNTU_VERSION} AS builder

ARG HOLD_VERSION=v0.3.2
ARG CLNURL_VERSION=v0.1.0

RUN apt-get update && apt-get install -y curl

# Download hold plugin
WORKDIR /hold
RUN curl -fsSL https://github.com/BoltzExchange/hold/releases/download/${HOLD_VERSION}/hold-linux-$(dpkg --print-architecture).tar.gz -o hold.tar.gz

RUN tar -xvf hold.tar.gz && mv build/hold-linux-$(dpkg --print-architecture) hold

# Download clnurl plugin
WORKDIR /clnurl
RUN ARCH=$(dpkg --print-architecture) && \
    if [ "$ARCH" = "amd64" ]; then \
        curl -fsSL https://github.com/michael1011/clnurl/releases/download/${CLNURL_VERSION}/clnurl.tar.gz -o clnurl.tar.gz; \
    else \
        curl -fsSL https://github.com/michael1011/clnurl/releases/download/${CLNURL_VERSION}/clnurl-arm64.tar.gz -o clnurl.tar.gz; \
    fi

RUN tar -xvf clnurl.tar.gz

FROM boltz/c-lightning:${VERSION}

COPY --from=builder /hold/hold /usr/local/bin/hold
COPY --from=builder /clnurl/clnurl /usr/local/bin/clnurl

ENTRYPOINT ["lightningd"]

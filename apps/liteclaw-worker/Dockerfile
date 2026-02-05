FROM docker.io/cloudflare/sandbox:0.7.0

ENV PATH="/root/.opencode/bin:${PATH}"

RUN curl -fsSL https://opencode.ai/install -o /tmp/install-opencode.sh \
    && bash /tmp/install-opencode.sh \
    && rm /tmp/install-opencode.sh \
    && opencode --version

RUN mkdir -p /workspace

WORKDIR /workspace

EXPOSE 4096

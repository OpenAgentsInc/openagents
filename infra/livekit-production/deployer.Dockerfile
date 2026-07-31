FROM node:24.13.1-bookworm-slim@sha256:85a395c77b811fa7f5b5e4aa69cd6eb4c3b80c7f1a8e34704dc0ce061e5b404e AS node
FROM docker.io/alpine/helm:3.18.6@sha256:c6d8088ddb279625a2e1ca3b08b22c18c946d1f65c8b810f28f1597435a1134c AS helm

FROM gcr.io/google.com/cloudsdktool/google-cloud-cli:562.0.0-slim@sha256:0da2de42ae51cd6092b5caf7c96d25003659fd15548760eb4c50c2623e7bf9c2

RUN apt-get update \
    && apt-get install --no-install-recommends --yes \
      ca-certificates \
      curl \
      dnsutils \
      git \
      google-cloud-cli-gke-gcloud-auth-plugin \
      openssl \
    && curl --fail --location --silent --show-error \
      --output /usr/local/bin/kubectl \
      https://dl.k8s.io/release/v1.34.1/bin/linux/amd64/kubectl \
    && echo "7721f265e18709862655affba5343e85e1980639395d5754473dafaadcaa69e3  /usr/local/bin/kubectl" \
      | sha256sum --check --strict \
    && chmod 0755 /usr/local/bin/kubectl \
    && curl --fail --location --silent --show-error \
      --output /usr/local/bin/yq \
      https://github.com/mikefarah/yq/releases/download/v4.53.3/yq_linux_amd64 \
    && echo "fa52a4e758c63d38299163fbdd1edfb4c4963247918bf9c1c5d31d84789eded4  /usr/local/bin/yq" \
      | sha256sum --check --strict \
    && chmod 0755 /usr/local/bin/yq \
    && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=helm /usr/bin/helm /usr/local/bin/helm

RUN node --version \
    && gcloud version \
    && gke-gcloud-auth-plugin --version \
    && helm version --short \
    && kubectl version --client \
    && yq --version

WORKDIR /workspace

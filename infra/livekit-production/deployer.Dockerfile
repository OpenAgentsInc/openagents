FROM node:24.13.1-bookworm-slim@sha256:85a395c77b811fa7f5b5e4aa69cd6eb4c3b80c7f1a8e34704dc0ce061e5b404e AS node
FROM docker.io/alpine/helm:3.18.6@sha256:c6d8088ddb279625a2e1ca3b08b22c18c946d1f65c8b810f28f1597435a1134c AS helm
FROM registry.k8s.io/kubectl:v1.34.1@sha256:59bafa07ff3a6d4b417e7633ddb9d79a9606ca98bf64bac080b3e65748669250 AS kubectl

FROM gcr.io/google.com/cloudsdktool/google-cloud-cli:562.0.0-slim@sha256:0da2de42ae51cd6092b5caf7c96d25003659fd15548760eb4c50c2623e7bf9c2

RUN apt-get update \
    && apt-get install --no-install-recommends --yes \
      ca-certificates \
      curl \
      dnsutils \
      git \
      google-cloud-cli-gke-gcloud-auth-plugin \
      openssl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=helm /usr/bin/helm /usr/local/bin/helm
COPY --from=kubectl /usr/local/bin/kubectl /usr/local/bin/kubectl

RUN node --version \
    && gcloud version \
    && gke-gcloud-auth-plugin --version \
    && helm version --short \
    && kubectl version --client

WORKDIR /workspace

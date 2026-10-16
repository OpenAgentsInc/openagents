FROM debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
ARG SOURCE_COMMIT
LABEL "tee.launch_policy.log_redirect"="always" \
      org.opencontainers.image.source="https://github.com/OpenAgentsInc/openagents" \
      org.opencontainers.image.revision="${SOURCE_COMMIT}"
COPY attestation_probe /usr/local/bin/attestation-probe
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/attestation-probe"]

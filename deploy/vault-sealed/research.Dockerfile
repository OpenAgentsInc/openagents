# One-shot synthetic vault qualification. The binary is built from the recorded
# commit with Cargo's locked dependencies; this recipe does not claim a rebuild.
FROM debian@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
ARG SOURCE_COMMIT
LABEL org.opencontainers.image.source="https://github.com/OpenAgentsInc/openagents" \
      org.opencontainers.image.revision="${SOURCE_COMMIT}" \
      tee.launch_policy.log_redirect="always" \
      tee.launch_policy.allow_env_override="OA_VAULT_RESEARCH_RECORDS"
COPY cloud_fixture /usr/local/bin/cloud_fixture
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/cloud_fixture", "run"]

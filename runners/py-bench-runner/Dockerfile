FROM gcr.io/google.com/cloudsdktool/google-cloud-cli:slim

WORKDIR /app
COPY openagents_bench ./openagents_bench

ENV PYTHONPATH=/app
# CND-047 uses Harbor as the first Terminal-Bench compatibility adapter.
# Install Harbor in a later image layer or bake it into a derived private image
# before running non-dry-run Terminal-Bench tasks.
# CND-048 uses Codex as the first OpenAgents coding-agent adapter. Install the
# Codex CLI and inject session-scoped auth through the existing Cloud grant path
# in a derived private image; do not bake credentials into this image.
ENTRYPOINT ["python3", "-m", "openagents_bench.run_task"]

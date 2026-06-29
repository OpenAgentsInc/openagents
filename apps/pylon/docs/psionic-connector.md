# Psionic Connector

Pylon treats Psionic as an optional attach-only ML substrate. The normal
`@openagentsinc/pylon` install does not bundle Psionic binaries, does not bundle
model weights, and does not download either on startup.

`pylon status --json` projects the connector as
`openagents.pylon.psionic_connector.v0.3` with four public-safe phases:

- `absent`: no configured Psionic endpoint and no discoverable local Psionic
  binary. The blocker is `blocker.psionic_qwen35.connector_unconfigured`.
- `configured`: a Psionic binary is discoverable, but no service endpoint is
  configured. Pylon does not launch the binary automatically.
- `negotiated`: a configured Psionic OpenAI-compatible service passed health,
  endpoint, model-row, and admission checks.
- `refused`: an explicit connector hint exists but cannot be used. Refusals use
  typed blocker refs such as `blocker.psionic_qwen35.binary_missing`,
  `blocker.psionic_qwen35.service_unconfigured`,
  `blocker.psionic_qwen35.health_unreachable`,
  `blocker.psionic_qwen35.execution_engine_not_psionic`, and
  `blocker.psionic_qwen35.qwen35_model_missing`.

Discovery sources:

- service endpoint: `PYLON_PSIONIC_BASE_URL`, then `PROBE_PSIONIC_BASE_URL`;
- binary override: `PYLON_PSIONIC_BIN`, then `PSIONIC_BIN`;
- path discovery: `psionic-openai-server` or `psionic-sidecar` on `PATH`.

The connector projection intentionally omits raw endpoint URLs and local binary
paths. It reports source refs, endpoint refs, capability refs, model refs,
blocker refs, and availability receipt refs only.

Psionic owns model serving, training jobs, evals, artifact identity, and ML
worker receipts. Pylon owns connector state, assignment/presence/wallet posture,
sandbox policy, and public-safe closeout refs.

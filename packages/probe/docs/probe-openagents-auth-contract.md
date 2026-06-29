# Probe/OpenAgents product surface ChatGPT Account Contract

Date: 2026-06-07

Status: implemented contract slice for Probe issue #157.

## Contract

OpenAgents product surface is the account authority for ChatGPT/Codex accounts. Probe consumes
account refs and grants issued by OpenAgents product surface; Probe does not own long-lived
ChatGPT/OAuth account material by default.

The provider key is `chatgpt_codex`. A usable provider account must be:

- `provider: "chatgpt_codex"`
- `status: "connected"`
- `health: "healthy"`
- backed by a public secret reference such as `codex-auth://...`

Probe public projections may include refs, labels, status, health, plan type,
fleet state, and public secret refs. They must not include raw bearer tokens,
OpenAI API keys, OAuth fields, auth JSON, or JWT-looking material.

## Implemented Probe Surface

The first Probe code slice lives in
`packages/runtime/src/contracts/provider-account.ts`. It uses the same
Effect v4 line as OpenAgents product surface (`effect@4.0.0-beta.70`) and exports:

- `CHATGPT_CODEX_PROVIDER`
- provider account status and health types
- `PublicProviderAccount`
- `ProbeAuthGrantRequest`
- `isPublicSecretRef`
- `assertPublicSecretRef`
- `containsSecretMaterial`
- `canIssueProviderAccountGrant`
- `assertProbePublicProjection`
- `validateProbePublicProjection`
- `sanitizeProbePublicProjection`

The tests in `packages/runtime/tests/provider-account.test.ts` cover multiple
connected accounts, grant readiness, Effect validation, redaction, and
rejection of raw credential material.

## Open Boundaries

OpenAgents product surface still needs to expose the runner-authorized grant resolution route and
Probe-shaped materialization names. The first assignment and grant-resolution
slice is documented in `docs/probe-openagents-run-assignment.md`. Later Probe
issues add per-run materialization, SHC/Pylon runner identity, CLI account
management, and fleet telemetry.

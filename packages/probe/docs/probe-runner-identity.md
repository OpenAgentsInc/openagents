# Probe Runner Identity For OpenAgents product surface Grants

Date: 2026-06-07

Status: implemented contract slice for Probe issue #160.

## Contract

Probe separates runner identity from ChatGPT/Codex provider account identity.

A runner identity proves which local install, SHC box, Pylon node, or sandbox
runner is allowed to talk to OpenAgents product surface. A provider account ref proves which
ChatGPT/Codex account OpenAgents product surface selected for the run. A grant binds those two only
for one assigned execution context.

## Implemented Probe Surface

`packages/runtime/src/runner/identity.ts` exports:

- `ProbeRunnerIdentity`
- `ProbeRunnerAssignmentProof`
- `authorizeRunnerForAssignment`
- `prepareAuthorizedProbeAuthRun`
- `makeStaticProbeSecretBroker`

The authorization gate checks:

- runner capabilities include `probe.run`
- ChatGPT/Codex assignments also include `openagents.grant.resolve`
- Apple FM assignments include `probe.backend.apple_fm_bridge`
- proof runner id matches linked runner id
- proof assignment id matches the assignment
- proof runner session id matches the assignment
- runner link is not expired
- runner/proof/assignment projections do not contain raw credentials

`prepareAuthorizedProbeAuthRun` authorizes the runner, resolves the OpenAgents product surface
grant, asks a secret broker for the matching secret ref, and materializes auth
inside the run home.

Apple FM backend assignments use the same runner/proof identity gate but skip
grant resolution and auth materialization. The assignment runner rejects the run
unless the linked runner declares `probe.backend.apple_fm_bridge` and live Apple
FM health is ready.

## Future Integration

The current proof object carries `proofKind` and optional `signatureRef`.
Pylon/NIP-98 signature verification and SHC broker-specific proof verification
should plug into this boundary before production runners resolve grants.

## Tests

`packages/runtime/tests/runner-identity.test.ts` covers:

- linked runner success
- unlinked/missing-capability denial
- mismatched runner proof denial
- mismatched assignment proof denial
- raw credential rejection in SHC/Pylon assignment payloads
- scrub receipt emission after sandbox closeout
- Apple FM backend assignments requiring backend capability without
  `openagents.grant.resolve`

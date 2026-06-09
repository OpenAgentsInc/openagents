# Probe Apple FM Backend

Date: 2026-06-07

Status: implemented contract, attach/status, plain-text smoke,
assignment-routing, snapshot-streaming, and tool-callback slices for Probe
issues #163 through #170.

## Contract

Apple Foundation Models is the first concrete backend family in the new Probe
Bun/Effect runtime. It is modeled as its own backend kind:

- kind: `apple_fm_bridge`
- profile: `apple-fm-local`
- model: `apple-foundation-model`
- default base URL: `http://127.0.0.1:11435`
- attach mode: `attach_existing`
- auth: `none`
- readiness path: `/health`
- stream mode: `snapshot`

The backend profile resolver preserves the old Probe/Psionic override order:

1. explicit assignment/profile override
2. `PROBE_APPLE_FM_BASE_URL`
3. `OPENAGENTS_APPLE_FM_BASE_URL`
4. default loopback URL

## Implemented Probe Surface

`packages/runtime/src/backends/apple-fm/contract.ts` defines Effect v4 schemas
for Apple FM health, unavailable reasons, chat messages, chat completion
requests/responses, usage truth, and snapshot stream events.

`packages/runtime/src/backends/apple-fm/receipts.ts` defines redacted
availability, failure, and transcript receipt helpers. Token usage is explicitly
`exact`, `estimated`, or `unknown`; Probe must not label approximate Apple FM
usage as exact.

`packages/runtime/src/backends/registry.ts` registers the first Apple FM local
profile and resolves base URL overrides.

`packages/runtime/src/backends/apple-fm/client.ts` implements the attach-only
readiness path. It checks `GET /health`, decodes typed availability, and returns
redacted availability receipts for ready, unavailable, unsupported, malformed,
and unreachable bridge states.

The same client implements the first inference path:

- `completePlainText(messages)`
- `smoke(prompt)`

Plain-text completion posts to `/v1/chat/completions`, normalizes the bridge
response into Probe's Apple FM contract, and emits redacted transcript receipts.
Usage truth is preserved as `exact`, `estimated`, or `unknown`; OpenAI-shaped
token counts without explicit truth are treated as `estimated`, not exact.

Snapshot streaming is implemented separately through
`streamPlainTextSnapshots(messages)`. It requests `streamMode: "snapshot"` and
maps bridge snapshots into runtime events:

- `assistant_stream_started`
- `assistant_snapshot`
- `assistant_stream_finished`
- `assistant_final_commit`

Each `assistant_snapshot` is a full replacement value. Probe does not fake
OpenAI-style token deltas for Apple FM, and the final transcript receipt is
attached only to the final commit event.

`packages/runtime/src/cli.ts` exposes:

- `probe apple-fm status [--base-url URL] [--profile apple-fm-local]`
- `probe apple-fm smoke [--base-url URL] [--profile apple-fm-local]
  [--prompt TEXT]`

The status command performs no inference. It exits with `0` only when live
health is ready, and exits nonzero with typed status output when the bridge is
unavailable, unsupported, unreachable, or malformed.

The smoke command runs readiness first. It sends one plain-text prompt only
after `requireReady()` succeeds, prints assistant text, reports usage truth, and
prints a redacted backend transcript or failure receipt.

## Assignment Routing

Probe assignments can now select Apple FM without provider account refs:

```json
{
  "backend": {
    "kind": "apple_fm_bridge",
    "profile": "apple-fm-local"
  }
}
```

`packages/runtime/src/runtime/backend-assignment.ts` implements the no-auth
assignment path. It requires runner capability
`probe.backend.apple_fm_bridge`, checks live Apple FM health, runs the same
plain-text client used by the CLI, and emits redacted backend
start/finish/failure events.

Apple FM assignments do not require ChatGPT accounts, OpenAI API keys, OpenAgents product surface
provider auth grants, or local auth materialization.

## Tool Callback Lane

`packages/runtime/src/backends/apple-fm/tools.ts` implements the first
Probe-owned Apple FM tool-callback runtime. Apple FM can receive projected tool
schemas and call back into a session-local loopback server, but Probe remains
the authority for:

- tool registry and schema normalization
- callback token validation
- approval-required and refused tools
- `maxModelRoundTrips`
- transcript entries
- redacted callback receipts
- resume from Probe transcript state

Projected tools are limited to the retained Probe names:

- `read_file`
- `list_files`
- `code_search`
- `shell`
- `apply_patch`
- `consult_oracle`
- `analyze_repository`

Raw callback tokens and callback URLs are not included in public descriptors,
transcript entries, or receipts. Callback receipts carry
`callbackTokenRedacted: true` and `callbackUrl: "[redacted]"`.

## Pylon And SHC Capability Reporting

`packages/runtime/src/fleet/backend-capability.ts` implements Apple FM backend
capability reporting for local, SHC, Pylon, and sandbox runners.

The report is based on live `GET /health` readiness. Static local config can
choose where to check, but Probe advertises `probe.backend.apple_fm_bridge`
only when live health returns ready.

The capability report includes:

- backend kind `apple_fm_bridge`
- profile id
- model id
- redacted base URL
- typed status and unavailable reason
- Apple Silicon and Apple Intelligence requirement facts
- snapshot-streaming support
- tool-callback support
- redacted availability receipt

Unavailable and unsupported states are still reported for operator visibility,
but `advertisedCapabilities` remains empty until live health is ready.

## Tests

`packages/runtime/src/backends/apple-fm/fake-server.test.ts` covers:

- Apple FM local profile resolution and env precedence
- CI-safe fake bridge health and completion response decoding
- redacted availability/transcript receipt behavior

`packages/runtime/tests/apple-fm-cli.test.ts` covers ready, unsupported, and
unreachable status output, smoke readiness gating, estimated usage
normalization, and typed completion failures without admitted Apple hardware.

`packages/runtime/tests/backend-assignment.test.ts` covers Apple FM assignment
routing, missing backend capability rejection, and non-ready live health
rejection with an availability receipt.

`packages/runtime/tests/apple-fm-streaming.test.ts` covers multi-snapshot
replacement, final commit separation, and typed stream failure receipts.

`packages/runtime/tests/apple-fm-tools.test.ts` covers loopback callback
execution, approval-pending transcript persistence, round-trip limits, and
resume from Probe transcript state.

`packages/runtime/tests/backend-capability.test.ts` covers ready capability
advertisement, unsupported health reporting, backend identity, support flags,
and redaction.

`packages/runtime/src/backends/apple-fm/acceptance.ts` defines the retained
Apple FM acceptance case names and comparison receipt shape. Receipts preserve
backend kind, model, availability, usage truth, and tool/refusal facts.

`docs/apple-fm-admitted-mac-acceptance.md` documents the live admitted-Mac
runbook. Live Apple FM checks are excluded from default CI; unsupported hardware
or unavailable Apple Intelligence must be recorded as `unsupported` or
`unavailable`, not `failed`.

`packages/runtime/tests/apple-fm-acceptance.test.ts` covers the six retained
cases through fake Probe tool/runtime behavior.

The fake bridge tests do not require admitted Apple hardware.

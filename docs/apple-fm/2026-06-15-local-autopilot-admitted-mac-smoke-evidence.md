# Local Apple FM Autopilot Admitted-Mac Smoke Evidence

Date: 2026-06-15

Issues: #5073, #5074

Promise: `autopilot.local_apple_fm_tool_chat.v1`

Status: passed for the source/local operator path; product promise moves to
yellow, not green, because the current public signed installer still needs a
helper launch/supervision recut before a from-install user claim is green.

## Build Under Test

- Repository: `OpenAgentsInc/openagents`
- Base local session implementation commit: `380dc19942001323a2f43b087e1749bdf3655ff7`
- Evidence update: the commit containing this document and
  `apps/autopilot-desktop/scripts/apple-fm-live-smoke.ts`
- Energy estimate update: the commit containing issue #5074 changes
- Product promise registry target: `2026-06-15.11`
- Pylon package version: `1.0.0-rc.2`
- Autopilot Desktop package version: `0.0.1`
- Swift bridge version: `0.1.1`

## Host

- OS: macOS 26.4, build 25E246
- Architecture: `arm64`
- CPU class: Apple M5 Max
- Swift: Apple Swift 6.3.2, target `arm64-apple-macosx26.0`

## Commands Run

Bridge build:

```sh
bash apps/pylon/swift/foundation-bridge/build.sh
```

Result: build completed and produced
`apps/pylon/bin/foundation-bridge`.

Bridge process:

```sh
apps/pylon/bin/foundation-bridge 11436
```

Result: helper listened on `127.0.0.1:11436`.

Bridge health:

```sh
curl -fsS http://127.0.0.1:11436/health
```

Public-safe result:

```json
{
  "ready": true,
  "model": "apple-foundation-model",
  "modelId": "apple-foundation-model",
  "platform": "macOS",
  "version": "0.1.1"
}
```

Direct local completion: passed with model `apple-foundation-model` and
estimated usage. The raw prompt is intentionally not recorded here.

Pylon runtime readiness:

```sh
PROBE_APPLE_FM_BASE_URL=http://127.0.0.1:11436 \
  bun run --cwd apps/pylon runtime -- apple-fm status
```

Public-safe result: backend `apple_fm_bridge`, profile `apple-fm-local`, model
`apple-foundation-model`, status `ready`, platform `macOS`, bridge version
`0.1.1`, and a redacted `probe_backend_availability` receipt.

Pylon plain local smoke: passed with model `apple-foundation-model` and
estimated usage. The raw prompt is intentionally not recorded here.

Desktop/Pylon control smoke:

```sh
PROBE_APPLE_FM_BASE_URL=http://127.0.0.1:11436 \
  bun run --cwd apps/autopilot-desktop smoke:apple-fm-local
```

Public-safe result:

```json
{
  "disabled": {
    "available": false,
    "blockerRefs": [
      "blocker.pylon.apple_fm.apple_intelligence_disabled",
      "blocker.pylon.apple_fm.live_health_not_ready"
    ],
    "ok": false,
    "status": "unsupported",
    "unavailableReason": "apple_intelligence_disabled"
  },
  "readiness": {
    "available": true,
    "backendKind": "apple_fm_bridge",
    "model": "apple-foundation-model",
    "ok": true,
    "platform": "macOS",
    "status": "ready",
    "version": "0.1.1"
  },
  "redaction": {
    "bearerLeaked": false,
    "callbackTokenLeaked": false,
    "callbackUrlLeaked": false,
    "fixtureBodyLeaked": false,
    "promptLeaked": false,
    "tempPathLeaked": false
  },
  "retained": {
    "adapter": "apple_fm",
    "commandCount": 1,
    "editedFileCount": 0,
    "energyEstimate": {
      "assumptionRefs": [
        "assumption.apple_fm.power.default_20w_modeled_load",
        "assumption.apple_fm.power.session_wall_clock_window"
      ],
      "caveatRefs": [
        "caveat.apple_fm.power.modeled_not_measured",
        "caveat.apple_fm.power.not_ao_kwh_without_accepted_outcome"
      ],
      "energyKwh": 0.000007861,
      "evidenceState": "modeled",
      "methodRef": "method.apple_fm.power.modeled_default_kw_wall_clock",
      "modeledPowerKw": 0.02,
      "wallClockHours": 0.000393056,
      "wallClockSeconds": 1.415
    },
    "executionMode": "local_bounded",
    "executionPathRef": "control_session.apple_fm_local",
    "externalSessionRefPrefix": "session.pylon.apple_fm_bridge.",
    "kind": "proof",
    "networkAccessEnabled": false,
    "outcome": "completed",
    "resourceUsageReceiptRef": null,
    "sandboxMode": "read-only",
    "schema": "openagents.pylon.control_session_artifact.v0.1",
    "totalTokens": 179
  },
  "row": {
    "adapter": "apple_fm",
    "cloudRunner": null,
    "lane": "local",
    "resourceUsageReceiptRef": null,
    "state": "completed"
  },
  "saw": {
    "backendReady": true,
    "localMode": true,
    "toolSuccess": true
  }
}
```

The exact token count and digest suffix vary run-to-run and are intentionally
not treated as stable evidence. The energy estimate above is also run-window
specific: it is modeled at 0.02 kW over the retained 1.415-second session
window, not measured device telemetry.

## What This Proves

- A supported Apple Silicon Mac can run the restored Swift Foundation Models
  bridge and report ready health.
- Pylon can consume that bridge readiness and project it through
  `apple_fm.status`.
- The desktop Bun-side control client can start a normal Pylon control session
  using adapter `apple_fm`.
- The session can execute one bounded read-only workspace tool callback through
  the local bridge path.
- Retained proof identifies local execution with no cloud runner, no resource
  usage receipt, no workspace writes, and no network-enabled executor mode.
- Retained proof now carries modeled Apple FM session-energy denominator
  evidence with an explicit modeled-vs-measured label and AO/kWh caveat.
- Disabled Apple Intelligence handling remains typed and blocked.
- Public evidence does not expose raw prompts, local file contents, callback
  tokens, callback URLs, bearer material, or local temporary paths.

## What This Does Not Prove

- The current public signed Autopilot Desktop installer already bundles,
  launches, or supervises the helper.
- Apple FM is a Codex replacement.
- Any Apple device is supported; the claim is admitted Apple Silicon only.
- Local Apple FM is market-provider eligible, paid-work eligible, or
  settlement eligible.
- OpenAgents hosted compute is involved in the local session path.
- That the modeled Apple FM session kWh is measured power telemetry.
- That the modeled Apple FM session kWh is an accepted-outcomes-per-kWh figure
  unless joined to a verified accepted-outcome receipt.

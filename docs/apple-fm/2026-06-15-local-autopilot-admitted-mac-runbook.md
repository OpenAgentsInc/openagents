# Local Apple FM Autopilot Admitted-Mac Runbook

Date: 2026-06-15

Status: current runbook for `autopilot.local_apple_fm_tool_chat.v1` and issue
#5073. Energy-estimate addendum covers issue #5074.

## Scope

This runbook proves the basic local Autopilot Apple FM path on an admitted Mac:

- a local Swift Foundation Models bridge is reachable;
- Pylon reports Apple FM readiness through the loopback control API;
- Autopilot Desktop's Bun-side control client can start a bounded local
  Apple FM chat/tool session through Pylon;
- the session uses read-only workspace tools and retains only public-safe
  proof fields;
- the retained proof carries a public-safe local-session energy estimate;
- no OpenAgents hosted model prompt path is used.

It does not prove Codex parity, market-provider eligibility, paid work,
compute resale, settlement, accepted-outcomes-per-kWh, measured power telemetry,
or that the current public signed installer already bundles and supervises the
helper.

## Preconditions

Run this only on a supported Apple Silicon Mac where Apple Foundation Models is
available for the current user:

- macOS 26 or newer;
- `arm64`;
- Apple Intelligence enabled and model assets available;
- Swift compiler/Xcode toolchain available;
- `bun` available in the repo workspace.

Unsupported, disabled, permission-denied, model-unavailable, malformed, and
unreachable cases are legitimate blocked states. Do not report them as a
failed green claim.

## Build And Start The Bridge

From the repo root:

```sh
bash apps/pylon/swift/foundation-bridge/build.sh
```

Start the helper on a local loopback port:

```sh
apps/pylon/bin/foundation-bridge 11436
```

Keep that process running while the checks below execute.

## Bridge And Runtime Checks

Check native bridge health:

```sh
curl -fsS http://127.0.0.1:11436/health
```

Check Pylon runtime readiness:

```sh
PROBE_APPLE_FM_BASE_URL=http://127.0.0.1:11436 \
  bun run --cwd apps/pylon runtime -- apple-fm status
```

Run a plain local completion smoke:

```sh
PROBE_APPLE_FM_BASE_URL=http://127.0.0.1:11436 \
  bun run --cwd apps/pylon runtime -- apple-fm smoke --prompt "<public-safe marker prompt>"
```

## Desktop/Pylon Local Tool Smoke

Run the public-safe desktop/Pylon control smoke:

```sh
PROBE_APPLE_FM_BASE_URL=http://127.0.0.1:11436 \
  bun run --cwd apps/autopilot-desktop smoke:apple-fm-local
```

The script creates a temporary fixture workspace, starts an in-process Pylon
control server, calls the same `fetchAppleFmReadiness`,
`startAppleFmSession`, and `fetchNodeState` functions used by the desktop Bun
host, and prints only refs/status booleans. It also starts a fake disabled
bridge to prove unsupported/disabled readiness handling.

The pass criteria are:

- readiness `ok: true`, `available: true`, `status: "ready"`,
  `backendKind: "apple_fm_bridge"`;
- session row `adapter: "apple_fm"`, `lane: "local"`, `state: "completed"`;
- `cloudRunner: null` and `resourceUsageReceiptRef: null`;
- observed event facts for backend ready, read-only `read_file` tool success,
  and local bounded mode;
- retained artifact `executionPathRef: "control_session.apple_fm_local"`,
  `executionMode: "local_bounded"`, `sandboxMode: "read-only"`,
  `networkAccessEnabled: false`, `outcome: "completed"`, and one command;
- retained artifact `energyEstimate.evidenceState: "modeled"` by default,
  `methodRef: "method.apple_fm.power.modeled_default_kw_wall_clock"`,
  `modeledPowerKw: 0.02`, a bounded wall-clock window, computed `energyKwh`,
  and the caveat refs
  `caveat.apple_fm.power.modeled_not_measured` and
  `caveat.apple_fm.power.not_ao_kwh_without_accepted_outcome`;
- external bridge session id retained only as
  `session.pylon.apple_fm_bridge.<digest>`;
- redaction booleans for prompt, fixture body, callback token, callback URL,
  bearer material, and temp path are all false;
- disabled fixture returns `unavailableReason: "apple_intelligence_disabled"`
  with typed blocker refs.

## Energy Estimate Interpretation

The default local Apple FM session estimate is modeled, not measured:

```txt
energyKwh = modeledPowerKw * sessionWallClockHours
```

The default modeled load is `0.02` kW (20 W) and is labeled with
`assumption.apple_fm.power.default_20w_modeled_load`. Operators may override the
modeled load for local experiments with `OPENAGENTS_APPLE_FM_MODELED_POWER_KW`
or `PROBE_APPLE_FM_MODELED_POWER_KW`. Operators may also force the proof to
publish `evidenceState: "unavailable"` with
`OPENAGENTS_APPLE_FM_POWER_ESTIMATE_MODE=disabled` or
`PROBE_APPLE_FM_POWER_ESTIMATE_MODE=disabled`.

This estimate is denominator evidence only. Per #5060, Apple FM local-session
kWh is not AO/kWh unless joined to a verified accepted-outcome receipt. Do not
describe modeled Apple FM kWh as measured telemetry, provider ranking evidence,
production routing evidence, paid-work eligibility, compute resale capacity, or
settlement evidence.

## CI-Safe Coverage

Hardware-free coverage for the same integration lane lives in:

```sh
bun test apps/pylon/tests/apple-fm-control-session.test.ts \
  apps/pylon/tests/control-protocol.test.ts \
  apps/pylon/tests/control-session-receipts.test.ts \
  apps/pylon/packages/runtime/tests/apple-fm-streaming.test.ts
```

```sh
cd apps/autopilot-desktop
bun test tests/apple-fm-loopback-integration.test.ts \
  tests/control-verbs.test.ts \
  tests/cl-53-foldkit.test.ts \
  tests/cl-53-sanitize.test.ts \
  tests/install-readiness.test.ts \
  tests/pylon-control.test.ts
```

Those tests use fake bridges so ordinary CI does not require Apple hardware.

## Public Evidence Rules

Public docs, issue comments, and product-promise evidence may include the
public-safe summary fields printed by `smoke:apple-fm-local`. Do not publish:

- raw prompts;
- local file contents;
- callback tokens;
- callback URLs;
- bearer/control tokens;
- local temporary paths;
- full model transcripts;
- raw telemetry dumps;
- serial numbers or private machine identifiers;
- private workspace material.

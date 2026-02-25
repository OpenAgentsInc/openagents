# Spacetime Cutover State Announcement

Date: 2026-02-25
Status: Active
Owner lanes: Runtime, Control, Ops

## Purpose

Publish and archive operator-visible cutover state proving Spacetime is the default sync transport and Spacetime is emergency-only.

## Command

```bash
./scripts/spacetime/announce-cutover-state.sh \
  --control-base-url "$OA_CONTROL_BASE_URL" \
  --auth-token "$OA_CONTROL_AUTH_TOKEN" \
  --runtime-base-url "$OA_RUNTIME_BASE_URL"
```

Offline/local smoke:

```bash
./scripts/spacetime/announce-cutover-state.sh --skip-remote
```

## Validation Contract

The announcement is `allow` only when:

1. control status exposes `syncCutover.defaultTransport = spacetime_ws`
2. runtime sync observability exposes `transport = spacetime_ws`

Any mismatch blocks the announcement.

## Artifact Location

`output/canary/spacetime/cutover-state-<timestamp>/`

Files:

1. `result.json`
2. `SUMMARY.md`
3. `control-status.json` (when remote probes enabled)
4. `runtime-spacetime-metrics.json` (when remote probes enabled)

Attach `SUMMARY.md` + `result.json` in operator change logs for cutover state communication.

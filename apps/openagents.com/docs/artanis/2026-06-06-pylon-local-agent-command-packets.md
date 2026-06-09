# Pylon Local-Agent Command Packets

Date: 2026-06-06

Issue: #409 / `ARTANIS-023`

Status: implemented as packet generation and public-safe projection on top of
the Pylon resource-mode setup contract.

## Purpose

Pylon resource modes describe how much local compute a machine may offer.
Local-agent command packets turn those modes into safe instructions that a
human can ask a local coding agent to execute.

The packet layer starts in dry-run mode. It records the intended resource
posture, approval prompt, dry-run command refs, private dry-run evidence refs,
telemetry refs, pause/resume expectations, checkpoint expectations, and earning
caveats before any local execution is allowed.

## Packet Coverage

The generator creates a packet for every current Pylon resource mode:

- `background_20`
- `balanced`
- `overnight_full`
- `dedicated_full_blast`

Each packet carries:

- CPU/GPU/memory ceiling intent
- network and storage intent refs
- pause/resume expectation refs
- checkpoint expectation refs
- telemetry refs
- safe instruction refs
- dry-run command refs
- private dry-run evidence refs
- public receipt refs
- explicit owner approval prompt/ref
- public earning caveats

Public projections redact private dry-run evidence. Operator projections can
see private evidence refs by reference, but the contract still rejects raw
local paths, raw command output, provider credentials, wallet material, node
secrets, customer data, and raw timestamps.

## Execution Boundary

Generated packets start as `dry_run_ready` with `localExecutionAllowed: false`.

Local execution is allowed only when a packet is explicitly changed to
`approved_for_local_execution` by an owner-approved path. The packet itself
does not install Pylon, launch Pylon, spend bitcoin, mutate provider state,
claim paid-work eligibility, assign work, accept work, dispatch payouts, or
settle anything.

The safe claim is:

```text
This packet is ready for a local dry run after explicit owner approval. Online
status is not paid-work eligibility, accepted work, payout, or settlement.
```

Disallowed claim shapes include:

```text
Run Pylon and earn money.
```

```text
This mode guarantees a payout.
```

## Verification

Coverage lives in `workers/api/src/pylon-resource-mode-setup.test.ts`.

The tests prove:

- local-agent packets are generated for all current resource modes;
- public projections hide private dry-run evidence refs;
- operator projections show private dry-run evidence refs by reference;
- packets carry resource intent, telemetry, pause/resume, checkpoint, owner
  approval, dry-run, and earning caveat refs;
- local execution cannot be allowed before approval state;
- missing owner approval, missing dry-run evidence, unsafe refs, raw local
  paths, provider credentials, wallet material, raw command output, and
  unconditional earning claims fail closed.

# Pylon Resource Mode Setup Contract

Date: 2026-06-06

Status: implemented in #399 / `ARTANIS-014` and extended in #409 /
`ARTANIS-023`.

## Purpose

Pylon should be configurable for different owner-selected resource levels:

- light background use while the owner is working;
- balanced use;
- fuller overnight operation;
- dedicated machine operation.

Artanis may recommend these modes, and a local coding agent may help set them
up, but only after explicit owner/operator approval.

## Implementation

Code lives in:

- `workers/api/src/pylon-resource-mode-setup.ts`
- `workers/api/src/pylon-resource-mode-setup.test.ts`

Source docs:

- `docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md`
- `docs/artanis/2026-06-06-work-routing-contract.md`
- `docs/artanis/2026-06-06-pylon-local-agent-command-packets.md`

## Modes

The implemented modes are:

| Mode | Family | Resource posture |
| --- | --- | --- |
| `background_20` | background | 20 percent CPU, no GPU, low memory/network/disk, owner-interruptible while working. |
| `balanced` | balanced | moderate CPU/GPU/memory/network/disk for owner-selected work. |
| `overnight_full` | overnight | high CPU/GPU/memory/network/disk for an owner-approved overnight window. |
| `dedicated_full_blast` | dedicated | full dedicated-machine posture for operator-managed machines. |

Every mode records:

- CPU/GPU/memory ceilings;
- disk and network budget refs;
- schedule window ref;
- pause/resume policy ref;
- owner approval refs;
- setup command refs;
- work-routing refs;
- eligibility caveats.

## Setup Commands

The command records are public-safe refs, not raw shell output:

- install launcher;
- launch Pylon;
- set resource mode;
- version check;
- runtime status;
- training status;
- balance check;
- history check.

Each command requires:

- explicit owner approval refs;
- private evidence refs for command output;
- `evidence_handling.private_by_default`;
- public receipt refs for safe completion markers.

Public projections show command refs, state, caveats, public receipts, and safe
instruction refs. Operator projections can inspect private evidence refs.

## Local-Agent Command Packets

#409 adds local-agent packets generated from the mode plan. These packets are
the bounded handoff object for a human asking a local coding agent to prepare
Pylon on a machine.

Every packet starts as `dry_run_ready` and includes:

- resource intent for CPU/GPU/memory/network/storage;
- owner approval prompt and owner approval refs;
- dry-run command refs;
- private dry-run evidence refs;
- telemetry refs;
- pause/resume expectations;
- checkpoint expectations;
- public receipt refs;
- safe instruction refs;
- earning caveats that keep online status separate from eligibility,
  accepted work, payout, and settlement.

Public projections redact private dry-run evidence refs. Operator projections
can inspect private evidence refs by reference. Local execution remains blocked
unless an owner-approved path marks the packet
`approved_for_local_execution`.

## Public Boundary

Public projections do not expose:

- raw command output;
- local filesystem paths;
- provider credentials;
- wallet material;
- node secrets;
- private evidence refs;
- private customer data;
- raw timestamps.

The projection also keeps resource mode separate from paid-work eligibility.
Being online in a mode does not imply eligibility, assigned work, accepted
work, payment, or settlement.

## Verification

Coverage lives in `workers/api/src/pylon-resource-mode-setup.test.ts`.

The tests cover:

- public-safe projection of background, balanced, overnight, and dedicated
  modes;
- private-by-default command evidence;
- operator-only private evidence visibility;
- work-routing refs and eligibility caveats;
- owner approval requirement;
- required mode and command coverage;
- local-agent command packet generation for every mode;
- dry-run evidence redaction and execution-before-approval rejection;
- rejection of raw local paths, wallet material, node secrets, provider
  credentials, raw command output refs, raw timestamps, and unconditional
  earning claims.

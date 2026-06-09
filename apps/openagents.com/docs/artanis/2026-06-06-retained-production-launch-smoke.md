# Retained Production-Equivalent Artanis Launch Smoke

Issue: #417 / `ARTANIS-031`

Status: implemented as a read-only retained-smoke evidence contract.

## Purpose

The original #397 launch smoke proves the public-safe chain:

```text
operator steering
-> loop claim
-> safe result
-> Forum post
-> /artanis public summary
```

#417 adds the retained production-equivalent evidence layer needed by the
production launch gate. It records the rows, receipts, public report refs, and
rollback refs that prove the chain happened under a controlled production or
production-equivalent window.

This is still evidence only. It does not enable the scheduler, deploy code,
post to Forum, dispatch Pylon work, mutate providers, launch training, spend
bitcoin, charge buyers, or settle providers.

## Implementation

Code lives in:

- `workers/api/src/artanis-retained-launch-smoke.ts`
- `workers/api/src/artanis-retained-launch-smoke.test.ts`

The contract exports:

- `ArtanisRetainedLaunchSmokeRecord`
- `ArtanisRetainedLaunchSmokeProjection`
- `projectArtanisRetainedLaunchSmoke`
- `artanisProductionLaunchGateCheckFromRetainedSmoke`

## Required Evidence

A valid retained smoke requires:

- operator approval refs;
- persisted runtime snapshot refs;
- persisted loop record refs;
- persisted loop tick refs;
- persisted health snapshot refs;
- persisted work-routing proposal refs;
- persisted Forum publication intent refs;
- public `/api/public/artanis/report` refs;
- rollback or scheduler-disable refs;
- delivered Forum post and delivery receipt refs, or explicit no-publish proof
  refs for no-launch/no-publish test mode.

## Modes

The smoke supports controlled production-equivalent modes:

- `fake_provider_one_tick`
- `one_tick_window`
- `no_launch`
- `disabled`

The Forum side supports:

- `delivered_post`
- `no_publish_test`

No-publish mode is useful for a production-equivalent binding smoke before an
operator wants to create another public Forum post. It must carry explicit
no-publish proof refs.

## Launch Gate Link

`artanisProductionLaunchGateCheckFromRetainedSmoke` converts a valid retained
smoke into a `production_e2e_smoke` launch-gate check.

That does not make Artanis continuously autonomous. The production launch gate
still remains blocked until the scheduler gate and every other required gate
also pass inside an operator-controlled window.

## Safety Boundary

Public projection redacts private evidence refs. Operator projection can retain
safe private refs by reference.

The contract rejects:

- missing persisted row refs;
- missing Forum delivery or no-publish proof refs;
- missing public report refs;
- missing rollback/disable refs;
- mutable authority;
- private/raw D1 refs;
- raw command output;
- wallet/payment/provider/customer/secret material;
- raw timestamps in projected refs.

## Verification

Focused checks:

```bash
bun run --cwd workers/api test -- \
  src/artanis-retained-launch-smoke.test.ts \
  src/artanis-launch-smoke.test.ts \
  src/artanis-production-launch-gate.test.ts
```

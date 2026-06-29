# Fake Cloudflare Container Runner

Date: 2026-06-06

Status: implemented for issue #282 / OPENAGENTS-RUNNER-004.

## Purpose

This slice adds a fake/staging Cloudflare Container runner adapter. It fits the
gateway contract from #280 but does not execute customer code, start a
Container, build source, read provider secrets, or charge anyone.

The purpose is to let OpenAgents product surface test the lifecycle, artifact, cancel, and redaction
path before a live Container runner exists.

## Implemented Contract

`workers/api/src/fake-cloudflare-container-runner.ts` provides:

- a gateway adapter for `cloudflare_container`;
- deterministic fake dispatch receipts;
- deterministic lifecycle events for:
  - queued;
  - started;
  - artifact;
  - completed;
  - failed;
  - cancelled;
- public-safe artifact manifest refs;
- operator-safe debug refs;
- cancellation receipts;
- callback receipt ingestion;
- health checks that return `healthy` for sanitized fake requests.

## Redaction Rules

The fake runner validates all inbound gateway payloads with the same secret
scanner used by the gateway contract.

It rejects raw bearer tokens, callback tokens, provider tokens/payloads, raw
runner logs, source archives, wallet/payment material, email addresses,
customer private data, and secret-shaped strings.

It also checks generated fake outputs before returning them so the staging path
does not accidentally prove a bad projection shape.

## Current Non-Goals

This issue does not:

- add a real Cloudflare Container class;
- add a Wrangler `containers` binding;
- execute a generated Site;
- run dependency installs or tests;
- resolve provider-account auth material;
- attach billing to fake runner usage.

Those remain for provider-boundary, cost/health projection, and later live
Container execution issues.

# Runner Backend Schema v1

Date: 2026-06-06

Status: implemented for issue #279 / OPENAGENTS-RUNNER-001.

## Purpose

This slice adds the shared typed runner backend schema boundary for
`cloudflare_container` without enabling live automatic Container execution.

The schema lets SHC, Cloudflare Containers, and future GCloud lanes appear in
the same projection model while dispatch policy remains unchanged.

## Modeled Backends

`OpenAgentsRunnerBackendKind` supports:

- `shc_vm`;
- `cloudflare_container`;
- `gcloud_vm`.

`OpenAgentsRunnerBackendRecord` tracks:

- backend kind;
- workload trust level;
- enabled/configured state;
- dispatch status;
- lifecycle event refs;
- artifact refs;
- health refs;
- capacity refs;
- cost refs;
- policy refs;
- receipt refs;
- public summary ref;
- operator diagnostic refs.

## Projection Boundary

`projectOpenAgentsRunnerBackend` supports public, customer, and operator
projections.

Public/customer projections hide operator diagnostics and reject raw runner
logs, callback tokens, provider payloads, provider tokens, source archives,
wallet/payment material, email addresses, and secret-shaped strings.

Operator projections may retain redacted operator-safe provider-account and
capacity diagnostics, while still rejecting raw tokens, callback tokens, raw
logs, source archives, and secrets.

## What It Does Not Do

This issue does not:

- dispatch work to a Container;
- enable automatic failover;
- create Cloudflare bindings;
- run builds;
- provision preview URLs;
- charge customers or agents.

Those remain behind later gateway, readiness, fake-runner, and policy-gated
execution issues.

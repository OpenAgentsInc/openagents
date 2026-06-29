# Runner Gateway Contract v1

Date: 2026-06-06

Status: implemented for issue #280 / OPENAGENTS-RUNNER-002.

## Purpose

This slice adds the backend-neutral runner gateway contract that later live
runner implementations must satisfy.

It does not dispatch work. It defines the typed request, callback, artifact,
selection, and error boundary that SHC, Cloudflare Containers, and future
GCloud adapters must use before any live execution path is enabled.

## Implemented Contract

`workers/api/src/runner-gateway.ts` defines:

- dispatch request envelopes;
- cancel request envelopes;
- health check request envelopes;
- lifecycle callback envelopes;
- artifact manifest refs;
- adapter state and policy-selected adapter selection;
- dispatch receipts;
- an adapter interface for dispatch, cancel, health, and callback ingestion;
- typed gateway errors and status mapping.

The gateway recognizes the same backend kind vocabulary introduced in #279:

- `shc_vm`;
- `cloudflare_container`;
- `gcloud_vm`.

## Policy Boundary

Execution stays denied unless a backend adapter state is:

- registered for the requested backend kind;
- explicitly selected by policy;
- enabled;
- configured.

This is deliberate. Cloudflare Containers can be represented in the gateway
without being silently used as an automatic dispatch target.

## Secret Boundary

Gateway payloads carry refs and grants, not raw secrets.

Allowed examples:

- provider account refs;
- auth grant refs;
- GitHub write grant refs;
- callback refs;
- artifact manifest refs;
- receipt refs.

Rejected examples:

- raw bearer tokens;
- callback tokens;
- provider tokens;
- provider payloads;
- raw runner logs;
- source archives;
- wallet/payment material;
- email addresses and raw customer/private data;
- secret-shaped strings.

## What It Does Not Do

This issue does not:

- connect the existing SHC HTTP client to the new gateway;
- add Cloudflare Container bindings;
- implement fake or live Container execution;
- choose failover automatically;
- resolve provider-account auth material;
- bill users or agents.

Those remain for the disabled readiness, fake runner, provider-boundary, and
operator health/cost issues that follow.

# Runner Provider Secret Boundary

Date: 2026-06-06

Status: implemented for issue #283 / OPENAGENTS-RUNNER-005.

## Purpose

This slice preserves the runner/provider-account secret boundary before SHC,
fake Container, or later live Container dispatch can carry work.

Runner dispatch payloads must carry refs and grants. Raw provider credentials,
OAuth material, cookies, GitHub tokens, API keys, callback tokens, wallet
material, source archives, and raw logs are rejected.

## Implemented Contract

`workers/api/src/runner-secret-boundary.ts` models:

- runner grant refs;
- provider-account grant refs;
- GitHub write grant refs;
- callback refs;
- resolution receipts;
- scrub receipts;
- denial reasons;
- public-safe boundary projections.

The gateway private-material scanner now also rejects common raw API key,
GitHub token, OAuth material, auth-content, and callback-token value shapes.

## Projection Boundary

Private/operator-side boundary records can retain grant refs so the runner or
service boundary can resolve scoped material and scrub it after closeout.

Public/customer projections only expose:

- backend kind;
- dispatch ref;
- runner session ref;
- grant count;
- whether required grants are present;
- neutral resolution receipt refs;
- neutral scrub receipt refs;
- denial reasons;
- public summary ref.

They do not expose provider account refs, grant refs, GitHub write refs,
callback token refs, OAuth material, raw tokens, source archives, wallet
material, or raw logs.

## Current Non-Goals

This issue does not:

- change provider-account resolver route authority;
- change GitHub write grant resolver route authority;
- resolve credentials in the Worker dispatch path;
- add live Container execution;
- add billing or settlement.

Those remain behind the runner adapter and provider service boundaries.

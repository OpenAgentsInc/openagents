# Cloudflare Container Closeout Receipts

Date: 2026-06-06

Status: implemented for issue #287 / OPENAGENTS-RUNNER-009.

## Purpose

This slice defines the lifecycle callback and artifact closeout receipt contract
for a future real Cloudflare Container runner. It lets a runner prove what
happened without leaking private runner state, raw logs, credentials, wallet
material, or customer private data.

It does not persist large artifacts in D1, deploy a Container image, or enable
live execution.

## Implemented Contract

`workers/api/src/cloudflare-container-closeout-receipts.ts` adds:

- lifecycle phases: accepted, started, progress, artifact, completed, failed,
  timed out, and cancelled;
- artifact closeout refs for generated files, diffs, screenshots, redacted
  build logs, validation results, redaction reports, and public artifacts;
- closeout receipt refs;
- credential scrub receipt refs;
- provider-account scrub receipt refs;
- event refs;
- callback refs;
- public/customer/team/operator projections;
- gateway lifecycle callback derivation;
- gateway artifact manifest derivation.

## Terminal Closeout Rule

Terminal phases require scrub evidence before the receipt is accepted:

- completed;
- failed;
- timed out;
- cancelled.

For those phases, both of these must be present:

- credential scrub receipt refs;
- provider-account scrub receipt refs.

Non-terminal phases such as accepted, started, progress, and artifact can be
recorded without scrub receipts because the run has not closed yet.

## Projection Rules

Public projection includes only:

- lifecycle phase;
- public summary;
- public artifact refs;
- closeout receipt refs;
- status caveats.

Customer projection adds safe artifact review detail:

- generated file refs;
- diff refs;
- screenshot refs;
- validation result refs;
- redaction report refs.

Team projection adds safe operational detail:

- redacted build-log refs;
- event refs;
- artifact manifest ref;
- scrub receipt refs.

Operator projection adds the remaining safe refs:

- callback ref;
- external run ref;
- operator diagnostic refs.

No projection can include raw run logs, source archives, auth material,
callback-token values, wallet/payment secrets, or customer PII.

## Gateway Integration

The module derives:

- `OpenAgentsRunnerGatewayLifecycleCallback`, mapping `timed_out` to the
  gateway's failed dispatch status while retaining timeout caveat refs;
- `OpenAgentsRunnerGatewayArtifactManifest`, collecting generated files, diffs,
  screenshots, redacted build logs, validation results, redaction reports,
  public artifact refs, and closeout receipt refs.

That keeps the closeout contract compatible with the backend-neutral gateway
from #280 and the real adapter from #285 without enabling live execution.

## Current Non-Goals

This issue does not:

- store large artifacts in D1;
- expose raw runner logs publicly;
- build or deploy a Container image;
- execute customer code;
- turn on automatic Container failover.

# Blueprint Program Type and Program Signature Schemas v1

Issue: OPENAGENTS-BP-004 / #224

This note records the OpenAgents product surface-owned Blueprint Program Type and Program Signature
schema slice. The source of truth is `workers/api/src/blueprint/schemas/program.ts`.

## Purpose

Program Types define governed behavior families. Program Signatures define the
stable input and output contract for a specific behavior version. They are not
prompts, and they do not grant write authority.

The v1 schema models:

- input and output schema refs;
- instruction refs and instruction version refs;
- decode and validation policy;
- evidence requirements;
- receipt requirements;
- tool scope;
- status;
- risk class;
- release gates;
- direct-mutation policy.

## Supported Families

The first family enum covers the behavior contracts needed by Autopilot Sites
and the broader Omni roadmap:

- continuation;
- routing;
- review;
- context;
- proof projection;
- research policy;
- email decisioning;
- source selection;
- action planning;
- artifact review.

## Safety Boundary

`directMutationAllowed` is modeled explicitly and should remain false for the
first fulfillment signatures. Program Runs may record typed decisions and
evidence. External writes still require future approval-gated Action
Submissions.

Tool scope uses read, evidence, and propose-action access. Any propose-action
scope is approval-bound in v1.

## Current Limits

This is a schema boundary only. Registry persistence, Program Run persistence,
JSON Schema export, OpenAPI export, signature contribution, release-gate
promotion, and runtime services are intentionally separate roadmap issues.

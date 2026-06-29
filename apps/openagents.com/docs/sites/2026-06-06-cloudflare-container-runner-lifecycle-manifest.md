# Cloudflare Container Runner Lifecycle Manifest

Date: 2026-06-06

Status: implemented for issue #286 / OPENAGENTS-RUNNER-008.

## Purpose

This slice defines the image/workspace lifecycle manifest for the real
Cloudflare Container runner path. It gives future OpenCode/Codex-compatible
runner images a stable contract before live execution is enabled.

The manifest is source of truth for safe refs, readiness inputs, command phases,
artifact roots, health probes, cancel behavior, cost/resource caveats, and
closeout receipts. It does not build, publish, or run a Container image.

## Implemented Contract

`workers/api/src/cloudflare-container-runner-manifest.ts` adds:

- `OpenAgentsCloudflareContainerRunnerImageLifecycleManifest`;
- command phase refs for workspace prep, grant resolution, health, start,
  progress, artifact collection, grant scrub, closeout, and cancel;
- health probe refs;
- cancel semantics;
- resource profile refs;
- status caveat refs;
- public/customer/operator projection;
- manifest validation and private-material detection;
- readiness derivation for the real adapter from #285;
- gateway artifact-manifest derivation for real and fake/staging conformance.

## Manifest Fields

The manifest models:

- image ref;
- runtime ref;
- workspace ref;
- class name ref;
- Durable Object binding ref;
- callback ref;
- artifact root ref;
- public artifact refs;
- closeout receipt refs;
- trust tier;
- timeout;
- resource refs;
- cost refs;
- allowed tool refs;
- command phases;
- health probes;
- cancel semantics;
- public summary and status caveats.

All fields are refs or caveats. The manifest must not include raw repositories,
raw source archives, private prompts, raw logs, tokens, credentials, wallet
material, payment material, callback secrets, or customer private data.

## Projection Rules

Public and customer projections expose only:

- manifest ref;
- version ref;
- backend kind;
- trust tier;
- status;
- public summary ref;
- status caveat refs.

They do not expose image refs, runtime refs, workspace refs, callback refs,
command phases, health probes, resource refs, cost refs, artifact roots,
allowed tools, or closeout mechanics.

Operator projection can include safe refs for the full manifest mechanics:

- image/runtime/workspace;
- command phases;
- health probes;
- cancel refs;
- resource and cost refs;
- allowed tools;
- artifact root and closeout receipts.

Even operator projection filters secret-shaped values.

## Adapter And Fake Runner Use

The manifest can derive:

- `OpenAgentsRealCloudflareContainerRunnerReadiness` for the real adapter;
- `OpenAgentsRunnerGatewayArtifactManifest` for real/fake runner conformance.

This keeps the fake/staging runner and future real runner pointed at the same
artifact and closeout shape.

## Current Non-Goals

This issue does not:

- create a Dockerfile;
- build or publish a Container image;
- mount customer repositories;
- resolve provider credentials;
- run customer code;
- enable live dispatch or automatic failover.

Those remain later runtime, closeout, and rollout tasks.

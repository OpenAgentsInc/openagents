# Cloudflare Container Disabled Binding Plan

Date: 2026-06-06

Status: implemented for issue #281 / OPENAGENTS-RUNNER-003.

## Purpose

This slice models the Cloudflare Container runner binding plan without adding a
live Container binding or enabling automatic Container dispatch.

Cloudflare's Worker configuration model represents Containers through a
`containers` entry and a matching Durable Object binding. The disabled OpenAgents product surface
plan records those requirements as inert config refs until a later issue adds
real Wrangler bindings and a fake/staging runner path.

## Config Fields

`RunnerBackendConfig.cloudflareContainer` now tracks:

- `enabled`;
- `configured`;
- `stagingSmokePassed`;
- `policyApproved`;
- `allowedWorkloadTrusts`;
- binding refs:
  - `className`;
  - `durableObjectBinding`;
  - `imageRef`;
  - `instanceType`;
  - `maxInstances`.

The default is intentionally safe:

- Container enabled: `false`;
- Container configured: `false`;
- staging smoke passed: `false`;
- policy approved: `false`;
- automatic failover enabled: `false`;
- allowed workload trust: `low`, `medium`.

## Readiness Gates

Container readiness requires all of the following:

- global runner backend policy includes the Container backup lane;
- Container enabled;
- Container configured;
- Container class name ref present;
- Container Durable Object binding ref present;
- Container image ref present;
- staging smoke passed;
- operator policy approved;
- workload trust is allowed.

If automatic failover is requested before these gates pass, the readiness check
returns `blocked` and explains which Container prerequisites are still required.

## Current Non-Goals

This issue does not:

- add `containers` to `wrangler.jsonc`;
- add a Container class;
- add a Durable Object binding;
- build or push a Container image;
- execute customer code;
- enable automatic failover.

Those are reserved for the fake runner, provider-boundary, operator health, and
later live execution issues.

# Real Cloudflare Container Runner Adapter Contract

Date: 2026-06-06

Status: implemented for issue #285 / OPENAGENTS-RUNNER-007.

## Purpose

This slice defines the real Cloudflare Container runner adapter boundary without
turning on live automatic Container execution.

The adapter is intentionally distinct from the fake/staging adapter. It can
validate dispatch, cancel, health, lifecycle callback, and artifact-compatible
gateway envelopes, then either return blocked receipts or call an injected
control-plane implementation. In production, that control plane must be backed
by reviewed Worker/Container bindings before it can execute work.

## Cloudflare Container Assumptions

The current Cloudflare configuration model uses a `containers` entry with an
image and `class_name`, plus a matching Durable Object binding. Cloudflare's
documentation also exposes optional `instance_type`, `max_instances`, rollout,
and image-build controls.

OpenAgents product surface stores only safe refs for those pieces at this stage:

- class name ref;
- Durable Object binding ref;
- image ref;
- capacity ref;
- health ref;
- policy and smoke refs.

The contract does not expose Cloudflare account IDs, raw binding internals,
runtime secrets, image build args, or container logs to public/customer
surfaces.

## Implemented Contract

`workers/api/src/real-cloudflare-container-runner.ts` adds:

- `OpenAgentsRealCloudflareContainerRunnerReadiness`;
- `OpenAgentsRealCloudflareContainerRunnerBindingRefs`;
- `OpenAgentsRealCloudflareContainerRunnerControlReceipt`;
- `OpenAgentsRealCloudflareContainerRunnerControlPlane`;
- blocked-gate calculation for enabled/configured/policy/smoke/control/trust;
- `makeRealCloudflareContainerRunnerAdapter`.

The adapter accepts the existing backend-neutral gateway request types:

- dispatch;
- cancel;
- health check;
- lifecycle callback.

It returns normal gateway dispatch receipts for dispatch, cancel, and callback
operations, and gateway health status for health checks.

## Gate Rules

The real adapter does not call its control plane unless all required gates pass:

- policy is selected;
- backend is enabled;
- class name, Durable Object binding, and image refs are configured;
- staging smoke has passed;
- operator policy approval exists;
- a real control plane is injected/bound;
- dispatch workload trust is allowed.

When a gate fails, the adapter returns a blocked receipt with only safe gate
refs. It does not throw for ordinary missing readiness. Unsafe payloads and
unsafe control-plane receipts still raise typed gateway errors.

## Secret Boundary

The adapter rejects:

- raw provider tokens;
- OAuth material;
- callback token values;
- raw source archives;
- raw runner logs;
- private prompts;
- wallet or payment secrets;
- customer emails or names;
- generic secret-shaped payloads.

Control-plane receipts are decoded and checked before they become gateway
receipts, so a future live Container path cannot accidentally return raw logs or
credentials through the adapter.

## Current Non-Goals

This issue does not:

- deploy a Container image;
- bind a live Container Durable Object;
- mount customer source;
- resolve provider account credentials;
- run OpenCode or Codex inside a Container;
- enable automatic failover;
- change customer-visible infrastructure claims.

Those belong to the later image lifecycle, closeout receipt, and failover
rollout issues.

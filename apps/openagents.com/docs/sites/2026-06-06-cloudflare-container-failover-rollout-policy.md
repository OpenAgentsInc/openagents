# Cloudflare Container Failover Rollout Policy

Date: 2026-06-06

Status: implemented for issue #288 / OPENAGENTS-RUNNER-010.

## Purpose

This slice defines the operator-selected staging rollout and failover policy for
Cloudflare Containers. It keeps SHC primary until Container readiness, smoke,
cost, capacity, trust, and policy gates are satisfied.

It models failover decisions separately from backend health projection.

## Implemented Contract

`workers/api/src/runner-failover-policy.ts` adds:

- failover triggers;
- decision statuses;
- safe failover decision receipts;
- blocked gate refs;
- previous and selected backend refs;
- trust tier;
- customer-safe status refs;
- automatic-failover requested/effective fields;
- private-material checks.

## Decision Rules

SHC remains primary by default.

Container can be selected only when:

- an operator explicitly selected Container;
- the runner policy enables the Container backup lane;
- Container is enabled;
- class name, Durable Object binding, and image refs are configured;
- staging smoke passed;
- operator policy approval exists;
- capacity gate is green;
- cost gate is approved;
- workload trust is low or medium;
- live automatic failover approval exists when automatic failover was requested.

Sensitive workloads are never routed to Container in this policy. If a
sensitive workload needs a non-SHC reference lane, the policy can select GCloud
only when the explicit GCloud reference path is ready. Otherwise it blocks.

## Receipt Shape

Each decision emits:

- previous backend kind and safe ref;
- selected backend kind and safe ref;
- trigger;
- trust tier;
- reason refs;
- blocked gate refs;
- automatic failover requested/effective flags;
- receipt ref;
- public summary ref;
- customer-safe status ref.

Receipts must not include raw provider data, callback tokens, source archives,
runner logs, wallet/payment secrets, customer PII, or other secret-shaped
material.

## Automatic Failover Status

Automatic live Container failover remains disabled until separate live smoke and
operator approval receipts exist.

The policy can represent a requested automatic failover setting, but it marks
automatic failover effective only when all gates pass and live automatic
approval is explicitly present.

## Current Non-Goals

This issue does not:

- enable automatic live Container failover in production;
- route sensitive work to Containers;
- deploy or run a Container image;
- change customer-visible claims about live runner infrastructure.

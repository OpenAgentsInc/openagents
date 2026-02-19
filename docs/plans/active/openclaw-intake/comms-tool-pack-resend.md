# OpenClaw Intake Record: Comms Tool Pack (Resend First)

Date: 2026-02-19
Owner: Runtime + Laravel control plane
Status: `active`

## Capability Identity

- Capability ID: `comms-tool-pack-resend-v1`
- Summary: OpenClaw-style extensible comms capability with runtime tool execution, provider adapters, and Laravel-authored integration intent.
- OpenClaw upstream repository: `~/code/openclaw`
- Upstream commit SHA: pinned during implementation issue execution
- Upstream paths:
  - `src/plugins/*` (manifest + registry concepts)
  - `src/agents/tool-policy*` (policy evaluation patterns)
  - `src/infra/net/*` (network safety seam patterns)
- License notes: Apache-2.0 compatible ingestion required.

## Classification

- Decision: `port`
- Decision rule trigger(s): affects side-effect execution, policy enforcement, security, receipts, and replay.
- Risk class: `high`

## Layer and Platform Routing (Required)

- Layer: `Kernel + Control plane + Protocol`
- Platform variance: `core_shared`
- Target platforms (if `client_specific` or `hybrid`): n/a
- Destination code areas:
  - `apps/openagents-runtime/lib/openagents_runtime/tools/comms/*`
  - `apps/openagents-runtime/lib/openagents_runtime/integrations/comms/*`
  - `apps/openagents.com/resources/js/pages/settings/integrations/*`
  - `docs/protocol/comms/*` (or proto layer-0 contracts once adopted)
- Routing justification:
  - runtime must own send-side effects, consent/suppression checks, and receipt/replay semantics,
  - Laravel must own integration authoring UX, encrypted secret management, and operator controls,
  - contracts must stay shared and versioned across runtime/control-plane boundaries.

## Parity and Verification Plan

- Fixture source: canonical comms policy/decision fixtures under `apps/openagents-runtime/test/fixtures/openclaw/comms/*`
- Canonical expected behavior: deterministic allow/deny outcomes with stable reason codes and receipt events.
- Verification harnesses: runtime parity tests + Laravel feature tests + cross-system integration tests.
- Required tests/lint/build:
  - runtime: ExUnit tests for policy + adapter + replay paths,
  - Laravel: feature/unit tests for UI, secret lifecycle, webhook verification.

## Security and Runtime Invariants

- Network egress risk review: all provider HTTP calls through runtime network seam.
- Secret handling/redaction review: keys encrypted at rest in Laravel and fetched ephemerally by runtime.
- Replay/receipt visibility review: send attempts/results and block reasons are receipt-visible.
- Abuse/failure mode review: suppression enforcement, retry bounds, idempotency keys, loop protection.

## Provenance and Governance

- Import date: 2026-02-19
- Import owner: Runtime + Laravel leads
- Ported module attribution requirement (file path + SHA): required for each imported concept/module.
- Vendored artifact/license preservation notes: preserve upstream notices for any direct adoptions.
- Follow-up drift report cadence: monthly drift scan against pinned SHAs + fixture parity checks.

## Rollout Plan

- Feature flag(s): `comms_tool_pack_v1`, `comms_resend_adapter_v1`
- Canary/shadow plan: shadow evaluation + internal tenants first.
- Rollback plan: disable flags and fall back to no-send policy-deny mode.
- Exit artifacts expected: parity report, security checklist, rollout decision record.

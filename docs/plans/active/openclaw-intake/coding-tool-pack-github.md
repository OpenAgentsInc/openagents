# OpenClaw Intake Record: Coding Tool Pack (GitHub First)

Date: 2026-02-19
Owner: Runtime + Laravel control plane
Status: `active`

## Capability Identity

- Capability ID: `coding-tool-pack-github-v1`
- Summary: OpenClaw-inspired coding extension surface for GitHub issue/PR context and coding workflow writebacks via deterministic runtime policy + receipts.
- OpenClaw upstream repository: `~/code/openclaw`
- Upstream commit SHA: `8e1f25631b220f139e79003caecabd11b7e1e748`
- Upstream paths:
  - `skills/github/SKILL.md` (GitHub operation surface and expected command intents)
  - `extensions/lobster/README.md` (coding workflow bridge expectations for GitHub operations)
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
  - `apps/openagents-runtime/lib/openagents_runtime/tools/coding/*`
  - `apps/openagents-runtime/lib/openagents_runtime/tools/extensions/coding_manifest_validator.ex`
  - `docs/protocol/coding/*`
  - `apps/openagents.com/app/AI/Runtime/*` (follow-up integration authoring surface)
- Routing justification:
  - runtime must own GitHub side effects, policy gates, and replayable receipts,
  - Laravel must own integration authoring UX and encrypted secret lifecycle,
  - protocol contracts must stay shared and versioned across runtime/control-plane boundaries.

## Parity and Verification Plan

- Fixture source: runtime contract tests under `apps/openagents-runtime/test/openagents_runtime/tools/coding/*`.
- Canonical expected behavior: deterministic allow/deny outcomes with stable reason codes and receipt events for read/write GitHub operations.
- Verification harnesses: runtime unit tests + manifest registry contract checks + runtime contract convergence checks.
- Required tests/lint/build:
  - runtime: ExUnit tests for manifest validation + adapter mapping + kernel policy/replay paths,
  - contracts: `mix runtime.contract.check` for reason taxonomy and proto/json/module convergence.

## Security and Runtime Invariants

- Network egress risk review: all GitHub HTTP calls through runtime guarded seam (`tools/network/*`) with allowlist + DNS pinning.
- Secret handling/redaction review: tokens fetched ephemerally via Laravel internal secret fetch API and redacted in outputs.
- Replay/receipt visibility review: coding operations emit deterministic reason-coded receipts and replay hashes.
- Abuse/failure mode review: write operations support enforce/audit modes and circuit breaker guardrails.

## Provenance and Governance

- Import date: 2026-02-19
- Import owner: Runtime + Laravel leads
- Ported module attribution requirement (file path + SHA): required for each imported concept/module.
- Vendored artifact/license preservation notes: preserve upstream notices for any direct adoptions.
- Follow-up drift report cadence: monthly drift scan against pinned SHA + parity harness checks.

## Rollout Plan

- Feature flag(s): `coding_tool_pack_v1`, `coding_github_adapter_v1`
- Canary/shadow plan: shadow read-only operations first, then write operations with `write_operations_mode=enforce`.
- Rollback plan: disable flags and fall back to deny mode for coding side effects.
- Exit artifacts expected: parity report, security checklist, rollout decision record.

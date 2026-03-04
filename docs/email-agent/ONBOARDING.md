# Email Lane Onboarding Runbook

## Purpose

This runbook brings a new tenant deployment online without relying on tribal knowledge.

## Preflight Checklist

- Confirm tenant isolation docs and lane boundaries:
  - `docs/email-agent/ARCHITECTURE.md`
  - `docs/email-agent/TENANT_ISOLATION.md`
  - `docs/email-agent/SECURITY_PRIVACY_RUNBOOK.md`
- Confirm release gate scripts are present:
  - `scripts/lint/email-agent-quality-gate.sh`
  - `scripts/lint/email-agent-release-gate.sh`
- Confirm runtime has required Rust toolchain and workspace dependencies.

## Day-0 Setup Checklist

1. Provision tenant lane
- Create tenant environment using `provision_tenant_environment` in `crates/email-agent/src/tenant_isolation.rs`.
- Verify hard isolation with `verify_hard_tenant_isolation`.

2. Configure credentials
- Load tenant-scoped OAuth secrets into tenant namespace (never global/shared scope).
- Required Gmail OAuth fields:
  - client id
  - client secret
  - redirect URI
  - access token
  - refresh token
  - token expiry unix timestamp
- Validate that secret scope version is recorded and non-zero.

3. Configure network boundaries
- Set tenant-specific Gmail/API egress allowlist.
- Set tenant-specific relay allowlist.
- Verify that allowlists are not shared through global defaults.

4. Configure retention/export controls
- Set retention windows via `RetentionPolicy`.
- Confirm export role policy (`FullContent` requires auditor role).
- Confirm debug/log redaction policy is enabled.

## Sync Verification Procedure

1. Backfill verification
- Run backfill and confirm deterministic checkpoint progression (`imported_count`, `next_page_token`).
- Confirm imported messages normalize successfully.

2. Incremental sync verification
- Run incremental sync and verify cursor advancement.
- Confirm duplicate deltas are dropped deterministically.
- Confirm stale cursor path triggers rebootstrap requirement.

3. Trace verification
- Generate correlation ID for a sample message.
- Confirm lifecycle events exist for required send trace stages:
  - ingest
  - retrieve
  - draft
  - approve
  - send

## Quality and Release Checks

Run before enabling production traffic:

- `scripts/lint/email-agent-quality-gate.sh`
- `scripts/lint/email-agent-release-gate.sh`

Expected result:
- Quality gate passes golden corpus thresholds.
- E2E harness passes success path and failure-injection scenarios.

## Go-Live Checklist

- Tenant isolation report passes with no violations.
- OAuth token lifecycle valid (not expired; refresh window healthy).
- Backfill and incremental sync verified.
- Approval queue controls verified (pause/resume + kill switch).
- Send execution idempotency + retry behavior verified.
- Follow-up scheduler policy verified (business/quiet hours + recipient limits).
- Security/privacy controls validated (retention/export/deletion/audit).

## Post-Go-Live Validation (First 24h)

- Verify send trace completeness for sample sends.
- Verify no redaction leaks in diagnostics.
- Verify daily retention sweep deletes expected stale records.
- Verify error taxonomy routes incidents to correct runbook entries.

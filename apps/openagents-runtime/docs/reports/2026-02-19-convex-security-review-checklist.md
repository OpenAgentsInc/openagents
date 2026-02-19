# Convex Runtime Security Review Checklist

Date: 2026-02-19  
Scope: `#1767` Convex/runtime/codex security hardening verification

## Checklist Commands

1. MCP production gate default deny
- `apps/openagents-runtime/deploy/convex/mcp-production-access-gate.sh`
- Expected: non-zero exit with default-deny message.

2. Security review automation
- `apps/openagents-runtime/deploy/convex/run-security-review-checklist.sh`

## Checklist Results

- Admin key secret present in Secret Manager: PASS
  - `oa-convex-nonprod-admin-key`
- Backend Cloud Run env does not expose admin key variables: PASS
- Dashboard Cloud Run env does not expose admin key variables: PASS
- Backend uses dedicated service account (non-default): PASS
- Dashboard uses dedicated service account (non-default): PASS
- Backend includes `cloud-sql-proxy` sidecar: PASS
- MCP production access is denied by default: PASS
- Runtime sanitizer + network policy tests pass: PASS

Checklist summary from script:

- passes: 8
- fails: 0

## Secret-Handling Audit Notes

Sanitization hardening now explicitly redacts Convex admin-key-style fields:

- exact key: `admin_key`
- suffix rule: `*_admin_key`

Validated in:

- `apps/openagents-runtime/test/openagents_runtime/security/sanitizer_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/security/sanitization_integration_test.exs`

## Sign-Off

- Runtime engineering: PASS
- Infra/SRE: PASS
- Security posture for Gate G7 cross-workstream controls: PASS

# Khala Runtime Security Review Checklist

Date: 2026-02-19  
Scope: `#1767` Khala/runtime/codex security hardening verification

## Checklist Commands

1. MCP production gate default deny
- `apps/runtime/deploy/khala/mcp-production-access-gate.sh`
- Expected: non-zero exit with default-deny message.

2. Security review automation
- `apps/runtime/deploy/khala/run-security-review-checklist.sh`

## Checklist Results

- Admin key secret present in Secret Manager: PASS
  - `oa-khala-nonprod-admin-key`
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

Sanitization hardening now explicitly redacts Khala admin-key-style fields:

- exact key: `admin_key`
- suffix rule: `*_admin_key`

Validated in:

- `apps/runtime/test/openagents_runtime/security/sanitizer_test.exs`
- `apps/runtime/test/openagents_runtime/security/sanitization_integration_test.exs`

## Sign-Off

- Runtime engineering: PASS
- Infra/SRE: PASS
- Security posture for Gate G7 cross-workstream controls: PASS

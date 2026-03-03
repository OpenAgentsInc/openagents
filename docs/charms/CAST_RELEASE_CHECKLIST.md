# CAST Release Checklist

Date: 2026-03-03

## Required Checks

1. Validate skills registry:
- `scripts/skills/validate_registry.sh`

2. Run CAST smoke check (non-broadcast):
- `scripts/lint/cast-skill-smoke-check.sh`

3. Confirm documentation consistency:
- `docs/charms/CAST_FIRSTCLASS_SUPPORT_PLAN.md`
- `docs/charms/CAST_OPERATOR_RUNBOOK.md`
- `docs/charms/CAST_FAILURE_MODES.md`
- `docs/charms/CAST_TEST_MATRIX.md`

4. Confirm contract lock values are unchanged or intentionally updated with rationale:
- `CAST_APP_VERSION`
- `CAST_APP_IDENTITY`
- `CAST_APP_BIN_NAME`
- `CAST_SCROLLS_DEFAULT_BASE_URL`

## Optional App-Layer Checks

Run only if CAST desktop pane/workflows were modified:

1. Verify pane opens from command palette and remains singleton.
2. Verify CAST pane actions update pane state without crashes.
3. Verify CAST activity events appear in Activity Feed with stable event IDs.

## Promotion Gate

Release is ready when all required checks pass and no unresolved high-severity CAST issues remain open.

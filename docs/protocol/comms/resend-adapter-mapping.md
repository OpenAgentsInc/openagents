# Resend Adapter Mapping

Runtime adapter: `apps/openagents-runtime/lib/openagents_runtime/tools/comms/providers/resend_adapter.ex`

## Request Mapping

Runtime comms request -> Resend payload:
- `recipient` -> `to[]`
- `template_id` + `variables` -> deterministic text fallback (and tags)
- `from` sourced from request or adapter opts
- `subject` sourced from request or template-derived default

## Error-to-Reason Mapping

- `200-299` -> `policy_allowed.default` (state: `sent`)
- `400`/`422` -> `manifest_validation.invalid_schema`
- `401`/`403` -> `policy_denied.explicit_deny`
- `429` -> `policy_denied.budget_exhausted`
- transport/5xx/unknown -> `comms_failed.provider_error`

These reason codes flow into runtime comms receipts for deterministic auditing and replay context.

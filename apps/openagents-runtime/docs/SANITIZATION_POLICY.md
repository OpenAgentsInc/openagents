# Runtime Sanitization Policy

This policy defines the enforced redaction layer for secrets/PII across runtime boundaries.

## Centralized sanitizer

- Module: `OpenAgentsRuntime.Security.Sanitizer`
- Entry point: `sanitize/2`
- Deterministic behavior:
  - key-based redaction for secrets (`authorization`, `*_token`, `*_secret`, `api_key`, etc.)
  - key-based PII redaction (`email`, `phone`, `address`, etc.)
  - string-pattern redaction for bearer tokens, API keys, JWT-like blobs, emails, and phone numbers

## Enforced boundaries

Sanitization is applied before data reaches persistent or observability surfaces:

1. Event log writes
   - `OpenAgentsRuntime.Runs.RunEvents.append_event/3`
2. Tool execution I/O
   - `OpenAgentsRuntime.Tools.ToolRunner` (tool input/progress/output/task metadata)
3. Trace storage
   - `OpenAgentsRuntime.DS.Traces.capture/4`
4. Tool replay context
   - `OpenAgentsRuntime.DS.ToolReplay`
5. Telemetry metadata/logging surfaces
   - `OpenAgentsRuntime.Telemetry.Events`
6. Comms provider outcomes/receipts
   - `OpenAgentsRuntime.Tools.Comms.Kernel`
   - `OpenAgentsRuntime.Tools.Comms.Providers.ResendAdapter`

## Operational rule

- Runtime should never persist raw secrets or direct PII in:
  - `runtime.run_events`
  - `runtime.tool_tasks`
  - trace payload surfaces
  - telemetry metadata/log fields

## Validation

- Unit sanitizer behavior:
  - `test/openagents_runtime/security/sanitizer_test.exs`
- Integration boundary coverage:
  - `test/openagents_runtime/security/sanitization_integration_test.exs`

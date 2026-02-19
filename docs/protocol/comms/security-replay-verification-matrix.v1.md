# Comms Security + Replay Verification Matrix (v1)

This matrix is the release gate for comms security, policy replayability, and cross-system behavior between Laravel and runtime.

## CI Gate

- Workflow: `.github/workflows/comms-security-replay-matrix.yml`
- Required lanes: `laravel`, `runtime`
- Gate rule: all lanes must pass before merge/deploy.

## Verification Lanes

| Lane | Coverage | Test entrypoints |
| --- | --- | --- |
| `laravel` | Encrypted secret storage, UI auth boundaries, webhook signature checks, audit trails | `tests/Feature/Settings/IntegrationSettingsTest.php`<br>`tests/Feature/Settings/IntegrationSecretLifecycleTest.php`<br>`tests/Feature/Api/Webhooks/ResendWebhookPipelineTest.php`<br>`tests/Feature/Api/Internal/RuntimeSecretFetchTest.php`<br>`tests/Feature/Settings/CommsOwnershipProjectionFlowTest.php` |
| `runtime` | Consent/suppression/policy gates, reason-coded receipts, no-secret redaction surfaces, cross-system send/revoke/replay flows | `test/openagents_runtime/tools/comms/kernel_test.exs`<br>`test/openagents_runtime/tools/comms/providers/resend_adapter_test.exs`<br>`test/openagents_runtime/integrations/laravel_secret_client_test.exs`<br>`test/openagents_runtime/integrations/comms_security_replay_matrix_test.exs`<br>`test/openagents_runtime/security/sanitization_integration_test.exs` |

## Incident-Style Cases

| Incident ID | Failure mode | Expected behavior | Verification |
| --- | --- | --- | --- |
| `INC-COMMS-201` | Runtime send cannot proceed unless scoped secret fetch succeeds. | Signed Laravel fetch is executed, provider send occurs, receipt state is `sent` with `policy_allowed.default`. | `CommsSecurityReplayMatrixTest` case `INC-COMMS-201` |
| `INC-COMMS-202` | Integration revoked between executions. | New execution scope gets `secret_not_found`; provider send is not attempted; adapter returns `policy_denied.explicit_deny`. | `CommsSecurityReplayMatrixTest` case `INC-COMMS-202` |
| `INC-COMMS-203` | Replay drift between execution and replayed policy decision. | Blocked execution and replay have matching `decision`, `reason_code`, and `evaluation_hash`; replay hash is deterministic. | `CommsSecurityReplayMatrixTest` case `INC-COMMS-203` |
| `INC-COMMS-204` | Secret/PII leakage in runtime comms surfaces. | Provider result and receipt surfaces are redacted (`[REDACTED]`, `[REDACTED_EMAIL]`) and raw secret values are absent. | `KernelTest` case `execute_send/3 redacts secret material from provider and receipt output surfaces` |
| `INC-COMMS-205` | Provider webhook signature mismatch or idempotency conflict in Laravel ingest. | Invalid signatures are rejected/audited; duplicate payload mismatch returns conflict and does not create divergent state. | `ResendWebhookPipelineTest` |

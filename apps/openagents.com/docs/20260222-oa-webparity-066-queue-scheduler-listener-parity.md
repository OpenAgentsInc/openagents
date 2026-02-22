# OA-WEBPARITY-066 Queue/Scheduler/Listener Parity + Laravel Async Shutdown

Date: 2026-02-22  
Status: pass (async-lane parity verification + shutdown automation)  
Issue: OA-WEBPARITY-066

## Deliverables

- Async lane parity harness:
  - `apps/openagents.com/scripts/run-async-lane-parity-harness.sh`
- Manual workflow dispatch:
  - `.github/workflows/web-async-lane-parity-harness.yml`
- Async lane retirement/shutdown automation:
  - `apps/openagents.com/service/scripts/verify-laravel-async-lanes-disabled.sh`
  - `apps/openagents.com/service/scripts/disable-legacy-laravel-async-jobs.sh`
  - `apps/openagents.com/service/docs/LARAVEL_ASYNC_RETIREMENT.md`

## Legacy-to-Rust Async Ownership Mapping

1. Legacy queued webhook forward job (`ForwardResendWebhookToRuntime`) is replaced in Rust by:
   - `spawn_resend_webhook_forward_task`
   - `forward_resend_webhook_to_runtime`
2. Runtime retry state transitions are persisted in Rust:
   - `forwarding` -> `forward_retrying` -> `forwarded`/`failed`
3. Scheduler/listener posture:
   - No active Laravel scheduler kernel in production lane.
   - Active docs/scripts are guarded against `queue:work`/`schedule:run` operational references.

## Verification Executed

```bash
bash -n apps/openagents.com/service/scripts/verify-laravel-async-lanes-disabled.sh
bash -n apps/openagents.com/service/scripts/disable-legacy-laravel-async-jobs.sh
cargo test --manifest-path apps/openagents.com/service/Cargo.toml resend_webhook_forwarding_retries_and_projects_delivery
cargo test --manifest-path apps/openagents.com/service/Cargo.toml resend_webhook_records_forward_retrying_state_before_success
./apps/openagents.com/scripts/run-async-lane-parity-harness.sh
```

Artifact produced:
- `apps/openagents.com/storage/app/async-lane-parity-harness/<timestamp>/summary.json`

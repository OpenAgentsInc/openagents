# Laravel Async Lane Retirement (Queue/Scheduler)

## Purpose

Retire legacy Laravel queue/scheduler runtime lanes after Rust parity is proven for async webhook forwarding and projection side effects.

## Legacy-to-Rust Mapping

1. Legacy queued job:
   - `app/Jobs/ForwardResendWebhookToRuntime.php`
2. Rust ownership replacement:
   - `spawn_resend_webhook_forward_task(...)`
   - `forward_resend_webhook_to_runtime(...)`
   - runtime retry state transitions persisted via:
     - `mark_webhook_event_forwarding`
     - `mark_webhook_event_retrying`
     - `mark_webhook_event_forwarded` / `mark_webhook_event_forward_failed`
3. Scheduler/listener posture:
   - No active Laravel scheduler kernel (`app/Console/Kernel.php` is absent).
   - No active Laravel schedule definitions in production lane.

## Verification

Run:

```bash
apps/openagents.com/service/scripts/verify-laravel-async-lanes-disabled.sh
```

This verifier fails if:
- active scheduler definitions are detected,
- active docs/scripts still reference production `queue:work` or `schedule:run`,
- Rust webhook async forwarding symbols are missing.

## Production Shutdown of Legacy Async Jobs

Dry-run first:

```bash
PROJECT=<gcp-project> \
REGION=us-central1 \
DRY_RUN=1 \
apps/openagents.com/service/scripts/disable-legacy-laravel-async-jobs.sh
```

Apply:

```bash
PROJECT=<gcp-project> \
REGION=us-central1 \
DRY_RUN=0 \
apps/openagents.com/service/scripts/disable-legacy-laravel-async-jobs.sh
```

Notes:
- Default Cloud Run job names: `openagents-queue`, `openagents-scheduler`.
- Default Cloud Scheduler name: `openagents-scheduler`.
- Override via `RUN_JOBS_CSV` and `SCHEDULER_JOBS_CSV` if environment uses different names.

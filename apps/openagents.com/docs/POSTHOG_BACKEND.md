# PostHog Backend Telemetry (Rust Services)

Status: active summary.

Telemetry events should be emitted from Rust control/runtime service codepaths with stable event naming and request-id correlation.

## Required fields

1. `event_name`
2. `user_id` or `org_id` when available
3. `request_id`
4. `service` (`control`, `runtime`, `khala`)
5. timestamp

## Source of truth

1. `docs/audits/` telemetry audits
2. service observability docs under `apps/runtime/docs/` and control service docs

Legacy Laravel-specific PostHog setup details were archived to backroom in OA-RUST-113.

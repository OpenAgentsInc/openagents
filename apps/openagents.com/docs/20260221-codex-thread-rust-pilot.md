# Codex Thread Rust Pilot (OA-RUST-031)

Status: active pilot route plan
Owner: `apps/openagents.com`
Issue: `#1846` (OA-RUST-031)
Updated: 2026-02-21

## Route scope

- Pilot route: `/chat/:thread_id`
- Rust target: `apps/openagents.com/web-shell`
- Rollback target: `OA_ROUTE_SPLIT_LEGACY_BASE_URL` + route-split override API

## Implemented for pilot

1. Rust route split serves `/chat/*` via web-shell in Rust cohorts.
2. Rust shell handles Codex thread state from Khala worker-event payloads:
   - ignores noisy `thread/started`
   - renders `turn/started` and `turn/completed` system events
   - assembles streaming assistant/reasoning deltas with overlap dedupe
   - dedupes local user sends against replayed `codex/event/user_message`
3. Command path exists for thread sends:
   - `POST /api/runtime/threads/:thread_id/messages`
   - auth required
   - validated payload
   - auditable acceptance response contract

## Parity checklist (pilot gate)

1. Authenticated user can open `/chat/:thread_id` in Rust cohort.
2. Khala replay/resume restores stream continuity after reload.
3. User send command returns accepted response and stays deterministic in local state.
4. Duplicate/out-of-order replay does not duplicate user/assistant text in local transcript.
5. Rollback to legacy route can be triggered immediately via route-split override.

## Rollout and rollback notes

### Canary rollout

1. Set `OA_ROUTE_SPLIT_ENABLED=true`
2. Set `OA_ROUTE_SPLIT_MODE=cohort`
3. Set `OA_ROUTE_SPLIT_RUST_ROUTES=/chat`
4. Start with `OA_ROUTE_SPLIT_COHORT_PERCENTAGE=5`
5. Monitor route-split decision audits + command error rates before increasing cohort

### Fast rollback

1. Call `POST /api/v1/control/route-split/override` with `{ "target": "legacy" }`
2. Validate `/chat/:thread_id` returns temporary redirect to legacy base URL
3. Keep Khala/session auth lanes running; clear override after remediation

## Verification commands

```bash
cargo test -p openagents-control-service route_split
cargo test -p openagents-control-service thread_message
cargo test -p openagents-web-shell codex_thread
```

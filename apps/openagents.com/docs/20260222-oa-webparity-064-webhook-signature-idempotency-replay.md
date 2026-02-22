# OA-WEBPARITY-064 Webhook Signature/Idempotency/Replay Parity

Date: 2026-02-22  
Status: pass (webhook parity contract tests + harness automation)  
Issue: OA-WEBPARITY-064

## Deliverables

- Webhook parity harness:
  - `apps/openagents.com/scripts/run-webhook-parity-harness.sh`
- Manual workflow dispatch: removed (workflow automation disabled by invariant).
- Webhook parity fixes and tests:
  - `apps/openagents.com/service/src/lib.rs`
  - `apps/openagents.com/service/src/domain_store.rs`

## Covered Contracts

1. Signature verification parity:
   - invalid signatures are rejected and replay remains invalid
   - stale Svix timestamp signatures are rejected per configured tolerance
2. Idempotency + replay parity:
   - duplicate webhook deliveries return idempotent replay responses
   - conflicting payloads on same idempotency key return conflict
3. Retry-state transitions:
   - forwarding retries record explicit transition state (`forward_retrying`)
   - runtime attempt count reflects real attempts before final `forwarded`/`failed` state

## Verification Executed

```bash
cargo fmt --manifest-path apps/openagents.com/service/Cargo.toml
cargo test --manifest-path apps/openagents.com/service/Cargo.toml resend_webhook_
bash -n apps/openagents.com/scripts/run-webhook-parity-harness.sh
./apps/openagents.com/scripts/run-webhook-parity-harness.sh
```

Artifact produced:
- `apps/openagents.com/storage/app/webhook-parity-harness/<timestamp>/summary.json`

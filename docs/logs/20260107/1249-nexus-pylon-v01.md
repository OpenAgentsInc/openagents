# Nexus + Pylon v0.1 Enablement Log

**Date:** 2026-01-07 12:49
**Status:** Worker deployed, Pylon defaults aligned, payment matching tightened

---

## Summary

- Deployed Nexus worker to Cloudflare and verified NIP-11 + AUTH challenge on `nexus.openagents.com`.
- Updated relay references from `relay.openagents.com` to `nexus.openagents.com` across relay-worker, Pylon desktop, and docs.
- Tightened DVM job handling to filter provider-targeted jobs and match payments by invoice across Spark/Lightning/Token details.
- Updated Pylon defaults + CLI to use Nexus relay, include relay tags in job requests, and set regtest examples for payments.

---

## Key Changes

### Nexus Worker
- Custom domain route added in `crates/nexus/worker/wrangler.toml`.
- Cloudflare D1 `nexus` database applied and worker deployed.
- Verified WebSocket AUTH challenge response and NIP-11 metadata.

### Compute (DVM)
- Skip job requests not targeted to provider pubkey (retain broadcast handling).
- Match invoices via `PaymentDetails::Lightning`, `::Spark`, and `::Token` and require `PaymentType::Receive` for completion.

### Pylon
- Default relay lists now include `wss://nexus.openagents.com`.
- Job submit adds relay tags from CLI flags.
- Provider relay service uses config relays when configured.
- Regtest examples documented for Spark payments.

---

## Verification

- `cargo test -p compute --lib`
- `cargo test -p pylon --lib`

---

## Notes

- End-to-end Pylon auth/subscribe + broadcast tests remain pending on live infra.
- Local `.wrangler/` artifacts not committed.

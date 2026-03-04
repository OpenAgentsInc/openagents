# Autopilot Earn Backroom Harvest Audit

Date: 2026-03-04
Source archive: `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp`
Issues: #2815, #2816, #2817, #2818

## Scope

Audit backroom NIP-90/provider/wallet assets and decide what to restore into the MVP repo without reintroducing archived surface area.

Scope note: this harvest targeted compute-provider MVP assets only, not future liquidity-solver codepaths.

## Candidate Inventory

| Backroom Source | Purpose | Keep/Drop | MVP Landing |
| --- | --- | --- | --- |
| `apps/autopilot-desktop/src/provider_domain.rs` | in-process pylon lifecycle + runtime status mapping | Keep (patterns) | `apps/autopilot-desktop/src/state/provider_runtime.rs` + Mission Control status projection |
| `apps/autopilot-desktop/src/wallet_domain.rs` | Spark wallet bridge/status normalization | Keep (patterns) | `apps/autopilot-desktop/src/spark_wallet.rs` and payout-gate/reconciliation modules |
| `apps/autopilot-desktop/src/main.rs::submit_nip90_text_generation` | submit + await NIP-90 helper | Keep (core behavior) | `crates/nostr/client/src/dvm.rs` (`submit_job_request_and_await_result`) |
| `apps/autopilot-desktop/src/inbox_domain.rs` | inbox projection contracts | Keep (concepts) | `apps/autopilot-desktop/src/state/job_inbox.rs` |
| `crates/pylon/` | legacy provider daemon/runtime | Drop (too broad for MVP pruning) | n/a |
| `crates/compute/` | archived compute orchestration + tests | Drop (superseded by MVP app-layer state) | n/a |
| `apps/openagents.com/` | archived web app surface | Drop for repo scope | n/a |

## Migration Risk Notes

- `provider_domain.rs` in backroom depends on `pylon` DB/runtime APIs removed from this MVP repo.
- `wallet_domain.rs` has environment/network assumptions tied to older pylon config layout; only mapping patterns are portable.
- The helper in backroom `main.rs` used a direct `DvmClient` type not present in the current desktop module graph; the portable piece is submit+await sequencing.

## Decisions

1. Preserve app-layer ownership for provider/wallet orchestration (`apps/autopilot-desktop`) per `docs/OWNERSHIP.md`.
2. Restore only portable logic patterns, not archived crate topology.
3. Prefer extending `crates/nostr/client` for reusable NIP-90 request/result helper behavior.
4. Track exact restored provenance in `docs/AUTOPILOT_EARN_BACKROOM_PROVENANCE.md`.

## Verification

- Backroom paths were enumerated and inspected directly from archive.
- Landing paths exist in current repo and compile under MVP ownership boundaries.

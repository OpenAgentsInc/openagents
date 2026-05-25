# Pylon Open Source Contributor Boundary

Status: contributor Pylon must remain open source

Contributor Pylon is the public, installable supply connector for people who
run OpenAgents on their own machines. It must stay auditable without access to
the private `OpenAgentsInc/cloud` repo.

## Public Surfaces

These surfaces remain public in this repo:

- `installable_app`: `apps/pylon`, release scripts, and the npm bootstrap path.
- `tui`: `apps/pylon-tui` and the explicit `pylon tui` shell.
- `contributor_wallet_ux`: built-in LDK wallet setup, backup, channel,
  invoice, payment, and withdraw surfaces.
- `provider_inventory_truth`: public provider availability, inventory,
  capability, and local status projections.
- `payout_behavior`: contributor payout target registration, accepted-work
  settlement records, wallet history, and payout proofs.
- `public_receipts`: contributor-facing receipts, local proof receipts, and
  public compatibility fixtures.

The private Cloud repo may build managed node and workroom infrastructure, but
it must not become a dependency of contributor Pylon. Public Pylon may share
public contract shapes such as `openagents.cloud_node.v1`, and it may expose
public-safe projections through `crates/pylon-core`.

## Audit Requirements

Contributor payout behavior remains auditable from public source:

- LDK wallet behavior is documented under `docs/pylon/LDK_*.md`.
- Pylon ledger, wallet, payout, and settlement record types live under
  `apps/pylon/src/`.
- Local proof runtime behavior is documented in `docs/pylon/README.md` and can
  run without private Cloud code.
- Cloud compatibility fixtures live under
  `docs/pylon/fixtures/cloud_node_v1/` and are validated by `crates/pylon-core`.

Private fleet topology, managed workroom sidecar policy, capacity-pool
placement, internal accounting credentials, and private rollout/quarantine
policy belong in the private `cloud` repo, not in contributor Pylon.

## Verification

```bash
cargo test -p pylon-core
cargo test -p pylon --test cloud_node_v1_fixture
```

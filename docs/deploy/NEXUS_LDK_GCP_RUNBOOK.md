# Nexus LDK GCP Runbook

Date: 2026-05-16

This runbook implements the LDK-06 topology for Nexus v0.2 and Pylon v0.2:
hosted `bitcoind` plus LDK Server on Google Cloud, with Nexus calling the LDK
node over a private interface. LDK-07 cuts the standard Nexus funding-invoice
path to LDK `Bolt11Receive`; LDK admin operations own peer, channel, and payout
smoke.

## Architecture

```mermaid
flowchart LR
  subgraph cf["Cloudflare"]
    web["openagents.com / Autopilot web\npublic UI, auth, API facade"]
  end

  subgraph gcp["Google Cloud us-central1"]
    nexus["nexus-mainnet-1\nNexus authority API"]
    ldk["nexus-ldk-mainnet-1\nLDK Server systemd service"]
    bitcoin["oa-bitcoind or replacement\nBitcoin Core RPC + chain truth"]
    disk["Persistent SSD\nkeys_seed, TLS, API key, SQLite"]
    snapshots["GCP snapshots + backup archive"]
  end

  subgraph ln["Bitcoin / Lightning"]
    chain["Bitcoin mainnet"]
    peers["Lightning peers and liquidity"]
  end

  web --> nexus
  nexus -- "private VPC tcp:3536\nHMAC + TLS pin" --> ldk
  ldk -- "private RPC tcp:8332" --> bitcoin
  bitcoin --> chain
  ldk <--> peers
  ldk --> disk
  disk --> snapshots
```

Hosting decision:

- Google Cloud is the right host for `bitcoind`, LDK Server, channel state,
  node seed, SQLite state, snapshots, and restore drills.
- Cloudflare remains the right host for web, edge API facades, auth, queues,
  read-only projections, and React/Three visualizations.
- Cloudflare Workers must not host the LDK node or Nexus treasury spend
  authority.

## Scripts

All scripts live under `scripts/deploy/nexus/`.

1. Provision the private topology:

```bash
NEXUS_LDK_TOPOLOGY_DRY_RUN=true \
scripts/deploy/nexus/22-provision-ldk-topology.sh

NEXUS_LDK_TOPOLOGY_DRY_RUN=false \
scripts/deploy/nexus/22-provision-ldk-topology.sh
```

The provisioning script creates or updates:

- `nexus-ldk-mainnet-1` with no external IP.
- `nexus-ldk-data-mainnet` persistent SSD.
- IAP SSH ingress only from `35.235.240.0/20`.
- private LDK gRPC ingress on `tcp:3536` only from the Nexus host tag.
- private `bitcoind` RPC ingress only from the LDK host tag.

It does not expose the LDK gRPC port publicly. Public Lightning P2P is skipped
by default. Set `NEXUS_LDK_ALLOW_PUBLIC_P2P=true` only after the node is ready
to announce and the operator has reviewed channel/liquidity policy.

2. Install and pin LDK Server:

```bash
NEXUS_LDK_INSTALL_DRY_RUN=true \
scripts/deploy/nexus/23-install-ldk-server-host.sh

NEXUS_LDK_SERVER_REF=<reviewed-commit-or-tag> \
NEXUS_BITCOIND_RPC_HOST=<private-bitcoind-ip> \
NEXUS_BITCOIND_RPC_PORT=8332 \
NEXUS_BITCOIND_RPC_USER=<rpc-user> \
NEXUS_BITCOIND_RPC_PASSWORD_PATH=/etc/ldk-server/bitcoind-rpc-password \
scripts/deploy/nexus/23-install-ldk-server-host.sh
```

The install script requires the `bitcoind` RPC password file before a real
install. For a bootstrap that intentionally writes a placeholder first, set
`NEXUS_LDK_INSTALL_ALLOW_PLACEHOLDER_BITCOIND=true`; do not treat that as a
passing read-only host until the real password is written and smoke passes.

The installer:

- mounts the LDK data disk at `/var/lib/ldk-server`;
- builds `ldk-server` and `ldk-server-cli` from the pinned
  `NEXUS_LDK_SERVER_REF`;
- writes `/etc/ldk-server/ldk-server.toml`;
- binds LDK gRPC/metrics to `0.0.0.0:3536`, protected by private VPC firewall;
- enables Prometheus metrics on the same port;
- installs `ldk-server.service` with `Restart=always`;
- installs `/etc/logrotate.d/ldk-server`;
- leaves LDK-generated API key and TLS files on disk without printing them.

LDK Server stores the API key as raw bytes at:

```text
/var/lib/ldk-server/<network>/api_key
```

The CLI expects hex when used manually:

```bash
xxd -p -c 64 /var/lib/ldk-server/bitcoin/api_key
```

Do not paste the raw or hex API key into docs, issue comments, logs, or commit
messages.

## Pylon v0.2 Registration

Pylon v0.2 nodes must register an LDK-compatible Lightning payout target before
Nexus treats them as eligible for new paid work. This treasury registration is
the source of truth for paid-work eligibility; stale settlement fields on node
heartbeats are ignored. A worker without a registered LDK target is blocked
before hosted starter auto-launch, manual homework launch, and default CS336
lease claim admission. Configure one of:

- a BOLT12 offer, preferred for durable registration;
- a BIP353 name;
- an LNURL-pay target;
- a per-payment BOLT11 invoice when the provider can rotate invoices safely.

Example config intent:

```text
payout_destination = "lno..."
```

For the OpenAgents-hosted Pylon proof fleet, use the idempotent rollout script
instead of hand-editing host config:

```bash
scripts/deploy/nexus/30-register-hosted-pylon-ldk-targets.sh
```

The script:

- connects to the configured hosted Pylon VMs through GCP IAP;
- skips existing LDK-compatible targets unless
  `NEXUS_PYLON_REPLACE_PAYOUT_TARGETS=true`;
- generates one unique BOLT12 offer per missing hosted Pylon from the Nexus LDK
  server;
- sets `payout_destination` through `pylon config set`;
- restarts `pylon.service`;
- logs only target kind, length, and short hash, never the raw payout target;
- waits for `/api/stats` to show the expected
  `nexus_ldk_payout_target_identities` count.

Useful controls:

```bash
# Limit rollout to a subset.
NEXUS_PYLON_HOSTS="pylon-gcp-1 pylon-gcp-2" \
scripts/deploy/nexus/30-register-hosted-pylon-ldk-targets.sh

# Replace an older hosted target with a fresh BOLT12 offer.
NEXUS_PYLON_HOSTS="pylon-gcp-1" \
NEXUS_PYLON_REPLACE_PAYOUT_TARGETS=true \
scripts/deploy/nexus/30-register-hosted-pylon-ldk-targets.sh

# Verify without changing Pylon hosts. This still asks LDK server to mint
# offers for hosts that would need a target, so use it as a rollout rehearsal,
# not as a no-contact unit test.
NEXUS_PYLON_REGISTER_DRY_RUN=true \
NEXUS_PYLON_WAIT_FOR_REGISTRATION=false \
scripts/deploy/nexus/30-register-hosted-pylon-ldk-targets.sh
```

Verification:

```bash
curl -fsS https://nexus.openagents.com/api/stats | jq '{
  nexus_ldk_payout_target_identities,
  homework_worker_eligible_pylons_online_now,
  nexus_missing_payout_target_blocked_beneficiaries_now,
  nexus_readiness_blocked_beneficiaries_now
}'
```

As of the 2026-05-19 rollout, all seven GCP-hosted Pylons had BOLT12 payout
destinations configured and Nexus reported seven LDK payout target identities.
Some non-hosted or old-version Pylons may still show
`homework_worker_payout_target_requires_ldk_v0_2`; that is expected until those
separate provider nodes are upgraded and configured.

Normal Pylon startup does not create or accept non-LDK payout destinations. The
only Pylon registration path is Lightning-only. Workers without registered LDK
payout targets remain visible for operator diagnostics, but new paid-work
eligibility should show
`payout_target_requires_ldk_v0_2`,
`homework_worker_payout_target_requires_ldk_v0_2`,
`homework_launch_target_payout_target_requires_ldk_v0_2`, or
`training_scheduler_payout_target_requires_ldk_v0_2`.

Validator claims are different from paid worker claims. A Pylon that can
validate a sealed window may claim validator challenges even when it does not
have a worker payout target. Pylon intake now tries paid worker claims before
validator backlog so a registered LDK worker is not starved behind old
retained validator artifacts. Validator materialization failures from stale
retained bundles, including missing `checkpoint_surface`, must be terminalized
or treated as nonfatal and must not abort fresh worker intake.

- a worker Pylon with an LDK target seals a starter window;
- stale validator backlog has unrecoverable retained artifacts;
- the validator materialization error aborts the whole intake pass;
- fresh LDK paid worker leases are never claimed;
- accepted-work payout proof never reaches the LDK dispatch rail.

Nexus lease admission must evaluate hard gates for the requested role, not the
worker role unconditionally. Otherwise validator-only or validator-first hosts
can never clear sealed windows.

## Retained Training Backlog Cleanup

Normal launch health must distinguish fresh work from historical retained
training state. Old active runs, unreconciled adapter windows, and queued or
leased validator challenges can remain useful as audit evidence, but they must
not keep `/api/stats` in `overall_status: bad` after the live worker path has
recovered.

Nexus exposes an explicit cleanup command:

```bash
nexus-control training backlog-cleanup \
  --retention-hours 24 \
  --report-path /var/lib/nexus-relay/reports/training-backlog-cleanup-dry-run.json
```

After reviewing the dry-run report:

```bash
nexus-control training backlog-cleanup \
  --apply \
  --retention-hours 24 \
  --report-path /var/lib/nexus-relay/reports/training-backlog-cleanup-applied.json
```

The command:

- cancels stale active training runs that do not have accepted outcomes;
- reconciles stale adapter windows that do not have accepted outcomes;
- terminalizes stale validator challenges with
  `stale_retained_backlog`;
- writes a `kernel.training.backlog.cleanup` receipt when it changes state;
- removes retired run IDs from scheduler indexes;
- leaves accepted-work payout evidence intact.

Public launch-health output includes fresh and retained counters for active
runs, pending validation windows, open validator challenges, and queued
validator challenges. Fresh counters drive `run_backlog` and
`validator_backlog` severity. Retained counters only emit the warning alert
`retained_training_backlog`.

The implementation proof and local dry-run report are documented in:

```text
docs/reports/nexus/2026-05-18-training-validator-backlog-cleanup.md
```

## Historical Payout Ledger Cleanup

The active payout rail is LDK-only, but older treasury state can still contain
failed provider-style or unknown-target rows. Those rows are historical audit
records, not current LDK payout failures, and must not be retried through the
LDK provider.

Run a dry-run report before any cleanup:

```bash
nexus-control treasury payout-ledger-cleanup \
  --report-path /var/lib/nexus-relay/payout-ledger-cleanup-dry-run.json \
  --json
```

If the report shows only stale non-LDK or unknown-target rows being retired,
apply it:

```bash
nexus-control treasury payout-ledger-cleanup \
  --apply \
  --report-path /var/lib/nexus-relay/payout-ledger-cleanup-apply.json \
  --json
```

After applying, inspect the summary:

```bash
nexus-control treasury status --json | jq '.training_payout_ledger_summary | {
  accepted_work_pending_payout_count,
  current_ldk_attention_payout_count,
  retired_historical_payout_count,
  retired_historical_accepted_work_payout_count,
  retired_historical_payout_sats
}'
```

`accepted_work_pending_payout_count` should remain zero unless fresh accepted
work is waiting. `current_ldk_attention_payout_count` is the live LDK problem
counter; `retired_historical_payout_count` is retained audit state.

## Hosted Pylon Runtime Install

Hosted Pylons must have the Psionic runtime surface installed before they can
honestly advertise training support. Installing only `/usr/local/bin/pylon` is
not enough for the retained training lane. Package the minimal Psionic runtime
from a reviewed Psionic commit, then install it on the Pylon hosts:

```bash
NEXUS_PYLON_RUNTIME_ARCHIVE=/tmp/psionic-runtime-<psionic-sha>.tar.gz \
scripts/deploy/nexus/29-install-pylon-psionic-runtime.sh
```

The script installs the archive into `/var/lib/pylon/psionic`, writes a
systemd drop-in with:

```text
OPENAGENTS_PSIONIC_REPO=/var/lib/pylon/psionic
```

and restarts `pylon.service`. Verify each host with:

```bash
sudo -u pylon /usr/local/bin/pylon training status --json
```

The minimum proof is:

- `runtime_surface_detected: true`
- `psionic_repo_root: "/var/lib/pylon/psionic"`
- `psionic_repo_source: "env_override"`
- `/var/lib/pylon/psionic/.openagents-psionic-revision` exists and contains
  the clean Psionic Git revision used for training admission identity.
- `/var/lib/pylon/psionic/fixtures/training/cs336_a1_reference_tiny_corpus.txt`
  exists. The bounded CS336 A1 paid-work smoke lane reads this packaged fixture
  from the runtime root; it must not resolve through a developer checkout path.

That proves runtime packaging only. Paid-work eligibility still requires a
real LDK payout target registered for the Pylon identity and usable Lightning
liquidity on the Nexus LDK node.

## Production Readiness Checks

Run read-only smoke:

```bash
# Local proof smoke, no hosted VM required.
scripts/deploy/nexus/24-smoke-ldk-server-readonly.sh

# Hosted VM smoke.
NEXUS_LDK_REMOTE_SMOKE=true \
scripts/deploy/nexus/24-smoke-ldk-server-readonly.sh
```

Hosted smoke checks:

- `ldk-server.service` is active;
- `keys_seed`, `tls.crt`, network `api_key`, and `ldk_node_data.sqlite` exist;
- `ldk-server-cli get-node-info` works;
- `ldk-server-cli get-balances` works;
- `GET /metrics` responds on the private gRPC port.

Sync LDK client material to Nexus:

```bash
scripts/deploy/nexus/28-sync-ldk-client-material.sh
```

This script copies only the client API key and TLS certificate needed by the
Nexus process from `nexus-ldk-mainnet-1` to `nexus-mainnet-1`. It does not
print secret bytes. It installs:

- `/etc/nexus-relay/ldk-server/api_key`
- `/etc/nexus-relay/ldk-server/tls.crt`
- `/etc/nexus-relay/ldk-server/client.env`

The Nexus Docker service mounts `/etc/nexus-relay` read-only, so these files
are visible to the container at the same paths. The generated `client.env` is
an operator receipt; the deploy script still writes the canonical runtime env.
The API key is owned by UID `60000`, matching the non-root `nexus` user inside
the `nexus-relay` image.

Deploy Nexus against LDK:

```bash
DEPLOY_IMAGE=<registry-image> \
NEXUS_TREASURY_PROVIDER=ldk \
NEXUS_LDK_SERVER_URL=auto \
NEXUS_LDK_API_KEY_PATH=/etc/nexus-relay/ldk-server/api_key \
NEXUS_LDK_TLS_CERT_PATH=/etc/nexus-relay/ldk-server/tls.crt \
NEXUS_LDK_NETWORK=bitcoin \
NEXUS_LDK_CHAIN_BACKEND=bitcoind \
NEXUS_CONTROL_TREASURY_POLICY_APPLY_ENV=true \
NEXUS_CONTROL_TREASURY_POLICY_CHANGE_REASON="cut production Nexus treasury provider to LDK Server" \
scripts/deploy/nexus/03-configure-and-start.sh
```

`NEXUS_LDK_SERVER_URL=auto` resolves to
`nexus-ldk-mainnet-1:3536`, not the private IP. The LDK Server TLS certificate
is valid for the VM hostname, and GCE internal DNS resolves that hostname from
the Nexus VM. Production deploys refuse to proceed unless
`NEXUS_TREASURY_PROVIDER` is
`ldk`, `NEXUS_LDK_NETWORK` is `bitcoin`/`mainnet`, and the LDK client paths are
set.

6. Back up LDK state:

```bash
NEXUS_LDK_BACKUP_DRY_RUN=true \
scripts/deploy/nexus/25-backup-ldk-server-state.sh

NEXUS_LDK_BACKUP_BUCKET=gs://openagentsgemini-nexus-ldk-backups \
scripts/deploy/nexus/25-backup-ldk-server-state.sh
```

The backup script creates:

- a GCP persistent-disk snapshot of the LDK data disk;
- a secret-bearing archive containing `keys_seed`, `tls.crt`, optional
  `tls.key`, network `api_key`, `ldk_node_data.sqlite`, optional
  `ldk_server_data.sqlite`, and `/etc/ldk-server`.

The archive contains custody material. Store it only in the restricted backup
bucket and do not copy it into the repo.

7. Run a restore drill:

```bash
NEXUS_LDK_RESTORE_DRY_RUN=true \
NEXUS_LDK_RESTORE_SNAPSHOT=<snapshot-name> \
scripts/deploy/nexus/26-restore-ldk-server-drill.sh

NEXUS_LDK_RESTORE_SNAPSHOT=<snapshot-name> \
scripts/deploy/nexus/26-restore-ldk-server-drill.sh
```

The drill creates a temporary read-only restore VM/disk, mounts the restored
disk read-only, verifies critical files, and leaves cleanup commands in the
output.

8. Run production readiness smoke:

```bash
NEXUS_BASE_URL=https://nexus.openagents.com \
NEXUS_CONTROL_ADMIN_BEARER_TOKEN=<admin-token> \
scripts/deploy/nexus/27-smoke-ldk-production-readiness.sh
```

The readiness smoke verifies the active Nexus API path:

- `GET /v1/treasury/status` reports `active_treasury_provider=ldk`,
  `active_treasury_rail=ldk`, and an `ldk_readiness` snapshot.
- The status payload separates `wallet_total_onchain_balance_sats`,
  `wallet_spendable_onchain_balance_sats`, `wallet_lightning_balance_sats`,
  and `wallet_balance_sats`. Treat `wallet_balance_sats` as usable payout
  liquidity; total on-chain sats may still be pending and are not enough for
  production readiness.
- `POST /v1/treasury/funding-target` returns a BOLT11 invoice from the LDK
  provider and no non-LDK invoice field.
- `POST /v1/admin/treasury/operations` can read `treasury.status`,
  `treasury.listPeers`, `treasury.listChannels`, and `treasury.listPayments`.
- JSON artifacts are written under `target/nexus-ldk-readiness/<timestamp>/`.

Optional write smoke is opt-in because it can connect peers, open channels, or
send payments:

```bash
NEXUS_LDK_WRITE_SMOKE=true \
NEXUS_LDK_SMOKE_PEER_NODE_ID=<peer-node-id> \
NEXUS_LDK_SMOKE_PEER_ADDRESS=<host:port> \
NEXUS_LDK_SMOKE_CHANNEL_AMOUNT_SATS=100000 \
scripts/deploy/nexus/27-smoke-ldk-production-readiness.sh
```

`NEXUS_LDK_SMOKE_CHANNEL_AMOUNT_SATS` must be at least
`NEXUS_LDK_SMOKE_MIN_CHANNEL_SATS`, which defaults to `20000`. This keeps the
write smoke from creating tiny channel probes that counterparties reject below
their policy minimum. A rejected pending open is reconciled back to a failed
operation and does not count as readiness capacity.

For payment send smoke, set one of:

```bash
NEXUS_LDK_SMOKE_PAY_INVOICE=<bolt11-invoice>
NEXUS_LDK_SMOKE_PAY_OFFER=<bolt12-offer>
NEXUS_LDK_SMOKE_PAY_AMOUNT_SATS=<amount-for-zero-amount-targets>
```

Each write command requires an idempotency key and records a redacted
`TreasuryOperationRecord`. Do not paste raw invoices, node secrets, API keys,
TLS keys, or bearer tokens into issue comments or docs.

## Nexus Client Configuration

After hosted LDK Server smoke passes, configure Nexus with disk paths that live
on the Nexus host and contain copied, pinned client material:

```bash
NEXUS_TREASURY_PROVIDER=ldk
NEXUS_LDK_SERVER_URL=nexus-ldk-mainnet-1:3536
NEXUS_LDK_API_KEY_PATH=/etc/nexus-relay/ldk-server/api_key
NEXUS_LDK_TLS_CERT_PATH=/etc/nexus-relay/ldk-server/tls.crt
NEXUS_LDK_NETWORK=bitcoin
NEXUS_LDK_CHAIN_BACKEND=bitcoind
```

Copy the API key and TLS cert through a secure operator path. The Nexus process
must load the key from disk and log only a TLS certificate fingerprint.

The supported operator path is:

```bash
scripts/deploy/nexus/28-sync-ldk-client-material.sh
```

After this configuration is active, `POST /v1/treasury/funding-target` should
return a BOLT11 invoice plus `phase_timings`. The standard funding path must
not create any non-LDK invoice.

## Production Readiness Gates

Production LDK is ready only when every gate below is green:

- `ldk_readiness.state` is `ready` on `/v1/treasury/status`, or the only
  remaining state is a documented warning accepted by the operator for that
  rollout.
- `ldk_readiness.min_ready_channel_count` is at least `2` and
  `ldk_readiness.projected_channel_count` meets or exceeds it. A single
  proof-scale channel is not production-ready even if small payments happen to
  settle.
- `ldk_readiness.min_ready_outbound_capacity_sats` is at least `20000` and
  `ldk_readiness.projected_outbound_capacity_sats` meets or exceeds it. The
  default initial target is deliberately small but must cover more than a
  one-off 25-sat proof payment.
- `wallet_spendable_onchain_balance_sats` and/or Lightning spendable outbound
  capacity have moved above zero after funding confirms.
- `ldk_readiness.registered_payout_target_count` is nonzero. A funded Nexus
  wallet with no Pylon v0.2 LDK payout target reports `needs_payout_targets`,
  not `ready`.
- `ldk_readiness.projected_outbound_capacity_sats` is sourced from live usable
  LDK channel outbound capacity, not from wallet balance alone, and is above
  the active payout reserve.
- `ldk_readiness.projected_inbound_capacity_sats` is nonzero once Pylon payout
  targets exist. Despite the historical field name, this readiness check is
  sourced from the live LDK provider channel list and represents usable payout
  capacity from Nexus toward registered Pylons.
- `ldk_readiness.recent_failed_payment_count_24h`,
  `recent_no_route_count_24h`, and `recent_insufficient_balance_count_24h` are
  below alert thresholds.
- `treasury.listPeers` shows expected peers or a documented reason to run
  without announced peers.
- `treasury.listChannels` shows the channel set expected for the rollout.
- `treasury.listPayments` and `treasury.reconcilePayments` agree on recent
  payment state.
- The latest LDK backup and restore drill succeeded after the active
  `NEXUS_LDK_SERVER_REF` was installed.
- A fresh LDK funding invoice was created and paid, and the wallet/status view
  observed the receive.
- A bounded payout smoke through `treasury.payInvoice` or `treasury.payOffer`
  completed from the active production binary.
- A fresh accepted-work closeout from a Pylon v0.2 worker moved through
  validator claim, acceptance, LDK dispatch, and payment receipt from the
  active production binaries. Old payout rows are not evidence for this gate.

### Current Production Accepted-Work Proof

The current production proof is recorded in
`docs/reports/nexus/2026-05-18-ldk-accepted-work-production-proof.md`.

Verified on 2026-05-18:

- OpenAgents hosted Pylon binary commit: `ad27f320b`
- Pylon binary SHA-256:
  `e839dd7857f2e8f7ddaaabb32a17c7415c3d5b773107282047b381cb0f6e0e16`
- Psionic runtime revision: `55e4b66f`
- Psionic runtime archive SHA-256:
  `2444877f67ed8f1d396b6a999dcb21272d99d2735bd7f65eda465e72f517108f`
- Hosted `psionic-train` SHA-256:
  `76c60acaf0dc9837c5679d92e9b404339d59a6cbd47b9bc5c9d2c19a60d29b67`
- Proof run:
  `run.cs336.a1.ldk-proof-20260518151532`
- Accepted window:
  `window.cs336.a1.ldk-proof-20260518151532.0001`
- Worker:
  `pylon-gcp-1`
- Validator:
  `pylon-gcp-3`
- Contribution id:
  `cf7c70416d7265f948fa78ee1e2f94b7bf03ef5975449e8eb89d244816b300d0`
- Payout:
  `25 sats`, `accepted_work`, `confirmed`, `settled`

The prior failure on `run.cs336.a1.ldk-proof-20260518094050` was not an LDK
payment failure. It was a hosted Pylon runtime packaging failure: the Pylon
binary had been updated, but `/var/lib/pylon/psionic` still contained a stale
runtime at revision `09b71872b24a934228f61c28e65e3aa544025f54`. Validators
therefore failed to resolve runtime identity. Future Pylon deploys that touch
training must update and verify both `/usr/local/bin/pylon` and the packaged
Psionic runtime.

## Rollback Conditions

Stop before LDK-07 if any of these are true:

- LDK gRPC is reachable from public internet source ranges.
- The VM has an external IP unexpectedly.
- `bitcoind` is not synced or RPC is unavailable from the LDK host.
- `ldk-server-cli get-node-info` fails.
- `ldk-server-cli get-balances` fails.
- Metrics are not available through the private path.
- `keys_seed`, `ldk_node_data.sqlite`, API key, or TLS files are missing.
- Backup fails, or restore drill cannot verify files read-only.
- The scripts or logs print raw API keys, TLS private keys, wallet seeds, or
  bearer tokens.
- The configured `NEXUS_LDK_SERVER_REF` does not match a reviewed commit or
  tag.

Rollback at this phase is straightforward: do not configure Nexus to use the
hosted remote LDK Server, leave funding/payout endpoints on their current
disabled or legacy path, and delete the temporary restore-drill resources.

## Verification Commands

Local script verification:

```bash
bash -n scripts/deploy/nexus/22-provision-ldk-topology.sh \
  scripts/deploy/nexus/23-install-ldk-server-host.sh \
  scripts/deploy/nexus/24-smoke-ldk-server-readonly.sh \
  scripts/deploy/nexus/25-backup-ldk-server-state.sh \
  scripts/deploy/nexus/26-restore-ldk-server-drill.sh \
  scripts/deploy/nexus/27-smoke-ldk-production-readiness.sh \
  scripts/deploy/nexus/28-sync-ldk-client-material.sh

scripts/deploy/nexus/test-ldk-topology-shell-guards.sh
scripts/deploy/nexus/24-smoke-ldk-server-readonly.sh
git diff --check
```

Hosted read-only smoke:

```bash
NEXUS_LDK_REMOTE_SMOKE=true \
scripts/deploy/nexus/24-smoke-ldk-server-readonly.sh
```

Manual host probes:

```bash
gcloud compute ssh "$NEXUS_LDK_VM" --tunnel-through-iap \
  --zone "$GCP_ZONE" \
  --command 'systemctl is-active ldk-server && sudo ss -ltnp | grep 3536'

gcloud compute ssh "$NEXUS_LDK_VM" --tunnel-through-iap \
  --zone "$GCP_ZONE" \
  --command 'API_KEY_HEX="$(sudo xxd -p -c 64 /var/lib/ldk-server/bitcoin/api_key)"; sudo ldk-server-cli --base-url localhost:3536 --api-key "$API_KEY_HEX" --tls-cert /var/lib/ldk-server/tls.crt get-node-info'

gcloud compute ssh "$NEXUS_LDK_VM" --tunnel-through-iap \
  --zone "$GCP_ZONE" \
  --command 'API_KEY_HEX="$(sudo xxd -p -c 64 /var/lib/ldk-server/bitcoin/api_key)"; sudo ldk-server-cli --base-url localhost:3536 --api-key "$API_KEY_HEX" --tls-cert /var/lib/ldk-server/tls.crt get-balances'

gcloud compute ssh "$NEXUS_LDK_VM" --tunnel-through-iap \
  --zone "$GCP_ZONE" \
  --command 'curl -fsSk https://localhost:3536/metrics | head'
```

Do not run two restored nodes with the same LDK identity at the same time.
Restore drills mount the disk read-only for that reason.

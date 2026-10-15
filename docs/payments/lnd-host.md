# Experimental LND host

Issue [#11752](https://github.com/OpenAgentsInc/openagents/issues/11752)
builds `oa-lnd-1` in `openagentsgemini`, zone `us-central1-a`. It uses an
`e2-standard-4`, Shielded VM controls, deletion protection, and a separate
50 GB SSD. The experimental start uses bbolt rather than Cloud SQL to keep
the initial cost down. It needs planned compaction downtime; channel recovery
uses the seed and static channel backup, never a database rollback.

The first network interface is on `default`; the second is on
`oa-lightning-us-central1` and reaches the synced `oa-bitcoind` at `10.42.0.2`.
The boot script sets loose reverse-path filtering so Debian accepts metadata
replies on either interface. Only peer port 9735 has public ingress. SSH requires IAP. The default VPC's
broad allow rules are overridden for this node: gRPC on 10009 admits only
`oa-pay-1` at `10.128.0.46`. REST binds to loopback.

## Install

Run `scripts/lnd-gcp.sh provision`, then install from a clean commit rebased
on current `origin/main`. Build `lnd-health` on Debian 12, copy it and
`deploy/lnd/install.sh` to the host, and run:

```sh
sudo bash install.sh COMMIT /path/to/lnd-health
```

The installer pins LND `v0.21.1-beta`, verifies its signed checksum manifest
against the pinned release key, and checks the archive before installing it.
The Rust binary and `/opt/openagents-lnd/COMMIT` come from the same commit.
Run `scripts/lnd-gcp.sh check` after installation.

LND generates a fresh wallet on this host. The password is wrapped with the
`openagents-lnd/oa-lnd-1-password` Cloud KMS key in `us-central1`, then stored
as `lnd-oa-lnd-1-password-kms`. The fresh recovery-word transcript goes
directly to `lnd-oa-lnd-1-seed`; the host can add seed versions but cannot
read them. No treasury secret is used. Initialization refuses an existing
password rather than creating another wallet under it.

Per-role credentials are stored as `lnd-oa-lnd-1-macaroon-{invoice,readonly,
payout,ops}`, and the certificate as `lnd-oa-lnd-1-tls-cert`. Service
credentials have IP restrictions. The admin macaroon stays in the node's
private directory. The daemon reads its password and Bitcoin RPC config
from tmpfs; the health user gets only its loopback read-only macaroon.

## Operate

```sh
sudo systemctl status openagents-lnd
sudo systemctl stop openagents-lnd
sudo systemctl start openagents-lnd
sudo journalctl -u openagents-lnd -n 30
sudo systemctl start openagents-lnd-health
sudo cat /var/lib/openagents-lnd-health/health.json
```

The timer checks every minute and reports failures through
`openagents-lnd-alert.service`. The typed policy checks sync, active
channels, pending force-closes, inbound liquidity, and the hot balance.
`/etc/openagents-lnd/health.json` starts with a 100,000-sat inbound floor and
a zero hot-wallet cap, so the unfunded node reports its missing liquidity.
The owner sets the funding budget and hot-wallet cap before use.

The data disk mounts at `/var/lib/openagents-lnd`; the node directory is
`lnd/` beneath it. For an upgrade, stop the only writer, take a diagnostic
snapshot, verify the new release, and rehearse it on regtest before starting
it. Never start from an older channel database or run a second copy of this
wallet. If current state is lost, stop the node and use the seed-plus-SCB
recovery procedure from #11753. A disk snapshot is not that procedure.

## Deployment record

On 2026-10-15, LND `v0.21.1-beta` was installed from its verified signed
release. The host uses `10.128.0.103` for the pay host, `10.42.0.192` for
Bitcoin Core, and `34.45.182.173:9735` for public peers. It reached block
971,623 with chain sync complete. Its node id is
`03bb4d4950167daf3f9693c6cdb70cbe160ebbd19afacc1ea509116b2d716c4acd`.
The seed and per-role credential secrets each have an enabled first version;
the seed was not read back. The health binary built from `43d39e8195` has
SHA-256 `6ac3dcbdfb8e1e203fbc9b20aa5572454d4e11e886b5bd0b1d3ed8e6d93b8086`.

Verification: the pay host reached private gRPC; a public probe reached 9735
and could not reach 10009. The health timer ran and its failure alert reached
the journal. Rust tests forced each of the five alert conditions. The
unfunded node reports its missing channels and inbound liquidity as expected.
A service restart fetched the wrapped password again, unlocked the wallet,
and preserved the same node identity. Binary password fetches use
`gcloud secrets versions access --out-file` to preserve ciphertext bytes.

## Channel backups and watchtowers

Run `scripts/lnd-backup-gcp.sh`, then install `lnd-backups` with
`deploy/lnd/install-backups.sh COMMIT BINARY` on `oa-lnd-1`. The worker streams
SCB changes and exports a complete snapshot every 30 seconds. A separate
uploader encrypts each snapshot with age before creating a unique object in
`gs://openagentsgemini-lnd-scb/oa-lnd-1/`. Its service account has only
`roles/storage.objectCreator` on that bucket: it cannot list, read, replace,
or delete backups. The age identity is in the `lnd-oa-lnd-1-backup-age-key`
Secret Manager secret; the host can add a version but cannot read it. Only
the public recipient stays on the host. Check both backup units and their
journals after an upgrade. A failed export or upload must alert before funding.

`oa-lnd-tower-1` is an unfunded `e2-small` in `us-central1-b`. Its private
watchtower port accepts only `oa-lnd-1`; RPC stays on loopback. Install it with
`deploy/lnd/install-tower.sh` and register the URI from `lncli tower info` on
the payment node with `lncli wtclient add URI`. Its disposable wallet owns no
payment funds. Keep its tower database when upgrading.

The second tower is the free US service published by the
[frznode operator](https://frznode.com/watchtower.html), checked on 2026-10-15.
Its public key is
`028829fcc05c0ba83d7fff053abf663448e2e1e440f345eaa9b3f0c11ac2ce44ea`.
`openagents-lnd-third-party-tower.service` connects to its onion address through
local Tor. Register that key at `127.0.0.1:19911`. The encrypted watchtower
handshake authenticates the remote key. Recheck the operator's published
address before changing it. Check `lncli wtclient towers --include_sessions`
for both registrations and acknowledged backups after channel activity.
Registration alone does not establish protection for a funded channel.

### Recovery rule and release drill

**Never restore a payment node's database from a snapshot or an old copy.**
Stop the original writer and fence its machine first. Recovery uses the
node's own aezeed plus its SCB, on a fresh data directory. An operator with
separate recovery access retrieves the age identity and the newest complete
SCB object, decrypts it on a private recovery machine, and supplies the seed
and `lncli create --multi_file=channel.backup`. Never copy `channel.db` or
`wallet.db` from an earlier point in time. Keep recovery inputs out of logs,
shell history, and issue comments. Do not fund or run the original writer
again. Wait for peers to force-close, confirm the closes, and verify sweeps
before making the recovered wallet available for spending. Offline peers can
delay recovery. Follow the pinned
[LND recovery instructions](https://github.com/lightningnetwork/lnd/blob/v0.21.1-beta/docs/recovery.md).

Repeat `scripts/lnd-sdk-regtest.sh` for every LND or SDK release. Its recovery
test generates a new aezeed, funds a private channel with regtest coins,
registers two isolated towers, verifies an acknowledged tower backup, and
captures the streamed SCB within a minute. It stops the only writer, deletes
all of that test node's databases, initializes a fresh wallet from seed plus
SCB, checks the same node identity, mines its peer's force-close, and checks
the recovered on-chain balance. It never reads a production seed or database.

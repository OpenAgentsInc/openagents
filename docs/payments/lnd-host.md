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

The second tower is the free service published by the
[Mineracks operator](https://bitcoin.mineracks.com/), checked on 2026-10-15.
Its URI is
`0281f4affcd16ad56a6a5be055d19e24a17f9932d057f61c5403fc0809ebc696aa@tower.mineracks.com:9911`.
The encrypted watchtower handshake authenticates the remote key. Recheck the
operator's published address before changing it. Check
`lncli wtclient towers --include_sessions` for both registrations and
acknowledged backups after channel activity.
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

The 2026-10-15 drill passed: 99,888,490 regtest sats were recovered on-chain.
The existing payment and settlement-restart drill also passed. On GCP,
`oa-lnd-tower-1` is at `10.42.0.193:9911`, with tower key
`03fb064f0ff4fbe972a6b82f586b812fa2332c40a56c869cd5ef048f031a8b4b42`.
Encrypted zero-channel SCBs first arrived at 21:29 UTC. Checks using the node's
service account returned HTTP 403 for both object reads and recovery-key reads.
A live channel-change timing check and funded-channel tower sessions await the
owner's funding decision; neither requires reading the production seed.

## Receiving agent credit purchases

The pay front supports `[lnd_receive]` in its routes TOML. Stage a checked
`openagents` binary with `deploy/lnd/stage-receiver.sh COMMIT BINARY` on
`oa-pay-1`. The pay host's service account needs Secret Manager accessor on
only the LND invoice macaroon, read-only macaroon, and TLS certificate secrets.
The credential unit writes a ready-to-copy stanza to
`/run/openagents-lnd-receive/receive.toml`; it starts with `backend = "mdk"`.
Staging keeps the active binary and routes. The invoice credential has
`info:read`, `invoices:read`, `invoices:write`, and `offchain:read` for explicit
private-channel hints. It cannot send payments or open channels. Reconciliation
uses the separate read-only credential.

After the owner approves funding and the canary, use the staged binary, add
that stanza to `/etc/openagents-pay/routes.toml`, and select `backend = "lnd"`.
Only the exact resource `x:openagents-agent-credit` changes issuer; plugin
calls and other hosted resources keep their existing receiver. Revert
`backend = "mdk"` and restart the front to roll back. Keep the LND stanza and
credentials: they let already-issued LND invoices settle after rollback.
Startup on MDK does not require connecting to LND. The signed invoice's payee
selects the original receiver when observing collection. Both receivers must
use the same Bitcoin network.

Pass `--routes /etc/openagents-pay/routes.toml` to `openagents pay reconcile`;
the shipped timer does this. It reads both complete payment histories and
balances, including LND's invoice amounts actually paid, and reports unknown
if either configured receiver cannot be read. Conflicting payment hashes across
nodes require review. It never sends a payment. Agent credit still passes
through the gateway's existing fsynced, payment-hash-deduplicated
`agent-credit.jsonl`; changing the issuing node does not create another credit
path. The live canary must check that journal and reconcile after a retry and
an LND restart.

The receive checks on 2026-10-15 passed the payment-front suite, 19 CLI payment
tests, five route-file/front tests, the rollback settings test, and the gateway
credit retry test. The real LND drill checked 21,001 msat collection with
`LookupInvoice`, private hints, and an open invoice settling after restart.
A baked receive-only macaroon issued invoices and observed payments; LND denied
its send request. The separate recovery drill passed again on GCP after testing
the case where an active private channel's graph entry is still arriving.

## Remote signer (#11756)

Production remains a standalone node until owner decision 5. The regtest
fixture `Regtest::remotely_signed` creates the seed only on its signer,
exports public accounts, and initializes Alice with `InitWallet.watch_only`.
The signer has no published port, peer listener, or chain backend. It uses
an internal Docker network shared only with Alice. Alice receives a
read-only mount containing the signer's public certificate and scoped RPC
credential, never its wallet volume. `tests/remote_signer.rs` checks channel
opening, both payment directions, all account watch-only flags, and failure
to issue an invoice after the signer stops.

For the owner-approved deployment, use a separate GCP project and an
`e2-small` Shielded VM on a dedicated subnet, without an external address.
Connect only the watch-only node through private VPC peering or a dedicated
VPN; export neither a default route nor the rest of the application subnet.
Apply an ingress deny at priority 900, then an allow at priority 800 for
TCP 10019 from the watch-only node's private `/32`. Keep REST on loopback.
Allow SSH through IAP only during an operator maintenance window, then
remove that rule. Deny other ingress and Internet egress; allow only the
private Google API path needed for password unlock and audit delivery.
Install pinned, verified packages through an operator maintenance path.

Use a dedicated signer service account. Give it accessor on its wrapped
password secret and decrypt on its password KMS key, both in the signer
project; give it no application-project or seed-secret read access. The
watch-only service account gets no signer-project IAM role. Its sole
authority is a signer-issued macaroon with `message:write signer:generate
address:read onchain:write`, bound to its private IP. Keep the signing
project's operator group separate from application deployers. The signer
may append a fresh seed version during an explicit ceremony; only the
offline recovery operator can read it. Never log seed words or passwords.

Start from [`signer-lnd.conf`](../../deploy/lnd/signer-lnd.conf) and
[`watch-only.conf`](../../deploy/lnd/watch-only.conf). Fetch the wrapped
password into tmpfs, decrypt with Cloud KMS, and pass its file to LND.
KMS wraps the password; LND performs BIP32 derivation and Lightning signing.
After the owner places the existing node's seed on the isolated signer,
check its public identity, save current SCBs, stop the only channel-database
writer, and enable the documented watch-only migration once. Remove the
migration flag after success. Check every wallet account is watch-only,
retest payments and backups, and remove old seed-bearing disk snapshots
and retired disks through the owner's retention procedure. Purging the
live wallet does not erase historical backups or SSD blocks.

The remote signer protects wallet key storage; its credential still
authorizes signing. A compromised watch-only process can request signatures.
Keep payout limits and monitoring in force. TLS and transport credentials
remain on the watch-only host. Follow the pinned upstream
[remote-signing instructions](https://github.com/lightningnetwork/lnd/blob/v0.21.1-beta/docs/remote-signing.md).

## Payout rollout (#11757)

Production remains on MDK. Before activating LND, complete owner decision 3
and the custody review in #11761. The worker uses the same ledger for
plugin author, affiliate, and Pylon shares. Spark-address payments still use
the payout Spark wallet; only its refill invoice changes sender.

1. Stage the checked CLI with `deploy/lnd/stage-payouts.sh COMMIT BINARY SHA256`.
   It creates settings only when absent and preserves the running worker.
   Keep `/etc/openagents-pay/payout-policy.json` outside release directories.
   The template selects `mdk` with no LND spending approval or account limit.
2. Install the pinned litd image in `deploy/lnd/litd.compose.yaml` on the
   LND host. Set `rpcmiddleware.enable=true` in LND and restart it during
   the approved window. Bind litd to loopback, disable its UI and unrelated
   services, and mount only its LND certificate and operator macaroon.
   Follow [litd's account documentation](https://github.com/lightninglabs/lightning-terminal/blob/v0.17.0-alpha/docs/accounts.md).
3. Choose a total LND allowance and expiration. Run `lnd-account` with an
   approved copy of `deploy/lnd/account.example.json`. It saves an intent
   before creation, writes the credential privately, and refuses repeated
   creation. An uncertain creation needs operator recovery, never a new
   account or balance reset. Put only the returned account credential in
   `lnd-oa-lnd-1-payout-account`; grant the pay-host service account accessor
   only on that secret. Deliver it as a mode-0600 tmpfs file at the configured
   `account_macaroon` path before enabling LND sends. Never give the payout
   process litd's operator credential or LND's unrestricted payout macaroon.
4. Set persisted `mode` to `shadow` and activate the candidate with
   `deploy/lnd/activate-payouts.sh COMMIT /etc/openagents-pay/lnd-payout.json`.
   Have an approved test payout queued. Shadow sends on MDK and records LND
   fee estimates for Lightning payouts and Spark refill invoices. Estimates
   time out after 12 seconds; estimate or observation failures do not change
   the sender. Observe one week and compare estimates with actual MDK fees.
5. After approval, put only our test payee IDs in `canary_parties`, enter the
   real account ID and allowance, set `mainnet_approved`, then select `canary`.
   Expand to one cohort only after a successful real-wallet test. An LND
   reference stores `lnd:node:hash` before dispatch. Legacy hashes mean MDK.
   The adapter checks the available allowance before dispatch. litd can mask
   an over-budget streaming request with a request-state error; concurrent
   allowance changes still require original-hash recovery.
   Recover every unresolved row directly on its original backend. An LND
   refill also keeps its original hash across restart and rollback.
6. Roll back by changing only policy `mode` to `mdk`. Leave the LND settings,
   account credential, and original node available for recovery. Deploys do
   not rewrite this file or recreate the litd account. A candidate stays
   active only after `payouts --check --require-sent-after UNIX` confirms a
   completed Lightning payout directly on its node. A failed gate restores
   the previous worker; it does not alter spending policy.

Use the existing reconciliation timer with `--routes` retaining both receive
nodes and `--resolve`. LND outbound records carry the original node in their
reference. Check node payment status, then ledger rows, then reconciliation,
then website stats. Unknown payouts over an hour are reported and fail the
rollout check. Reconciliation must remain clean daily for two weeks, with
zero duplicate sends or overdue unknown rows. Save a real-wallet rollback
and reversal test. Record liquidity fees as operating expenses separately
from payee shares; unexpected outbound payments still require investigation.

After those checks and approval, `[lnd_receive] all_resources = true` with
`backend = "lnd"` selects LND for plugin and hosted x402 receipts. Keep
`all_resources` and both credentials configured during rollback; change only
`backend` to `mdk` so old invoices still settle. Pro retains its separate MDK
compatibility hold. Keep the LDK receiver warm for a month; retiring it is an
owner action. No live payout or receipt switch was made for #11757.

## Liquidity automation (#11758)

`lnd-autoloop` reads the versioned [channel policy](../../deploy/lnd/channel-policy.toml)
every five minutes. Its checked-in policy disables spending. The current node
only reports balances, capacity, channel sizing, the proposed fee schedule, and
idle-close candidates. A channel becomes idle only after a full observation
period with unchanged sent and received totals; pending HTLCs prevent a close
recommendation. Open, fee update, and cooperative close remain operator actions.
Both ZEUS and Megalith LSPS1 clients are available through `lnd-liquidity`;
select the second provider in a separate reviewed budget when the first cannot
serve a request. Never repeat an uncertain purchase with the other provider.

Before enabling Loop, the owner must approve decision 4: the hot cap, a cold
Bitcoin address on the correct network, daily capital and fee budget, swap size,
and fee limits. Keep the confirmed channel reserve and payout outbound floor.
Run a pinned Loop **v0.34.x** daemon against this node, with its RPC accessible
only locally. Give this service its Loop-specific macaroon and certificate pin
in `autoloop.json`'s `loop` object (the same fields as `lnd`). Do not give it the
LND operator macaroon. Disable Loop's independent autolooper so only this policy
initiates scheduled swaps. Keep Loop's database through upgrades and rollback.
Verify Loop's backing LND configuration during deployment: its GetInfo RPC
reports the network but does not report the LND identity. A changed node or Loop
certificate requires explicit review of the saved journal binding.

With approval, low inbound or excess hot value requests Loop Out to the cold
address; low payout capacity requests wallet-funded Loop In. Insufficient
outbound or confirmed reserves produces an alert. Each action quotes fees first,
saves its full policy and unique label before dispatch, and sends once. On every
run, it lists daemon swaps, saves actual server, on-chain, and routing costs,
and matches each intent by label, amount, direction, and swap ID. Missing,
ambiguous, or unfinished swaps block new work. The daily budget reserves capital
and configured fees even for failed or uncertain attempts; policy edits and
restarts do not clear it. Cooldowns prevent immediate reversal.

Loop can exceed its initial miner-fee setting after revealing the preimage to
recover an in-flight swap. The daily reservation is an admission budget, not an
absolute limit on the daemon's later recovery fees. Review actual fees and cold
wallet receipts against `latest.json` and `journal.json` every day. Investigate
any unmanaged swap or unexplained Bitcoin transaction; retain Loop records and
both transaction IDs with the report. Never delete a pending intent to retry it.
See the [pinned upstream RPC contract](../../crates/lnd-sdk/proto/loop/v0.34.0-beta/looprpc/client.proto).

Install `openagents-lnd-autoloop.service` and its timer with the checked binary
and existing read-only credential. An unset policy returns exit 1 and uses the
existing alert hook. After approval, test a small Loop Out and Loop In, restart
between submission and observation, reconcile each swap and its fees, and then
observe 30 days without manual rebalancing. No funded Loop swap or observation
period was performed during implementation.

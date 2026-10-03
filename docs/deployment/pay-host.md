# Pay host: the central receiver

The one Lightning node that receives every paid call
([central receive and splits](../payments/2026-10-02-central-receive-and-splits.md),
decisions P1 and P2; issue
[#10185](https://github.com/OpenAgentsInc/openagents/issues/10185)). It is
`openagents wallet serve` from [`crates/wallet`](../../crates/wallet) on
bitcoin mainnet with `--lsp mdk`: MoneyDevKit's `ldk-node` fork and
MoneyDevKit's LSPS4 just-in-time liquidity, so the first payment opens the
channel and nothing is funded first. Its node id is the x402 `payTo`.

It is not the production MDK treasury (`MdkTreasuryContainer`). That
container, its mnemonic, and its secret file are out of this host's path:
nothing here reads them, and no node here ever runs on that mnemonic.

## The host

| Thing | Value |
| --- | --- |
| Instance | `oa-pay-1`, `us-central1-a`, project `openagentsgemini`, `e2-small`, Debian 12, Shielded VM, deletion protection on |
| Network | `default` VPC, no external address; outbound through the region's Cloud NAT; SSH through IAP only (firewall `oa-pay-host-iap-ssh`, tag `oa-pay-host`) |
| Service account | `oa-pay-host@openagentsgemini.iam.gserviceaccount.com`: read and add versions on `openagents-pay-wallet-seed`, add versions on `openagents-pay-backup-age-key`, create objects in the backup bucket. No project roles. |
| Data disk | `oa-pay-data`, 10 GB `pd-balanced`, mounted at `/var/lib/openagents-pay` by label, kept when the instance is deleted |
| Seed | Secret Manager `openagents-pay-wallet-seed`, generated on the host |
| Backups | `gs://openagentsgemini-pay-backups/oa-pay-1/`, age-encrypted, hourly, deleted after 30 days; the host can create objects but not read or delete them |
| Backup key | Secret Manager `openagents-pay-backup-age-key` (the age identity); only the recipient is on the host |
| Cost | About $14.50 a month: `e2-small` $12.23, two 10 GB `pd-balanced` disks $2.00, Secret Manager and storage a few cents. The LSP's fee is per payment (about 2% of a just-in-time open), not a host cost. |

## Layout

```text
/opt/openagents-pay/releases/<commit>/openagents   immutable release
/opt/openagents-pay/releases/<commit>/pay-host     the flow and stats server
/opt/openagents-pay/current -> releases/<commit>
/etc/openagents-pay/openagents-pay.env             deploy/openagents-pay.env.example, filled in
/usr/local/sbin/openagents-pay-seed                deploy/pay/
/usr/local/sbin/openagents-pay-health              deploy/pay/
/usr/local/sbin/openagents-pay-backup              deploy/backup/
/usr/local/sbin/openagents-pay-restore             deploy/backup/
/var/lib/openagents-pay/                           the data disk (StateDirectory)
  wallet/          OPENAGENTS_WALLET_HOME: config.json, ldk/ (channel store),
                   control.sock, seed -> /run/openagents-pay/seed
  x402/replay/     OPENAGENTS_X402_HOME: the one replay store for this payTo
  ledger/          crates/pay-ledger (#10187): ledger.sqlite
  spark/           the payout Spark wallet's store; seed -> /run/openagents-pay-payouts/spark-seed
  flow/            pay-host's projection (flow.sqlite), rebuilt from the ledger
  reconcile/       latest.json/.txt, daily/DATE.json/.txt, spark-scratch/
  backups/         encrypted copies, newest 48; health.json
/run/openagents-pay/seed                           tmpfs, 0400, written at each start
/run/openagents-pay-payouts/spark-seed             the same for the payout Spark wallet
/etc/openagents-pay/flow-salt.env                  OPENAGENTS_PAY_FLOW_SALT, root 0400
```

The units are `deploy/systemd/openagents-pay.service` (the node),
`deploy/systemd/openagents-pay-payouts.service` (the payout worker),
`deploy/systemd/openagents-pay-flow.service` (`pay-host`, the flow and stats),
`deploy/backup/openagents-pay-backup.{service,timer}` (hourly),
`deploy/pay/openagents-pay-health.{service,timer}` (every five minutes), and
`deploy/pay/openagents-pay-reconcile.{service,timer}` (every ten minutes).

### The pay front (#10186, #10193, #10194)

`deploy/systemd/openagents-pay-front.service` runs `openagents pay serve
--routes /etc/openagents-pay/routes.toml` as the same `openagents-pay` user
with the same environment file (installed from
`deploy/pay/openagents-pay-routes.toml`). It reaches the node through the
resident's `control.sock`, keeps its one replay store, challenge key, plugin
cache, and hosted registry under `OPENAGENTS_X402_HOME`
(`/var/lib/openagents-pay/x402`, which the backup copies), and writes every
settlement to the ledger (`ledger = "/var/lib/openagents-pay/ledger/ledger.sqlite"`
makes `crates/pay-ledger` the sink). Its routes:

| Route | What it sells |
| --- | --- |
| `POST /v1/plugins/{id}/invoke` | One run of a published plugin's newest signed release (resolved on `wss://relay.openagents.com`): 5 sats plus the release's `fee_msat`, both named in the `402` (#10193) |
| `GET`/`POST /x/{resource}` | Author-hosted resources registered with `openagents x402 publish` (#10194) |
| `POST /v1/resources`, `GET /v1/paid-key` | A hosted registration (NIP-98 signed) and the key the `OpenAgents-Paid` header is signed with |

The ledger's v1 split rule took effect 2026-10-02 (a settlement dated before
it is refused, so its call gets a `503` and nothing runs). The unit leaves
`MemoryDenyWriteExecute` off: plugin guests run in wasmtime, which maps code
writable and then executable.

It listens on `0.0.0.0:8402` for the `api.openagents.com` load balancer
(`one-production-url-map`, path matcher `api`). Its route rules send
`/v1/plugins/{id=*}/invoke` (a path template), `/x/*`, `/v1/resources`, and
`/v1/paid-key` to backend service `oa-pay-front-backend`
(`EXTERNAL_MANAGED`, HTTP, timeout 120 s), whose zonal NEG
`oa-pay-front-neg` (`GCE_VM_IP_PORT`, `us-central1-a`) holds `oa-pay-1:8402`
and whose health check `oa-pay-front-hc` asks `GET /v1/paid-key`. `/v1/sessions`
and `/v1/sessions/*` still go to the voice backend, everything else to the
API service; the map's tests cover each. Firewall `oa-pay-front-from-lb`
admits only the load balancer's ranges (`35.191.0.0/16`, `130.211.0.0/22`)
on 8402 to tag `oa-pay-host`. The instance still has no external address.

```sh
sudo systemctl status openagents-pay-front
sudo journalctl -u openagents-pay-front -f      # one JSON line per request
curl -s -i -X POST --data-binary @err.txt https://api.openagents.com/v1/plugins/explain-error/invoke   # 402
gcloud compute backend-services get-health oa-pay-front-backend --global --project openagentsgemini
```

To change the routing, export the map, edit it, and check it before import:
`gcloud compute url-maps export one-production-url-map --global --destination map.yaml`,
then `gcloud compute url-maps validate --source map.yaml --global
--load-balancing-scheme EXTERNAL_MANAGED` (its `tests` must pass), then
`gcloud compute url-maps import one-production-url-map --source map.yaml --global`.
Removing the pay front from `api.openagents.com` is dropping route rules 2
and 3.

### The payout worker (#10190)

`deploy/systemd/openagents-pay-payouts.service` runs `openagents pay
payouts` as the same user against the same ledger
(`/var/lib/openagents-pay/ledger/ledger.sqlite`). Every 60 seconds it pays
each payee whose accrued shares reached the threshold (100 sats to a Spark
address, 1,000 sats to a Lightning address, or anything over 1 sat once its
oldest share is a day old), as the
[design](../payments/2026-10-02-central-receive-and-splits.md#how-payouts-go-out)
describes:

- Lightning addresses are paid from the receiver wallet through the
  resident's `control.sock` (fee cap 1%, at least 5 sats).
- Spark addresses are paid from the payout Spark wallet in
  `/var/lib/openagents-pay/spark` (Breez SDK, mainnet). Its seed is Secret
  Manager `openagents-pay-spark-seed` (hex entropy, never on the data disk):
  `openagents-pay-seed fetch spark` writes it to
  `/run/openagents-pay-payouts/spark-seed` before each start and the Spark
  home's `seed` links there. The wallet refills itself, at least 1,000 sats
  at a time, by having the receiver wallet pay its invoice.
- Stopping or killing the unit is safe at any moment: the payment hash or
  Spark transfer id is written before the send, and a restart settles an
  interrupted payout only by looking it up, never by sending again.

```sh
pay pay payout-list --ledger /var/lib/openagents-pay/ledger/ledger.sqlite --open
sudo journalctl -u openagents-pay-payouts -f     # one JSON line per payout step
```

An `unknown` payout whose wallet has no record of its reference keeps its
shares reserved (a crash between dispatch and the wallet's own record looks
the same as a send that never started). Check it with `pay x402 node lookup
HASH` (Lightning) or the Spark wallet's payments before doing anything by
hand.

The Spark seed (once):

```sh
sudo -u openagents-pay env HOME=/var/lib/openagents-pay \
  /opt/openagents-pay/current/openagents --json pay payout-spark-init \
  --spark-home /var/lib/openagents-pay/spark      # prints the Spark address only
sudo /usr/local/sbin/openagents-pay-seed store spark
```

### Flow and stats for the website (#10195)

`deploy/systemd/openagents-pay-flow.service` runs the `pay-host` binary
(`crates/pay-host`): it projects the ledger, read-only, into
`/var/lib/openagents-pay/flow/flow.sqlite` and serves `GET /flow/snapshot`,
`GET /flow/stream` (SSE), and `GET /stats` on `0.0.0.0:4400`. Nothing else
is served. `OPENAGENTS_PAY_FLOW_SALT` is in `/etc/openagents-pay/flow-salt.env`
(root, 0400), made once on the host:

```sh
sudo sh -c 'umask 077; printf "OPENAGENTS_PAY_FLOW_SALT=%s\n" "$(openssl rand -hex 32)" \
  > /etc/openagents-pay/flow-salt.env'
```

The host has no public address. The website (Cloud Run `coder`) reaches it
over the VPC: the service has Direct VPC egress on the `default` subnet with
`private-ranges-only`, so only RFC 1918 traffic takes the VPC, and the `web`
container has `OPENAGENTS_WEB_PAY_HOST=http://10.128.0.46:4400`. The
address is reserved as `oa-pay-1-internal`. The VPC's
`default-allow-internal` rule (10.128.0.0/9) already admits it; port 4400 is
reachable from nowhere outside the VPC. A registry plugin (`<publisher>:<slug>`) is named
by its slug, and its author by npub, once its signed listing is a line of
`/var/lib/openagents-pay/flow/publications.ndjson`
(`OPENAGENTS_PAY_PUBLICATIONS`; `scripts/payments-demo.sh --publish
--operator` appends one); otherwise both are salted aliases.
`openagents.com/api/flow/*` and
`/api/stats` proxy there (`crates/openagents-web`), so `/live` and `/stats`
read it same-origin.

### Reconciliation (#10191)

`deploy/pay/openagents-pay-reconcile.timer` runs `openagents pay reconcile
--resolve` every ten minutes (`crates/pay-ledger/src/reconcile.rs`, the
[design](../payments/2026-10-02-central-receive-and-splits.md#reconciliation)).
It checks the ledger against the receiver wallet (through the node's
`control.sock`; the node lists every Lightning payment) and the payout Spark
wallet (read from a copy of its store in `reconcile/spark-scratch`, so the
payout worker's store is never written):

| Finding | Severity |
| --- | --- |
| A Lightning settlement with no succeeded inbound payment, or another amount (invariant 6) | drift |
| A `sent` payout with no succeeded outbound record on its rail, or another amount | drift |
| A `failed` payout (shares returned) that the wallet records as sent | drift |
| An outbound payment no payout explains (the receiver's payments into the Spark wallet are its top-ups) | drift |
| A payout `unknown` for more than 10 minutes (less: a notice) | drift |
| Holdings (receiver Lightning plus Spark) below what the ledger owes (accrued plus reserved shares, OpenAgents' own included) | drift |
| An inbound payment the ledger never settled (paid, never redeemed) | notice |
| A wallet that could not be read | notice; the state is `unknown` |

`--resolve` settles an `unknown` payout only when its wallet record proves
the outcome: succeeded with the amount that went out is `sent`, failed is
`failed` (its shares return). It never sends, refunds, or retries anything;
every other finding is for a person.

Each run writes `reconcile/latest.json` and `latest.txt`, and the day's
report `reconcile/daily/YYYY-MM-DD.json` and `.txt` (the day's last run,
with its run count, drift runs, and the drift kinds seen). A drift logs an
error-priority line, `{"event":"reconciliation_drift",...}`, in the unit's
journal until a run clears it (`reconciliation_cleared`). `pay-host` reads
`latest.json` (`OPENAGENTS_PAY_RECONCILIATION`) every 10 seconds, so
`/stats` shows `reconciliation: ok`, `drift`, or `unknown` (no report, or
one older than 30 minutes). The Spark side is read with the payout worker's
seed copy, so it is `unknown` while `openagents-pay-payouts` is stopped.

```sh
sudo journalctl -u openagents-pay-reconcile -p err         # drift alerts
sudo cat /var/lib/openagents-pay/reconcile/latest.txt
sudo -u openagents-pay sh -c 'set -a; . /etc/openagents-pay/openagents-pay.env; exec \
  /opt/openagents-pay/current/openagents --json pay reconcile \
  --ledger /var/lib/openagents-pay/ledger/ledger.sqlite'   # on demand; resolves nothing
```

## Install from a checkout

The binary is built on the host (or a builder with the same Debian release),
from the commit being installed, with the pinned toolchain:

```sh
cargo build --locked --release -p openagents-cli --bin openagents
```

The first install built on the same instance at `e2-standard-8` with a
temporary 60 GB scratch disk, then the instance was stopped, resized to
`e2-small`, and the scratch disk deleted. Repeat that for a rebuild, or build
on any Debian 12 x86_64 builder and copy the binary in.

```sh
sudo useradd --system --home /var/lib/openagents-pay --shell /usr/sbin/nologin openagents-pay
sudo apt-get install -y age jq curl
V=$(git rev-parse --short=12 HEAD)
sudo install -d -m 0755 /opt/openagents-pay/releases/$V
sudo install -m 0755 target/release/openagents /opt/openagents-pay/releases/$V/openagents
sudo ln -sfn /opt/openagents-pay/releases/$V /opt/openagents-pay/current

sudo install -m 0755 deploy/pay/openagents-pay-seed deploy/pay/openagents-pay-health \
  deploy/backup/openagents-pay-backup deploy/backup/openagents-pay-restore /usr/local/sbin/
sudo install -m 0644 deploy/systemd/openagents-pay.service \
  deploy/backup/openagents-pay-backup.service deploy/backup/openagents-pay-backup.timer \
  deploy/pay/openagents-pay-health.service deploy/pay/openagents-pay-health.timer \
  deploy/pay/openagents-pay-reconcile.service deploy/pay/openagents-pay-reconcile.timer \
  /etc/systemd/system/
sudo install -d -o root -g openagents-pay -m 0750 /etc/openagents-pay
sudo install -o root -g openagents-pay -m 0640 deploy/openagents-pay.env.example \
  /etc/openagents-pay/openagents-pay.env
sudoedit /etc/openagents-pay/openagents-pay.env      # fill every <PLACEHOLDER>
sudo chown openagents-pay:openagents-pay /var/lib/openagents-pay
sudo systemd-analyze verify /etc/systemd/system/openagents-pay*.service
```

### The backup key (once)

The identity goes straight from `age-keygen` into Secret Manager; only the
recipient (`age1…`, public) stays, in the environment file.

```sh
sudo sh -c 'umask 077; age-keygen -o /root/pay-backup.key 2>/dev/null'
sudo gcloud secrets versions add openagents-pay-backup-age-key \
  --data-file=/root/pay-backup.key --project openagentsgemini
sudo age-keygen -y /root/pay-backup.key          # the recipient, for the env file
sudo shred -u /root/pay-backup.key
```

### The seed (once)

`init` generates a fresh BIP39 seed on the host and prints only the node's
info, never the mnemonic. `openagents-pay-seed store` adds it as the
secret's first version, reads it back and compares digests, shreds the file,
and leaves the symlink the unit expects. It refuses if the secret already
has a version.

```sh
sudo -u openagents-pay env OPENAGENTS_WALLET_HOME=/var/lib/openagents-pay/wallet \
  HOME=/var/lib/openagents-pay \
  /opt/openagents-pay/current/openagents --json x402 node init --network bitcoin --lsp mdk
sudo /usr/local/sbin/openagents-pay-seed store
```

MoneyDevKit publishes no minimum for LSPS4 forwards (the LSPS4
registration carries no fee parameters, and MDK's checkout accepts 1 sat),
so `--lsp-min-msat` is not set. Set it with `x402 node init --lsp-min-msat N`
(init keeps the seed) if the first receives show a floor.

## Start, stop, logs

```sh
sudo systemctl enable --now openagents-pay openagents-pay-backup.timer openagents-pay-health.timer
sudo systemctl status openagents-pay
sudo systemctl restart openagents-pay           # the seed is fetched again before each start
sudo systemctl stop openagents-pay
sudo journalctl -u openagents-pay -f            # node events as JSON lines
sudo systemctl status openagents-pay-payouts openagents-pay-flow
sudo journalctl -u openagents-pay-backup -n 20
```

Run any node command as the service user with the environment file; it
acts through the resident. Since `29f3ac33d6` the node's commands are
`openagents x402 node …` (`openagents wallet` is the person's Spark wallet;
the unit's `wallet serve` still starts the node under its old name):

```sh
pay() { sudo -u openagents-pay env $(sudo grep -v '^#' /etc/openagents-pay/openagents-pay.env | xargs) \
  /opt/openagents-pay/current/openagents --json "$@"; }
pay x402 node info           # node id (payTo), balances, channels, last backup
pay x402 node channel list
pay x402 node lookup PAYMENT_HASH
```

## Health

`openagents-pay-health` prints one JSON line and exits 1 when unhealthy: the
resident must answer on `control.sock`, the network must be bitcoin, and the
newest encrypted backup must be younger than
`OPENAGENTS_PAY_BACKUP_MAX_AGE_SECS` (two hours). The timer runs it every
five minutes; the last result is in `/var/lib/openagents-pay/health.json`,
and a failure shows in `systemctl --failed`.

```sh
sudo systemctl start openagents-pay-health && sudo cat /var/lib/openagents-pay/health.json
```

## Backups

Every hour `openagents-pay-backup` takes `openagents x402 node backup` (seed,
`config.json`, a `VACUUM INTO` snapshot of the channel store, a digest
manifest), the replay store, and the ledger, streams the tar into `age`
for the recipient in the environment file, keeps the newest 48 on the data
disk, and uploads each to `gs://openagentsgemini-pay-backups/oa-pay-1/`.
The host cannot decrypt, list, or delete them.

```sh
sudo systemctl start openagents-pay-backup
sudo ls -l /var/lib/openagents-pay/backups/
```

## Restore

Two rules come before any command:

- **Never run an older channel store while the live one exists.** The
  counterparty holds the only other copy of each channel's state; a node
  that runs from an older copy can broadcast a revoked commitment and lose
  the channel's balance as a penalty. If the data disk survived, there is
  nothing to restore: attach it to a new instance and start the unit.
- **Never reuse an older replay store.** It forgets the proofs settled after
  the backup, and a forgotten proof can be presented again. If the live
  replay store is lost, serve paid routes again only after the longest
  invoice expiry plus the paid-retry grace has passed since the backup was
  taken: x402 refuses a proof for an expired invoice
  (`invalid_exact_lnbtc_invoice_expired`), so by then no forgotten proof can
  settle.

With the data disk lost: bring up a new instance and install as above
(not `init`, not `seed store`), bring the newest archive and the identity
from Secret Manager to the host, restore, and remove the identity.

```sh
gcloud storage cp gs://openagentsgemini-pay-backups/oa-pay-1/NEWEST.tar.age .    # as an operator
sudo sh -c 'umask 077; gcloud secrets versions access latest \
  --secret=openagents-pay-backup-age-key --project openagentsgemini > /root/pay-backup.key'
sudo /usr/local/sbin/openagents-pay-restore NEWEST.tar.age /root/pay-backup.key
sudo shred -u /root/pay-backup.key
sudo systemctl start openagents-pay
```

The host's own service account cannot read the identity or the bucket;
these two reads need an operator account. `openagents-pay-restore` refuses a
non-empty wallet home, checks every digest through `x402 node restore`, checks
the restored seed equals the Secret Manager seed, and keeps any live replay
store and ledger; `--with-replay` restores them when none exist, under the
replay rule above. If no archive is newer than the last channel update,
restore from the seed alone (`x402 node init --mnemonic -` from the secret) and
have the LSP force-close, as [Backup and restore](../cli/README.md#backup-and-restore)
describes.

## First receive (the LSPS4 check)

The first payment to a node with no channel makes the LSP open one; it holds
the payment about 45 seconds meanwhile, and on Mutinynet the open took about
60 seconds and the payment failed back. The mainnet check:

```sh
pay x402 node invoice --msat 2000000 --request-hash $(openssl rand -hex 32)
```

Pay the `bolt11` from any mainnet wallet, then watch `journalctl -u
openagents-pay -f` for `channel_pending`, `channel_ready`, and the payment,
and `pay x402 node lookup PAYMENT_HASH`. Either it settles (the open fit in the
hold), or the payer sees a failure and the channel is open anyway, and a
second invoice is an ordinary one that settles at once. Record which, with
the times, below.

## Record

| Date | Event |
| --- | --- |
| 2026-10-02 | `oa-pay-1` provisioned and built from `869fbe155c` (`openagents 1.0.0-rc.2`). Node `0343a0f10d0856187ad55e8e64427b8902479ef510d9f5587ba6f124280db32e27` (the `payTo`) initialised on bitcoin with `--lsp mdk` (LSPS4 peer `02a63339…473b`); seed generated on the host and stored in Secret Manager. Synced to block 969,621; first encrypted backup uploaded and decrypted back as a check; the unit came back by itself after the stop and resize to `e2-small`; health `healthy`. |
| 2026-10-02 | First-receive invoice issued: 2,000 sats, payment hash `55421ae0230a630201fce522fc6f63e46d1341e93f4032547d8f979249749cae`, expiry 7 days, with the LSPS4 route hint. Paying it is an owner step (workspace `NEEDS_OWNER.md`); record here whether it settled inside MDK's 45 s hold or failed back while the channel opened. |
| 2026-10-03 | Payout worker and flow server deployed (#10190). Release `29f3ac33d6` (built in `rust:1.97.1-bookworm` on a Boat sandbox; `openagents` sha256 `f4608d20…51ed9bf`, `pay-host` sha256 `05cbda9d…a11b01`) is `current`; the node restarted on it with the same node id. Ledger created empty at `ledger/ledger.sqlite`. Payout Spark wallet made on the host, seed stored in `openagents-pay-spark-seed` (versions readable and addable by the host's account only); its Spark address is `spark1pgss8je5hl8eprmtgml379sdxewt82pnsvazh7jf8xlu7c5gkkcljp3j2lzey0`, balance 0. `openagents-pay-payouts` and `openagents-pay-flow` enabled and running. Internal address `10.128.0.46` reserved as `oa-pay-1-internal`. Cloud Run `coder` revision `coder-web-3a46b3c415-pay` (Direct VPC egress, `private-ranges-only`, `OPENAGENTS_WEB_PAY_HOST=http://10.128.0.46:4400`) took 100% of traffic; `coder-web-3a46b3c415` is the rollback. `openagents.com/stats` shows "No payments yet" and `/api/flow/stream` holds open. No payout has been sent: the receiver has no channel or balance yet, so the first real payout is the owner's end-to-end step (#10199). |
| 2026-10-03 | `openagents-pay-health` and `openagents-pay-backup` had failed since `29f3ac33d6` (they called `wallet info` and `wallet backup`, which became `x402 node info` and `x402 node backup`); the last good backup was from before that release. Both scripts and `openagents-pay-restore` now call `x402 node`; health came back `healthy` and a backup uploaded at once (#10199). |
| 2026-10-03 | Pay front deployed (#10199): `openagents-pay-front.service` on release `ee6daf9950` with `deploy/pay/openagents-pay-routes.toml`; `api.openagents.com` routes the invoke, hosted, registration, and paid-key paths to it (NEG `oa-pay-front-neg`, backend `oa-pay-front-backend`, health check `oa-pay-front-hc`, firewall `oa-pay-front-from-lb`). The relay was updated to accept `release.fee_msat` (revision `openagents-nostr-relay-00041-fed`, image `44edd848ed`). The pay host published the check plugin `explain-error-check` (fee 10 sats, payout the payout Spark wallet) and a keyless `POST https://api.openagents.com/v1/plugins/explain-error-check/invoke` answered `402` for 15 sats (endpoint 5 + author fee 10) with a mainnet invoice from the node; the challenged calls show on `/api/flow/snapshot` and `/stats`. No paid call yet: the node has no channel until the first-receive invoice is paid. Run record: [the demo](../payments/2026-10-03-end-to-end-demo.md). |

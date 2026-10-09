# Account storage: durable stores for the web stack (#11127)

The 1.0 web stack (`deploy/staging/`, one Cloud Run service with the
`web`, `gateway` and `worker` containers) keeps its accounts in files.
Until 2026-10-09 those files sat on an in-memory volume, so every new
revision or instance started empty: people were signed out and lost their
keys. They now live on an NFS disk that outlives the instance. This page
is the decision, how it works, backups and restore, and the production
steps.

## What is kept

| Where on the share | Written by | What |
| --- | --- | --- |
| `stack/gateway/registry/accounts.json`, `accounts-history/` | gateway (`tenancy::Accounts`) | Accounts, workspaces, memberships, GitHub identities |
| `stack/gateway/registry/sessions.json`, `sessions-history/` | gateway (`tenancy::sessions`) | Sign-in sessions (web, device, app) |
| `stack/gateway/registry/keys.json` | gateway (`tenancy::keys`) | `oak_` API keys (SHA-256 digests only) |
| `stack/gateway/registry/github-access/` | gateway (`oa_auth::repos`) | GitHub user tokens, sealed with AES-256-GCM under the OAuth file's `token_encryption_key` |
| `stack/gateway/registry/inference-provider-keys.json` | gateway (`inference_byok`) | Workspaces' own OpenRouter and Vercel keys, sealed under the BYOK keyring |
| `stack/gateway/registry/registry.json`, `quota-ledger.jsonl`, `receipts.jsonl`, `revisions.jsonl`; `stack/gateway/attempts/` | gateway | Tenant registry, free-tier quota, receipts, attempt journal |
| `stack/service.key` | gateway's first start (`bootstrap_registry`) | The `house` service key the chat worker calls inference with |
| `stack/worker/usage/` | chat worker | One line per job |
| `stack/handoff/` | `gateway.sh` | Which instance's gateway holds the store (below) |
| `web/byo/` | web (`cloud::custody`) | Saved own-Claude credentials, sealed under `OPENAGENTS_WEB_CLOUD_BYO_KEYS` |

Every key that opens what is sealed here stays in Secret Manager and is
written only to the container's own memory (`/tmp/private`), never to the
share: the GitHub OAuth file (with `token_encryption_key`), the BYOK
keyring (`openagents-gateway-<env>-byok-keyring`), the stored-response
key (`openagents-gateway-<env>-store-key`, `INFERENCE_STORE_KEY`) and the
web's custody keyring. A copy of the disk or a snapshot holds ciphertext,
digests, and one plaintext credential: `service.key`, the internal `house`
key, which can call inference as the house tenant and nothing else.

Not kept: the web's Pro plan meter (`--plan-meter /tmp/plan-meter.sqlite`)
stays in the instance. It is SQLite, and SQLite needs file locks that a
Cloud Run NFS mount does not have; it does not belong on this share.

## What the stores need from a filesystem

Read from `crates/tenancy` (`accounts.rs`, `sessions.rs`, `keys.rs`,
`quota.rs`, `registry.rs`, `private_fs.rs`), `crates/oa-auth/src/repos.rs`,
`crates/gateway/src/inference_byok.rs` and
`crates/openagents-web/src/cloud/custody.rs`:

- **Atomic replace.** Every store writes a temporary file and `rename`s it
  over the old one; readers must see the old or the new file, never half.
- **Exclusive create as a lock.** `accounts.lock`, `sessions.lock`,
  `quota-ledger.lock` and the GitHub-access locks are `O_CREAT|O_EXCL`
  files: the filesystem must refuse a second create.
- **fsync** of files and directories, and stable inode numbers: a writer
  checks that its lock file and directory are still the same inodes.
- **Owner and mode checks.** Directories and private files must belong to
  the process's uid and be mode 0700/0600.
- **One writer process.** The quota ledger holds its lock for the
  gateway's whole life, the provider-key store is cached in memory, and
  `registry.lock` (and, when configured, money and billing) are `flock`
  locks. Two gateways on one directory is not a supported state.
- **No per-request cache.** The account and session stores re-read
  `accounts.json` and `sessions.json` on every request, so reads must be
  fresh after another writer's rename.

## The options

| Option | Correct for these stores? | Cost | Verdict |
| --- | --- | --- | --- |
| Cloud Storage FUSE volume | No. Google's page says it is "not a fully POSIX-compliant file system", has no locking ("the last write wins"), writes reach the bucket only on close or fsync, rename is copy-and-delete, and owner and mode are fixed per mount. The exclusive-create locks and the owner checks cannot hold. | Cents | Rejected |
| Filestore (NFS) | Yes: a real NFS server, atomic rename, exclusive create, stable inodes. | Basic HDD is 1 TiB minimum, roughly $200 a month or more per environment; the Filestore API is not enabled in `openagentsgemini` | Correct, but 100 times the price of the data. The managed upgrade path (same mount, other server address) |
| Cloud SQL | Only after rewriting the stores: `tenancy` has no SQL backend; every store is a file. | ~$10-50 a month | Rejected for 1.0: a rewrite of the account service |
| Small GCE VM with a persistent disk, exported over NFS | Yes: Linux's NFS server on ext4 on a persistent disk is the same NFS semantics as Filestore. | e2-small plus 20 GB pd-balanced, about $15 a month; snapshots of a few MB | **Chosen** |

Both NFS options share one Cloud Run limit: "Cloud Run does not support
NFS locking. NFS volumes are automatically mounted in no-lock mode." So
`flock` locks hold only within one instance. Exclusive create, rename and
fsync are server operations and do hold across instances. The handoff
below makes sure only one gateway process uses the share at a time, which
keeps the `flock` and in-memory-cache assumptions true.

## How it is built

- **Server:** `oa-accounts-nfs-<env>` (us-central1-a, e2-small, Debian 12,
  no external address, no service account, Shielded VM, deletion
  protection, unattended security upgrades). Data disk
  `oa-accounts-nfs-<env>-data` (10 GB pd-balanced, ext4, device name
  `accounts`, kept if the VM is deleted), mounted at `/srv/accounts`.
  `deploy/accounts-nfs/startup.sh` (the VM's startup script, run at every
  boot) formats the disk once, makes `stack/` and `web/` owned by uid 10001
  mode 0700, and exports `/srv/accounts` with
  `rw,sync,no_subtree_check,root_squash` to the Cloud Run subnet only.
  `sync` puts each write on the disk before the client is told.
- **Network:** a subnet of its own in the `default` network for the
  service's Direct VPC egress (`run.googleapis.com/network-interfaces`,
  `vpc-access-egress: private-ranges-only`, so only private addresses use
  the VPC). The VM has the address `.2` in that subnet. Firewall: NFS
  (111, 2049, 20048) from the subnet only, SSH from IAP only, everything
  else denied (the deny outranks the network's allow-internal and
  allow-ssh rules).
- **Service:** two NFS volumes, `stack` (`/srv/accounts/stack`, mounted at
  `/stack` in `gateway` and `worker`) and `webstate`
  (`/srv/accounts/web`, at `/state` in `web`). A container does not start
  unless its NFS volume mounts.
- **The handoff** (`deploy/staging/gateway.sh`). A deploy runs the old and
  the new instance side by side for a few seconds. A starting gateway
  writes its claim to `stack/handoff/takeover` and waits. The running one
  sees the claim within 2 seconds, stops its gateway, and writes
  `released` to `stack/handoff/holder`. The new one then deletes any lock
  file left on the share (no other gateway runs now), writes itself as the
  holder, and starts. A holder that never answers (a lost instance) is
  taken over after 150 seconds; the gateway's startup probe allows 240. A
  restart of the gateway container in the same instance takes the store
  straight back. Every read is `cat`, an open(), which NFS revalidates with
  the server. During the switch the old instance answers pages but not
  account calls, for the few seconds until traffic moves (12 seconds on
  2026-10-09).
- **Roll back** by deploying the previous images as a new revision
  (`render.py` with the earlier digests). Do not move traffic back to an
  old revision with `update-traffic`: its instance has handed the store
  over and runs without a gateway.
- **If the VM is down,** a new instance does not start (its volume cannot
  mount), and a running one waits on the mount; account calls hang until
  the VM is back. GCE live-migrates e2 VMs for host maintenance.

## Backups

- **Hourly snapshots** of the data disk by the schedule
  `oa-accounts-nfs-<env>-hourly` (kept 7 days on staging, 30 on
  production; stored in the `us` multi-region; kept if the disk is
  deleted). The stores write by atomic rename with fsync, so a
  crash-consistent snapshot is a consistent copy.
- **The stores' own history.** `accounts-history/` and
  `sessions-history/` keep every sealed revision by digest, so a bad write
  can be read back without a snapshot.
- **Drill, 2026-10-09 (staging):** a snapshot of the live disk, restored
  to a new disk, attached read-only to the server and mounted, held the
  two accounts made by then (`accounts.json` parsed); the drill disk and
  snapshot were deleted afterwards.

## Restore

Whole disk (the disk is lost or corrupted), with the automation account:

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
ENV=staging   # or production
P="--project openagentsgemini"; Z="--zone us-central1-a"
gcloud compute snapshots list $P --filter="sourceDisk~oa-accounts-nfs-$ENV-data" \
  --sort-by=~creationTimestamp --limit 5          # pick SNAPSHOT
gcloud compute disks create oa-accounts-nfs-$ENV-restored $P $Z \
  --source-snapshot SNAPSHOT --type pd-balanced \
  --resource-policies oa-accounts-nfs-$ENV-hourly
gcloud compute instances stop oa-accounts-nfs-$ENV $P $Z
gcloud compute instances detach-disk oa-accounts-nfs-$ENV $P $Z --disk oa-accounts-nfs-$ENV-data
gcloud compute instances attach-disk oa-accounts-nfs-$ENV $P $Z \
  --disk oa-accounts-nfs-$ENV-restored --device-name accounts
gcloud compute instances start oa-accounts-nfs-$ENV $P $Z
```

The startup
script mounts the restored disk and re-exports it; the running instance's
mounts recover on their own (hard NFS mounts wait). Then force a new
revision (`scripts/smoke/staging.sh --only durable --restart`) and keep
the old disk until the restore is checked.

One file (a store went wrong, the disk is fine): restore the snapshot to
a new disk, attach it read-only (`--device-name drill --mode ro`), and on
the server (`gcloud compute ssh oa-accounts-nfs-$ENV $P $Z
--tunnel-through-iap`) mount it with `mount -o ro,noload
/dev/disk/by-id/google-drill /mnt/drill`, copy the file into place as uid
10001, mode 0600, then unmount, detach and delete the drill disk.

## Proof (staging, 2026-10-09)

`scripts/smoke/staging.sh --only durable --restart` makes an account, its
session, an API key, a saved OpenRouter key and a saved own-Claude key,
checks them, forces a new revision (`gcloud run services update
--update-labels`), and checks them again. Run on staging: 10 passed
(`openagents-web-1-staging-nfs1` to `openagents-web-1-staging-00008-6jp`),
and the full suite afterwards: 55 passed, 0 failed, 1 skipped.

## Production (live since 2026-10-09 20:57 UTC)

Applied on the owner's go (#11094). openagents.com (service `coder`,
revision `coder-web-accounts-d4f5b07301-b`) runs `web`, `coder-serve` and
`gateway`, one instance (min and max 1, the one-writer rule), gen2, with
Direct VPC egress on `openagents-web-production`. What differs from
staging:

- No chat worker container: the site's chat keeps using the production
  chat worker VM over the relay (the web's default worker).
- The web container starts through `deploy/production/web.sh` (inlined by
  `deploy/production/render.py`) with the live revision's own arguments
  after it; paths the site doesn't own still go to `coder-serve`.
- The gateway runs `deploy/staging/gateway.sh` with `GATEWAY_HOLD=serving`:
  it takes the store only while its revision has traffic, so a no-traffic
  candidate (`scripts/deploy/web.sh promote`) never takes the store from
  the revision serving openagents.com, and an `update-traffic` rollback to
  an earlier gateway revision hands the store back within seconds. It has
  no startup probe and the web does not wait for it.
- Secrets: `openagents-web-production-{github-oauth,csrf-key,byo-keys}`,
  `openagents-gateway-production-{admin-token,byok-keyring,store-key}`
  (made by `deploy/accounts-nfs/secrets.py production --runtime
  157437760789-compute@developer.gserviceaccount.com --web
  ~/work/.secrets/github-oauth-production.json`), the existing
  `openagents-web-ask-salt`, and the production upstream keys
  `openagents-{vertex-sa-key,openrouter-api-key,vercel-gateway-api-key}`.
- Invite-only: only the owner (GitHub id 14167547) may sign in, as admin
  ([GitHub sign-in](../auth/github.md#invite-only-sign-in)). There is no
  operator sign-up token on production.
- Firewall: besides the NFS rules and the pay-host rule,
  `allow-internal-from-web-production` gives the subnet the same internal
  reach the `default` subnet had (coder-serve's pool host).

A later site build: `scripts/deploy/web.sh promote DIGEST` (copies this
spec, gateway included). A later stack build: `python3
deploy/production/render.py` with the live service and serving revision
JSON and `--stack-image` (see its docstring), applied with `gcloud run
services replace` as `chris@`. Roll back with `scripts/deploy/web.sh
rollback REVISION`.

The steps it was built with, kept for another environment:

The same design, in its own subnet, server and secrets. With the
automation account, from the repository root:

1. `sh deploy/accounts-nfs/provision.sh production`: subnet
   `openagents-web-production` (10.42.27.0/26), server
   `oa-accounts-nfs-production` at 10.42.27.2, 30-day hourly snapshots,
   the NFS firewall, and a rule letting the subnet reach the pay host
   (`oa-pay-1`, tcp:4400), which the site calls today over the `default`
   subnet. If the first boot cannot reach the Debian mirror (Cloud NAT is
   slow to see a new subnet), `gcloud compute instances reset
   oa-accounts-nfs-production --zone us-central1-a --project
   openagentsgemini` once and check the serial log for `accounts NFS:
   /srv/accounts exported`.
2. `python3 deploy/accounts-nfs/secrets.py production --runtime
   RUNTIME_ACCOUNT` (the production service's runtime account): makes
   `openagents-gateway-production-byok-keyring` and
   `openagents-gateway-production-store-key`.
3. In the production service spec (rendered like `deploy/staging/render.py`
   with production names): `NFS_SERVER = "10.42.27.2"`,
   `EGRESS_SUBNET = "openagents-web-production"`, the two NFS volumes, the
   `network-interfaces` and `vpc-access-egress: private-ranges-only`
   annotations (these replace the `coder` service's current
   `default`/`default` interface; the pay host stays reachable through the
   rule in step 1), `WEB_STATE=/state`, `STACK_STATE=/stack`,
   `BYOK_KEYRING_JSON` and `INFERENCE_STORE_KEY` from the step-2 secrets,
   and the gateway startup probe of 4 s by 60.
4. Deploy, then run the durable check against production with its own
   operator token, or by hand: sign in with GitHub, add an API key and an
   OpenRouter key, force a new revision (`gcloud run services update
   SERVICE --update-labels restart=$(date +%s) --region us-central1
   --project openagentsgemini`), and check you are still signed in and
   both keys are listed.

## Follow-ups

- #11150: `tenancy::keys::save` writes `keys.json` through one fixed temporary
  name without a lock, so two keys issued at the same moment can lose one.
  It predates this change (one process has the same race).
- Each `--restart` smoke run leaves one test account behind, now that
  accounts persist.

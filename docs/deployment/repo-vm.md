# The repo VM

Issue: [#13326](https://github.com/OpenAgentsInc/openagents/issues/13326)
(indexed repos, phase 1). Spec: [docs/repos/2026-11-11-indexed-repos.md](../repos/2026-11-11-indexed-repos.md),
"The service".
Code: `coax/web/repo/vm-deploy.sh`, `coax/web/repo/vm/`.

The repo service (`oa-repo`) keeps a bare mirror of each indexed repo and
answers the runner's and oa-api's reads over the VPC. It runs on one GCE VM
because git wants a real disk, and Cloud Run has none. No public address
serves it.

## Topology

| Piece | Name |
| --- | --- |
| VM | `oa-repo-1`, e2-standard-2 (2 vCPU, 8 GB), Debian 12, us-central1-a, labels `app=oa-repo`, `owner=openagents`, `issue=13326`, tag `oa-repo`, no service account, no external address |
| Address | `oa-repo-1-internal`, static internal, in subnet `default` (10.128.0.0/20) |
| Data disk | `oa-repo-1-data`, 100 GB pd-balanced (SSD-backed), device name `oa-repo-data`, mounted at `/var/lib/oa-repo`, not deleted with the VM |
| Snapshots | schedule `oa-repo-1-daily`: daily at 09:00 UTC, kept 7 days, stored in us-central1 |
| Subnets | `oa-run-production` (10.42.29.0/26) and `oa-run-staging` (10.42.30.0/26) on `default`: the callers' Direct VPC egress |
| Firewall | `oa-repo-production-callers` (TCP 8080 from 10.42.29.0/26), `oa-repo-staging-callers` (TCP 8081 from 10.42.30.0/26), `oa-repo-iap-ssh` (TCP 22 from IAP only), `oa-repo-deny-other-ingress` (everything else) |
| Units | `oa-repo@production` (port 8080, user `oa-repo-production`), `oa-repo@staging` (port 8081, user `oa-repo-staging`) |
| Keys | Secret Manager `oa-repo-grant-key` (production) and `oa-repo-grant-key-staging`, 32 random bytes as hex |
| Outbound | the default network's Cloud NAT in us-central1 (`hydralisk-default-nat-us-central1`, all subnets) for HTTPS to github.com and apt |

```
oa-api, oa-agent ── Direct VPC egress (oa-run-production) ── oa-repo-1:8080 (oa-repo@production)
oa-api-staging, oa-agent-staging ── (oa-run-staging) ────── oa-repo-1:8081 (oa-repo@staging)
oa-repo-1 ── Cloud NAT ── https://github.com (git fetch)
```

### Staging and production

Both units run on the one VM, as the room VM does: one machine is enough for
phase 1, and a second would double the cost. They are kept apart four ways:

- **Port and network.** Production listens on 8080 and only production's
  subnet reaches it; staging listens on 8081 and only staging's subnet reaches
  it. A staging service can't call production.
- **Key.** Each unit has its own grant key, so a grant minted by staging's
  oa-api is refused by production.
- **User.** Each unit runs as its own unprivileged user.
- **Disk.** Production's tree is the disk's root (`/var/lib/oa-repo`);
  staging's is `/var/lib/oa-repo/staging`, a 0700 directory production's user
  can't enter. Each unit creates its files with umask 0077.

A deploy goes to staging first, and production gets the exact binary staging
ran.

## Create

```sh
coax/web/repo/vm-deploy.sh create --dry-run   # print every command, run none
coax/web/repo/vm-deploy.sh create
```

`create` makes each missing piece in the table above and leaves what exists.
It makes the two grant keys when they are missing (`openssl rand -hex 32`,
piped into Secret Manager, never printed), waits for SSH through IAP, runs
`vm/setup.sh` on the VM, and writes the keys to the VM.

`vm/setup.sh` installs git, curl and CA certificates, formats the data disk
only when it holds no filesystem, mounts it by UUID at `/var/lib/oa-repo`,
makes the two users and their directories, writes `/etc/oa-repo/production.env`
and `staging.env`, and installs `oa-repo@.service`. It can run again:

```sh
coax/web/repo/vm-deploy.sh setup
```

Each unit's environment (`/etc/oa-repo/UNIT.env`):

| Variable | Production | Staging |
| --- | --- | --- |
| `OA_REPO_LISTEN` | `0.0.0.0:8080` | `0.0.0.0:8081` |
| `OA_REPO_DIR` | `/var/lib/oa-repo` | `/var/lib/oa-repo/staging` |
| `OA_REPO_REPOS` | `openagents=https://github.com/OpenAgentsInc/openagents.git` | the same |
| `OA_GIT` | `/usr/bin/git` | `/usr/bin/git` |

To index another repo, set `OA_REPO_REPOS` (a `;`-separated list of
`slug=url`) and run `setup` again, then restart the units with a deploy.

The grant key is in `/etc/oa-repo/UNIT.secret.env` (root, 0600). systemd reads
it before it drops to the unit's user, so the key is never on a command line
or in the unit file. The VM has no service account and reads nothing from
Google. To write the keys again, after a rotation for example:

```sh
coax/web/repo/vm-deploy.sh keys
```

A unit reads its key when it restarts.

## Deploy

```sh
coax/web/repo/vm-deploy.sh deploy --staging   # build, ship, run it as oa-repo@staging
coax/web/repo/vm-deploy.sh deploy             # promote staging's binary to oa-repo@production
```

`deploy --staging` builds `oa-repo` on a build host
(`coax/remote/remote.sh run --no-fallback -- coax/web/repo/build.sh OUT`). The
build hosts run Debian 12 on x86-64, as the VM does. It stops when
`coax/web/repo/build.sh` is missing or the result is not a Linux x86-64
executable. It ships the binary to `/opt/oa-repo/releases/NAME/oa-repo`
(NAME is the commit, `+` when the checkout had changes, and the time), checks
that every library it links is on the VM, points `/opt/oa-repo/staging` at it,
restarts `oa-repo@staging` and waits for `/healthz` on port 8081 to answer
`ok`.

`deploy` checks that staging is healthy, moves `/opt/oa-repo/production` to
staging's release (the old one becomes `/opt/oa-repo/production.previous`),
restarts `oa-repo@production` and waits for `/healthz` on port 8080. The VM
keeps the five newest releases and every release a link points at.

Both take `--dry-run`.

## Roll back

```sh
coax/web/repo/vm-deploy.sh rollback             # production to its previous binary
coax/web/repo/vm-deploy.sh rollback --staging   # staging to its previous binary
```

The unit's link and its `.previous` link swap, and the unit restarts and must
answer `/healthz`.

## Check it

```sh
coax/web/repo/vm-deploy.sh status
```

It prints the VM's state and address, the data disk and its last snapshot,
the disk's use, each unit's state, release, previous release and `/healthz`,
and each Cloud Run caller's network and `OA_REPO_URL`.

On the VM (`gcloud compute ssh oa-repo-1 --zone us-central1-a --tunnel-through-iap`):

```sh
systemctl status oa-repo@production
journalctl -u oa-repo@production -n 100
curl -s http://127.0.0.1:8080/healthz
```

## How the runner reaches it

The runner (`oa-agent`, `oa-agent-staging`) and oa-api (`oa-api`,
`oa-api-staging`) reach the VM over Direct VPC egress. Print the change, then
apply it:

```sh
coax/web/repo/vm-deploy.sh connect           # print the four commands
coax/web/repo/vm-deploy.sh connect --apply   # run them
```

Each is one `gcloud run services update`:

| Service | Subnet | `OA_REPO_URL` |
| --- | --- | --- |
| `oa-api` | `oa-run-production` | `http://INTERNAL_IP:8080` |
| `oa-agent` | `oa-run-production` | `http://INTERNAL_IP:8080` |
| `oa-api-staging` | `oa-run-staging` | `http://INTERNAL_IP:8081` |
| `oa-agent-staging` | `oa-run-staging` | `http://INTERNAL_IP:8081` |

with `--network default --vpc-egress private-ranges-only`. Only traffic to
private addresses goes into the VPC; the internet, Google APIs and the Cloud
SQL connector keep their current path, so no other call changes. Each update
makes a new revision. INTERNAL_IP is `oa-repo-1-internal`'s address, which
never changes while the address is reserved.

A later `gcloud run deploy` keeps the network settings. It replaces every
environment variable, though, so `coax/web/api/deploy.sh` and
`coax/web/agent/deploy.sh` read `OA_REPO_URL` from the live service and set it
again.

To undo it on one service:

```sh
gcloud run services update oa-agent --region us-central1 --project openagentsgemini \
  --clear-network --remove-env-vars OA_REPO_URL
```

## What is on the disk

```
/var/lib/oa-repo/              production's OA_REPO_DIR (owner oa-repo-production, 0711)
  mirrors/SLUG.git             a bare mirror of each repo
  ...                          the index
  staging/                     staging's OA_REPO_DIR (owner oa-repo-staging, 0700)
    mirrors/SLUG.git
```

A mirror can be fetched again from GitHub, so the only thing the snapshots
protect is open changes.

### The disk fills

`status` prints the disk's use. To find what holds it:

```sh
sudo du -sh /var/lib/oa-repo/mirrors/* /var/lib/oa-repo/staging/mirrors/*
```

Grow the disk; the VM keeps running:

```sh
gcloud compute disks resize oa-repo-1-data --zone us-central1-a --size 200GB --project openagentsgemini
gcloud compute ssh oa-repo-1 --zone us-central1-a --tunnel-through-iap --command 'sudo resize2fs /dev/disk/by-id/google-oa-repo-data'
```

Each extra 100 GB of pd-balanced costs $10 a month. Staging's mirrors can be
deleted at any time (stop `oa-repo@staging` first); staging fetches them
again.

### A mirror is corrupt

Fetch it again. With the unit stopped, move the mirror aside, start the unit,
and it makes a fresh mirror on its next fetch:

```sh
sudo systemctl stop oa-repo@production
sudo mv /var/lib/oa-repo/mirrors/openagents.git /var/lib/oa-repo/mirrors/openagents.git.bad
sudo systemctl start oa-repo@production
```

Delete the `.bad` copy once the new mirror answers. Open changes that lived
only in the bad mirror come back from the newest snapshot:

```sh
gcloud compute snapshots list --project openagentsgemini --filter='sourceDisk~oa-repo-1-data' --sort-by=~creationTimestamp
```

Make a disk from the snapshot, attach it to the VM, and copy the changes
across.

## Cost

List prices read on 2026-11-11 from the Cloud Billing Catalog API
(us-central1, on demand). Cloud NAT is an estimate; its price was not read.

| Item | Per month |
| --- | --- |
| e2-standard-2: 2 vCPU x $0.02181159/h + 8 GB x $0.00292353/h = $0.06701/h, x 730 h | $48.92 |
| Data disk, 100 GB pd-balanced x $0.10 | $10.00 |
| Boot disk, 10 GB pd-balanced x $0.10 | $1.00 |
| Snapshots, about 10 GB stored x $0.05 | about $0.50 |
| Cloud NAT, one VM ($0.0014/h) and a few GB of fetches ($0.045/GB) | about $1.20 |
| Internal address, subnets, firewall rules, Direct VPC egress | $0 |
| **Total** | **about $62** |

pd-ssd instead of pd-balanced would add $7 a month; at 100 GB both give the
same baseline of about 3,000 IOPS. The VM has no external address, so there
is no IPv4 charge.

# coder-sync on a single-writer VM

`coder-sync` (`sync.openagents.com`) keeps its rooms, presence, live event
fan-out, and device relay in one process's memory. A live chat row reaches
only the sockets in the same process, so the service needs exactly one
writer. On Cloud Run, a single instance kept being declared "no available
instance" while idle (#12120), and the three-instance mitigation split live
rooms. Since #12122 the service runs on one Compute Engine VM behind a global
external HTTPS load balancer. The Cloud Run service stays deployed as the
rollback backend.

## Layout

| Piece | Name | Notes |
| --- | --- | --- |
| VM | `coder-sync-1` (`us-central1-a`, `e2-standard-2`) | Container-Optimized OS, service account `coder-sync@`, tag `coder-sync` |
| Boot configuration | [`deploy/coder-sync-vm/cloud-init.yaml`](../../deploy/coder-sync-vm/cloud-init.yaml) | Applied on every boot from the `user-data` metadata key |
| Database | `coder-sync-pg` through the Cloud SQL Auth Proxy | The proxy serves `/cloudsql`, the socket path the Cloud Run DSN already uses |
| Secret | `coder-sync-database-url` | Read at each server start into `/run/coder-sync/env` (tmpfs, root only) |
| Instance group | `coder-sync-ig` | Unmanaged, named port `http:8080` |
| Backends | `coder-sync-vm`, `coder-sync-run` | The VM, and Cloud Run through the serverless NEG `coder-sync-run-neg` |
| Health check | `coder-sync-health` | `GET /health` on port 8080 every 5 seconds |
| URL map | `coder-sync` | The default service is the serving backend |
| Front end | `coder-sync-ip`, `coder-sync-https` | Port 443, certificate map `coder-sync-map` |
| Certificate | `coder-sync-cert` | Certificate Manager with DNS authorization `coder-sync-dns` |
| DNS | Cloudflare, `sync.openagents.com` | `A` record to the load balancer address, not proxied, TTL 60 |

The backend timeout is one day, so a websocket lasts up to 24 hours before
the load balancer closes it and the client reconnects. There is no
per-instance request cap.

On the VM, three systemd units run:

- `cloud-sql-proxy.service` runs the Auth Proxy container.
- `coder-sync.service` reads the environment and the secret, then runs the
  image in `coder-sync-image` on port 8080. It restarts on exit.
- `coder-sync-watch.service` checks the metadata every 10 seconds. When
  `coder-sync-image` changes, it pulls the new image, then restarts the
  server, so a deploy drops sockets for a few seconds only. It also restarts
  a server whose `/health` has failed for a minute.

## Reaching one backend directly

The URL map sends a request with the header `x-coder-sync-backend: vm` to
the VM and `x-coder-sync-backend: run` to Cloud Run, whichever backend
serves by default. Use it to check either one before moving traffic:

```sh
IP=$(gcloud compute addresses describe coder-sync-ip --global --format='value(address)')
curl --resolve sync.openagents.com:443:$IP -H 'x-coder-sync-backend: vm' https://sync.openagents.com/health
```

## Commands

Run from a checkout on `origin/main`, with
`CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config` (the automation account).

| Task | Command |
| --- | --- |
| Create the VM and load balancer (idempotent) | `scripts/coder-sync/deploy.py vm-provision` |
| Show the serving backend, certificate, and health | `scripts/coder-sync/deploy.py vm-status` |
| Build an image | `scripts/coder-sync/deploy.py build` |
| Deploy to staging first (Cloud Run) | `scripts/coder-sync/deploy.py deploy --staging` |
| Deploy to the VM | `scripts/coder-sync/deploy.py vm-deploy` |
| Roll the VM back to the previous image | `scripts/coder-sync/deploy.py vm-rollback` |
| Serve from the VM | `scripts/coder-sync/deploy.py cutover vm` |
| Serve from Cloud Run (rollback) | `scripts/coder-sync/deploy.py cutover run` |

`vm-deploy` refuses an image that `coder-sync-staging` or the production
Cloud Run service does not already run, so staging stays first. It records
the running image as `coder-sync-previous-image`, which `vm-rollback`
restores. Pass `--commit SHA` to deploy an image other than `HEAD`.

`cutover` changes only the load balancer, so it takes effect within a
minute and needs no DNS change. Keep the Cloud Run service deployed (`deploy`
without `--staging`) when you change server code, so `cutover run` serves
current code.

## DNS

The `dns` command edits the Cloudflare zone with `CLOUDFLARE_API_TOKEN`
(from `~/work/.secrets/cloudflare-openagents.env`):

- `dns cert` adds the certificate's DNS authorization `CNAME`.
- `dns lb` points `sync.openagents.com` at the load balancer.
- `dns run` points it back at the Cloud Run domain mapping
  (`ghs.googlehosted.com`). Use it only if the load balancer itself fails;
  `cutover run` is the usual rollback.

## First rollout

1. `vm-provision`, then `dns cert`.
2. Wait until `vm-status` shows the certificate `ACTIVE` and `vm /health`
   answers with the expected revision.
3. Check the VM's websockets with the test account through
   `x-coder-sync-backend: vm`.
4. `cutover vm`, then `dns lb`.
5. Check live delivery between two clients through `sync.openagents.com`.

# oa-boat deployment record

The service, its API and how to operate it: [docs/cloud/oa-boat.md](../cloud/oa-boat.md).
Issue: [#11256](https://github.com/OpenAgentsInc/openagents/issues/11256).

## 2026-10-10/11: first deploy

What was made, in project `openagentsgemini` (all with the automation
account except the two project roles, which needed the owner account):

```text
secrets      oa-boat-api-key (our service token), oa-boat-ssh-key (ed25519, for the VMs)
subnet       oa-boat-run  10.42.28.0/26  network default, us-central1
firewall     oa-boat-ssh-from-service  tcp:22  10.42.28.0/26 -> tag oa-boat-sandbox
account      oa-boat@  roles/compute.instanceAdmin.v1, roles/compute.storageAdmin, accessor on both secrets
service      Cloud Run oa-boat  min=max=1, no CPU throttling, 2 vCPU / 2 GiB, direct VPC egress, --no-invoker-iam-check
readers      oa-boat-api-key: 157437760789-compute@ (production web), oa-vertex-inference@ (staging web), oa-mvp-automation@ (template job)
```

Revisions, each built by `deploy/oa-boat/cloudbuild.yaml` from a pushed
commit:

| Revision | Commit | Why |
| --- | --- | --- |
| `oa-boat-00001-wsn` | `c80c5266f1` | First deploy. Every create failed: Compute Engine answers 411 to a POST with no body (the operation wait) |
| `oa-boat-00002-dw9` | `852d03aabe` | Empty JSON body on such POSTs; `/health` (Google's front end keeps `/healthz`) |
| `oa-boat-00003-r29` | `f8c4754ba4` | A VM whose insert wait failed but exists is used, not leaked; the reaper readies VMs it never prepared |
| `oa-boat-00004-8ct` | `4ec6a2ed96` | After a restart, a VM already ready for its boot answers at once (the `00003` rollout made the template build's next command answer 409) |
| `oa-boat-00005-k6h` | `c1008ae902` | A VM that keeps refusing our SSH key gets its `ssh-keys` metadata rewritten once a minute, so the guest agent puts the key back (a staging setup VM lost key login for 90 minutes) |

Two client-side finds on the way: Cloud Run's front end also answers 411 to
the SDK's body-less `stop` (the SDK now sends `Content-Length: 0`), and the
service's first `--allow-unauthenticated` did not take; `--no-invoker-iam-check`
lets our own bearer token through.

## Environments on our backend

| Where | Revision | Result |
| --- | --- | --- |
| Staging | `6cc41064ec` | Smoke `92 passed`; a full setup of `octocat/Hello-World` (setup machine, clean build image, fresh-machine check) ready to save in `594 s`, all on oa-boat VMs |
| Production | `coder-web-2785ffe336-20261011034506` (same image, promoted with the candidate smoke, then shifted) | Candidate smoke `51 passed`, `8` waiting for traffic; after the shift `59 passed`. The log says "Environments are on at /environments", and the backend probe answers 200 every 30 s |

The first staging setup failed at its last step: the fresh-machine check's
start met its own job's lock, held by the owners' loop while a verifier VM
came up. Starting is idempotent, so the setup now waits and asks again
(`6cc41064ec`).

Production's previous revision, for rollback:
`scripts/deploy/web.sh rollback coder-web-a74a2376f9-20261011031455`.

## Idle stop and TTL, live

- `bx_glifpdc4gk` (a staging setup machine) was stopped by the reaper
  after 30 idle minutes ("stop bx_glifpdc4gk (idle)").
- `bx_gh548gv7v2`, which the service could not reach, was stopped at its
  TTL ("stop bx_gh548gv7v2 (ttl)"): the deadline does not need SSH.

## The daily template on our backend

The Cloud Run job `oa-boat-template` (image from `9e82dc419e`, key
`oa-boat-api-key`, base `BOAT_API_BASE` = this service) built
`oa-coder-main-20261011` on a `large` sandbox: cold build (no shared cache
on sandboxes) from `02:44` to `03:52` UTC, then a `32498907520`-byte GCE
image saved in `214.5s`; the job exited 0. The schedule
`oa-boat-template-daily` runs the same job each day.

A sandbox from it (`bx_olo0d58cat`, `large`): ready in `33 s`; the first
`cargo build -p openagents-cli` in the warm slot compiled one crate in
`93 s`; the whole check cost `$0.010421`. There is no lazy restore to wait
for, which on hosted Boat took minutes before a warm build could start.

## Measured

Live, against the deployed service, with `curl` and the SDK's own calls:

| What | Result |
| --- | --- |
| Create to ready, `small` (e2-standard-2, newest `oa-coder-host` image, 200 GB disk) | `65.4 s`, `46.8 s`, `44 s` |
| Create to ready, `large` (n2d-standard-8) | `35.2 s`, `34.0 s` (the template build's sandbox) |
| Hosted Boat, for comparison ([plan §7](../cloud/2026-10-02-boat-sdk-plan.md)) | fork to ready 7 to 76 s, then a lazy restore of 72 to 830 s before a warm build |
| Lifecycle | env, synchronous and streamed commands, files both ways, detached command and its status, usage, stop, latest snapshot, delete: all as the SDK expects |
| A Claude Code task (`claude -p`, owner's token in the VM's tmpfs env only) | `fib.py` written, run, and read back; then stopped and deleted |
| Cost reported by `GET /usage` | `small` `$0.06701`/h compute + `$0.0274`/h disk; `large` `$0.33797`/h + disk. A whole small lifecycle cost under a tenth of a cent |

Hosted Boat's `large` was `$0.072`/h; ours is on demand and dedicated.
Spot (`provisioning: spot`) roughly halves the compute price.

## Rollback

- Service: `gcloud run services update-traffic oa-boat --to-revisions REVISION=100 --region us-central1 --project openagentsgemini`.
- Back to hosted Boat for one client: `BOAT_HOSTED=1` with the hosted key
  (`~/work/.secrets/boat-hosted.env`, Secret Manager `boat-api-key`); for the
  web, point `BOAT_API_KEY` at `boat-api-key`, drop `BOAT_API_BASE` and set
  `BOAT_HOSTED=1` on the revision.
- Template job: `gcloud run jobs update oa-boat-template --update-secrets BOAT_API_KEY=boat-api-key:latest --remove-env-vars BOAT_API_BASE --update-env-vars BOAT_HOSTED=1`.

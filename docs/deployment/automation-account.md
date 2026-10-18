# The automation account: deploys and the GCE pool without a person's login

Issue [#12058](https://github.com/OpenAgentsInc/openagents/issues/12058),
2026-10-17.

Every agent-run GCP call in project `openagentsgemini` runs as
`oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com`. Its key lives
in an isolated gcloud configuration on the owner's Mac
(`CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config`, see the workspace
`CLAUDE.md`). The owner's `chris@openagents.com` login must reauthenticate
about once a day and can't do it without a person, so nothing scripted
falls back to it any more.

## Production deploys

Production deploys run on the automation account, as of 2026-10-17 21:24
CDT. Before that, every write that needed `iam.serviceAccounts.actAs` on
production's runtime account `157437760789-compute@developer.gserviceaccount.com`
(services `coder`, `coder-stage`, `oa-computers` and others) was refused,
and `scripts/deploy/web.sh` retried it as `chris@`.

**Why it was refused.** The project's IAM deny policy
`deny-legacy-livekit-privileged-impersonation` (created 2026-07-31 by the
LiveKit lane) denies `iam.googleapis.com/serviceAccounts.actAs` to the
automation account on every service account tagged
`openagentsgemini/livekit-privileged-identity=protected`. The runtime
account carried that tag, though it isn't a LiveKit identity. The account's
own policy already granted the automation account
`roles/iam.serviceAccountUser`, but a deny always wins over an allow. The
Terraform that made the deny (`infra/livekit-production/main.tf`) was
deleted with the LiveKit stack in `d7574f6669`, so the deny and its tags
exist only as live state.

**The change (owner-approved, 2026-10-17).** The tag binding was removed
from the runtime account only:

```sh
gcloud resource-manager tags bindings delete \
  --parent=//iam.googleapis.com/projects/157437760789/serviceAccounts/106936049731819339613 \
  --tag-value=tagValues/281480240796907 --location=global
```

The deny policy itself is unchanged and still covers the eight accounts that
keep the tag: `livekit-secret-reader`, `oa-livekit-sarah-secret-reader`,
`oa-livekit-prod-deployer`, `oa-livekit-prod-nodes`, `oa-sarah-nostr-signer`,
`oa-livekit-server`, `oa-livekit-agent` and `oa-livekit-cert-manager-reader`.
Changing tags needs an owner-level role; the automation account can't list
or edit deny policies, so this step was applied with the owner's login,
once.

**The risk.** The runtime account holds `roles/editor`, `roles/run.admin`,
`roles/cloudsql.client` and `roles/secretmanager.secretAccessor` on the
project. An identity that can act as it, by deploying a revision that runs
as it, can reach whatever those roles reach, so a leaked automation key
now reaches editor-level access through a deploy. The automation account
already held broad roles (`compute.admin`, `run.admin`, `secretmanager.admin`,
`iam.serviceAccountAdmin`, `iam.serviceAccountKeyAdmin` and others), so the
increase is real but bounded. The key stays chmod 600, gitignored, and is
never printed.

**To undo it:**

```sh
gcloud resource-manager tags bindings create \
  --parent=//iam.googleapis.com/projects/157437760789/serviceAccounts/106936049731819339613 \
  --tag-value=tagValues/281480240796907 --location=global
```

**No new roles were needed.** Production deploys need Cloud Run admin
(`roles/run.admin`), Artifact Registry read (`roles/artifactregistry.admin`),
Secret Manager reads for the revision's secrets (`roles/secretmanager.admin`),
Cloud SQL for migrations (`roles/cloudsql.client`, `roles/cloudsql.admin`),
Cloud Build (`roles/cloudbuild.builds.editor`), and actAs on the runtime and
build accounts (`roles/iam.serviceAccountUser`). The automation account held
all of them already; the deny was the only thing in the way.

**Proof (2026-10-17, 21:27 CDT).** With only the automation configuration
active, `WEB_DEPLOY_TAG=saproof scripts/deploy/web.sh promote
sha256:e8c3eeb0…` (the image production already served, no new code)
applied candidate `coder-web-e8c3eeb00a-20261018022721` with
`gcloud run services replace` and no traffic, and with no retry as `chris@`.
The candidate smoke was 58 passed, 0 failed, 2 skipped, and 8 waiting for
traffic, as expected. The `saproof` tag was then removed.
`coder-web-e8c3eeb00a-20261017185349` served 100% of traffic throughout,
and `/health`, `/openapi.json` and `/docs` answered 200 before and after.

**Scripts.** `scripts/deploy/web.sh` (staging, promote, shift, rollback, and
the gateway sidecar through `promote --stack`),
`scripts/deploy/promote-2026-10-17.sh`, and `deploy/oa-computers/deploy.sh`
run as the automation account. When it's refused, they stop and print the
refusal instead of retrying as a person.

## The GCE pool and Foreman's fan-out

The fan-out path (`openagents cloud up|down|status`, `openagents chat work
--on gce`, its hosts, and the credentials it sends) was audited for every
GCP call. Each one needs a permission the automation account already holds:

| Call | Permission | Held through |
| --- | --- | --- |
| `compute instances list` (every `cloud` command, every run) | `compute.instances.list` | `roles/compute.admin` |
| `compute instances create` from image family `oa-coder-host` | `compute.instances.create`, `compute.disks.create`, `compute.images.useReadOnly`, `compute.images.getFromFamily`, `compute.subnetworks.use`, `compute.instances.setMetadata`, `compute.instances.setLabels`, `compute.instances.setServiceAccount` | `roles/compute.admin` |
| … as `oa-coder-host@` | `iam.serviceAccounts.actAs` on `oa-coder-host@` | `roles/iam.serviceAccountUser` (not tagged, so the deny doesn't apply) |
| `compute instances add-iam-policy-binding` (each host may delete itself) | `compute.instances.setIamPolicy` | `roles/compute.admin` |
| `get-serial-port-output`, `get-guest-attributes` (ready marker, ssh host keys) | `compute.instances.getSerialPortOutput`, `compute.instances.getGuestAttributes` | `roles/compute.admin` |
| `start-iap-tunnel` (ssh) | `iap.tunnelInstances.accessViaIAP` | `roles/iap.tunnelResourceAccessor` |
| `compute instances delete` (`cloud down`) | `compute.instances.delete` | `roles/compute.admin` |
| Secret Manager `coder-pool-git-token`, `openagents-xai-api-key`, `openagents-work-host-token` | `secretmanager.versions.access` | `roles/secretmanager.admin` |
| Logs | `logging.logEntries.list` | `roles/logging.viewer` |

`coder-openai-api-key` (Codex's API key for `--engine-fallback` only) doesn't
exist; runs use the ChatGPT login copy instead.

The hosts themselves run as `oa-coder-host@`: the sccache bucket, the
delete of their own instance through a role on that instance alone, and,
since 2026-10-18 02:27 CDT, the landing queue. That's
`roles/storage.objectUser` on `gs://openagentsgemini-coder-artifacts`
with the IAM condition `land-queue-only`, which covers the
`land-queue/` objects and listings under that prefix and nothing else in
the bucket. Without it, a pool run's `openagents land submit` was refused
(`storage.objects.get`) and the change stayed a local commit. The
integrator's VM (`oa-dev-env-1`) isn't the hosts' to start: when it's
stopped, the orchestrator starts it as the automation account
(`roles/compute.admin`). The
`--on boat` path (`POST /v1/work`) authenticates to openagents.com with the
computer's sign-in, not gcloud. Its work hosts run as the automation account.
The Computers service (`oa-computers`) runs as the production runtime account
and deploys with `deploy/oa-computers/deploy.sh`, which the deny change above
unblocked. SPACE is a measured spike on disposable hosts
([space.md](../cloud/space.md)), not part of the fan-out path yet.

Three things still reached for the person's login, and all three are fixed:

1. `scripts/deploy/web.sh` retried refused writes as `chris@`. It no longer
   does.
2. `scripts/deploy/promote-2026-10-17.sh` ran the `oa-computers` deploy with
   `env -u CLOUDSDK_CONFIG`, which meant `chris@`. It now runs as the
   automation account.
3. A pool command started without `CLOUDSDK_CONFIG` (an agent's tool call,
   for example) ran gcloud as the default account, which is `chris@`. The
   pool record now remembers the configuration the grant was made with and
   every later gcloud and ssh/IAP call of the pool uses it. `cloud up` warns
   when gcloud is signed in as a person, and `cloud status` names the
   account.

## The live proof (2026-10-18)

The proof ran from a shell whose only gcloud configuration was the
automation account's (`gcloud auth list` showed only
`oa-mvp-automation@`), with the CLI built from `main`:

| Step | Result |
| --- | --- |
| `openagents cloud up --hosts 2` | 00:15:50 to 00:24:43 CDT: two spot `c3-standard-8` hosts created, ready, reached over IAP, and built `e0ea6ed25f` (526 s). `cloud status` named the account and showed "about $0.12 of $300.00 spent over the last 24 hours". |
| `openagents chat work --on gce --issues 12098 --land queue` | 02:28 to 02:39 CDT: the briefed agent on Codex worked [#12098](https://github.com/OpenAgentsInc/openagents/issues/12098) on host `oa-pool-pb22a59-5bd5`. Checks were replayed 2/2 (`cargo fmt`, `cargo test -p openagents-cli`), and the change was queued as entry `20261018T073841Z-oa-pool-pb22a59-5bd5-5d65`, at about $0.018 of host time. The integrator (`oa-dev-env-1`, started as the automation account) landed it on `main` as `9a70ce012b` at 03:10 CDT and closed the issue. |
| Idle shutdown | Every host deleted itself after 10 idle minutes. |
| `openagents cloud down` | 02:52 CDT: the grant was revoked, and "Leak check: no pool host is left". Spend over the 24 hours was about $0.62, every attempt included. |

The proof found five faults on `main`, all fixed in this change:
- `cloud up` had failed on every host since `d765f537cb` (2026-10-10). The
  build stamped `tree: unknown`, and the install refused it.
- The briefed engine's built-in scripts lacked `kinds`, `acceptance` and
  the rest of its bench modules.
- The bundled CLI tree went stale with the new help.
- A template test assumed umask 022, but pool runs use 077.
- Pool hosts couldn't reach the landing queue.

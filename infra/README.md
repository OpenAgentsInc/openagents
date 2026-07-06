# infra/ — Terraform baseline for the OpenAgents GCP footprint

Issue: #8527 (CFG-12, epic #8515). Companion audit:
`docs/cloud/2026-07-06-cloudflare-to-google-consolidation-audit.md` (§6).

This is a **baseline, not a framework**: it puts the critical live resources
in project `openagentsgemini` under Terraform state so drift is visible and
future changes are reviewable. It was built by importing the live resources
and writing HCL until `plan` was a no-op — nothing was created or modified
except the state bucket itself.

We run it with **OpenTofu** (`brew install opentofu`); plain Terraform >= 1.6
works identically.

## Layout

```
infra/
  README.md            this file
  Makefile             make init / plan / validate / fmt
  prod/                root module (single env today)
    backend.tf         GCS remote state (bucket openagentsgemini-terraform-state, prefix prod)
    providers.tf       google provider ~> 6.0
    variables.tf       project_id / region
    main.tf            module instantiations for every managed resource
  modules/
    cloud-sql-postgres/  instance + flags + backup/PITR + users (no passwords)
    cloud-run-service/   service SHELL only — revisions stay with gcloud/CI
    gcs-bucket/          bucket with optional versioning + lifecycle rules
    scheduler-job/       HTTP-target Cloud Scheduler job (not yet instantiated)
    service-account/     SA + non-authoritative role grants (not yet instantiated)
    monitoring-alerts/   CPU / connections / 5xx / budget policies (not yet instantiated)
    secret-manager-secret/  secret CONTAINER + accessor grants — versions stay out-of-band
    global-external-lb/  static IP + Certificate Manager cert (DNS-auth pre-provisioning) + serverless-NEG LB (CFG-10)
```

Note: `backend.tf` and `providers.tf` live inside `prod/` (not at `infra/`)
because Terraform backends are per-root-module; a future `staging/` root gets
its own copies with a different state prefix.

## What is under management (imported 2026-07-06)

| Resource | Address |
| --- | --- |
| Cloud SQL `khala-sync-pg` (PG17, HA, PITR) + 4 users | `module.khala_sync_pg` |
| Cloud SQL `l402-aperture-db` (PG15) + 3 users | `module.l402_aperture_db` |
| Cloud SQL `autopilot4-pg` (PG16) + 2 users | `module.autopilot4_pg` |
| Cloud SQL `oa-convex-nonprod-pg` (PG16) + 2 users | `module.oa_convex_nonprod_pg` |
| Cloud Run `oa-updates` (shell) | `module.oa_updates` |
| Cloud Run `oa-cloud-run-bridge` (shell) | `module.oa_cloud_run_bridge` |
| GCS `openagentsgemini-oa-updates` | `module.oa_updates_bucket` |
| GCS `openagentsgemini-terraform-state` | `module.terraform_state_bucket` |
| Secret Manager `oa-updates-codesign-key` + accessor grant (#8530) | `module.oa_updates_codesign_key` |
| Cloud Run `openagents-monolith` (shell pre-created for CFG-9/#8524) | `module.openagents_monolith` |
| Global External LB for `openagents.com` + `auth.openagents.com` — static IP, Certificate Manager cert + DNS authorizations, serverless NEG, backend, URL maps, HTTP(S) proxies, forwarding rules (CFG-10/#8525) | `module.openagents_lb` |

19 resources on import day, +2 imported for #8530, +16 created for #8525
(CFG-10 LB pre-stage; the LB IP receives no traffic until the DNS flip in
`docs/cloud/2026-07-06-openagents-domain-cutover-runbook.md`). `tofu plan`
was a **no-op** against live as of import day; there are no accepted diffs.

## Deliberate design decisions

- **Cloud Run: shell ownership only.** Terraform owns existence, location,
  and ingress. `lifecycle.ignore_changes` covers `template`, `traffic`,
  `labels`, `annotations`, etc., so ordinary `gcloud run deploy` releases
  never show up as drift and runtime env values never need to live in HCL.
- **No credentials in HCL.** SQL users are tracked for existence only;
  passwords are set/rotated with `gcloud sql users set-password` and ignored
  by the provider config. Secret Manager secrets are tracked as **containers
  only** (`modules/secret-manager-secret`): versions/payloads are added
  out-of-band with `gcloud secrets versions add`, never through Terraform.
  The `oa-updates` `OA_SIGNING_KEY` moved from inline Cloud Run env to the
  `oa-updates-codesign-key` secret on 2026-07-06 (#8530 / CFG-14); state was
  refreshed to the secret-ref shape and all superseded GCS state versions
  containing the inline key were purged (they linger in the bucket's 7-day
  soft-delete window until ~2026-07-13, admin-restore only, then are gone).
- **Destroy guards.** `deletion_protection = true` (Terraform-side) on every
  SQL instance and Cloud Run service, so a bad refactor cannot plan a
  destroy/replace of live data.
- **Reconciliations made during import** (config adjusted to match live, not
  the other way around): `enable_dataplex_integration = true` on
  `autopilot4-pg`; `location_preference.secondary_zone` left unset on
  `khala-sync-pg` (the API reports a top-level secondary zone but the
  location preference is unset); no `versioning` block emitted for
  non-versioned buckets.
- The `0.0.0.0/0` authorized network on `khala-sync-pg` / `l402-aperture-db`
  is the **live** setting, mirrored as-is. Tightening it is a deliberate
  future change to be made *through* Terraform, not silently during import.

## Workflow

```sh
cd infra
make init      # tofu init in prod/
make plan      # read-only plan — the default, safe command
make validate  # fmt-check + validate
```

Auth: the provider and GCS backend use Application Default Credentials. If
ADC is stale (`invalid_rapt` errors) but the gcloud CLI works, either run
`gcloud auth application-default login` or export a CLI token:

```sh
export GOOGLE_OAUTH_ACCESS_TOKEN=$(gcloud auth print-access-token)
```

(the Makefile does the token export automatically).

### Plan-on-PR / apply-on-main (intent — CI not wired yet)

- Every PR touching `infra/` should run `tofu plan` and post the plan as a PR
  artifact/comment. A non-empty plan requires explicit review.
- `tofu apply` runs only from `main` after merge, by an operator (or later a
  dedicated CI service account with a locked-down role set).
- Until CI exists, run `make plan` locally before and after merging.

### Adding a resource

1. Instantiate (or write) a module in `prod/main.tf`.
2. If the resource already exists in GCP, `tofu import <address> <id>` (see
   git history of this file for the exact IDs used on import day), then
   iterate on HCL until `make plan` is clean.
3. If it is new, review the plan and apply from `main`.

### Adding staging later

Copy `prod/` to `staging/`, change the backend `prefix` to `staging`, and
override `project_id`/`region` via tfvars. All environment-specific values
already flow through module variables.

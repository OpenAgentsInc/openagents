# Prod baseline: every resource below was IMPORTED from the live
# openagentsgemini project (issue #8527). HCL mirrors the live settings as of
# 2026-07-06 so `plan` is a no-op. See infra/README.md for the workflow.

# ---------------------------------------------------------------------------
# Cloud SQL
# ---------------------------------------------------------------------------

# Khala Sync production Postgres (the critical one).
module "khala_sync_pg" {
  source = "../modules/cloud-sql-postgres"

  project          = var.project_id
  name             = "khala-sync-pg"
  region           = var.region
  database_version = "POSTGRES_17"

  tier              = "db-custom-8-53248"
  edition           = "ENTERPRISE"
  availability_type = "REGIONAL"
  zone              = "us-central1-a"
  # NOTE: the live instance reports secondaryGceZone us-central1-b at the
  # API top level, but location_preference.secondary_zone is unset on the
  # instance, so it is deliberately unset here to keep the plan a no-op.
  disk_size_gb = 250

  backups_enabled                = true
  backup_start_time              = "08:00"
  pitr_enabled                   = true
  transaction_log_retention_days = 7
  retained_backups               = 7

  database_flags = {
    "cloudsql.logical_decoding" = "on"
  }

  # TODO(#8515): tighten to Cloud Run egress / auth proxy instead of open.
  authorized_networks = [
    { name = "", value = "0.0.0.0/0" },
  ]
  ssl_mode = "ENCRYPTED_ONLY"

  users = ["khala_app", "khala_capture", "khala_migrate", "postgres"]
}

# VP-1 retained cold evidence only. All L402 services are deleted, three
# database exports are retention-locked, and Terraform must never restart this
# instance during an unrelated apply.
module "l402_aperture_db" {
  source = "../modules/cloud-sql-postgres"

  project           = var.project_id
  name              = "l402-aperture-db"
  region            = var.region
  database_version  = "POSTGRES_15"
  activation_policy = "NEVER"

  tier              = "db-f1-micro"
  availability_type = "ZONAL"
  zone              = "us-central1-f"
  disk_size_gb      = 10

  backups_enabled   = false
  backup_start_time = "08:00"
  pitr_enabled      = false

  authorized_networks = [
    { name = "", value = "0.0.0.0/0" },
  ]
  ssl_mode = "ALLOW_UNENCRYPTED_AND_ENCRYPTED"

  users = ["aperture", "openagents_web", "postgres"]
}

# Autopilot4 Postgres.
module "autopilot4_pg" {
  source = "../modules/cloud-sql-postgres"

  project          = var.project_id
  name             = "autopilot4-pg"
  region           = var.region
  database_version = "POSTGRES_16"

  tier              = "db-f1-micro"
  availability_type = "ZONAL"
  zone              = "us-central1-a"
  disk_size_gb      = 10

  backups_enabled   = false
  backup_start_time = "07:00"
  pitr_enabled      = false

  ssl_mode = "ALLOW_UNENCRYPTED_AND_ENCRYPTED"

  # Live setting on this instance (Dataplex integration was enabled on it).
  enable_dataplex_integration = true

  users = ["autopilot4_app", "postgres"]
}

# Convex non-prod Postgres.
module "oa_convex_nonprod_pg" {
  source = "../modules/cloud-sql-postgres"

  project          = var.project_id
  name             = "oa-convex-nonprod-pg"
  region           = var.region
  database_version = "POSTGRES_16"

  tier              = "db-custom-1-3840"
  availability_type = "ZONAL"
  zone              = "us-central1-a"
  disk_size_gb      = 20

  backups_enabled   = false
  backup_start_time = "10:00"
  pitr_enabled      = false

  ssl_mode = "ALLOW_UNENCRYPTED_AND_ENCRYPTED"

  users = ["convex", "postgres"]
}

# ---------------------------------------------------------------------------
# Cloud Run (shell ownership only; revisions deploy via gcloud/CI)
# ---------------------------------------------------------------------------

locals {
  automation_service_account = "oa-mvp-automation@${var.project_id}.iam.gserviceaccount.com"
}

data "google_artifact_registry_repository" "oa_cloud" {
  project       = var.project_id
  location      = var.region
  repository_id = "oa-cloud"
}

resource "google_service_account" "cloud_image_builder" {
  project      = var.project_id
  account_id   = "oa-cloud-image-builder"
  display_name = "OpenAgents immutable image builder"
  description  = "Build-only identity selected explicitly by general OpenAgents container builds."
}

resource "google_artifact_registry_repository_iam_member" "cloud_image_builder" {
  project    = data.google_artifact_registry_repository.oa_cloud.project
  location   = data.google_artifact_registry_repository.oa_cloud.location
  repository = data.google_artifact_registry_repository.oa_cloud.repository_id
  role       = "roles/artifactregistry.writer"
  member     = "serviceAccount:${google_service_account.cloud_image_builder.email}"
}

resource "google_project_iam_member" "cloud_image_builder_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.cloud_image_builder.email}"
}

resource "google_storage_bucket" "cloud_build_source" {
  project                     = var.project_id
  name                        = "${var.project_id}-cloud-build-source"
  location                    = var.region
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"
  force_destroy               = false

  lifecycle_rule {
    action {
      type = "Delete"
    }
    condition {
      age = 7
    }
  }
}

resource "google_storage_bucket_iam_member" "cloud_image_builder_source_reader" {
  bucket = google_storage_bucket.cloud_build_source.name
  role   = "roles/storage.objectViewer"
  member = "serviceAccount:${google_service_account.cloud_image_builder.email}"
}

resource "google_service_account_iam_member" "cloud_build_cloud_image_builder" {
  service_account_id = google_service_account.cloud_image_builder.name
  role               = "roles/iam.serviceAccountTokenCreator"
  member             = "serviceAccount:service-${var.project_number}@gcp-sa-cloudbuild.iam.gserviceaccount.com"
}

resource "google_service_account_iam_member" "automation_cloud_image_builder" {
  service_account_id = google_service_account.cloud_image_builder.name
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${local.automation_service_account}"
}

resource "google_service_account" "cloud_run_source_builder" {
  project      = var.project_id
  account_id   = "oa-cloud-run-source-builder"
  display_name = "OpenAgents Cloud Run source builder"
  description  = "Build-only identity selected explicitly by Cloud Run source deployments."
}

resource "google_project_iam_member" "cloud_run_source_builder" {
  project = var.project_id
  role    = "roles/run.builder"
  member  = "serviceAccount:${google_service_account.cloud_run_source_builder.email}"
}

resource "google_service_account_iam_member" "automation_cloud_run_source_builder" {
  service_account_id = google_service_account.cloud_run_source_builder.name
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${local.automation_service_account}"
}

module "oa_updates" {
  source = "../modules/cloud-run-service"

  project = var.project_id
  name    = "oa-updates"
  region  = var.region
}

module "oa_cloud_run_bridge" {
  source = "../modules/cloud-run-service"

  project = var.project_id
  name    = "oa-cloud-run-bridge"
  region  = var.region
}

# ---------------------------------------------------------------------------
# Secret Manager (containers only; versions are added out-of-band)
# ---------------------------------------------------------------------------

# OTA manifest code-signing private key for oa-updates (#8530 / CFG-14).
# Mounted into the Cloud Run service as the OA_SIGNING_KEY env var via
# `--set-secrets` (see apps/oa-updates/scripts/deploy-cloudrun.sh). The key
# bytes live only in Secret Manager versions (added out-of-band) and the
# local operator backup; never in HCL or state.
module "oa_updates_codesign_key" {
  source = "../modules/secret-manager-secret"

  project   = var.project_id
  secret_id = "oa-updates-codesign-key"

  # Default compute SA — the runtime service account of the oa-updates
  # Cloud Run service.
  accessor_members = [
    "serviceAccount:157437760789-compute@developer.gserviceaccount.com",
  ]
}

# The Forge Git service and the monolith policy endpoint use one private
# service bearer. Terraform owns only the secret container. An operator adds
# the same random value as a secret version outside Terraform state.
module "forge_git_policy_authority_token" {
  source = "../modules/secret-manager-secret"

  project   = var.project_id
  secret_id = "openagents-forge-git-policy-authority-token"
}

# IDE-13 KMS authority. This grant does not create a key. It applies only when
# the operator supplies an existing full CryptoKey resource. The Cloud Run
# revision must also receive these non-secret values through the normal deploy:
# PORTABLE_CHECKPOINT_KMS_KEY_RESOURCE and PORTABLE_CHECKPOINT_KMS_KEY_REF.
# The runtime gets its OAuth token from its workload identity metadata service.
resource "google_kms_crypto_key_iam_member" "portable_checkpoint_dek" {
  count = var.portable_checkpoint_kms_crypto_key_resource == null ? 0 : 1

  crypto_key_id = var.portable_checkpoint_kms_crypto_key_resource
  role          = "roles/cloudkms.cryptoKeyEncrypterDecrypter"
  member        = "serviceAccount:${var.portable_checkpoint_kms_runtime_service_account_email}"
}

# ---------------------------------------------------------------------------
# GCS buckets
# ---------------------------------------------------------------------------

module "oa_updates_bucket" {
  source = "../modules/gcs-bucket"

  project  = var.project_id
  name     = "${var.project_id}-oa-updates"
  location = "US-CENTRAL1"
}

# The bucket that holds this very state (created out of band, then imported).
module "terraform_state_bucket" {
  source = "../modules/gcs-bucket"

  project    = var.project_id
  name       = "${var.project_id}-terraform-state"
  location   = "US-CENTRAL1"
  versioning = true
}

# ---------------------------------------------------------------------------
# Artifact/blob storage (CFG-8, #8523). Google Cloud Storage is the sole
# production artifact authority.
# ---------------------------------------------------------------------------

module "oa_artifacts_bucket" {
  source = "../modules/gcs-bucket"

  project  = var.project_id
  name     = "${var.project_id}-oa-artifacts"
  location = "US-CENTRAL1"
}

module "oa_artifacts_staging_bucket" {
  source = "../modules/gcs-bucket"

  project  = var.project_id
  name     = "${var.project_id}-oa-artifacts-staging"
  location = "US-CENTRAL1"
}

# Dedicated service account whose HMAC key backs the workerd/Bun-compatible
# GCS BlobStore (oa-infra `blob-store-gcs-hmac`). Bucket-scoped grants only —
# no project-level roles.
module "oa_artifacts_rw_sa" {
  source = "../modules/service-account"

  project      = var.project_id
  account_id   = "oa-artifacts-rw"
  display_name = "OpenAgents artifacts BlobStore (CFG-8)"
  description  = "HMAC-key identity for the oa-infra GCS BlobStore backing the former R2 ARTIFACTS surface (#8523)."
}

resource "google_storage_bucket_iam_member" "oa_artifacts_rw" {
  bucket = module.oa_artifacts_bucket.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${module.oa_artifacts_rw_sa.email}"
}

resource "google_storage_bucket_iam_member" "oa_artifacts_staging_rw" {
  bucket = module.oa_artifacts_staging_bucket.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${module.oa_artifacts_rw_sa.email}"
}

# ---------------------------------------------------------------------------
# openagents.com domain cutover (CFG-10, #8525)
# ---------------------------------------------------------------------------

# Shell for the CFG-9 (#8524) monolith. Pre-created here (placeholder hello
# image) so the LB's serverless NEG has a real target before CFG-9's first
# `gcloud run deploy openagents-monolith` lands — that deploy simply becomes
# the next revision of this shell, per the standard shell-ownership split.
module "openagents_monolith" {
  source = "../modules/cloud-run-service"

  project = var.project_id
  name    = "openagents-monolith"
  region  = var.region
}

# FORGE-02: one persistent-disk bare-repository store, one owned NFS host, and
# one stock Git transport service. GCS is evidence and a mirror, not ref truth.
module "forge_git" {
  source = "../modules/forge-git-service"

  project                           = var.project_id
  region                            = var.region
  zone                              = "${var.region}-a"
  pack_evidence_bucket              = module.oa_artifacts_bucket.name
  database_instance_connection_name = "${var.project_id}:${var.region}:khala-sync-pg"
}

# Global External Application LB fronting the monolith for openagents.com +
# auth.openagents.com. Pre-staged: the static IP receives no traffic until
# the DNS flip described in
# docs/cloud/2026-07-06-openagents-domain-cutover-runbook.md.
module "openagents_lb" {
  source = "../modules/global-external-lb"

  project                     = var.project_id
  name                        = "openagents"
  region                      = var.region
  domains                     = ["openagents.com", "auth.openagents.com"]
  cloud_run_service           = module.openagents_monolith.service_name
  forge_git_cloud_run_service = module.forge_git.service_name
  monolith_hosts              = ["openagents.com", "auth.openagents.com"]
  components_host             = "components.openagents.com"
  components_backend_service  = "effect-native-gallery-backend"
}

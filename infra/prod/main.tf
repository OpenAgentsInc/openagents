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

# L402 Aperture + openagents.com web Postgres.
module "l402_aperture_db" {
  source = "../modules/cloud-sql-postgres"

  project          = var.project_id
  name             = "l402-aperture-db"
  region           = var.region
  database_version = "POSTGRES_15"

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

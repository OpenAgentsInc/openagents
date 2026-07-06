# Cloud SQL Postgres instance + users.
#
# Passwords are NOT managed here (never put credentials in HCL/state on
# purpose). Users are tracked for existence only; rotate passwords with
# `gcloud sql users set-password`.

resource "google_sql_database_instance" "this" {
  project          = var.project
  name             = var.name
  region           = var.region
  database_version = var.database_version

  # Terraform-side guard: refuse to plan a destroy of the instance.
  deletion_protection = true

  settings {
    tier                        = var.tier
    edition                     = var.edition
    availability_type           = var.availability_type
    disk_size                   = var.disk_size_gb
    disk_type                   = var.disk_type
    disk_autoresize             = true
    disk_autoresize_limit       = 0
    activation_policy           = "ALWAYS"
    pricing_plan                = "PER_USE"
    deletion_protection_enabled = var.gcp_deletion_protection
    enable_dataplex_integration = var.enable_dataplex_integration

    backup_configuration {
      enabled                        = var.backups_enabled
      start_time                     = var.backup_start_time
      point_in_time_recovery_enabled = var.pitr_enabled
      transaction_log_retention_days = var.transaction_log_retention_days

      backup_retention_settings {
        retained_backups = var.retained_backups
        retention_unit   = "COUNT"
      }
    }

    dynamic "database_flags" {
      for_each = var.database_flags
      content {
        name  = database_flags.key
        value = database_flags.value
      }
    }

    ip_configuration {
      ipv4_enabled = true
      ssl_mode     = var.ssl_mode

      dynamic "authorized_networks" {
        for_each = var.authorized_networks
        content {
          name  = authorized_networks.value.name
          value = authorized_networks.value.value
        }
      }
    }

    dynamic "location_preference" {
      for_each = var.zone == null ? [] : [var.zone]
      content {
        zone           = location_preference.value
        secondary_zone = var.secondary_zone
      }
    }
  }
}

resource "google_sql_user" "this" {
  for_each = toset(var.users)

  project  = var.project
  instance = google_sql_database_instance.this.name
  name     = each.value

  lifecycle {
    # Passwords are set/rotated out of band; never diff on them.
    ignore_changes = [password, password_wo, password_wo_version]
  }
}

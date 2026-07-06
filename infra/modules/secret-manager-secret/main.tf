# Secret Manager secret CONTAINER ownership only.
#
# Deliberate split mirroring cloud-run-service: Terraform owns the secret's
# existence, replication, and accessor grants. Secret VERSIONS (the actual
# sensitive payload) are added out-of-band with
# `gcloud secrets versions add <name> --data-file=-` so the value never
# appears in committed HCL or in Terraform state.

resource "google_secret_manager_secret" "this" {
  project   = var.project
  secret_id = var.secret_id

  replication {
    auto {}
  }

  # Terraform-side guard: refuse to plan a destroy of the secret container
  # (losing it would break the Cloud Run services that mount it).
  lifecycle {
    prevent_destroy = true
  }
}

resource "google_secret_manager_secret_iam_member" "accessor" {
  for_each = toset(var.accessor_members)

  project   = var.project
  secret_id = google_secret_manager_secret.this.secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = each.value
}

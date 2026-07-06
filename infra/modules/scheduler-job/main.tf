resource "google_cloud_scheduler_job" "this" {
  project          = var.project
  name             = var.name
  region           = var.region
  description      = var.description
  schedule         = var.schedule
  time_zone        = var.time_zone
  attempt_deadline = var.attempt_deadline

  http_target {
    uri         = var.http_uri
    http_method = var.http_method

    dynamic "oidc_token" {
      for_each = var.oidc_service_account_email == null ? [] : [var.oidc_service_account_email]
      content {
        service_account_email = oidc_token.value
      }
    }
  }
}

# Cloud Run service SHELL ownership.
#
# Deliberate split: Terraform owns the service's existence, location, and
# ingress. Revisions (image, env, scaling, VPC access, probes) are deployed
# via `gcloud run deploy` / CI and are explicitly ignored here so that
# `terraform plan` stays a no-op across ordinary deploys. This also keeps
# runtime env values (including anything sensitive) out of committed HCL.

resource "google_cloud_run_v2_service" "this" {
  project  = var.project
  name     = var.name
  location = var.region
  ingress  = var.ingress

  # Terraform-side guard: refuse to plan a destroy of the service.
  deletion_protection = true

  template {
    containers {
      image = var.placeholder_image
    }
  }

  lifecycle {
    ignore_changes = [
      template,
      traffic,
      labels,
      annotations,
      client,
      client_version,
      build_config,
      scaling,
      launch_stage,
    ]
  }
}

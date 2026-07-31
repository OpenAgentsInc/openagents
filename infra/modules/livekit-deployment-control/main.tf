locals {
  service_account_id = "oa-livekit-prod-deployer"
  trigger_name       = "oa-livekit-prod-runtime"
  membership_name    = "oa-livekit-prod"
  receipt_bucket     = "${var.project_id}-livekit-deployment-receipts"
  boundary_digest = filesha256(
    "${path.root}/deployment-control-boundary.json"
  )
}

resource "google_service_account" "deployer" {
  project      = var.project_id
  account_id   = local.service_account_id
  display_name = "LiveKit production deployer"
  description  = "Dedicated identity for the fixed production LiveKit runtime trigger."
}

resource "google_service_account_iam_member" "cloud_build_service_agent" {
  service_account_id = google_service_account.deployer.name
  role               = "roles/iam.serviceAccountTokenCreator"
  member             = "serviceAccount:service-${var.project_number}@gcp-sa-cloudbuild.iam.gserviceaccount.com"
}

resource "google_project_iam_member" "gateway_roles" {
  for_each = toset([
    "roles/gkehub.gatewayAdmin",
    "roles/gkehub.viewer",
    "roles/cloudbuild.builds.viewer",
    "roles/container.clusterViewer",
    "roles/compute.viewer",
    "roles/iam.securityReviewer",
    "roles/redis.viewer",
    "roles/logging.logWriter",
  ])

  project = var.project_id
  role    = each.value
  member  = "serviceAccount:${google_service_account.deployer.email}"
}

resource "google_secret_manager_secret_iam_member" "preflight_reader" {
  for_each = var.managed_secret_ids

  project   = var.project_id
  secret_id = each.value
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.deployer.email}"
}

data "google_secret_manager_secret" "sarah_openai_source" {
  project   = var.project_id
  secret_id = "sarah-openai-api-key"
}

resource "google_secret_manager_secret_iam_member" "source_preflight_reader" {
  project   = var.project_id
  secret_id = data.google_secret_manager_secret.sarah_openai_source.secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.deployer.email}"
}

resource "google_storage_bucket" "receipts" {
  project                     = var.project_id
  name                        = local.receipt_bucket
  location                    = var.region
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"
  force_destroy               = false
  labels                      = var.labels

  versioning {
    enabled = true
  }

  retention_policy {
    retention_period = 2592000
    is_locked        = true
  }
}

resource "google_project_iam_custom_role" "receipt_object_writer" {
  project     = var.project_id
  role_id     = "livekitDeploymentReceiptWriter"
  title       = "LiveKit Deployment Receipt Writer"
  description = "Can create and verify only objects in the bound receipt bucket."
  permissions = [
    "storage.buckets.get",
    "storage.objects.create",
    "storage.objects.get",
    "storage.objects.list",
  ]
}

resource "google_storage_bucket_iam_member" "receipt_writer" {
  bucket = google_storage_bucket.receipts.name
  role   = google_project_iam_custom_role.receipt_object_writer.id
  member = "serviceAccount:${google_service_account.deployer.email}"
}

resource "google_gke_hub_membership" "production" {
  project       = var.project_id
  location      = "global"
  membership_id = local.membership_name

  endpoint {
    gke_cluster {
      resource_link = "//container.googleapis.com/${var.cluster_id}"
    }
  }
}

resource "google_cloudbuild_trigger" "production_runtime" {
  project         = var.project_id
  location        = var.region
  name            = local.trigger_name
  description     = "Fixed reviewed LiveKit production runtime deployment; callers cannot supply config or substitutions."
  service_account = google_service_account.deployer.id

  source_to_build {
    uri       = "https://github.com/OpenAgentsInc/openagents.git"
    ref       = "refs/heads/main"
    repo_type = "GITHUB"
  }

  build {
    step {
      name       = var.deployment_executor_image
      entrypoint = "node"
      args = [
        "scripts/cloud/livekit-gcp-ops.mjs",
        "--operation",
        "production-runtime-apply",
        "--bundle",
        "infra/livekit/bundle.json",
        "--receipt",
        "docs/ops/receipts/livekit/receipt.json",
        "--apply",
      ]
      env = [
        "BUILD_ID=$BUILD_ID",
        "COMMIT_SHA=$COMMIT_SHA",
        "OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST",
        "OA_LIVEKIT_EXECUTION_MODE=cloud_build_trigger",
        "OA_LIVEKIT_EXECUTION_CONFIG_DIGEST=${local.boundary_digest}",
        "KUBECONFIG=/workspace/.kubeconfig",
        "USE_GKE_GCLOUD_AUTH_PLUGIN=True",
      ]
    }

    artifacts {
      objects {
        location = "gs://${google_storage_bucket.receipts.name}/production-runtime/$BUILD_ID/"
        paths    = ["docs/ops/receipts/livekit/receipt.json"]
      }
    }

    options {
      logging = "CLOUD_LOGGING_ONLY"
    }

    timeout = "5400s"
  }

  depends_on = [
    google_gke_hub_membership.production,
    google_project_iam_member.gateway_roles,
    google_secret_manager_secret_iam_member.preflight_reader,
    google_secret_manager_secret_iam_member.source_preflight_reader,
    google_storage_bucket_iam_member.receipt_writer,
  ]
}

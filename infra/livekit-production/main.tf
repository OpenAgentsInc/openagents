locals {
  environment                = "production"
  automation_service_account = "oa-mvp-automation@${var.project_id}.iam.gserviceaccount.com"
  labels = {
    environment = local.environment
    program     = "ep263-livekit"
    service     = "livekit"
  }
}

resource "google_org_policy_policy" "cloudbuild_use_build_service_account" {
  name   = "projects/${var.project_id}/policies/cloudbuild.useBuildServiceAccount"
  parent = "projects/${var.project_id}"

  spec {
    rules {
      enforce = "FALSE"
    }
  }
}

resource "google_org_policy_policy" "cloudbuild_use_compute_service_account" {
  name   = "projects/${var.project_id}/policies/cloudbuild.useComputeServiceAccount"
  parent = "projects/${var.project_id}"

  spec {
    rules {
      enforce = "FALSE"
    }
  }
}

data "google_service_account" "default_compute" {
  project    = var.project_id
  account_id = "${var.project_number}-compute@developer.gserviceaccount.com"
}

resource "google_tags_tag_key" "privileged_service_account" {
  parent      = "projects/${var.project_number}"
  short_name  = "livekit-privileged-identity"
  description = "Identities that the legacy automation service account must never impersonate."
}

resource "google_tags_tag_value" "privileged_service_account" {
  parent      = google_tags_tag_key.privileged_service_account.id
  short_name  = "protected"
  description = "Protected LiveKit runtime, secret, node, or deployer service account."
}

data "google_artifact_registry_repository" "oa_cloud" {
  project       = var.project_id
  location      = var.region
  repository_id = "oa-cloud"
}

resource "google_service_account" "image_builder" {
  project      = var.project_id
  account_id   = "oa-livekit-image-builder"
  display_name = "LiveKit immutable image builder"
  description  = "Build-only identity for Sarah and production deployer images; no runtime or secret access."
}

resource "google_artifact_registry_repository_iam_member" "image_builder" {
  project    = data.google_artifact_registry_repository.oa_cloud.project
  location   = data.google_artifact_registry_repository.oa_cloud.location
  repository = data.google_artifact_registry_repository.oa_cloud.repository_id
  role       = "roles/artifactregistry.writer"
  member     = "serviceAccount:${google_service_account.image_builder.email}"
}

resource "google_project_iam_member" "image_builder_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.image_builder.email}"
}

resource "google_storage_bucket" "image_build_source" {
  project                     = var.project_id
  name                        = "${var.project_id}-livekit-build-source"
  location                    = var.region
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"
  force_destroy               = false
  labels                      = local.labels

  lifecycle_rule {
    action {
      type = "Delete"
    }
    condition {
      age = 7
    }
  }
}

resource "google_storage_bucket_iam_member" "image_builder_source_reader" {
  bucket = google_storage_bucket.image_build_source.name
  role   = "roles/storage.objectViewer"
  member = "serviceAccount:${google_service_account.image_builder.email}"
}

resource "google_service_account_iam_member" "cloud_build_image_builder" {
  service_account_id = google_service_account.image_builder.name
  role               = "roles/iam.serviceAccountTokenCreator"
  member             = "serviceAccount:service-${var.project_number}@gcp-sa-cloudbuild.iam.gserviceaccount.com"
}

resource "google_service_account_iam_member" "automation_image_builder" {
  service_account_id = google_service_account.image_builder.name
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${local.automation_service_account}"
}

module "network" {
  source = "../modules/livekit-network"

  project_id   = var.project_id
  region       = var.region
  environment  = local.environment
  network_name = "oa-livekit-prod"

  node_subnet_cidr = "10.80.0.0/20"
  pod_cidr         = "10.81.0.0/16"
  service_cidr     = "10.82.0.0/20"

  sfu_network_tag = "oa-livekit-prod-sfu"
  enable_turn_udp = var.enable_turn_udp
  labels          = local.labels
}

module "platform" {
  source = "../modules/livekit-gke"

  project_id   = var.project_id
  region       = var.region
  zones        = var.zones
  environment  = local.environment
  cluster_name = "oa-livekit-prod"

  network_id         = module.network.network_id
  subnetwork_id      = module.network.subnetwork_id
  pod_range_name     = module.network.pod_range_name
  service_range_name = module.network.service_range_name

  master_authorized_networks = var.master_authorized_networks
  deletion_protection        = true

  sfu_machine_type = "c2-standard-8"
  sfu_min_nodes    = 3
  sfu_max_nodes    = 7
  sfu_network_tag  = "oa-livekit-prod-sfu"

  app_machine_type = "e2-standard-8"
  app_min_nodes    = 3
  app_max_nodes    = 4

  redis_name           = "oa-livekit-redis"
  redis_memory_size_gb = 5
  redis_version        = "REDIS_7_2"

  namespace                              = "livekit-system"
  node_service_account_id                = "oa-livekit-prod-nodes"
  server_service_account_id              = "oa-livekit-server"
  agent_service_account_id               = "oa-livekit-agent"
  secret_reader_service_account_id       = "livekit-secret-reader"
  dns_secret_reader_service_account_id   = "oa-livekit-cert-manager-reader"
  sarah_secret_reader_service_account_id = "oa-livekit-sarah-secret-reader"

  labels = local.labels

  depends_on = [module.network]
}

module "observability" {
  source = "../modules/livekit-observability"

  project_id        = var.project_id
  project_number    = var.project_number
  environment       = local.environment
  cluster_name      = module.platform.cluster_name
  cluster_location  = module.platform.cluster_location
  redis_instance_id = "oa-livekit-redis"
  signal_hostname   = var.signal_hostname

  notification_channel_ids = var.notification_channel_ids
  billing_account_id       = var.billing_account_id
  monthly_budget_usd       = var.monthly_budget_usd
  max_rooms                = 20
  max_participants         = 60
  labels                   = local.labels
}

resource "google_service_account" "sarah_nostr_signer" {
  count = var.sarah_nostr_signer_image == null ? 0 : 1

  project      = var.project_id
  account_id   = "oa-sarah-nostr-signer"
  display_name = "Sarah Nostr signing boundary"
  description  = "Server-only principal.sarah signer; access is limited to the stable Nostr identity secret."
}

resource "google_secret_manager_secret_iam_member" "sarah_nostr_signer_identity" {
  count = var.sarah_nostr_signer_image == null ? 0 : 1

  project   = var.project_id
  secret_id = "sarah-nostr-identity-secret"
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.sarah_nostr_signer[0].email}"
}

resource "google_cloud_run_v2_service" "sarah_nostr_signer" {
  count = var.sarah_nostr_signer_image == null ? 0 : 1

  project             = var.project_id
  name                = "oa-sarah-nostr-signer"
  location            = var.region
  ingress             = "INGRESS_TRAFFIC_INTERNAL_ONLY"
  deletion_protection = true

  template {
    service_account                  = google_service_account.sarah_nostr_signer[0].email
    execution_environment            = "EXECUTION_ENVIRONMENT_GEN2"
    timeout                          = "15s"
    max_instance_request_concurrency = 20

    scaling {
      min_instance_count = 1
      max_instance_count = 2
    }

    containers {
      image = var.sarah_nostr_signer_image

      ports {
        container_port = 8080
      }

      env {
        name  = "SARAH_NOSTR_SIGNER_COMMUNITIES_JSON"
        value = jsonencode(var.sarah_nostr_signer_communities)
      }

      env {
        name  = "SARAH_NOSTR_EXPECTED_PUBKEY"
        value = var.sarah_nostr_signer_expected_pubkey
      }

      env {
        name = "SARAH_NOSTR_IDENTITY_SECRET"

        value_source {
          secret_key_ref {
            secret  = "sarah-nostr-identity-secret"
            version = "latest"
          }
        }
      }

      resources {
        limits = {
          cpu    = "1"
          memory = "512Mi"
        }
      }

      startup_probe {
        initial_delay_seconds = 0
        timeout_seconds       = 2
        period_seconds        = 5
        failure_threshold     = 12

        http_get {
          path = "/health"
        }
      }
    }
  }

  labels = merge(local.labels, { service = "sarah-nostr-signer" })

  depends_on = [google_secret_manager_secret_iam_member.sarah_nostr_signer_identity]
}

resource "google_cloud_run_v2_service_iam_member" "sarah_nostr_signer_agent_invoker" {
  count = var.sarah_nostr_signer_image == null ? 0 : 1

  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.sarah_nostr_signer[0].name
  role     = "roles/run.invoker"
  member   = "serviceAccount:${module.platform.agent_service_account_email}"
}

resource "terraform_data" "deployment_control_configuration" {
  count = var.enable_deployment_control ? 1 : 0

  input = {
    github_app_installation_id       = var.deployment_source_github_app_installation_id
    github_authorizer_secret_version = var.deployment_source_github_authorizer_secret_version
  }

  lifecycle {
    precondition {
      condition = (
        var.deployment_source_github_app_installation_id != null &&
        var.deployment_source_github_authorizer_secret_version != null
      )
      error_message = "Deployment control requires the Cloud Build GitHub App installation ID and immutable authorizer token secret version."
    }
  }
}

module "deployment_control" {
  count  = var.enable_deployment_control ? 1 : 0
  source = "../modules/livekit-deployment-control"

  project_id                = var.project_id
  project_number            = var.project_number
  organization_id           = var.organization_id
  region                    = var.region
  cluster_id                = module.platform.cluster_id
  managed_secret_ids        = module.platform.secret_ids
  deployment_executor_image = var.deployment_executor_image
  github_app_installation_id = coalesce(
    var.deployment_source_github_app_installation_id,
    1,
  )
  github_authorizer_token_secret_version = coalesce(
    var.deployment_source_github_authorizer_secret_version,
    "projects/${var.project_id}/secrets/disabled/versions/1",
  )
  labels = local.labels

  depends_on = [module.platform, terraform_data.deployment_control_configuration]
}

locals {
  protected_service_account_unique_ids = merge(
    {
      default_compute = data.google_service_account.default_compute.unique_id
    },
    module.platform.privileged_service_account_unique_ids,
    var.enable_deployment_control ? {
      production_deployer = module.deployment_control[0].service_account_unique_id
    } : {},
    var.sarah_nostr_signer_image == null ? {} : {
      sarah_nostr_signer = google_service_account.sarah_nostr_signer[0].unique_id
    },
  )
}

resource "google_tags_tag_binding" "privileged_service_account" {
  for_each = local.protected_service_account_unique_ids

  parent    = "//iam.googleapis.com/projects/${var.project_number}/serviceAccounts/${each.value}"
  tag_value = google_tags_tag_value.privileged_service_account.id
}

resource "google_iam_deny_policy" "legacy_automation_privileged_impersonation" {
  parent       = urlencode("cloudresourcemanager.googleapis.com/projects/${var.project_id}")
  name         = "deny-legacy-livekit-privileged-impersonation"
  display_name = "Block LiveKit privileged identity impersonation"

  rules {
    description = "Only service accounts tagged as protected are in scope; narrow build-only identities remain usable."

    deny_rule {
      denied_principals = [
        "principal://iam.googleapis.com/projects/-/serviceAccounts/${local.automation_service_account}",
      ]
      denied_permissions = [
        "iam.googleapis.com/serviceAccounts.actAs",
      ]

      denial_condition {
        title       = "Target is a protected LiveKit identity"
        description = "Resource-level service account tag keeps this deny from affecting unrelated project service accounts."
        expression  = "resource.matchTag('${var.project_id}/livekit-privileged-identity', 'protected')"
      }
    }
  }

  lifecycle {
    prevent_destroy = true
  }

  depends_on = [google_tags_tag_binding.privileged_service_account]
}

check "deployment_control_executor" {
  assert {
    condition     = !var.enable_deployment_control || var.deployment_executor_image != null
    error_message = "Enable deployment control only after the reviewed executor image is pinned by digest."
  }
}

check "turn_udp_rollout" {
  assert {
    condition     = !var.enable_turn_udp
    error_message = "TURN/UDP 443 is deferred. Admit its connectivity gate before changing this production root."
  }
}

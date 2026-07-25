# The Forge Git service keeps bare repositories on one Filestore share.
# Cloud Run mounts NFS without network file locks. One service instance keeps
# the stock Git ref-lock protocol inside one write authority.

resource "google_filestore_instance" "repositories" {
  project  = var.project
  name     = var.filestore_name
  location = var.filestore_zone
  tier     = "BASIC_HDD"

  file_shares {
    capacity_gb = var.capacity_gb
    name        = var.file_share_name
  }

  networks {
    network = var.network
    modes   = ["MODE_IPV4"]
  }

  labels = var.labels

  lifecycle {
    prevent_destroy = true
  }
}

resource "google_service_account" "runtime" {
  project      = var.project
  account_id   = var.runtime_service_account_id
  display_name = "OpenAgents Forge Git runtime"
  description  = "Runtime identity for the owned Smart HTTP Git service."
}

# GCS keeps pack evidence and mirrors. It does not keep authoritative refs.
resource "google_storage_bucket_iam_member" "pack_evidence_writer" {
  bucket = var.pack_evidence_bucket
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.runtime.email}"
}

resource "google_secret_manager_secret_iam_member" "database_reader" {
  project   = var.project
  secret_id = var.database_secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.runtime.email}"
}

resource "google_compute_firewall" "filestore_egress" {
  project   = var.project
  name      = "${var.service_name}-filestore-egress"
  network   = var.network
  direction = "EGRESS"
  priority  = 900

  destination_ranges = [
    "${google_filestore_instance.repositories.networks[0].ip_addresses[0]}/32",
  ]
  target_tags = [var.network_tag]

  allow {
    protocol = "tcp"
    ports    = ["111", "2046", "2049", "2050", "4045"]
  }
}

resource "google_cloud_run_v2_service" "this" {
  project  = var.project
  name     = var.service_name
  location = var.region

  # The global external load balancer can reach the service. The direct
  # run.app endpoint cannot accept external traffic.
  ingress             = "INGRESS_TRAFFIC_INTERNAL_LOAD_BALANCER"
  deletion_protection = true

  template {
    service_account                  = google_service_account.runtime.email
    execution_environment            = "EXECUTION_ENVIRONMENT_GEN2"
    timeout                          = var.request_timeout
    max_instance_request_concurrency = var.request_concurrency

    scaling {
      min_instance_count = 0
      max_instance_count = 1
    }

    containers {
      image = var.placeholder_image

      ports {
        container_port = 8080
      }

      env {
        name  = "FORGE_GIT_REPOSITORY_ROOT"
        value = var.repository_mount_path
      }

      env {
        name  = "OA_INFRA_GCS_BUCKET"
        value = var.pack_evidence_bucket
      }

      env {
        name  = "OA_INFRA_GCS_PREFIX"
        value = var.pack_evidence_prefix
      }

      volume_mounts {
        name       = "forge-repositories"
        mount_path = var.repository_mount_path
      }
    }

    vpc_access {
      egress = "PRIVATE_RANGES_ONLY"

      network_interfaces {
        network    = var.network
        subnetwork = var.subnetwork
        tags       = [var.network_tag]
      }
    }

    volumes {
      name = "forge-repositories"

      nfs {
        server    = google_filestore_instance.repositories.networks[0].ip_addresses[0]
        path      = "/${var.file_share_name}"
        read_only = false
      }
    }
  }

  labels = var.labels

  # Terraform owns the store mount, Direct VPC egress, service identity, and
  # single-instance limit. The deploy command owns runtime image and process
  # configuration.
  lifecycle {
    ignore_changes = [
      template[0].containers[0].args,
      template[0].containers[0].command,
      template[0].containers[0].env,
      template[0].containers[0].image,
      template[0].containers[0].liveness_probe,
      template[0].containers[0].resources,
      template[0].containers[0].startup_probe,
      traffic,
      client,
      client_version,
      build_config,
    ]
  }

  depends_on = [google_compute_firewall.filestore_egress]
}

# Smart HTTP uses application bearer or NIP-98 authentication. The load
# balancer must invoke the application before the Effect auth boundary runs.
resource "google_cloud_run_v2_service_iam_member" "load_balancer_invoker" {
  project  = var.project
  location = var.region
  name     = google_cloud_run_v2_service.this.name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

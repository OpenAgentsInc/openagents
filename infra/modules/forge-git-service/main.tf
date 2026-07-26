# FORGE-02 keeps all bare repositories on one persistent disk. One small GCE
# host exports that disk to the one Cloud Run Git service over a dedicated
# Direct VPC subnet. GCS keeps pack evidence and mirrors, never refs.

resource "google_compute_subnetwork" "forge_git" {
  project                  = var.project
  name                     = var.subnetwork_name
  region                   = var.region
  network                  = var.network
  ip_cidr_range            = var.subnetwork_cidr
  private_ip_google_access = true

  lifecycle {
    prevent_destroy = true
  }
}

# The first bootstrap created this empty router before Google Cloud reported
# that one existing regional NAT already covers all default-VPC subnets.
# The empty router has no NAT or recurring cost.
resource "google_compute_router" "forge_git" {
  project = var.project
  name    = "${var.service_name}-router"
  region  = var.region
  network = var.network
}

resource "google_service_account" "nfs" {
  project      = var.project
  account_id   = var.nfs_service_account_id
  display_name = "OpenAgents Forge Git NFS"
  description  = "Service identity for the owned Forge Git NFS host."
}

data "google_compute_image" "nfs" {
  project = "debian-cloud"
  family  = "debian-12"
}

resource "google_compute_disk" "repositories" {
  project = var.project
  name    = var.repository_disk_name
  zone    = var.zone
  type    = "pd-balanced"
  size    = var.repository_disk_size_gb

  labels = var.labels

  lifecycle {
    prevent_destroy = true
  }
}

resource "google_compute_resource_policy" "repository_snapshots" {
  project = var.project
  name    = "${var.repository_disk_name}-daily"
  region  = var.region

  snapshot_schedule_policy {
    schedule {
      daily_schedule {
        days_in_cycle = 1
        start_time    = var.snapshot_start_time
      }
    }

    retention_policy {
      max_retention_days    = var.snapshot_retention_days
      on_source_disk_delete = "KEEP_AUTO_SNAPSHOTS"
    }

    snapshot_properties {
      storage_locations = [var.region]
      labels            = var.labels
    }
  }
}

resource "google_compute_disk_resource_policy_attachment" "repository_snapshots" {
  project = var.project
  name    = google_compute_resource_policy.repository_snapshots.name
  disk    = google_compute_disk.repositories.name
  zone    = var.zone
}

resource "google_compute_instance" "nfs" {
  project      = var.project
  name         = var.nfs_instance_name
  zone         = var.zone
  machine_type = var.nfs_machine_type

  allow_stopping_for_update = true
  can_ip_forward            = false
  deletion_protection       = true

  tags = [var.nfs_network_tag]

  boot_disk {
    initialize_params {
      image = data.google_compute_image.nfs.self_link
      size  = 10
      type  = "pd-balanced"
    }
  }

  attached_disk {
    source      = google_compute_disk.repositories.id
    device_name = var.repository_disk_name
    mode        = "READ_WRITE"
  }

  network_interface {
    network    = var.network
    subnetwork = google_compute_subnetwork.forge_git.id
    network_ip = cidrhost(var.subnetwork_cidr, 2)
  }

  service_account {
    email  = google_service_account.nfs.email
    scopes = ["https://www.googleapis.com/auth/logging.write"]
  }

  metadata = {
    block-project-ssh-keys = "TRUE"
    enable-oslogin         = "TRUE"
    repository-disk-name   = var.repository_disk_name
    export-path            = var.nfs_export_path
    allowed-cidr           = var.subnetwork_cidr
  }

  metadata_startup_script = file("${path.module}/nfs-startup.sh")

  shielded_instance_config {
    enable_secure_boot          = true
    enable_vtpm                 = true
    enable_integrity_monitoring = true
  }

  labels = var.labels

  depends_on = [
    google_compute_subnetwork.forge_git,
    google_compute_firewall.nfs,
    google_compute_firewall.nfs_iap_ssh,
    google_compute_firewall.nfs_deny_other_ingress,
  ]
}

# Only clients in the dedicated Cloud Run Direct VPC range can mount NFS.
resource "google_compute_firewall" "nfs" {
  project   = var.project
  name      = "${var.service_name}-nfs-ingress"
  network   = var.network
  direction = "INGRESS"
  priority  = 900

  source_ranges = [var.subnetwork_cidr]
  target_tags   = [var.nfs_network_tag]

  allow {
    protocol = "tcp"
    ports    = ["111", "2049", "20048", "32765", "32766"]
  }

  allow {
    protocol = "udp"
    ports    = ["111", "2049", "20048", "32765", "32766"]
  }

  log_config {
    metadata = "INCLUDE_ALL_METADATA"
  }
}

# IAP is the only SSH path. The VM has no external address.
resource "google_compute_firewall" "nfs_iap_ssh" {
  project   = var.project
  name      = "${var.service_name}-nfs-iap-ssh"
  network   = var.network
  direction = "INGRESS"
  priority  = 900

  source_ranges = ["35.235.240.0/20"]
  target_tags   = [var.nfs_network_tag]

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }
}

# Override broad legacy VPC allow rules for this tagged host. The two priority
# 900 rules above are the only admitted ingress paths.
resource "google_compute_firewall" "nfs_deny_other_ingress" {
  project   = var.project
  name      = "${var.service_name}-nfs-deny-other-ingress"
  network   = var.network
  direction = "INGRESS"
  priority  = 910

  source_ranges = ["0.0.0.0/0"]
  target_tags   = [var.nfs_network_tag]

  deny {
    protocol = "all"
  }

  log_config {
    metadata = "INCLUDE_ALL_METADATA"
  }
}

resource "google_service_account" "runtime" {
  project      = var.project
  account_id   = var.runtime_service_account_id
  display_name = "OpenAgents Forge Git runtime"
  description  = "Runtime identity for the owned Smart HTTP Git service."
}

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

resource "google_secret_manager_secret_iam_member" "policy_authority_reader" {
  project   = var.project
  secret_id = var.policy_authority_secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.runtime.email}"
}

resource "google_project_iam_member" "database_client" {
  project = var.project
  role    = "roles/cloudsql.client"
  member  = "serviceAccount:${google_service_account.runtime.email}"
}

resource "google_cloud_run_v2_service" "this" {
  project  = var.project
  name     = var.service_name
  location = var.region

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

      volume_mounts {
        name       = "cloudsql"
        mount_path = "/cloudsql"
      }
    }

    vpc_access {
      egress = "PRIVATE_RANGES_ONLY"

      network_interfaces {
        network    = var.network
        subnetwork = google_compute_subnetwork.forge_git.id
        tags       = [var.cloud_run_network_tag]
      }
    }

    volumes {
      name = "forge-repositories"

      nfs {
        server    = google_compute_instance.nfs.network_interface[0].network_ip
        path      = var.nfs_export_path
        read_only = false
      }
    }

    volumes {
      name = "cloudsql"

      cloud_sql_instance {
        instances = [var.database_instance_connection_name]
      }
    }
  }

  labels = var.labels

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

  depends_on = [google_compute_firewall.nfs]
}

# Smart HTTP does membership authentication inside the Effect application.
resource "google_cloud_run_v2_service_iam_member" "load_balancer_invoker" {
  project  = var.project
  location = var.region
  name     = google_cloud_run_v2_service.this.name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

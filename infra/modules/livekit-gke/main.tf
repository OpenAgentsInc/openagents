locals {
  resource_labels = merge(var.labels, {
    environment = var.environment
    managed-by  = "opentofu"
    service     = "livekit"
  })

  server_workload_identity_member              = "serviceAccount:${var.project_id}.svc.id.goog[${var.namespace}/${var.server_ksa_name}]"
  agent_workload_identity_member               = "serviceAccount:${var.project_id}.svc.id.goog[${var.namespace}/${var.agent_ksa_name}]"
  secret_reader_workload_identity_member       = "serviceAccount:${var.project_id}.svc.id.goog[${var.namespace}/${var.secret_reader_ksa_name}]"
  dns_secret_reader_workload_identity_member   = "serviceAccount:${var.project_id}.svc.id.goog[cert-manager/${var.dns_secret_reader_ksa_name}]"
  sarah_secret_reader_workload_identity_member = "serviceAccount:${var.project_id}.svc.id.goog[${var.namespace}/${var.sarah_secret_reader_ksa_name}]"
}

resource "google_service_account" "nodes" {
  project      = var.project_id
  account_id   = var.node_service_account_id
  display_name = "${var.cluster_name} GKE nodes"
  description  = "Least-privilege node identity for the isolated LiveKit GKE cluster."
}

resource "google_project_iam_member" "node_roles" {
  for_each = toset([
    "roles/artifactregistry.reader",
    "roles/logging.logWriter",
    "roles/monitoring.metricWriter",
    "roles/monitoring.viewer",
    "roles/stackdriver.resourceMetadata.writer",
  ])

  project = var.project_id
  role    = each.value
  member  = "serviceAccount:${google_service_account.nodes.email}"
}

resource "google_container_cluster" "livekit" {
  project  = var.project_id
  name     = var.cluster_name
  location = var.region

  node_locations           = var.zones
  network                  = var.network_id
  subnetwork               = var.subnetwork_id
  networking_mode          = "VPC_NATIVE"
  remove_default_node_pool = true
  initial_node_count       = 1
  deletion_protection      = var.deletion_protection

  release_channel {
    channel = var.release_channel
  }

  workload_identity_config {
    workload_pool = "${var.project_id}.svc.id.goog"
  }

  ip_allocation_policy {
    cluster_secondary_range_name  = var.pod_range_name
    services_secondary_range_name = var.service_range_name
  }

  private_cluster_config {
    enable_private_endpoint = false
    enable_private_nodes    = false
  }

  master_authorized_networks_config {
    dynamic "cidr_blocks" {
      for_each = var.master_authorized_networks
      content {
        cidr_block   = cidr_blocks.value.cidr_block
        display_name = cidr_blocks.value.display_name
      }
    }
  }

  addons_config {
    dns_cache_config {
      enabled = true
    }

    gce_persistent_disk_csi_driver_config {
      enabled = true
    }

    gcp_filestore_csi_driver_config {
      enabled = false
    }

    horizontal_pod_autoscaling {
      disabled = false
    }

    http_load_balancing {
      disabled = false
    }

    network_policy_config {
      disabled = false
    }
  }

  network_policy {
    enabled  = true
    provider = "CALICO"
  }

  logging_config {
    enable_components = [
      "APISERVER",
      "CONTROLLER_MANAGER",
      "SCHEDULER",
      "SYSTEM_COMPONENTS",
      "WORKLOADS",
    ]
  }

  monitoring_config {
    enable_components = [
      "APISERVER",
      "CONTROLLER_MANAGER",
      "DAEMONSET",
      "DEPLOYMENT",
      "HPA",
      "POD",
      "SCHEDULER",
      "STATEFULSET",
      "STORAGE",
      "SYSTEM_COMPONENTS",
    ]

    managed_prometheus {
      enabled = true
    }
  }

  maintenance_policy {
    recurring_window {
      start_time = "2026-01-04T08:00:00Z"
      end_time   = "2026-01-04T12:00:00Z"
      recurrence = "FREQ=WEEKLY;BYDAY=SU"
    }
  }

  resource_labels = local.resource_labels

  lifecycle {
    precondition {
      condition     = var.sfu_min_nodes >= length(var.zones)
      error_message = "The SFU pool minimum must allow at least one SFU node in every selected zone."
    }

    precondition {
      condition     = var.app_min_nodes >= length(var.zones)
      error_message = "The app pool minimum must allow at least one app node in every selected zone."
    }
  }
}

resource "google_container_node_pool" "sfu" {
  project  = var.project_id
  name     = "${var.cluster_name}-sfu"
  location = var.region
  cluster  = google_container_cluster.livekit.name

  node_locations     = var.zones
  initial_node_count = 1

  autoscaling {
    total_min_node_count = var.sfu_min_nodes
    total_max_node_count = var.sfu_max_nodes
    location_policy      = "BALANCED"
  }

  management {
    auto_repair  = true
    auto_upgrade = true
  }

  upgrade_settings {
    strategy        = "SURGE"
    max_surge       = 1
    max_unavailable = 0
  }

  node_config {
    machine_type    = var.sfu_machine_type
    disk_type       = "pd-balanced"
    disk_size_gb    = var.sfu_disk_size_gb
    image_type      = "COS_CONTAINERD"
    service_account = google_service_account.nodes.email
    oauth_scopes    = ["https://www.googleapis.com/auth/cloud-platform"]
    tags            = [var.sfu_network_tag]
    resource_labels = local.resource_labels

    labels = merge(local.resource_labels, {
      "openagents.com/livekit-workload" = "sfu"
    })

    taint {
      key    = "openagents.com/livekit-workload"
      value  = "sfu"
      effect = "NO_SCHEDULE"
    }

    workload_metadata_config {
      mode = "GKE_METADATA"
    }

    shielded_instance_config {
      enable_integrity_monitoring = true
      enable_secure_boot          = true
    }

    metadata = {
      disable-legacy-endpoints = "true"
    }
  }

  lifecycle {
    precondition {
      condition     = var.sfu_max_nodes >= var.sfu_min_nodes
      error_message = "The SFU maximum node count must be at least the minimum."
    }
  }
}

resource "google_container_node_pool" "app" {
  project  = var.project_id
  name     = "${var.cluster_name}-app"
  location = var.region
  cluster  = google_container_cluster.livekit.name

  node_locations     = var.zones
  initial_node_count = 1

  autoscaling {
    total_min_node_count = var.app_min_nodes
    total_max_node_count = var.app_max_nodes
    location_policy      = "BALANCED"
  }

  management {
    auto_repair  = true
    auto_upgrade = true
  }

  upgrade_settings {
    strategy        = "SURGE"
    max_surge       = 1
    max_unavailable = 0
  }

  node_config {
    machine_type    = var.app_machine_type
    disk_type       = "pd-balanced"
    disk_size_gb    = var.app_disk_size_gb
    image_type      = "COS_CONTAINERD"
    service_account = google_service_account.nodes.email
    oauth_scopes    = ["https://www.googleapis.com/auth/cloud-platform"]
    tags            = ["livekit-app"]
    resource_labels = local.resource_labels

    labels = merge(local.resource_labels, {
      "openagents.com/livekit-workload" = "app"
    })

    workload_metadata_config {
      mode = "GKE_METADATA"
    }

    shielded_instance_config {
      enable_integrity_monitoring = true
      enable_secure_boot          = true
    }

    metadata = {
      disable-legacy-endpoints = "true"
    }
  }

  lifecycle {
    precondition {
      condition     = var.app_max_nodes >= var.app_min_nodes
      error_message = "The app maximum node count must be at least the minimum."
    }
  }
}

resource "google_compute_global_address" "signal" {
  project      = var.project_id
  name         = "${var.cluster_name}-signal"
  address_type = "EXTERNAL"
  description  = "Reserved Google Cloud address for the LiveKit signaling endpoint."
  labels       = local.resource_labels

  lifecycle {
    prevent_destroy = true
  }
}

resource "google_compute_address" "turn" {
  project      = var.project_id
  name         = "${var.cluster_name}-turn"
  region       = var.region
  address_type = "EXTERNAL"
  network_tier = "PREMIUM"
  description  = "Reserved Google Cloud address for the LiveKit TURN endpoint."
  labels       = local.resource_labels

  lifecycle {
    prevent_destroy = true
  }
}

resource "google_redis_instance" "livekit" {
  project            = var.project_id
  name               = var.redis_name
  display_name       = "${var.cluster_name} HA Redis"
  region             = var.region
  tier               = "STANDARD_HA"
  memory_size_gb     = var.redis_memory_size_gb
  redis_version      = var.redis_version
  authorized_network = var.network_id
  connect_mode       = "PRIVATE_SERVICE_ACCESS"

  location_id             = var.zones[0]
  alternative_location_id = var.zones[1]

  transit_encryption_mode = "SERVER_AUTHENTICATION"
  auth_enabled            = false

  maintenance_policy {
    weekly_maintenance_window {
      day = "SUNDAY"
      start_time {
        hours   = 8
        minutes = 0
        seconds = 0
        nanos   = 0
      }
    }
  }

  labels = local.resource_labels

  lifecycle {
    prevent_destroy = true
  }
}

resource "google_compute_firewall" "redis_allow_sfu_pool" {
  project     = var.project_id
  name        = "${var.cluster_name}-redis-allow-sfu"
  network     = var.network_id
  direction   = "EGRESS"
  priority    = 700
  target_tags = [var.sfu_network_tag]

  destination_ranges = ["${google_redis_instance.livekit.host}/32"]

  allow {
    protocol = "tcp"
    ports    = ["6378"]
  }

  log_config {
    metadata = "INCLUDE_ALL_METADATA"
  }
}

resource "google_compute_firewall" "redis_deny_non_sfu" {
  project   = var.project_id
  name      = "${var.cluster_name}-redis-deny-non-sfu"
  network   = var.network_id
  direction = "EGRESS"
  priority  = 800

  destination_ranges = ["${google_redis_instance.livekit.host}/32"]

  deny {
    protocol = "tcp"
    ports    = ["6378"]
  }

  log_config {
    metadata = "INCLUDE_ALL_METADATA"
  }
}

resource "google_service_account" "server" {
  project      = var.project_id
  account_id   = var.server_service_account_id
  display_name = "${var.cluster_name} LiveKit server"
  description  = "Workload Identity target for LiveKit server pods."
}

resource "google_service_account" "agent" {
  project      = var.project_id
  account_id   = var.agent_service_account_id
  display_name = "${var.cluster_name} Sarah agent"
  description  = "Workload Identity target for Sarah LiveKit agent pods."
}

resource "google_service_account" "secret_reader" {
  project      = var.project_id
  account_id   = var.secret_reader_service_account_id
  display_name = "${var.cluster_name} LiveKit secret reader"
  description  = "External Secrets identity limited to LiveKit server keys and Redis auth."
}

resource "google_service_account" "dns_secret_reader" {
  project      = var.project_id
  account_id   = var.dns_secret_reader_service_account_id
  display_name = "${var.cluster_name} DNS secret reader"
  description  = "External Secrets identity limited to the cert-manager Cloudflare DNS token."
}

resource "google_service_account" "sarah_secret_reader" {
  project      = var.project_id
  account_id   = var.sarah_secret_reader_service_account_id
  display_name = "${var.cluster_name} Sarah secret reader"
  description  = "External Secrets identity limited to Sarah's LiveKit server keys, OpenAI API key, and control root."
}

resource "google_service_account_iam_member" "server_workload_identity" {
  service_account_id = google_service_account.server.name
  role               = "roles/iam.workloadIdentityUser"
  member             = local.server_workload_identity_member

  depends_on = [google_container_cluster.livekit]
}

resource "google_service_account_iam_member" "agent_workload_identity" {
  service_account_id = google_service_account.agent.name
  role               = "roles/iam.workloadIdentityUser"
  member             = local.agent_workload_identity_member

  depends_on = [google_container_cluster.livekit]
}

resource "google_service_account_iam_member" "secret_reader_workload_identity" {
  service_account_id = google_service_account.secret_reader.name
  role               = "roles/iam.workloadIdentityUser"
  member             = local.secret_reader_workload_identity_member

  depends_on = [google_container_cluster.livekit]
}

resource "google_service_account_iam_member" "dns_secret_reader_workload_identity" {
  service_account_id = google_service_account.dns_secret_reader.name
  role               = "roles/iam.workloadIdentityUser"
  member             = local.dns_secret_reader_workload_identity_member

  depends_on = [google_container_cluster.livekit]
}

resource "google_service_account_iam_member" "sarah_secret_reader_workload_identity" {
  service_account_id = google_service_account.sarah_secret_reader.name
  role               = "roles/iam.workloadIdentityUser"
  member             = local.sarah_secret_reader_workload_identity_member

  depends_on = [google_container_cluster.livekit]
}

resource "google_secret_manager_secret" "server_keys" {
  project             = var.project_id
  secret_id           = "${var.cluster_name}-server-keys"
  deletion_protection = var.secret_deletion_protection

  replication {
    user_managed {
      replicas {
        location = var.region
      }
    }
  }

  labels = local.resource_labels
}

resource "google_secret_manager_secret" "openai_api_key" {
  project             = var.project_id
  secret_id           = "${var.cluster_name}-openai-api-key"
  deletion_protection = var.secret_deletion_protection

  replication {
    user_managed {
      replicas {
        location = var.region
      }
    }
  }

  labels = local.resource_labels
}

resource "google_secret_manager_secret" "sarah_control_root" {
  project             = var.project_id
  secret_id           = "${var.cluster_name}-sarah-control-root"
  deletion_protection = var.secret_deletion_protection

  replication {
    user_managed {
      replicas {
        location = var.region
      }
    }
  }

  labels = local.resource_labels
}

resource "google_secret_manager_secret" "redis_auth" {
  project             = var.project_id
  secret_id           = "${var.cluster_name}-redis-auth"
  deletion_protection = var.secret_deletion_protection

  replication {
    user_managed {
      replicas {
        location = var.region
      }
    }
  }

  labels = local.resource_labels
}

resource "google_secret_manager_secret" "cloudflare_dns" {
  project             = var.project_id
  secret_id           = "${var.cluster_name}-cloudflare-dns"
  deletion_protection = var.secret_deletion_protection

  replication {
    user_managed {
      replicas {
        location = var.region
      }
    }
  }

  labels = local.resource_labels
}

resource "google_secret_manager_secret_iam_member" "livekit_secret_reader" {
  for_each = {
    server_keys = google_secret_manager_secret.server_keys.secret_id
    redis_auth  = google_secret_manager_secret.redis_auth.secret_id
  }

  project   = var.project_id
  secret_id = each.value
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.secret_reader.email}"
}

resource "google_secret_manager_secret_iam_member" "dns_secret_reader" {
  project   = var.project_id
  secret_id = google_secret_manager_secret.cloudflare_dns.secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.dns_secret_reader.email}"
}

resource "google_secret_manager_secret_iam_member" "sarah_secret_reader" {
  for_each = {
    server_keys        = google_secret_manager_secret.server_keys.secret_id
    openai_api_key     = google_secret_manager_secret.openai_api_key.secret_id
    sarah_control_root = google_secret_manager_secret.sarah_control_root.secret_id
  }

  project   = var.project_id
  secret_id = each.value
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.sarah_secret_reader.email}"
}

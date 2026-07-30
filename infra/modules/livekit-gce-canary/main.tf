locals {
  service_account_id = substr(replace("${var.name}-runtime", "_", "-"), 0, 30)
  network_tag        = "${var.name}-media"
  resource_labels = merge(var.labels, {
    environment           = "staging"
    managed-by            = "opentofu"
    openagents-component  = "livekit-canary"
    openagents-expires-at = var.expires_at_unix == null ? "disabled" : tostring(var.expires_at_unix)
    service               = "livekit-canary"
  })
}

resource "google_service_account" "canary" {
  project      = var.project_id
  account_id   = local.service_account_id
  display_name = "${var.name} runtime"
  description  = "Disposable LiveKit canary identity with access only to its secret containers."
}

resource "google_secret_manager_secret" "api_key" {
  project   = var.project_id
  secret_id = "${var.name}-api-key"

  replication {
    user_managed {
      replicas {
        location = var.region
      }
    }
  }

  labels = local.resource_labels
}

resource "google_secret_manager_secret" "api_secret" {
  project   = var.project_id
  secret_id = "${var.name}-api-secret"

  replication {
    user_managed {
      replicas {
        location = var.region
      }
    }
  }

  labels = local.resource_labels
}

resource "google_secret_manager_secret" "tls_certificate" {
  project   = var.project_id
  secret_id = "${var.name}-tls-certificate"

  replication {
    user_managed {
      replicas {
        location = var.region
      }
    }
  }

  labels = local.resource_labels
}

resource "google_secret_manager_secret" "tls_private_key" {
  project   = var.project_id
  secret_id = "${var.name}-tls-private-key"

  replication {
    user_managed {
      replicas {
        location = var.region
      }
    }
  }

  labels = local.resource_labels
}

resource "google_secret_manager_secret_iam_member" "canary" {
  for_each = {
    api_key         = google_secret_manager_secret.api_key.secret_id
    api_secret      = google_secret_manager_secret.api_secret.secret_id
    tls_certificate = google_secret_manager_secret.tls_certificate.secret_id
    tls_private_key = google_secret_manager_secret.tls_private_key.secret_id
  }

  project   = var.project_id
  secret_id = each.value
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.canary.email}"
}

resource "google_compute_address" "canary" {
  project      = var.project_id
  name         = var.name
  region       = var.region
  address_type = "EXTERNAL"
  network_tier = "PREMIUM"
  description  = "Disposable LiveKit connectivity canary address."
  labels       = local.resource_labels
}

resource "google_compute_firewall" "canary" {
  project   = var.project_id
  name      = "${var.name}-media"
  network   = var.network_id
  direction = "INGRESS"
  priority  = 900

  source_ranges = ["0.0.0.0/0"]
  target_tags   = [local.network_tag]

  allow {
    protocol = "tcp"
    ports = [
      "443",
      tostring(var.tcp_fallback_port),
      tostring(var.turn_tls_port),
    ]
  }

  allow {
    protocol = "udp"
    ports = concat(
      ["${var.media_udp_port_range.start}-${var.media_udp_port_range.end}"],
      var.enable_turn_udp ? [tostring(var.turn_udp_port)] : [],
    )
  }

  log_config {
    metadata = "INCLUDE_ALL_METADATA"
  }
}

resource "google_compute_firewall" "iap_ssh" {
  project   = var.project_id
  name      = "${var.name}-iap-ssh"
  network   = var.network_id
  direction = "INGRESS"
  priority  = 900

  source_ranges = ["35.235.240.0/20"]
  target_tags   = [local.network_tag]

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }

  log_config {
    metadata = "INCLUDE_ALL_METADATA"
  }
}

resource "google_compute_instance" "canary" {
  count = var.enable_instance ? 1 : 0

  project      = var.project_id
  name         = var.name
  zone         = var.zone
  machine_type = var.machine_type

  can_ip_forward            = false
  deletion_protection       = false
  allow_stopping_for_update = true
  tags                      = [local.network_tag]
  labels                    = local.resource_labels

  boot_disk {
    auto_delete = true
    initialize_params {
      image = var.boot_image
      size  = 50
      type  = "pd-balanced"
    }
  }

  network_interface {
    subnetwork = var.subnetwork_id
    access_config {
      nat_ip       = google_compute_address.canary.address
      network_tier = "PREMIUM"
    }
  }

  service_account {
    email  = google_service_account.canary.email
    scopes = ["https://www.googleapis.com/auth/cloud-platform"]
  }

  metadata = {
    block-project-ssh-keys = "true"
    enable-oslogin         = "true"
  }

  metadata_startup_script = templatefile("${path.module}/startup.sh.tftpl", {
    project_id                = var.project_id
    livekit_server_image      = var.livekit_server_image
    reverse_proxy_image       = var.reverse_proxy_image
    api_key_secret_id         = google_secret_manager_secret.api_key.secret_id
    api_secret_secret_id      = google_secret_manager_secret.api_secret.secret_id
    tls_certificate_secret_id = google_secret_manager_secret.tls_certificate.secret_id
    tls_private_key_secret_id = google_secret_manager_secret.tls_private_key.secret_id
    livekit_config_base64     = base64encode(var.livekit_config)
  })

  scheduling {
    automatic_restart   = false
    on_host_maintenance = "TERMINATE"
    provisioning_model  = "STANDARD"

    max_run_duration {
      seconds = var.max_run_duration_seconds
    }

    instance_termination_action = "DELETE"
  }

  shielded_instance_config {
    enable_integrity_monitoring = true
    enable_secure_boot          = true
    enable_vtpm                 = true
  }

  depends_on = [google_secret_manager_secret_iam_member.canary]

  lifecycle {
    precondition {
      condition     = var.expires_at_unix != null
      error_message = "An enabled canary requires an explicit Unix expiry label; the operator gate verifies its 30-minute to 6-hour window."
    }
  }
}

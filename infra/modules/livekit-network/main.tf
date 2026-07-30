resource "google_compute_network" "livekit" {
  project                 = var.project_id
  name                    = var.network_name
  auto_create_subnetworks = false
  routing_mode            = "REGIONAL"
  mtu                     = 1460
}

resource "google_compute_subnetwork" "nodes" {
  project                  = var.project_id
  name                     = "${var.network_name}-nodes"
  region                   = var.region
  network                  = google_compute_network.livekit.id
  ip_cidr_range            = var.node_subnet_cidr
  private_ip_google_access = true

  secondary_ip_range {
    range_name    = "${var.network_name}-pods"
    ip_cidr_range = var.pod_cidr
  }

  secondary_ip_range {
    range_name    = "${var.network_name}-services"
    ip_cidr_range = var.service_cidr
  }

  log_config {
    aggregation_interval = "INTERVAL_5_SEC"
    flow_sampling        = 0.5
    metadata             = "INCLUDE_ALL_METADATA"
  }
}

resource "google_compute_global_address" "service_networking" {
  count = var.enable_private_service_access ? 1 : 0

  project       = var.project_id
  name          = "${var.network_name}-service-networking"
  purpose       = "VPC_PEERING"
  address_type  = "INTERNAL"
  prefix_length = var.service_networking_prefix_length
  network       = google_compute_network.livekit.id
  labels        = var.labels
}

resource "google_service_networking_connection" "service_networking" {
  count = var.enable_private_service_access ? 1 : 0

  network                 = google_compute_network.livekit.id
  service                 = "servicenetworking.googleapis.com"
  reserved_peering_ranges = [google_compute_global_address.service_networking[0].name]
}

resource "google_compute_firewall" "sfu_direct_media" {
  count = var.enable_sfu_firewalls ? 1 : 0

  project   = var.project_id
  name      = "${var.network_name}-sfu-direct-media"
  network   = google_compute_network.livekit.name
  direction = "INGRESS"
  priority  = 900

  source_ranges = ["0.0.0.0/0"]
  target_tags   = [var.sfu_network_tag]

  allow {
    protocol = "udp"
    ports = concat(
      ["${var.media_udp_port_range.start}-${var.media_udp_port_range.end}"],
      var.enable_turn_udp ? [tostring(var.turn_udp_port)] : [],
    )
  }

  allow {
    protocol = "tcp"
    ports = [
      tostring(var.tcp_fallback_port),
      tostring(var.turn_tls_port),
    ]
  }

  log_config {
    metadata = "INCLUDE_ALL_METADATA"
  }
}

resource "google_compute_firewall" "health_checks" {
  count = var.enable_sfu_firewalls ? 1 : 0

  project   = var.project_id
  name      = "${var.network_name}-google-health-checks"
  network   = google_compute_network.livekit.name
  direction = "INGRESS"
  priority  = 900

  source_ranges = [
    "35.191.0.0/16",
    "130.211.0.0/22",
  ]
  target_tags = [var.sfu_network_tag]

  allow {
    protocol = "tcp"
    ports    = ["7880", tostring(var.turn_tls_port)]
  }

  log_config {
    metadata = "INCLUDE_ALL_METADATA"
  }
}

resource "google_compute_firewall" "iap_ssh" {
  count = length(var.iap_ssh_target_tags) == 0 ? 0 : 1

  project   = var.project_id
  name      = "${var.network_name}-iap-ssh"
  network   = google_compute_network.livekit.name
  direction = "INGRESS"
  priority  = 900

  source_ranges = ["35.235.240.0/20"]
  target_tags   = sort(tolist(var.iap_ssh_target_tags))

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }

  log_config {
    metadata = "INCLUDE_ALL_METADATA"
  }
}

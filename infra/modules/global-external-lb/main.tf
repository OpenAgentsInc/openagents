# Global External Application Load Balancer fronting a Cloud Run service
# via a serverless NEG (CFG-10, #8525).
#
# Chosen over Cloud Run domain mappings deliberately:
#   - domain mappings are still preview, region-limited, and cannot
#     pre-provision certs (cert issuance starts only AFTER the domain points
#     at Google, guaranteeing a TLS outage window during cutover);
#   - the LB gives one static anycast IP for all hostnames, WebSocket
#     support, and a later Cloud Armor / Cloud CDN attachment point.
#
# Certificates use Certificate Manager with DNS AUTHORIZATION so the
# Google-managed cert can reach ACTIVE while the domains still point at the
# old origin — the actual traffic flip is then a pure DNS change.
#
# Everything in this module is safe to pre-create: the forwarding rules
# listen on a brand-new static IP that receives no traffic until DNS moves.

resource "google_compute_global_address" "this" {
  project = var.project
  name    = "${var.name}-lb-ip"
}

# ---------------------------------------------------------------------------
# Certificate Manager: DNS authorizations + managed cert + cert map
# ---------------------------------------------------------------------------

# One DNS authorization per hostname. Each emits a CNAME record
# (_acme-challenge.<domain> -> <token>.authorize.certificatemanager.goog)
# that must be added at the current DNS host BEFORE the cert can go ACTIVE.
# See the module outputs for the exact records.
resource "google_certificate_manager_dns_authorization" "domains" {
  for_each = toset(var.domains)

  project  = var.project
  name     = "${var.name}-dnsauth-${replace(each.value, ".", "-")}"
  location = "global"
  domain   = each.value
}

resource "google_certificate_manager_certificate" "this" {
  project  = var.project
  name     = "${var.name}-cert"
  location = "global"

  managed {
    domains = var.domains
    dns_authorizations = [
      for auth in google_certificate_manager_dns_authorization.domains : auth.id
    ]
  }
}

resource "google_certificate_manager_certificate_map" "this" {
  project = var.project
  name    = "${var.name}-cert-map"
}

resource "google_certificate_manager_certificate_map_entry" "domains" {
  for_each = toset(var.domains)

  project      = var.project
  name         = "${var.name}-cert-${replace(each.value, ".", "-")}"
  map          = google_certificate_manager_certificate_map.this.name
  hostname     = each.value
  certificates = [google_certificate_manager_certificate.this.id]
}

# ---------------------------------------------------------------------------
# Serverless NEG -> backend service
# ---------------------------------------------------------------------------

resource "google_compute_region_network_endpoint_group" "this" {
  project               = var.project
  name                  = "${var.name}-neg"
  region                = var.region
  network_endpoint_type = "SERVERLESS"

  cloud_run {
    service = var.cloud_run_service
  }
}

resource "google_compute_backend_service" "this" {
  project               = var.project
  name                  = "${var.name}-backend"
  load_balancing_scheme = "EXTERNAL_MANAGED"
  protocol              = "HTTPS"

  backend {
    group = google_compute_region_network_endpoint_group.this.id
  }

  # NOTE: no timeout_sec — for serverless NEG backends the backend-service
  # timeout is not honored; Cloud Run's own request timeout governs
  # long-lived requests/WebSockets.

  log_config {
    enable      = true
    sample_rate = 1.0
  }
}

# ---------------------------------------------------------------------------
# URL map, proxies, forwarding rules
# ---------------------------------------------------------------------------

resource "google_compute_url_map" "this" {
  project         = var.project
  name            = "${var.name}-url-map"
  default_service = google_compute_backend_service.this.id

  # All served hostnames route to the same monolith backend today; the
  # explicit host rule keeps the intended host set visible and gives a seam
  # for per-host routing later.
  host_rule {
    hosts        = var.domains
    path_matcher = "monolith"
  }

  path_matcher {
    name            = "monolith"
    default_service = google_compute_backend_service.this.id
  }
}

resource "google_compute_target_https_proxy" "this" {
  project         = var.project
  name            = "${var.name}-https-proxy"
  url_map         = google_compute_url_map.this.id
  certificate_map = "//certificatemanager.googleapis.com/${google_certificate_manager_certificate_map.this.id}"
}

resource "google_compute_global_forwarding_rule" "https" {
  project               = var.project
  name                  = "${var.name}-https"
  ip_address            = google_compute_global_address.this.id
  port_range            = "443"
  target                = google_compute_target_https_proxy.this.id
  load_balancing_scheme = "EXTERNAL_MANAGED"
}

# Port 80 -> 301 to HTTPS.
resource "google_compute_url_map" "http_redirect" {
  project = var.project
  name    = "${var.name}-http-redirect"

  default_url_redirect {
    https_redirect         = true
    redirect_response_code = "MOVED_PERMANENTLY_DEFAULT"
    strip_query            = false
  }
}

resource "google_compute_target_http_proxy" "this" {
  project = var.project
  name    = "${var.name}-http-proxy"
  url_map = google_compute_url_map.http_redirect.id
}

resource "google_compute_global_forwarding_rule" "http" {
  project               = var.project
  name                  = "${var.name}-http"
  ip_address            = google_compute_global_address.this.id
  port_range            = "80"
  target                = google_compute_target_http_proxy.this.id
  load_balancing_scheme = "EXTERNAL_MANAGED"
}

output "ip_address" {
  description = "Static global anycast IP — the DNS flip target"
  value       = google_compute_global_address.this.address
}

output "dns_authorization_records" {
  description = "CNAME records the DNS owner must add so the managed cert can pre-provision (safe to add any time; no traffic impact)"
  value = {
    for domain, auth in google_certificate_manager_dns_authorization.domains :
    domain => {
      name = auth.dns_resource_record[0].name
      type = auth.dns_resource_record[0].type
      data = auth.dns_resource_record[0].data
    }
  }
}

output "certificate_id" {
  value = google_certificate_manager_certificate.this.id
}

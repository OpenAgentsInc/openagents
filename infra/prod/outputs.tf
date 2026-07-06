# CFG-10 (#8525): everything the DNS owner needs for the cutover.

output "openagents_lb_ip" {
  description = "Static IP for the openagents.com / auth.openagents.com DNS flip"
  value       = module.openagents_lb.ip_address
}

output "openagents_lb_dns_authorization_records" {
  description = "Cert pre-provisioning CNAMEs — add at Cloudflare DNS immediately (no traffic impact)"
  value       = module.openagents_lb.dns_authorization_records
}

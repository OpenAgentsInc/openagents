# CFG-10 (#8525): everything the DNS owner needs for the cutover.

output "openagents_lb_ip" {
  description = "Static IP for the openagents.com / auth.openagents.com DNS flip"
  value       = module.openagents_lb.ip_address
}

output "openagents_lb_dns_authorization_records" {
  description = "Google-managed certificate DNS authorization records"
  value       = module.openagents_lb.dns_authorization_records
}

output "forge_git_service_name" {
  description = "Cloud Run service for authenticated Smart HTTP Git"
  value       = module.forge_git.service_name
}

output "forge_git_filestore_instance_name" {
  description = "Filestore instance that owns Forge bare repository refs"
  value       = module.forge_git.filestore_instance_name
}

output "forge_git_filestore_ip_address" {
  description = "Private NFS address for the Forge Git repository store"
  value       = module.forge_git.filestore_ip_address
  sensitive   = true
}

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

output "forge_git_nfs_instance_name" {
  description = "Dedicated GCE NFS host for Forge bare repositories"
  value       = module.forge_git.nfs_instance_name
}

output "forge_git_nfs_ip_address" {
  description = "Private NFS address for the Forge Git repository store"
  value       = module.forge_git.nfs_ip_address
  sensitive   = true
}

output "forge_git_repository_disk_name" {
  description = "Persistent disk that owns Forge Git refs"
  value       = module.forge_git.repository_disk_name
}

output "cloud_run_source_builder_service_account" {
  description = "Explicit build-only identity for gcloud run deploy --source."
  value       = google_service_account.cloud_run_source_builder.email
}

output "cloud_image_builder_service_account" {
  description = "Explicit build-only identity for general Cloud Build image submissions."
  value       = google_service_account.cloud_image_builder.email
}

output "cloud_build_source_bucket" {
  description = "Dedicated seven-day source staging bucket for general Cloud Build submissions."
  value       = google_storage_bucket.cloud_build_source.name
}

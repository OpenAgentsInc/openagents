output "service_name" {
  description = "Forge Git Cloud Run service name"
  value       = google_cloud_run_v2_service.this.name
}

output "service_uri" {
  description = "Internal and load-balancer Cloud Run service URI"
  value       = google_cloud_run_v2_service.this.uri
}

output "filestore_instance_name" {
  description = "Authoritative repository Filestore instance name"
  value       = google_filestore_instance.repositories.name
}

output "filestore_ip_address" {
  description = "Private Filestore NFS address"
  value       = google_filestore_instance.repositories.networks[0].ip_addresses[0]
}

output "file_share_name" {
  description = "Authoritative repository NFS share name"
  value       = var.file_share_name
}

output "runtime_service_account_email" {
  description = "Forge Git Cloud Run service identity"
  value       = google_service_account.runtime.email
}

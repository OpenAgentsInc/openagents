output "service_name" {
  description = "Forge Git Cloud Run service name"
  value       = google_cloud_run_v2_service.this.name
}

output "service_uri" {
  description = "Internal and load-balancer Cloud Run service URI"
  value       = google_cloud_run_v2_service.this.uri
}

output "nfs_instance_name" {
  description = "Dedicated GCE NFS host name"
  value       = google_compute_instance.nfs.name
}

output "nfs_ip_address" {
  description = "Private NFS address"
  value       = google_compute_instance.nfs.network_interface[0].network_ip
}

output "repository_disk_name" {
  description = "Sole authoritative repository disk name"
  value       = google_compute_disk.repositories.name
}

output "subnetwork_name" {
  description = "Dedicated Cloud Run Direct VPC subnetwork"
  value       = google_compute_subnetwork.forge_git.name
}

output "runtime_service_account_email" {
  description = "Forge Git Cloud Run service identity"
  value       = google_service_account.runtime.email
}

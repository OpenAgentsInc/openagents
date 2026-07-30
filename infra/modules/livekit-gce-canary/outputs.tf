output "instance_name" {
  description = "Exact disposable canary VM name."
  value       = try(google_compute_instance.canary[0].name, null)
}

output "external_ip" {
  description = "Reserved public address for canary DNS."
  value       = google_compute_address.canary.address
}

output "service_account_email" {
  description = "Canary runtime service account."
  value       = google_service_account.canary.email
}

output "secret_ids" {
  description = "Canary Secret Manager container IDs. Secret versions are created out of band."
  value = {
    api_key         = google_secret_manager_secret.api_key.secret_id
    api_secret      = google_secret_manager_secret.api_secret.secret_id
    tls_certificate = google_secret_manager_secret.tls_certificate.secret_id
    tls_private_key = google_secret_manager_secret.tls_private_key.secret_id
  }
}

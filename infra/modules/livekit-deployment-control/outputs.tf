output "service_account_email" {
  value = google_service_account.deployer.email
}

output "service_account_unique_id" {
  value = google_service_account.deployer.unique_id
}

output "trigger_name" {
  value = google_cloudbuild_trigger.production_runtime.name
}

output "connection_name" {
  value = google_cloudbuildv2_connection.source.name
}

output "repository_id" {
  value = google_cloudbuildv2_repository.source.id
}

output "membership_name" {
  value = google_gke_hub_membership.production.membership_id
}

output "receipt_bucket" {
  value = google_storage_bucket.receipts.name
}

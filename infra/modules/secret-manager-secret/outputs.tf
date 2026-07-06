output "id" {
  description = "Full resource ID of the secret (projects/<project>/secrets/<secret_id>)"
  value       = google_secret_manager_secret.this.id
}

output "secret_id" {
  description = "Short secret ID"
  value       = google_secret_manager_secret.this.secret_id
}

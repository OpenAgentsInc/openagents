output "cluster_id" {
  description = "Regional GKE cluster resource ID."
  value       = google_container_cluster.livekit.id
}

output "cluster_name" {
  description = "Regional GKE cluster name."
  value       = google_container_cluster.livekit.name
}

output "cluster_location" {
  description = "Regional GKE cluster location."
  value       = google_container_cluster.livekit.location
}

output "signal_address" {
  description = "Reserved external address for the signaling endpoint."
  value       = google_compute_global_address.signal.address
}

output "turn_address" {
  description = "Reserved external address for the TURN endpoint."
  value       = google_compute_address.turn.address
}

output "redis_host" {
  description = "Private HA Redis host."
  value       = google_redis_instance.livekit.host
}

output "redis_port" {
  description = "Private HA Redis port."
  value       = google_redis_instance.livekit.port
}

output "redis_server_ca_cert" {
  description = "Public server CA certificate required for TLS to HA Redis."
  value       = google_redis_instance.livekit.server_ca_certs[0].cert
  sensitive   = false
}

output "server_service_account_email" {
  description = "Google service account mapped to the LiveKit server KSA."
  value       = google_service_account.server.email
}

output "agent_service_account_email" {
  description = "Google service account mapped to the Sarah agent KSA."
  value       = google_service_account.agent.email
}

output "secret_reader_service_account_emails" {
  description = "Dedicated Google service accounts mapped to External Secrets KSAs."
  value = {
    livekit = google_service_account.secret_reader.email
    dns     = google_service_account.dns_secret_reader.email
    sarah   = google_service_account.sarah_secret_reader.email
  }
}

output "secret_ids" {
  description = "Secret Manager container IDs. Versions are created out of band; Redis material contains only host and CA certificate."
  value = {
    server_keys    = google_secret_manager_secret.server_keys.secret_id
    redis_auth     = google_secret_manager_secret.redis_auth.secret_id
    openai_api_key = google_secret_manager_secret.openai_api_key.secret_id
    cloudflare_dns = google_secret_manager_secret.cloudflare_dns.secret_id
  }
}

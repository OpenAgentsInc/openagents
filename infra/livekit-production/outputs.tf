output "cluster_name" {
  description = "Exact regional GKE Standard cluster name."
  value       = module.platform.cluster_name
}

output "cluster_location" {
  description = "Exact regional GKE Standard cluster location."
  value       = module.platform.cluster_location
}

output "signal_address" {
  description = "Global external address for Cloudflare DNS-only signaling."
  value       = module.platform.signal_address
}

output "turn_address" {
  description = "Regional external address for Cloudflare DNS-only TURN/TLS."
  value       = module.platform.turn_address
}

output "signal_hostname" {
  description = "WSS signaling hostname."
  value       = var.signal_hostname
}

output "turn_hostname" {
  description = "TURN/TLS hostname."
  value       = var.turn_hostname
}

output "redis_host" {
  description = "Private HA Redis host."
  value       = module.platform.redis_host
}

output "redis_port" {
  description = "Private HA Redis port."
  value       = module.platform.redis_port
}

output "redis_server_ca_cert" {
  description = "Public Redis server CA certificate for the structured auth secret."
  value       = module.platform.redis_server_ca_cert
}

output "server_service_account_email" {
  description = "Google service account mapped to the LiveKit server KSA."
  value       = module.platform.server_service_account_email
}

output "agent_service_account_email" {
  description = "Google service account mapped to the Sarah agent KSA."
  value       = module.platform.agent_service_account_email
}

output "secret_reader_service_account_emails" {
  description = "Dedicated External Secrets Workload Identity targets."
  value       = module.platform.secret_reader_service_account_emails
}

output "secret_ids" {
  description = "Secret Manager containers. Operators create structured versions out of band; Redis material has no password."
  value       = module.platform.secret_ids
}

output "dashboard_id" {
  description = "LiveKit Cloud Monitoring dashboard resource ID."
  value       = module.observability.dashboard_id
}

output "alert_policy_ids" {
  description = "LiveKit Cloud Monitoring alert-policy IDs."
  value       = module.observability.alert_policy_ids
}

output "budget_name" {
  description = "Google Cloud Billing budget resource name."
  value       = module.observability.budget_name
}

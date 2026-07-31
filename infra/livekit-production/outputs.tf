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

output "deployment_control" {
  description = "Fixed-trigger production deployment control resources."
  value = var.enable_deployment_control ? {
    service_account = module.deployment_control[0].service_account_email
    trigger_name    = module.deployment_control[0].trigger_name
    connection_name = module.deployment_control[0].connection_name
    repository_id   = module.deployment_control[0].repository_id
    membership_name = module.deployment_control[0].membership_name
    receipt_bucket  = module.deployment_control[0].receipt_bucket
  } : null
}

output "image_builder_service_account" {
  description = "Narrow service account required by LiveKit image build scripts."
  value       = google_service_account.image_builder.email
}

output "image_build_source_bucket" {
  description = "Dedicated seven-day source staging bucket for explicit LiveKit image builds."
  value       = google_storage_bucket.image_build_source.name
}

output "sarah_nostr_signer" {
  description = "Private Workload-Identity-authenticated Sarah signing service."
  value = var.sarah_nostr_signer_image == null ? null : {
    service_name = google_cloud_run_v2_service.sarah_nostr_signer[0].name
    audience     = google_cloud_run_v2_service.sarah_nostr_signer[0].uri
    invoker      = module.platform.agent_service_account_email
  }
}

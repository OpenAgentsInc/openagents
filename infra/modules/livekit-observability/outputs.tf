output "dashboard_id" {
  description = "Google Cloud Monitoring dashboard resource ID."
  value       = google_monitoring_dashboard.livekit.id
}

output "uptime_check_id" {
  description = "Public signaling uptime-check ID."
  value       = google_monitoring_uptime_check_config.signal.uptime_check_id
}

output "alert_policy_ids" {
  description = "LiveKit alert-policy resource IDs."
  value = {
    certificate_expiry   = google_monitoring_alert_policy.certificate_expiry.id
    participant_capacity = google_monitoring_alert_policy.participant_capacity.id
    redis_memory         = google_monitoring_alert_policy.redis_memory.id
    room_capacity        = google_monitoring_alert_policy.room_capacity.id
    sfu_cpu              = google_monitoring_alert_policy.sfu_cpu.id
    signal_unavailable   = google_monitoring_alert_policy.signal_unavailable.id
    workload_errors      = google_monitoring_alert_policy.workload_errors.id
  }
}

output "budget_name" {
  description = "Billing budget resource name, or null when billing_account_id was not supplied."
  value       = try(google_billing_budget.livekit[0].name, null)
}

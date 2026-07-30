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
    certificate_expiry         = google_monitoring_alert_policy.certificate_expiry.id
    livekit_cpu                = google_monitoring_alert_policy.livekit_cpu.id
    participant_capacity       = google_monitoring_alert_policy.participant_capacity.id
    participant_join_failures  = google_monitoring_alert_policy.participant_join_failures.id
    redis_memory               = google_monitoring_alert_policy.redis_memory.id
    redis_rejected_connections = google_monitoring_alert_policy.redis_rejected_connections.id
    redis_unavailable          = google_monitoring_alert_policy.redis_unavailable.id
    room_capacity              = google_monitoring_alert_policy.room_capacity.id
    server_scrape_quorum       = google_monitoring_alert_policy.server_scrape_quorum.id
    signal_unavailable         = google_monitoring_alert_policy.signal_unavailable.id
    turn_certificate_expiry    = google_monitoring_alert_policy.turn_certificate_expiry.id
    workload_errors            = google_monitoring_alert_policy.workload_errors.id
  }
}

output "budget_name" {
  description = "Billing budget resource name, or null when billing_account_id was not supplied."
  value       = try(google_billing_budget.livekit[0].name, null)
}

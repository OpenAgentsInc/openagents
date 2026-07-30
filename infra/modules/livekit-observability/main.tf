locals {
  alert_documentation = <<-EOT
  This alert is scoped to the isolated ${var.environment} LiveKit environment.
  Stop new admission before capacity is exhausted. Inspect only opaque room,
  participant, worker-job, provider-generation, and infrastructure references.
  Do not place transcripts, spoken text, owner identity, or credentials in telemetry.
  EOT

  dashboard_metrics = [
    {
      title  = "Rooms"
      metric = "prometheus.googleapis.com/livekit_room_total/gauge"
    },
    {
      title  = "Participants"
      metric = "prometheus.googleapis.com/livekit_participant_total/gauge"
    },
    {
      title  = "Lost packets"
      metric = "prometheus.googleapis.com/livekit_packet_loss_total/counter"
    },
  ]
}

resource "google_monitoring_uptime_check_config" "signal" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment} signaling TLS"
  timeout      = "10s"
  period       = "60s"

  selected_regions = [
    "USA",
    "EUROPE",
    "ASIA_PACIFIC",
  ]

  monitored_resource {
    type = "uptime_url"
    labels = {
      host       = var.signal_hostname
      project_id = var.project_id
    }
  }

  http_check {
    path           = "/"
    port           = 443
    request_method = "GET"
    use_ssl        = true
    validate_ssl   = true
  }
}

resource "google_logging_metric" "livekit_errors" {
  project = var.project_id
  name    = "livekit_${var.environment}_error_count"
  filter = join(" ", [
    "resource.type=\"k8s_container\"",
    "resource.labels.cluster_name=\"${var.cluster_name}\"",
    "resource.labels.location=\"${var.cluster_location}\"",
    "resource.labels.namespace_name=\"livekit-system\"",
    "resource.labels.container_name=\"livekit-server\"",
    "severity>=ERROR",
  ])

  description = "Count of error-level LiveKit workload records; payloads are not copied into the metric."

  metric_descriptor {
    metric_kind = "DELTA"
    value_type  = "INT64"
    unit        = "1"
  }
}

resource "google_monitoring_alert_policy" "signal_unavailable" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: signaling unavailable"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "TLS uptime check fails"
    condition_threshold {
      filter = join(" ", [
        "metric.type=\"monitoring.googleapis.com/uptime_check/check_passed\"",
        "resource.type=\"uptime_url\"",
        "metric.label.check_id=\"${google_monitoring_uptime_check_config.signal.uptime_check_id}\"",
      ])
      comparison      = "COMPARISON_LT"
      threshold_value = 1
      duration        = "120s"

      aggregations {
        alignment_period   = "60s"
        per_series_aligner = "ALIGN_NEXT_OLDER"
      }

      trigger {
        count = 1
      }
    }
  }
}

resource "google_monitoring_alert_policy" "certificate_expiry" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: signaling certificate expiry"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "TLS certificate has less than fourteen days remaining"
    condition_threshold {
      filter = join(" ", [
        "metric.type=\"monitoring.googleapis.com/uptime_check/time_until_ssl_cert_expires\"",
        "resource.type=\"uptime_url\"",
        "metric.label.check_id=\"${google_monitoring_uptime_check_config.signal.uptime_check_id}\"",
      ])
      comparison      = "COMPARISON_LT"
      threshold_value = 1209600
      duration        = "0s"

      aggregations {
        alignment_period   = "3600s"
        per_series_aligner = "ALIGN_MIN"
      }

      trigger {
        count = 1
      }
    }
  }
}

resource "google_monitoring_alert_policy" "turn_certificate_expiry" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: TURN certificate expiry"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "TURN TLS certificate has less than fourteen days remaining"
    condition_prometheus_query_language {
      query                     = "min(certmanager_certificate_expiration_timestamp_seconds{cluster=\"${var.cluster_name}\",location=\"${var.cluster_location}\",name=\"livekit-turn\",exported_namespace=\"livekit-system\"} - time()) < 1209600 or absent(certmanager_certificate_expiration_timestamp_seconds{cluster=\"${var.cluster_name}\",location=\"${var.cluster_location}\",name=\"livekit-turn\",exported_namespace=\"livekit-system\"})"
      duration                  = "0s"
      evaluation_interval       = "300s"
      disable_metric_validation = true
    }
  }
}

resource "google_monitoring_alert_policy" "livekit_cpu" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: server CPU saturation"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "Maximum LiveKit container CPU limit utilization exceeds 80%"
    condition_threshold {
      filter = join(" ", [
        "metric.type=\"kubernetes.io/container/cpu/limit_utilization\"",
        "resource.type=\"k8s_container\"",
        "resource.label.cluster_name=\"${var.cluster_name}\"",
        "resource.label.location=\"${var.cluster_location}\"",
        "resource.label.namespace_name=\"livekit-system\"",
        "resource.label.container_name=\"livekit-server\"",
      ])
      comparison      = "COMPARISON_GT"
      threshold_value = 0.8
      duration        = "300s"

      aggregations {
        alignment_period     = "60s"
        per_series_aligner   = "ALIGN_MEAN"
        cross_series_reducer = "REDUCE_MAX"
        group_by_fields      = ["resource.label.pod_name"]
      }

      trigger {
        count = 1
      }
    }
  }
}

resource "google_monitoring_alert_policy" "redis_memory" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: Redis memory pressure"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "Redis memory utilization exceeds 80%"
    condition_threshold {
      filter = join(" ", [
        "metric.type=\"redis.googleapis.com/stats/memory/system_memory_usage_ratio\"",
        "resource.type=\"redis_instance\"",
        "resource.label.instance_id=\"${var.redis_instance_id}\"",
      ])
      comparison      = "COMPARISON_GT"
      threshold_value = 0.8
      duration        = "300s"

      aggregations {
        alignment_period   = "60s"
        per_series_aligner = "ALIGN_MEAN"
      }

      trigger {
        count = 1
      }
    }
  }
}

resource "google_monitoring_alert_policy" "redis_unavailable" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: Redis unavailable"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "Redis uptime telemetry is absent"
    condition_absent {
      filter = join(" ", [
        "metric.type=\"redis.googleapis.com/server/uptime\"",
        "resource.type=\"redis_instance\"",
        "resource.label.instance_id=\"${var.redis_instance_id}\"",
      ])
      duration = "300s"

      aggregations {
        alignment_period   = "60s"
        per_series_aligner = "ALIGN_MAX"
      }

      trigger {
        count = 1
      }
    }
  }
}

resource "google_monitoring_alert_policy" "redis_rejected_connections" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: Redis rejected connections"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "Redis rejected one or more connections"
    condition_threshold {
      filter = join(" ", [
        "metric.type=\"redis.googleapis.com/stats/reject_connections_count\"",
        "resource.type=\"redis_instance\"",
        "resource.label.instance_id=\"${var.redis_instance_id}\"",
      ])
      comparison      = "COMPARISON_GT"
      threshold_value = 0
      duration        = "0s"

      aggregations {
        alignment_period   = "60s"
        per_series_aligner = "ALIGN_DELTA"
      }

      trigger {
        count = 1
      }
    }
  }
}

resource "google_monitoring_alert_policy" "server_scrape_quorum" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: server scrape quorum"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "Fewer than two LiveKit server targets are up"
    condition_prometheus_query_language {
      query                     = "sum(up{cluster=\"${var.cluster_name}\",location=\"${var.cluster_location}\",namespace=\"livekit-system\",job=~\".*livekit-server.*\"}) < 2 or absent(up{cluster=\"${var.cluster_name}\",location=\"${var.cluster_location}\",namespace=\"livekit-system\",job=~\".*livekit-server.*\"})"
      duration                  = "120s"
      evaluation_interval       = "30s"
      disable_metric_validation = true
    }
  }
}

resource "google_monitoring_alert_policy" "participant_join_failures" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: participant join failures"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "Participant join failures are sustained"
    condition_prometheus_query_language {
      query                     = "sum(rate(livekit_participant_join_total{cluster=\"${var.cluster_name}\",location=\"${var.cluster_location}\",state=~\"signal_failed|signal_validation_failed|signal_upgrade_failed|signal_write_initial_response_failed|rtc_failure\"}[5m])) > 0.1"
      duration                  = "300s"
      evaluation_interval       = "30s"
      disable_metric_validation = true
    }
  }
}

resource "google_monitoring_alert_policy" "workload_errors" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: workload errors"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "Five or more error-level records in five minutes"
    condition_threshold {
      filter          = "metric.type=\"logging.googleapis.com/user/${google_logging_metric.livekit_errors.name}\" AND resource.type=\"k8s_container\""
      comparison      = "COMPARISON_GT"
      threshold_value = 4
      duration        = "0s"

      aggregations {
        alignment_period     = "300s"
        per_series_aligner   = "ALIGN_SUM"
        cross_series_reducer = "REDUCE_SUM"
      }

      trigger {
        count = 1
      }
    }
  }
}

resource "google_monitoring_alert_policy" "room_capacity" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: room capacity"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "Admitted room capacity reached"
    condition_prometheus_query_language {
      query                     = "sum(livekit_room_total{cluster=\"${var.cluster_name}\",location=\"${var.cluster_location}\"}) >= ${var.max_rooms}"
      duration                  = "120s"
      evaluation_interval       = "30s"
      disable_metric_validation = true
    }
  }
}

resource "google_monitoring_alert_policy" "participant_capacity" {
  project      = var.project_id
  display_name = "LiveKit ${var.environment}: participant capacity"
  combiner     = "OR"
  enabled      = true

  notification_channels = var.notification_channel_ids

  documentation {
    content   = local.alert_documentation
    mime_type = "text/markdown"
  }

  conditions {
    display_name = "Admitted participant capacity reached"
    condition_prometheus_query_language {
      query                     = "sum(livekit_participant_total{cluster=\"${var.cluster_name}\",location=\"${var.cluster_location}\"}) >= ${var.max_participants}"
      duration                  = "120s"
      evaluation_interval       = "30s"
      disable_metric_validation = true
    }
  }
}

resource "google_monitoring_dashboard" "livekit" {
  project = var.project_id

  dashboard_json = jsonencode({
    displayName = "LiveKit ${var.environment}"
    labels      = var.labels
    mosaicLayout = {
      columns = 48
      tiles = [
        for index, panel in local.dashboard_metrics : {
          xPos   = (index % 2) * 24
          yPos   = floor(index / 2) * 16
          width  = 24
          height = 16
          widget = {
            title = panel.title
            xyChart = {
              dataSets = [{
                plotType   = "LINE"
                targetAxis = "Y1"
                timeSeriesQuery = {
                  timeSeriesFilter = {
                    filter = "metric.type=\"${panel.metric}\" AND resource.type=\"prometheus_target\" AND resource.label.cluster=\"${var.cluster_name}\" AND resource.label.location=\"${var.cluster_location}\""
                    aggregation = {
                      alignmentPeriod  = "60s"
                      perSeriesAligner = "ALIGN_MEAN"
                    }
                  }
                }
              }]
              yAxis = {
                label = panel.title
                scale = "LINEAR"
              }
            }
          }
        }
      ]
    }
  })
}

resource "google_billing_budget" "livekit" {
  count = var.billing_account_id == null ? 0 : 1

  billing_account = var.billing_account_id
  display_name    = "LiveKit ${var.environment}"

  budget_filter {
    projects = ["projects/${var.project_number}"]
    labels = {
      service = "livekit"
    }
  }

  amount {
    specified_amount {
      currency_code = "USD"
      units         = var.monthly_budget_usd
    }
  }

  threshold_rules {
    threshold_percent = 0.5
    spend_basis       = "CURRENT_SPEND"
  }

  threshold_rules {
    threshold_percent = 0.8
    spend_basis       = "CURRENT_SPEND"
  }

  threshold_rules {
    threshold_percent = 1.0
    spend_basis       = "FORECASTED_SPEND"
  }

  all_updates_rule {
    monitoring_notification_channels = var.notification_channel_ids
    disable_default_iam_recipients   = false
  }
}

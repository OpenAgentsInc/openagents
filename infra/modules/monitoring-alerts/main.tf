# Baseline alert policies. Every policy is opt-in via its variable so this
# module can be instantiated incrementally without creating resources the
# operator has not asked for yet.

resource "google_monitoring_alert_policy" "cloudsql_cpu" {
  count = var.cloudsql_instance == null ? 0 : 1

  project      = var.project
  display_name = "Cloud SQL ${var.cloudsql_instance} CPU > ${var.cloudsql_cpu_threshold * 100}%"
  combiner     = "OR"

  conditions {
    display_name = "CPU utilization"
    condition_threshold {
      filter          = "resource.type = \"cloudsql_database\" AND resource.labels.database_id = \"${var.project}:${var.cloudsql_instance}\" AND metric.type = \"cloudsql.googleapis.com/database/cpu/utilization\""
      comparison      = "COMPARISON_GT"
      threshold_value = var.cloudsql_cpu_threshold
      duration        = "300s"
      aggregations {
        alignment_period   = "300s"
        per_series_aligner = "ALIGN_MEAN"
      }
    }
  }

  notification_channels = var.notification_channels
}

resource "google_monitoring_alert_policy" "cloudsql_connections" {
  count = var.cloudsql_instance == null ? 0 : 1

  project      = var.project
  display_name = "Cloud SQL ${var.cloudsql_instance} connections > ${var.cloudsql_connections_threshold}"
  combiner     = "OR"

  conditions {
    display_name = "Active connections"
    condition_threshold {
      filter          = "resource.type = \"cloudsql_database\" AND resource.labels.database_id = \"${var.project}:${var.cloudsql_instance}\" AND metric.type = \"cloudsql.googleapis.com/database/postgresql/num_backends\""
      comparison      = "COMPARISON_GT"
      threshold_value = var.cloudsql_connections_threshold
      duration        = "300s"
      aggregations {
        alignment_period     = "300s"
        per_series_aligner   = "ALIGN_MEAN"
        cross_series_reducer = "REDUCE_SUM"
      }
    }
  }

  notification_channels = var.notification_channels
}

resource "google_monitoring_alert_policy" "cloud_run_5xx" {
  count = var.cloud_run_service == null ? 0 : 1

  project      = var.project
  display_name = "Cloud Run ${var.cloud_run_service} 5xx > ${var.cloud_run_5xx_threshold}/5m"
  combiner     = "OR"

  conditions {
    display_name = "5xx responses"
    condition_threshold {
      filter          = "resource.type = \"cloud_run_revision\" AND resource.labels.service_name = \"${var.cloud_run_service}\" AND metric.type = \"run.googleapis.com/request_count\" AND metric.labels.response_code_class = \"5xx\""
      comparison      = "COMPARISON_GT"
      threshold_value = var.cloud_run_5xx_threshold
      duration        = "0s"
      aggregations {
        alignment_period     = "300s"
        per_series_aligner   = "ALIGN_DELTA"
        cross_series_reducer = "REDUCE_SUM"
      }
    }
  }

  notification_channels = var.notification_channels
}

resource "google_billing_budget" "monthly" {
  count = var.billing_account == null ? 0 : 1

  billing_account = var.billing_account
  display_name    = "${var.project} monthly budget"

  budget_filter {
    projects = ["projects/${var.project}"]
  }

  amount {
    specified_amount {
      currency_code = "USD"
      units         = tostring(var.budget_amount_usd)
    }
  }

  threshold_rules {
    threshold_percent = 0.5
  }
  threshold_rules {
    threshold_percent = 0.9
  }
  threshold_rules {
    threshold_percent = 1.0
  }
}

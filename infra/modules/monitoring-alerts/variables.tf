variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "notification_channels" {
  description = "Notification channel IDs to attach to all policies"
  type        = list(string)
  default     = []
}

variable "cloudsql_instance" {
  description = "Cloud SQL instance name to alert on (null = no Cloud SQL alerts)"
  type        = string
  default     = null
}

variable "cloudsql_cpu_threshold" {
  description = "Cloud SQL CPU utilization alert threshold (0..1)"
  type        = number
  default     = 0.8
}

variable "cloudsql_connections_threshold" {
  description = "Cloud SQL active-connections alert threshold"
  type        = number
  default     = 400
}

variable "cloud_run_service" {
  description = "Cloud Run service name to alert on 5xx (null = no Cloud Run alerts)"
  type        = string
  default     = null
}

variable "cloud_run_5xx_threshold" {
  description = "Cloud Run 5xx responses per 5-minute window before alerting"
  type        = number
  default     = 20
}

variable "billing_account" {
  description = "Billing account ID for a budget alert (null = no budget)"
  type        = string
  default     = null
}

variable "budget_amount_usd" {
  description = "Monthly budget amount in USD"
  type        = number
  default     = 1000
}

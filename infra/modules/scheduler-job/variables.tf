variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "name" {
  description = "Scheduler job name"
  type        = string
}

variable "region" {
  description = "Scheduler region"
  type        = string
}

variable "schedule" {
  description = "Cron schedule, e.g. */5 * * * *"
  type        = string
}

variable "time_zone" {
  description = "IANA time zone"
  type        = string
  default     = "Etc/UTC"
}

variable "description" {
  description = "Job description"
  type        = string
  default     = null
}

variable "http_uri" {
  description = "HTTP target URI"
  type        = string
}

variable "http_method" {
  description = "HTTP method"
  type        = string
  default     = "POST"
}

variable "oidc_service_account_email" {
  description = "Service account for OIDC auth (null = no auth header)"
  type        = string
  default     = null
}

variable "attempt_deadline" {
  description = "Attempt deadline"
  type        = string
  default     = "180s"
}

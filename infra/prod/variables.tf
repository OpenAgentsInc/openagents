# Single-env (prod) today. To add staging later: copy this root module to
# infra/staging/, change the backend prefix + tfvars, and reuse the same
# modules — everything environment-specific already flows through variables.

variable "project_id" {
  description = "GCP project ID"
  type        = string
  default     = "openagentsgemini"
}

variable "region" {
  description = "Default region"
  type        = string
  default     = "us-central1"
}

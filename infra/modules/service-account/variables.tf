variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "account_id" {
  description = "Service account ID (local part of the email)"
  type        = string
}

variable "display_name" {
  description = "Human-readable display name"
  type        = string
  default     = null
}

variable "description" {
  description = "Service account description"
  type        = string
  default     = null
}

variable "project_roles" {
  description = "Project-level IAM roles to grant to this service account"
  type        = list(string)
  default     = []
}

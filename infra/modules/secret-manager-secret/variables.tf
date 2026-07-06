variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "secret_id" {
  description = "Secret Manager secret ID (the short name, not the full resource path)"
  type        = string
}

variable "accessor_members" {
  description = "IAM members granted roles/secretmanager.secretAccessor on this secret (e.g. serviceAccount:...)"
  type        = list(string)
  default     = []
}

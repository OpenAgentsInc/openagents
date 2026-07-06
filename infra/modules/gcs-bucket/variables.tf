variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "name" {
  description = "Bucket name (globally unique)"
  type        = string
}

variable "location" {
  description = "Bucket location, e.g. US-CENTRAL1"
  type        = string
}

variable "storage_class" {
  description = "Default storage class"
  type        = string
  default     = "STANDARD"
}

variable "versioning" {
  description = "Enable object versioning"
  type        = bool
  default     = false
}

variable "soft_delete_retention_seconds" {
  description = "Soft delete retention (0 disables; 604800 = 7 days default)"
  type        = number
  default     = 604800
}

variable "lifecycle_rules" {
  description = "Lifecycle rules: action type (Delete/SetStorageClass), optional storage_class, and condition fields"
  type = list(object({
    action_type          = string
    action_storage_class = optional(string)
    age                  = optional(number)
    num_newer_versions   = optional(number)
    with_state           = optional(string)
  }))
  default = []
}

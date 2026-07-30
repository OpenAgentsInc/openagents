variable "project_id" {
  description = "Google Cloud project that owns the LiveKit telemetry."
  type        = string
}

variable "project_number" {
  description = "Numeric Google Cloud project number for Cloud Billing budget scope."
  type        = string

  validation {
    condition     = can(regex("^[0-9]+$", var.project_number))
    error_message = "The Google Cloud project number must contain only digits."
  }
}

variable "environment" {
  description = "Deployment environment name."
  type        = string

  validation {
    condition     = contains(["staging", "production"], var.environment)
    error_message = "The LiveKit environment must be staging or production."
  }
}

variable "cluster_name" {
  description = "Regional GKE cluster name."
  type        = string
}

variable "cluster_location" {
  description = "Regional GKE cluster location."
  type        = string
}

variable "redis_instance_id" {
  description = "Memorystore Redis instance ID."
  type        = string
}

variable "signal_hostname" {
  description = "Public DNS hostname used for WSS signaling and certificate checks."
  type        = string

  validation {
    condition     = can(regex("^[a-z0-9][a-z0-9.-]+[a-z0-9]$", var.signal_hostname))
    error_message = "The signaling hostname must be a DNS hostname without a URL scheme."
  }
}

variable "notification_channel_ids" {
  description = "Existing Monitoring notification channel resource IDs."
  type        = list(string)
  default     = []
}

variable "billing_account_id" {
  description = "Billing account ID used for the isolated LiveKit budget. Null skips budget creation."
  type        = string
  default     = null
  nullable    = true
}

variable "monthly_budget_usd" {
  description = "Monthly LiveKit Google Cloud budget amount in USD."
  type        = number
  default     = 5000

  validation {
    condition     = var.monthly_budget_usd > 0
    error_message = "The LiveKit monthly budget must be positive."
  }
}

variable "max_rooms" {
  description = "Hard admitted room capacity mirrored by the room-count alert."
  type        = number
  default     = 20
}

variable "max_participants" {
  description = "Hard admitted participant capacity mirrored by the participant alert."
  type        = number
  default     = 60
}

variable "labels" {
  description = "Public-safe labels applied to supported resources."
  type        = map(string)
  default     = {}
}

variable "project_id" {
  description = "Google Cloud project ID."
  type        = string
  default     = "openagentsgemini"

  validation {
    condition     = var.project_id == "openagentsgemini"
    error_message = "Production LiveKit is admitted only in the openagentsgemini project."
  }
}

variable "project_number" {
  description = "Numeric Google Cloud project number."
  type        = string
  default     = "157437760789"
}

variable "region" {
  description = "Google Cloud region."
  type        = string
  default     = "us-central1"

  validation {
    condition     = var.region == "us-central1"
    error_message = "The production candidate is pinned to us-central1."
  }
}

variable "zones" {
  description = "Three production zones."
  type        = list(string)
  default = [
    "us-central1-a",
    "us-central1-b",
    "us-central1-c",
  ]

  validation {
    condition     = length(var.zones) == 3 && alltrue([for zone in var.zones : startswith(zone, "us-central1-")])
    error_message = "Production must use exactly three us-central1 zones."
  }
}

variable "master_authorized_networks" {
  description = "Bounded operator CIDRs allowed to reach the GKE control plane."
  type = list(object({
    cidr_block   = string
    display_name = string
  }))
}

variable "signal_hostname" {
  description = "Public WSS hostname. Cloudflare remains DNS-only."
  type        = string
  default     = "livekit.openagents.com"
}

variable "turn_hostname" {
  description = "Public TURN/TLS hostname. Cloudflare remains DNS-only."
  type        = string
  default     = "turn.livekit.openagents.com"
}

variable "notification_channel_ids" {
  description = "Existing Google Cloud Monitoring notification channel resource IDs."
  type        = list(string)

  validation {
    condition     = length(var.notification_channel_ids) > 0
    error_message = "Production requires at least one Monitoring notification channel."
  }
}

variable "billing_account_id" {
  description = "Google Cloud billing account ID for the production budget."
  type        = string
}

variable "monthly_budget_usd" {
  description = "Monthly alert budget for LiveKit infrastructure."
  type        = number
  default     = 5000
}

variable "enable_turn_udp" {
  description = "Open TURN/UDP 443 only after the dedicated connectivity gate passes."
  type        = bool
  default     = false
}

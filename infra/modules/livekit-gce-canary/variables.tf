variable "project_id" {
  description = "Google Cloud project that owns the disposable connectivity canary."
  type        = string
}

variable "region" {
  description = "Google Cloud region for the reserved canary address."
  type        = string
}

variable "zone" {
  description = "Google Cloud zone for the disposable canary."
  type        = string
}

variable "name" {
  description = "Exact canary instance name."
  type        = string
}

variable "enable_instance" {
  description = "Create the canary VM only after all four Secret Manager versions and DNS records exist."
  type        = bool
  default     = false
}

variable "expires_at_unix" {
  description = "Unix expiry time for the disposable VM; required when enable_instance is true."
  type        = number
  default     = null
  nullable    = true
}

variable "network_id" {
  description = "Dedicated staging VPC resource ID."
  type        = string
}

variable "subnetwork_id" {
  description = "Dedicated staging subnetwork resource ID."
  type        = string
}

variable "machine_type" {
  description = "Compute-optimized canary machine type."
  type        = string
  default     = "c3-standard-8"
}

variable "boot_image" {
  description = "Immutable full resource name of the canary boot image."
  type        = string

  validation {
    condition     = can(regex("^projects/[a-z][a-z0-9-]+/global/images/[a-z0-9-]+$", var.boot_image))
    error_message = "The canary boot image must be an immutable full image resource, not an image family."
  }
}

variable "livekit_server_image" {
  description = "LiveKit server OCI image pinned by sha256 digest."
  type        = string

  validation {
    condition     = can(regex("@sha256:[a-f0-9]{64}$", var.livekit_server_image))
    error_message = "The LiveKit server image must be pinned by sha256 digest."
  }
}

variable "reverse_proxy_image" {
  description = "TLS reverse-proxy OCI image pinned by sha256 digest."
  type        = string

  validation {
    condition     = can(regex("@sha256:[a-f0-9]{64}$", var.reverse_proxy_image))
    error_message = "The reverse proxy image must be pinned by sha256 digest."
  }
}

variable "turn_domain" {
  description = "Trusted TURN/TLS DNS name covered by the canary certificate."
  type        = string

  validation {
    condition     = can(regex("^[a-z0-9][a-z0-9.-]+[a-z0-9]$", var.turn_domain))
    error_message = "The TURN domain must be a DNS hostname without a URL scheme."
  }
}

variable "max_run_duration_seconds" {
  description = "Maximum canary lifetime before Compute Engine deletes the VM."
  type        = number
  default     = 14400

  validation {
    condition     = var.max_run_duration_seconds >= 1800 && var.max_run_duration_seconds <= 21600
    error_message = "The canary lifetime must be between 30 minutes and 6 hours."
  }
}

variable "media_udp_port_range" {
  description = "Direct WebRTC UDP range."
  type = object({
    start = number
    end   = number
  })
  default = {
    start = 50000
    end   = 60000
  }
}

variable "tcp_fallback_port" {
  description = "LiveKit TCP fallback port."
  type        = number
  default     = 7881
}

variable "turn_tls_port" {
  description = "TURN/TLS port."
  type        = number
  default     = 5349
}

variable "turn_udp_port" {
  description = "TURN/UDP port."
  type        = number
  default     = 443
}

variable "enable_turn_udp" {
  description = "Open and advertise TURN/UDP only after its separate rollout gate is admitted."
  type        = bool
  default     = false
}

variable "labels" {
  description = "Public-safe labels applied to canary resources."
  type        = map(string)
  default     = {}
}

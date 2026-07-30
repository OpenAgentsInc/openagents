variable "project_id" {
  description = "Google Cloud project ID."
  type        = string
  default     = "openagentsgemini"

  validation {
    condition     = var.project_id == "openagentsgemini"
    error_message = "The LiveKit canary is admitted only in the openagentsgemini project."
  }
}

variable "region" {
  description = "Google Cloud region."
  type        = string
  default     = "us-central1"

  validation {
    condition     = var.region == "us-central1"
    error_message = "The LiveKit canary is pinned to us-central1."
  }
}

variable "zone" {
  description = "Google Cloud zone for the disposable canary."
  type        = string
  default     = "us-central1-a"
}

variable "enable_canary_instance" {
  description = "Create the VM only after secret versions and DNS are ready."
  type        = bool
  default     = false
}

variable "canary_expires_at_unix" {
  description = "Unix expiry label required by the canary TTL gate."
  type        = number
  default     = null
  nullable    = true
}

variable "signal_hostname" {
  description = "Trusted WSS hostname covered by the canary certificate."
  type        = string
  default     = "livekit-staging.openagents.com"
}

variable "turn_hostname" {
  description = "Trusted TURN/TLS hostname covered by the canary certificate."
  type        = string
  default     = "turn-livekit-staging.openagents.com"
}

variable "canary_boot_image" {
  description = "Immutable Container-Optimized OS image."
  type        = string
  default     = "projects/cos-cloud/global/images/cos-stable-121-18867-528-21"
}

variable "livekit_server_image" {
  description = "Released LiveKit server OCI image pinned by manifest digest."
  type        = string
  default     = "livekit/livekit-server:v1.13.4@sha256:189f7c81b704a36642bc5c7e2d3e1ae83744627c11978a23a251bf19fbec64e0"
}

variable "reverse_proxy_image" {
  description = "Caddy OCI image pinned by manifest digest."
  type        = string
  default     = "caddy:2.10.2-alpine@sha256:4c6e91c6ed0e2fa03efd5b44747b625fec79bc9cd06ac5235a779726618e530d"
}

variable "enable_turn_udp" {
  description = "Open TURN/UDP 443 only after the dedicated connectivity gate passes."
  type        = bool
  default     = false
}
